use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use reqwest;
use crate::security::audit::{SecurityAuditLog, AuditSeverity};

/// Data cached from an oracle source
#[derive(Clone)]
pub struct CachedData {
    pub value: Value,
    pub timestamp: Instant,
}

/// Oracle data source status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OracleSourceStatus {
    Operational,
    Degraded(String),
    Failed(String),
}

/// Data validation rule type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationRuleType {
    NumericRange,
    StringPattern,
    BooleanValue,
    TimestampRange,
    Enumeration,
    Custom,
}

/// Data validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub name: String,
    pub rule_type: ValidationRuleType,
    pub parameters: Value,
    pub error_message: String,
}

/// Result of data validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub rule_name: String,
    pub error_message: Option<String>,
    pub data_field: String,
    pub value: Value,
}

/// Data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleSourceConfig {
    pub name: String,
    pub url: String,
    pub source_type: String, // "REST", "GraphQL", "WebSocket"
    pub auth_header: Option<String>,
    pub default_params: Option<Value>,
    pub validation_rules: Vec<ValidationRule>,
    pub weight: u8, // 1-100
    pub timeout_ms: u64,
    pub rate_limit: Option<u32>, // requests per minute
    pub requires_auth: bool,
    pub path: Vec<String>, // Path to extract data from response (e.g., ["data", "temperature"])
    pub required_fields: Vec<String>, // Fields that must be present in the extracted data
}

/// Generic Oracle Source trait
#[async_trait]
pub trait OracleSource: Send + Sync {
    fn name(&self) -> &str;
    fn config(&self) -> &OracleSourceConfig;
    async fn fetch(&self, params: &Value) -> Result<Value>;
    fn validate(&self, data: &Value) -> Vec<ValidationResult>;
    fn status(&self) -> OracleSourceStatus;
    async fn run_background_updates(&self, update_interval: Duration);
}

/// REST API data source implementation
pub struct RestApiOracleSource {
    client: reqwest::Client,
    config: OracleSourceConfig,
    status: Arc<Mutex<OracleSourceStatus>>,
    last_request: Arc<Mutex<Option<Instant>>>,
    request_count: Arc<Mutex<u32>>,
    audit_log: Option<Arc<SecurityAuditLog>>,
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
    cache_duration: Duration,
}

impl RestApiOracleSource {
    pub fn new(
        config: OracleSourceConfig,
        audit_log: Option<Arc<SecurityAuditLog>>,
        cache: Arc<Mutex<HashMap<String, CachedData>>>,
        cache_duration: Duration,
    ) -> Result<Self> {
        if config.source_type != "REST" {
            return Err(anyhow!("Invalid source type for RestApiSource"));
        }
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            client,
            config,
            status: Arc::new(Mutex::new(OracleSourceStatus::Operational)),
            last_request: Arc::new(Mutex::new(None)),
            request_count: Arc::new(Mutex::new(0)),
            audit_log,
            cache,
            cache_duration,
        })
    }

    fn check_rate_limit(&self) -> bool {
        if let Some(rate_limit) = self.config.rate_limit {
            let mut count = self.request_count.lock().unwrap();
            let mut last_req_opt = self.last_request.lock().unwrap();

            if let Some(last_req) = *last_req_opt {
                if last_req.elapsed().as_secs() > 60 {
                    *count = 1;
                    *last_req_opt = Some(Instant::now());
                    return false;
                }
                if *count >= rate_limit {
                    return true;
                }
                *count += 1;
            } else {
                *count = 1;
                *last_req_opt = Some(Instant::now());
            }
        }
        false
    }

    fn validate_numeric_range(&self, value: &Value, params: &Value) -> bool {
        if let Some(num) = value.as_f64() {
            let min = params.get("min").and_then(Value::as_f64);
            let max = params.get("max").and_then(Value::as_f64);
            match (min, max) {
                (Some(min_v), Some(max_v)) => num >= min_v && num <= max_v,
                (Some(min_v), None) => num >= min_v,
                (None, Some(max_v)) => num <= max_v,
                (None, None) => true,
            }
        } else {
            false
        }
    }

    fn validate_string_pattern(&self, value: &Value, params: &Value) -> bool {
        if let Some(str_val) = value.as_str() {
            if let Some(pattern) = params.get("pattern").and_then(Value::as_str) {
                // Basic substring check; use regex crate for real patterns
                str_val.contains(pattern)
            } else if let Some(allowed) = params.get("allowed").and_then(Value::as_array) {
                allowed.iter().any(|v| v.as_str() == Some(str_val))
            } else {
                true
            }
        } else {
            false
        }
    }

    fn apply_rule(&self, rule: &ValidationRule, data: &Value) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        // Simplified: apply rule to all fields. Enhance to target specific fields.
        if let Some(obj) = data.as_object() {
            for (field, value) in obj {
                let passed = match rule.rule_type {
                    ValidationRuleType::NumericRange => self.validate_numeric_range(value, &rule.parameters),
                    ValidationRuleType::StringPattern => self.validate_string_pattern(value, &rule.parameters),
                    _ => true, // Assume pass for unimplemented rules
                };

                if !passed || value.is_number() || value.is_string() { // Only log results for relevant types or failures
                    results.push(ValidationResult {
                        passed,
                        rule_name: rule.name.clone(),
                        error_message: if passed { None } else { Some(rule.error_message.clone()) },
                        data_field: field.clone(),
                        value: value.clone(),
                    });
                }
            }
        }
        results
    }

    /// Extracts a value from a JSON object using a path.
    fn extract_value<'a>(&self, data: &'a Value, path: &[String]) -> Option<&'a Value> {
        let mut current = data;
        for key in path {
            if let Some(obj) = current.as_object() {
                current = obj.get(key)?;
            } else if let Some(arr) = current.as_array() {
                if let Ok(index) = key.parse::<usize>() {
                    current = arr.get(index)?;
                } else {
                    return None; // Path element is not a valid index for an array
                }
            } else {
                return None; // Path element encountered but current value is not an object or array
            }
        }
        Some(current)
    }

    /// Checks if all required fields are present in the extracted data.
    fn check_required_fields(&self, data: &Value) -> bool {
        if self.config.required_fields.is_empty() {
            return true;
        }
        if let Some(obj) = data.as_object() {
            self.config.required_fields.iter().all(|field| obj.contains_key(field))
        } else {
            false // Required fields check only makes sense for objects
        }
    }
}

#[async_trait]
impl OracleSource for RestApiOracleSource {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn config(&self) -> &OracleSourceConfig {
        &self.config
    }

    async fn fetch(&self, params: &Value) -> Result<Value> {
        let cache_key = format!("{}:{}", self.config.name, serde_json::to_string(params)?);

        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached_data) = cache.get(&cache_key) {
                 if cached_data.timestamp.elapsed() < self.cache_duration {
                     return Ok(cached_data.value.clone());
                 }
            }
        }

        if self.check_rate_limit() {
            *self.status.lock().unwrap() = OracleSourceStatus::Degraded("Rate limit reached".to_string());
            return Err(anyhow!("Rate limit reached for {}", self.config.name));
        }

        let merged_params = self.config.default_params.as_ref()
            .and_then(Value::as_object)
            .map(|default| {
                params.as_object().map_or_else(
                    || Value::Object(default.clone()),
                    |p| {
                        let mut merged = default.clone();
                        merged.extend(p.iter().map(|(k, v)| (k.clone(), v.clone())));
                        Value::Object(merged)
                    }
                )
            })
            .unwrap_or_else(|| params.clone());

        let mut request = self.client.get(&self.config.url);
        if let Some(auth) = &self.config.auth_header {
            request = request.header("Authorization", auth);
        }
        if let Some(obj) = merged_params.as_object() {
            request = request.query(obj);
        }

        let response_result = request.send().await;
        *self.last_request.lock().unwrap() = Some(Instant::now());

        let response = match response_result {
            Ok(resp) => resp,
            Err(e) => {
                *self.status.lock().unwrap() = OracleSourceStatus::Failed(format!("Request failed: {}", e));
                if let Some(log) = &self.audit_log {
                    let _ = log.log_external_api("RestApiOracleSource", &format!("{} request failed: {}", self.config.name, e), AuditSeverity::Error);
                }
                return Err(anyhow!("Request failed: {}", e));
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            *self.status.lock().unwrap() = OracleSourceStatus::Failed(format!("API Error: {}", status));
            if let Some(log) = &self.audit_log {
                let _ = log.log_external_api("RestApiOracleSource", &format!("{} returned error: {}", self.config.name, status), AuditSeverity::Error);
            }
            return Err(anyhow!("API returned error status: {}", status));
        }

        let data = match response.json::<Value>().await {
            Ok(d) => d,
            Err(e) => {
                *self.status.lock().unwrap() = OracleSourceStatus::Failed(format!("JSON parse failed: {}", e));
                if let Some(log) = &self.audit_log {
                    let _ = log.log_external_api("RestApiOracleSource", &format!("{} JSON parse failed: {}", self.config.name, e), AuditSeverity::Error);
                }
                return Err(anyhow!("Failed to parse JSON: {}", e));
            }
        };

        // Extract the relevant part of the data using the path
        let extracted_data = self.extract_value(&data, &self.config.path)
                                 .ok_or_else(|| anyhow!("Failed to extract data using path for {}", self.config.name))?;

        // Check for required fields in the extracted data
        if !self.check_required_fields(extracted_data) {
             *self.status.lock().unwrap() = OracleSourceStatus::Failed("Missing required fields".to_string());
             if let Some(log) = &self.audit_log {
                 let _ = log.log_external_api("RestApiOracleSource", &format!("{} missing required fields", self.config.name), AuditSeverity::Error);
             }
             return Err(anyhow!("Data from {} missing required fields", self.config.name));
         }

        *self.status.lock().unwrap() = OracleSourceStatus::Operational;
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api("RestApiOracleSource", &format!("Successfully fetched from {}", self.config.name), AuditSeverity::Info);
        }

        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(cache_key, CachedData {
                value: extracted_data.clone(),
                timestamp: Instant::now()
            });
        }

        Ok(extracted_data.clone())
    }

    fn validate(&self, data: &Value) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        for rule in &self.config.validation_rules {
            results.extend(self.apply_rule(rule, data));
        }

        if let Some(log) = &self.audit_log {
            let failures = results.iter().filter(|r| !r.passed).count();
            if failures > 0 {
                let _ = log.log_external_api("RestApiOracleSource", &format!("{} validation failures: {}", self.config.name, failures), AuditSeverity::Warning);
            }
        }
        results
    }

    fn status(&self) -> OracleSourceStatus {
        self.status.lock().unwrap().clone()
    }

    async fn run_background_updates(&self, update_interval: Duration) {
        if update_interval.is_zero() {
            return;
        }
        let params = self.config.default_params.clone().unwrap_or(json!({}));
        loop {
            tokio::time::sleep(update_interval).await;
            // Use a unique key for background updates to avoid conflicting with specific requests
            let background_key = format!("{}:background_update", self.config.name);
            let params_for_update = params.clone(); // Clone params for the async block

            // Construct a query_id similar to how get_consensus_data might do it
            // This ensures the cache key matches potential direct queries.
            let query_id = match serde_json::to_string(&params_for_update) {
                Ok(s) => format!("{}:{}", self.config.name, s),
                Err(_) => background_key, // Fallback if params serialization fails
            };

            match self.fetch(&params_for_update).await {
                Ok(data) => {
                    // Cache the fetched data
                    let mut cache = self.cache.lock().unwrap();
                     cache.insert(query_id, CachedData {
                         value: data,
                         timestamp: Instant::now()
                     });
                    if let Some(log) = &self.audit_log {
                        let _ = log.log_external_api("RestApiOracleSource", &format!("Background update success for {}", self.config.name), AuditSeverity::Info);
                    }
                }
                Err(e) => {
                    if let Some(log) = &self.audit_log {
                        let _ = log.log_external_api("RestApiOracleSource", &format!("Background update failed for {}: {}", self.config.name, e), AuditSeverity::Warning);
                    }
                }
            }
        }
    }
}

/// Wrapper to make OracleSource cloneable for Arc
struct CloneableOracleSource(Arc<dyn OracleSource>);

impl Clone for CloneableOracleSource {
    fn clone(&self) -> Self {
        CloneableOracleSource(Arc::clone(&self.0))
    }
}

// --- Oracle Manager --- (Coordinates multiple sources)

pub struct OracleManager {
    sources: HashMap<String, Arc<dyn OracleSource>>,
    audit_log: Option<Arc<SecurityAuditLog>>,
    consensus_threshold: f64, // 0.0 to 1.0
    min_sources_for_consensus: usize,
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
    cache_duration: Duration,
    background_update_interval: Duration,
    background_tasks: tokio::task::JoinHandle<()>, // Handle for background tasks
}

impl OracleManager {
    pub fn new(
        audit_log: Option<Arc<SecurityAuditLog>>,
        consensus_threshold: Option<f64>,
        min_sources_for_consensus: Option<usize>,
        cache_duration: Option<Duration>,
        background_update_interval: Option<Duration>,
    ) -> Self {
        let cache_duration = cache_duration.unwrap_or_else(|| Duration::from_secs(300)); // Default 5 mins
        let background_update_interval = background_update_interval.unwrap_or_else(|| Duration::from_secs(60)); // Default 1 min
        let cache = Arc::new(Mutex::new(HashMap::new()));

        // Spawn a dummy task initially, will be replaced when sources are added
        let background_tasks = tokio::spawn(async {});

        Self {
            sources: HashMap::new(),
            audit_log,
            consensus_threshold: consensus_threshold.unwrap_or(0.51), // Default 51%
            min_sources_for_consensus: min_sources_for_consensus.unwrap_or(2), // Default 2
            cache,
            cache_duration,
            background_update_interval,
            background_tasks,
        }
    }

    pub fn add_source(&mut self, source: Arc<dyn OracleSource>) -> Result<()> {
        let name = source.name().to_string();
        if self.sources.contains_key(&name) {
            return Err(anyhow!("Source '{}' already exists", name));
        }
        self.sources.insert(name.clone(), source.clone());

        // Restart background tasks with the new source
        self.restart_background_tasks();

        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api("OracleManager", &format!("Added source '{}'", name), AuditSeverity::Info);
        }
        Ok(())
    }

    fn restart_background_tasks(&mut self) {
        // Abort existing tasks
        self.background_tasks.abort();

        let sources_clone = self.sources.values().cloned().collect::<Vec<_>>();
        let interval = self.background_update_interval;
        let audit_log_clone = self.audit_log.clone();

        // Spawn new combined task
        self.background_tasks = tokio::spawn(async move {
            if interval.is_zero() {
                return; // No background updates needed
            }
            let mut interval_timer = tokio::time::interval(interval);
            loop {
                interval_timer.tick().await;
                if let Some(log) = &audit_log_clone {
                     let _ = log.log_external_api("OracleManager", "Running background source updates", AuditSeverity::Info);
                 }

                let futures = sources_clone.iter().map(|source| {
                    let source = source.clone(); // Clone Arc for the async block
                    async move {
                        // Use default params if available, otherwise empty JSON object
                        let params = source.config().default_params.clone().unwrap_or_else(|| json!({}));
                        match source.fetch(&params).await {
                            Ok(_) => { /* Data is implicitly cached by fetch */ }
                            Err(e) => {
                                // Log error, status is updated within fetch
                                eprintln!("Background update failed for {}: {}", source.name(), e);
                            }
                        }
                    }
                });
                futures::future::join_all(futures).await;
            }
        });
    }

    pub async fn get_consensus_data(&self, query_id: &str, params: &Value) -> Result<Value> {
        let cache_key = format!("{}:{}", query_id, serde_json::to_string(params)?);

        // Check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached) = cache.get(&cache_key) {
                if cached.timestamp.elapsed() < self.cache_duration {
                    return Ok(cached.value.clone());
                }
            }
        }

        let operational_sources: Vec<_> = self.sources.values()
            .filter(|s| matches!(s.status(), OracleSourceStatus::Operational | OracleSourceStatus::Degraded(_)))
            .cloned()
            .collect();

        if operational_sources.len() < self.min_sources_for_consensus {
             return Err(anyhow!("Insufficient operational sources ({}/{})", operational_sources.len(), self.min_sources_for_consensus));
         }

        let futures = operational_sources.iter().map(|source| {
            let source_clone = source.clone();
            let params_clone = params.clone();
            async move {
                match source_clone.fetch(&params_clone).await {
                    Ok(data) => {
                        let validation_results = source_clone.validate(&data);
                        if validation_results.iter().all(|r| r.passed) {
                            Some((data, source_clone.config().weight))
                        } else {
                            eprintln!("Validation failed for {}", source_clone.name());
                            None
                        }
                    }
                    Err(e) => {
                        eprintln!("Fetch failed for {}: {}", source_clone.name(), e);
                        None
                    }
                }
            }
        });

        let results: Vec<Option<(Value, u8)>> = futures::future::join_all(futures).await;
        let valid_responses: Vec<(Value, u8)> = results.into_iter().flatten().collect();

        if valid_responses.len() < self.min_sources_for_consensus {
            return Err(anyhow!("Insufficient valid responses after fetch/validation ({}/{})", valid_responses.len(), self.min_sources_for_consensus));
        }

        // Calculate total weight of valid responses
        let total_weight: u32 = valid_responses.iter().map(|(_, w)| *w as u32).sum();
        // Calculate total possible weight from all originally operational sources
        let max_possible_weight: u32 = operational_sources.iter().map(|s| s.config().weight as u32).sum();
        let required_weight = (max_possible_weight as f64 * self.consensus_threshold) as u32;

        if total_weight < required_weight {
             return Err(anyhow!("Consensus weight threshold not met ({} < {})", total_weight, required_weight));
         }

        // Determine consensus based on the type of the first valid response
        let consensus_value = match valid_responses.get(0) {
            Some((first_value, _)) => match first_value {
                 Value::Number(_) => self.numerical_consensus(&valid_responses)?,
                 Value::String(_) | Value::Bool(_) | Value::Null => self.categorical_consensus(&valid_responses)?,
                 Value::Object(_) => self.object_consensus(&valid_responses)?,
                 Value::Array(_) => self.array_consensus(&valid_responses)?,
            },
            None => return Err(anyhow!("No valid responses available to determine consensus type")),
        };


        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(cache_key.clone(), CachedData {
                value: consensus_value.clone(),
                timestamp: Instant::now()
            });
        }

        if let Some(log) = &self.audit_log {
             let _ = log.log_external_api("OracleManager", &format!("Consensus reached for '{}'", query_id), AuditSeverity::Info);
         }

        Ok(consensus_value)
    }

    // --- Consensus Helper Functions ---

    fn numerical_consensus(&self, responses: &[(Value, u8)]) -> Result<Value> {
        let mut weighted_values: Vec<(f64, u8)> = responses.iter()
            .filter_map(|(v, w)| v.as_f64().map(|n| (n, *w)))
            .collect();

        if weighted_values.is_empty() {
            return Err(anyhow!("No valid numerical values for consensus"));
        }

        // Basic outlier rejection (IQR)
        weighted_values.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let values: Vec<f64> = weighted_values.iter().map(|(v, _)| *v).collect();
        let q1_idx = (values.len() as f64 * 0.25).floor() as usize;
        let q3_idx = (values.len() as f64 * 0.75).floor() as usize;
        let q1 = values.get(q1_idx).cloned().unwrap_or(0.0);
        let q3 = values.get(q3_idx).cloned().unwrap_or(0.0);
        let iqr = q3 - q1;
        let lower_bound = q1 - 1.5 * iqr;
        let upper_bound = q3 + 1.5 * iqr;

        let filtered_weighted_values: Vec<(f64, u8)> = weighted_values.into_iter()
            .filter(|(v, _)| *v >= lower_bound && *v <= upper_bound)
            .collect();

        if filtered_weighted_values.is_empty() {
            return Err(anyhow!("All numerical values rejected as outliers"));
        }

        // Weighted Median
        let mut sorted_filtered = filtered_weighted_values;
        sorted_filtered.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let total_weight: u32 = sorted_filtered.iter().map(|(_, w)| *w as u32).sum();
        let mid_weight = total_weight / 2;
        let mut current_weight: u32 = 0;

        for (value, weight) in &sorted_filtered {
            current_weight += *weight as u32;
            if current_weight > mid_weight {
                // Safely create Number from f64
                return Ok(serde_json::json!(*value));
            }
        }
        // Fallback: return the last value if loop completes (shouldn't happen with non-zero weight)
        let last_val = sorted_filtered.last().map(|(v,_)| *v).unwrap_or(0.0);
        Ok(serde_json::json!(last_val))
    }

    fn categorical_consensus(&self, responses: &[(Value, u8)]) -> Result<Value> {
        let mut value_weights: HashMap<String, u32> = HashMap::new();
        let mut total_weight: u32 = 0;

        for (value, weight) in responses {
            let key = match value {
                Value::Null => "null".to_string(),
                Value::Bool(b) => b.to_string(),
                Value::String(s) => s.clone(),
                _ => continue,
            };
            *value_weights.entry(key).or_insert(0) += *weight as u32;
            total_weight += *weight as u32;
        }

        if total_weight == 0 {
            return Err(anyhow!("No valid categorical values for consensus"));
        }

        let threshold_weight = (total_weight as f64 * self.consensus_threshold) as u32;

        let consensus_entry = value_weights.into_iter().max_by_key(|&(_, w)| w);

        if let Some((value_str, weight)) = consensus_entry {
            if weight >= threshold_weight {
                match value_str.as_str() {
                    "null" => Ok(Value::Null),
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    s => Ok(Value::String(s.to_string())),
                }
            } else {
                Err(anyhow!("Categorical consensus threshold not met (max weight {} < threshold {})", weight, threshold_weight))
            }
        } else {
            Err(anyhow!("No categorical consensus value found"))
        }
    }

    // Simplified object/array consensus using string representation
    fn object_consensus(&self, responses: &[(Value, u8)]) -> Result<Value> {
        self.stringified_consensus(responses, "object")
    }

    fn array_consensus(&self, responses: &[(Value, u8)]) -> Result<Value> {
        self.stringified_consensus(responses, "array")
    }

    fn stringified_consensus(&self, responses: &[(Value, u8)], value_type: &str) -> Result<Value> {
        let mut value_weights: HashMap<String, u32> = HashMap::new();
        let mut total_weight: u32 = 0;

        for (value, weight) in responses {
            if (value_type == "object" && value.is_object()) || (value_type == "array" && value.is_array()) {
                match serde_json::to_string(value) {
                    Ok(s) => {
                        *value_weights.entry(s).or_insert(0) += *weight as u32;
                        total_weight += *weight as u32;
                    }
                    Err(_) => continue, // Skip if cannot serialize
                }
            }
        }

        if value_weights.is_empty() {
             return Err(anyhow!("No valid {} values for consensus", value_type));
         }
         if total_weight == 0 {
             return Err(anyhow!("Total weight is zero for {} consensus", value_type));
         }

        let threshold_weight = (total_weight as f64 * self.consensus_threshold).ceil() as u32; // Use ceil for threshold
        let consensus_entry = value_weights.into_iter().max_by_key(|&(_, w)| w);

        if let Some((value_str, weight)) = consensus_entry {
            if weight >= threshold_weight {
                serde_json::from_str(&value_str).map_err(|e| anyhow!("Failed to parse consensus {}: {}", value_type, e))
            } else {
                Err(anyhow!("{} consensus threshold not met (max weight {} < threshold {})", value_type, weight, threshold_weight))
            }
        } else {
            Err(anyhow!("No {} consensus value found", value_type))
        }
    }
}

impl Drop for OracleManager {
    fn drop(&mut self) {
        self.background_tasks.abort();
    }
}

// --- Factory Functions --- (Moved from specific API modules)

pub fn create_numeric_range_rule(
    name: &str, min: Option<f64>, max: Option<f64>, error_message: &str
) -> ValidationRule {
    let params = serde_json::json!({ "min": min, "max": max });
    ValidationRule {
        name: name.to_string(),
        rule_type: ValidationRuleType::NumericRange,
        parameters: params,
        error_message: error_message.to_string(),
    }
}

pub fn create_string_pattern_rule(
    name: &str, pattern: Option<&str>, allowed_values: Option<Vec<&str>>, error_message: &str
) -> ValidationRule {
    let params = serde_json::json!({ "pattern": pattern, "allowed": allowed_values });
    ValidationRule {
        name: name.to_string(),
        rule_type: ValidationRuleType::StringPattern,
        parameters: params,
        error_message: error_message.to_string(),
    }
}

pub fn create_weather_api_source(
    api_key: &str,
    audit_log: Option<Arc<SecurityAuditLog>>,
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
    cache_duration: Duration,
) -> Result<RestApiOracleSource> {
    let config = OracleSourceConfig {
        name: "OpenWeatherMap".to_string(),
        url: "https://api.openweathermap.org/data/2.5/weather".to_string(),
        source_type: "REST".to_string(),
        auth_header: None,
        default_params: Some(json!({ "appid": api_key, "units": "metric" })),
        validation_rules: vec![
            create_numeric_range_rule("temp_range", Some(-100.0), Some(100.0), "Temp out of range"),
            create_numeric_range_rule("humidity_range", Some(0.0), Some(100.0), "Humidity out of range"),
        ],
        weight: 100,
        timeout_ms: 5000,
        rate_limit: Some(60),
        requires_auth: true,
        path: vec!["main".to_string()], // Extract the 'main' object
        required_fields: vec!["temp".to_string(), "humidity".to_string()], // Require temp and humidity
    };
    RestApiOracleSource::new(config, audit_log, cache, cache_duration)
}

pub fn create_flight_api_source(
    api_key: &str,
    audit_log: Option<Arc<SecurityAuditLog>>,
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
    cache_duration: Duration,
) -> Result<RestApiOracleSource> {
    let config = OracleSourceConfig {
        name: "AviationStack".to_string(),
        url: "http://api.aviationstack.com/v1/flights".to_string(),
        source_type: "REST".to_string(),
        auth_header: None, // Key is passed as query param
        default_params: Some(json!({ "access_key": api_key })),
        validation_rules: vec![
             create_numeric_range_rule("delay_range", Some(0.0), Some(86400.0*2.0), "Delay out of range"), // Allow up to 2 days delay
             create_string_pattern_rule("status", None, Some(vec!["scheduled", "active", "landed", "cancelled", "incident", "diverted"]), "Invalid status"),
        ],
        weight: 100,
        timeout_ms: 10000,
        rate_limit: Some(100), // Check free tier limits
        requires_auth: true,
         path: vec!["data".to_string(), "0".to_string()], // Extract the first flight object in the 'data' array
         required_fields: vec!["flight_status".to_string(), "departure".to_string(), "arrival".to_string()], // Require status and airport info
    };
    RestApiOracleSource::new(config, audit_log, cache, cache_duration)
}

/// Creates a complete weather oracle manager with multiple sources.
pub fn create_weather_oracle(
    audit_log: Option<Arc<SecurityAuditLog>>,
    cache_duration: Option<Duration>,
    update_interval: Option<Duration>,
) -> Result<OracleManager> {
    let mut manager = OracleManager::new(audit_log.clone(), Some(0.6), Some(1), cache_duration, update_interval);
    let cache = manager.cache.clone(); // Use manager's cache
    let effective_cache_duration = manager.cache_duration;

    // Source 1: OpenWeatherMap
    if let Ok(api_key) = std::env::var("OPENWEATHERMAP_API_KEY") {
        if !api_key.is_empty() {
            match create_weather_api_source(&api_key, audit_log.clone(), cache.clone(), effective_cache_duration) {
                Ok(source) => {
                    println!("Adding OpenWeatherMap source...");
                    manager.add_source(Arc::new(source))?;
                }
                Err(e) => eprintln!("Failed to create OpenWeatherMap source: {}", e),
            }
        } else {
             eprintln!("OPENWEATHERMAP_API_KEY is set but empty, skipping source.");
        }
    } else {
        eprintln!("OPENWEATHERMAP_API_KEY not set, skipping source.");
    }

    // Add more sources here if available (e.g., WeatherAPI, AccuWeather)
    // Ensure they use different API keys and potentially different weights/configs

    if manager.sources.is_empty() {
        eprintln!("WARN: No weather oracle sources could be created. Check API keys and environment variables.");
        // Optionally return an error, or allow the manager to exist with no sources
        // return Err(anyhow!("No weather oracle sources could be created."));
    }

    println!("Weather Oracle Manager created with {} sources.", manager.sources.len());
    Ok(manager)
}

pub async fn create_weather_oracle_async(
    config: &OracleSourceConfig,
    http_client: reqwest::Client,
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
    update_interval: Duration,
    cache_duration: Duration,
) -> Result<Box<dyn OracleSource>> { // Return Box<dyn OracleSource>
    let _config = config.clone(); // Clone config for the closure
    let url_template = config.url.clone();
    let path_template = config.path.clone();

    // Background task to update cache
    let cache_clone = cache.clone();
    let client_clone = http_client.clone();
    let url_clone = url_template.clone(); // Clone for the background task
    let path_clone = path_template.clone(); // Clone for the background task
    tokio::spawn(async move {
        if update_interval == Duration::from_secs(0) {
            return; // No background updates needed
        }
        let mut interval = tokio::time::interval(update_interval);
        loop {
            interval.tick().await;
            // Fetch data (replace with actual API call logic)
            match client_clone.get(&url_clone).send().await {
                Ok(response) => {
                    if let Ok(data) = response.json::<Value>().await {
                         // Extract data using path_clone
                         let mut current = &data;
                         let mut extracted = false;
                         for key in &path_clone {
                             if let Some(obj) = current.as_object() {
                                 if let Some(next) = obj.get(key) {
                                     current = next;
                                     extracted = true; // Mark as extracted if we successfully navigate
                                 } else {
                                     extracted = false; break; // Path broken
                                 }
                             } else {
                                extracted = false; break; // Not an object
                             }
                         }

                        if extracted {
                            let mut cache_guard = cache_clone.lock().unwrap();
                             cache_guard.insert("weather_data".to_string(), CachedData {
                                 value: current.clone(), // Cache the extracted value
                                 timestamp: Instant::now(),
                             });
                         } else {
                            eprintln!("Background weather update: Failed to extract data with path.");
                         }
                    } else {
                        eprintln!("Background weather update: Failed to parse JSON.");
                    }
                }
                Err(e) => {
                    eprintln!("Background weather update failed: {}", e);
                }
            }
        }
    });

    // Return the OracleSource implementation
    let source = SimpleOracleSource {
        name: config.name.clone(),
        url_template,
        path: path_template,
        client: http_client,
        cache,
        cache_duration,
    };

    Ok(Box::new(source) as Box<dyn OracleSource>)
}

// A simplified OracleSource for the async creation function
struct SimpleOracleSource {
    name: String,
    url_template: String,
    path: Vec<String>,
    client: reqwest::Client,
    cache: Arc<Mutex<HashMap<String, CachedData>>>,
    cache_duration: Duration,
}

#[async_trait]
impl OracleSource for SimpleOracleSource {
    fn name(&self) -> &str {
        &self.name
    }

    fn config(&self) -> &OracleSourceConfig {
        // This is simplified; a real implementation would store the full config
         panic!("SimpleOracleSource does not fully store config");
    }

    async fn fetch(&self, _params: &Value) -> Result<Value> { // Changed params to _params
        let cache_key = "weather_data".to_string(); // Simplified key

        // Check cache
        {
            let cache = self.cache.lock().unwrap();
            if let Some(cached_data) = cache.get(&cache_key) {
                 if cached_data.timestamp.elapsed() < self.cache_duration {
                     return Ok(cached_data.value.clone());
                 }
            }
        }

        // Fetch from API (replace with actual logic using url_template and params)
        let response = self.client.get(&self.url_template).send().await?;
        let data = response.json::<Value>().await?;

         // Extract data using path
         let mut current = &data;
         for key in &self.path {
             if let Some(obj) = current.as_object() {
                 current = obj.get(key).ok_or_else(|| anyhow!("Invalid path key: {}", key))?;
             } else {
                 return Err(anyhow!("Expected object in path, found something else"));
             }
         }
         let extracted_value = current.clone();

        // Update cache
        {
            let mut cache = self.cache.lock().unwrap();
            cache.insert(cache_key, CachedData {
                value: extracted_value.clone(),
                timestamp: Instant::now(),
            });
        }

        Ok(extracted_value)
    }

    fn validate(&self, _data: &Value) -> Vec<ValidationResult> {
        // Simplified: Assume valid
        vec![]
    }

    fn status(&self) -> OracleSourceStatus {
        // Simplified: Assume operational
        OracleSourceStatus::Operational
    }

    async fn run_background_updates(&self, _update_interval: Duration) {
        // Background task is already running from the creation function
    }
} 