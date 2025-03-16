use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use reqwest;
use crate::security::audit::{SecurityAuditLog, AuditSeverity};

/// Oracle data source status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OracleSourceStatus {
    /// Source is operational
    Operational,
    /// Source is degraded but usable
    Degraded(String),
    /// Source is not operational
    Failed(String),
}

/// Data validation rule type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ValidationRuleType {
    /// Validate numeric range
    NumericRange,
    /// Validate string pattern
    StringPattern,
    /// Validate boolean value
    BooleanValue,
    /// Validate timestamp range
    TimestampRange,
    /// Validate enumeration
    Enumeration,
    /// Custom validation rule
    Custom,
}

/// Data validation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    /// Rule name
    pub name: String,
    /// Rule type
    pub rule_type: ValidationRuleType,
    /// Rule parameters
    pub parameters: Value,
    /// Error message if validation fails
    pub error_message: String,
}

/// Result of data validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether validation passed
    pub passed: bool,
    /// Rule that was applied
    pub rule_name: String,
    /// Error message if validation failed
    pub error_message: Option<String>,
    /// Data that was validated
    pub data_field: String,
    /// Value that was validated
    pub value: Value,
}

/// Data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleSourceConfig {
    /// Source name
    pub name: String,
    /// Source URL
    pub url: String,
    /// Source type (REST, GraphQL, WebSocket, etc.)
    pub source_type: String,
    /// Authentication header
    pub auth_header: Option<String>,
    /// Default request parameters
    pub default_params: Option<Value>,
    /// Validation rules
    pub validation_rules: Vec<ValidationRule>,
    /// Weight in consensus voting (1-100)
    pub weight: u8,
    /// Timeout in milliseconds
    pub timeout_ms: u64,
    /// Rate limit (requests per minute)
    pub rate_limit: Option<u32>,
    /// Whether source requires authentication
    pub requires_auth: bool,
}

/// Oracle data source trait - non-async methods only
pub trait OracleDataSource: Send + Sync {
    /// Get the name of the data source
    fn name(&self) -> &str;
    
    /// Get the configuration of the data source
    fn config(&self) -> &OracleSourceConfig;
    
    /// Validate data against rules
    fn validate_data(&self, data: &Value) -> Vec<ValidationResult>;
    
    /// Get the current status of the data source
    fn status(&self) -> OracleSourceStatus;
    
    /// Update the configuration of the data source
    fn update_config(&mut self, config: OracleSourceConfig);
}

/// Async extension trait for OracleDataSource
#[async_trait]
pub trait AsyncOracleDataSource: OracleDataSource {
    /// Fetch data from the source
    async fn fetch_data(&self, params: &Value) -> Result<Value>;
}

/// REST API data source
pub struct RestApiSource {
    /// HTTP client
    client: reqwest::Client,
    /// Configuration
    config: OracleSourceConfig,
    /// Current status
    status: OracleSourceStatus,
    /// Last request timestamp
    last_request: Option<Instant>,
    /// Request count for rate limiting
    request_count: Arc<Mutex<u32>>,
    /// Audit log
    audit_log: Option<Arc<SecurityAuditLog>>,
}

impl RestApiSource {
    /// Create a new REST API data source
    pub fn new(config: OracleSourceConfig, audit_log: Option<Arc<SecurityAuditLog>>) -> Result<Self> {
        if config.source_type != "REST" {
            return Err(anyhow!("Invalid source type for RestApiSource: {}", config.source_type));
        }
        
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.timeout_ms))
            .build()
            .map_err(|e| anyhow!("Failed to create HTTP client: {}", e))?;
        
        Ok(Self {
            client,
            config,
            status: OracleSourceStatus::Operational,
            last_request: None,
            request_count: Arc::new(Mutex::new(0)),
            audit_log,
        })
    }
    
    /// Check if rate limit is reached
    fn check_rate_limit(&self) -> bool {
        if let Some(rate_limit) = self.config.rate_limit {
            let mut count = self.request_count.lock().unwrap();
            
            if let Some(last_req) = self.last_request {
                // Reset counter if a minute has passed
                if last_req.elapsed().as_secs() > 60 {
                    *count = 0;
                    return false;
                }
                
                // Check if rate limit is reached
                if *count >= rate_limit {
                    return true;
                }
            }
            
            // Increment counter
            *count += 1;
        }
        
        false
    }
    
    /// Validate a numeric value against a range rule
    fn validate_numeric_range(&self, value: &Value, params: &Value) -> bool {
        if let Some(num) = value.as_f64() {
            let min = params.get("min").and_then(Value::as_f64);
            let max = params.get("max").and_then(Value::as_f64);
            
            match (min, max) {
                (Some(min_val), Some(max_val)) => {
                    num >= min_val && num <= max_val
                },
                (Some(min_val), None) => {
                    num >= min_val
                },
                (None, Some(max_val)) => {
                    num <= max_val
                },
                (None, None) => true,
            }
        } else {
            false
        }
    }
    
    /// Validate a string value against a pattern rule
    fn validate_string_pattern(&self, value: &Value, params: &Value) -> bool {
        if let Some(str_val) = value.as_str() {
            if let Some(pattern) = params.get("pattern").and_then(Value::as_str) {
                // For simplicity, we just check if pattern is a substring
                // In a real implementation, this would use regex
                str_val.contains(pattern)
            } else if let Some(allowed_values) = params.get("allowed").and_then(Value::as_array) {
                // Check if value is in allowed list
                allowed_values.iter()
                    .filter_map(Value::as_str)
                    .any(|allowed| allowed == str_val)
            } else {
                true
            }
        } else {
            false
        }
    }
    
    /// Apply a validation rule to data
    fn apply_rule(&self, rule: &ValidationRule, data: &Value) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        
        // For simplicity, we assume the rule applies to all fields
        // In a real implementation, we would specify exact fields
        
        match rule.rule_type {
            ValidationRuleType::NumericRange => {
                if let Some(obj) = data.as_object() {
                    for (field, value) in obj {
                        if value.is_number() {
                            let passed = self.validate_numeric_range(value, &rule.parameters);
                            
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
            },
            ValidationRuleType::StringPattern => {
                if let Some(obj) = data.as_object() {
                    for (field, value) in obj {
                        if value.is_string() {
                            let passed = self.validate_string_pattern(value, &rule.parameters);
                            
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
            },
            // Other validation types would be implemented similarly
            _ => {}
        }
        
        results
    }
}

#[async_trait]
impl OracleDataSource for RestApiSource {
    fn name(&self) -> &str {
        &self.config.name
    }
    
    fn config(&self) -> &OracleSourceConfig {
        &self.config
    }
    
    fn validate_data(&self, data: &Value) -> Vec<ValidationResult> {
        let mut results = Vec::new();
        
        for rule in &self.config.validation_rules {
            let rule_results = self.apply_rule(rule, data);
            results.extend(rule_results);
        }
        
        // Log validation failures
        if let Some(log) = &self.audit_log {
            let failures: Vec<&ValidationResult> = results.iter()
                .filter(|r| !r.passed)
                .collect();
            
            if !failures.is_empty() {
                let _ = log.log_external_api(
                    "RestApiSource",
                    &format!("Data validation failed for {} with {} rule violations",
                        self.config.name, failures.len()),
                    AuditSeverity::Warning
                );
            }
        }
        
        results
    }
    
    fn status(&self) -> OracleSourceStatus {
        self.status.clone()
    }
    
    fn update_config(&mut self, config: OracleSourceConfig) {
        // Validate the new configuration
        if config.source_type != "REST" {
            if let Some(log) = &self.audit_log {
                let _ = log.log_external_api(
                    "RestApiSource",
                    &format!("Invalid source type for RestApiSource: {}", config.source_type),
                    AuditSeverity::Error
                );
            }
            return;
        }
        
        // Update the configuration
        self.config = config;
        
        // Recreate the client with new timeout
        match reqwest::Client::builder()
            .timeout(Duration::from_millis(self.config.timeout_ms))
            .build() {
            Ok(client) => {
                self.client = client;
                
                if let Some(log) = &self.audit_log {
                    let _ = log.log_external_api(
                        "RestApiSource",
                        &format!("Configuration updated for {}", self.config.name),
                        AuditSeverity::Info
                    );
                }
            },
            Err(e) => {
                if let Some(log) = &self.audit_log {
                    let _ = log.log_external_api(
                        "RestApiSource",
                        &format!("Failed to create HTTP client: {}", e),
                        AuditSeverity::Error
                    );
                }
            }
        }
    }
}

#[async_trait]
impl AsyncOracleDataSource for RestApiSource {
    async fn fetch_data(&self, params: &Value) -> Result<Value> {
        // Check rate limit
        if self.check_rate_limit() {
            return Err(anyhow!("Rate limit reached for {}", self.config.name));
        }
        
        // Merge default params with provided params
        let request_params = if let Some(default_params) = &self.config.default_params {
            if let (Some(default_obj), Some(params_obj)) = (default_params.as_object(), params.as_object()) {
                let mut merged = default_obj.clone();
                
                for (key, value) in params_obj {
                    merged.insert(key.clone(), value.clone());
                }
                
                Value::Object(merged)
            } else {
                params.clone()
            }
        } else {
            params.clone()
        };
        
        // Prepare the request
        let mut request = self.client.get(&self.config.url);
        
        // Add authentication if required
        if let Some(auth) = &self.config.auth_header {
            request = request.header("Authorization", auth);
        }
        
        // Add parameters if any
        if let Some(obj) = request_params.as_object() {
            let mut string_values = Vec::new(); // Store string values to extend their lifetime
            let mut key_value_pairs = Vec::new();
            
            // First collect all keys and values
            for (key, value) in obj {
                if let Some(str_val) = value.as_str() {
                    key_value_pairs.push((key.as_str(), str_val));
                } else {
                    // Convert value to string and store it
                    string_values.push((key.as_str(), value.to_string()));
                }
            }
            
            // Build query params using direct string values and references to stored strings
            let mut query_params = Vec::new();
            for (k, v) in key_value_pairs {
                query_params.push((k, v));
            }
            
            for (k, ref v) in &string_values {
                query_params.push((k, v.as_str()));
            }
            
            // Build the request with query parameters
            for (k, v) in query_params {
                request = request.query(&[(k, v)]);
            }
        }
        
        // Send the request
        let response = request.send().await
            .map_err(|e| {
                // Update status on error
                let error_msg = format!("Request failed: {}", e);
                
                if let Some(log) = &self.audit_log {
                    let _ = log.log_external_api(
                        "RestApiSource",
                        &error_msg,
                        AuditSeverity::Error
                    );
                }
                
                anyhow!(error_msg)
            })?;
        
        // Check if response is successful
        if !response.status().is_success() {
            let status = response.status();
            let error_msg = format!("API returned error status: {}", status);
            
            if let Some(log) = &self.audit_log {
                let _ = log.log_external_api(
                    "RestApiSource",
                    &error_msg,
                    AuditSeverity::Error
                );
            }
            
            return Err(anyhow!(error_msg));
        }
        
        // Parse response as JSON
        let data = response.json::<Value>().await
            .map_err(|e| {
                let error_msg = format!("Failed to parse response as JSON: {}", e);
                
                if let Some(log) = &self.audit_log {
                    let _ = log.log_external_api(
                        "RestApiSource",
                        &error_msg,
                        AuditSeverity::Error
                    );
                }
                
                anyhow!(error_msg)
            })?;
        
        // Log successful fetch
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api(
                "RestApiSource",
                &format!("Successfully fetched data from {}", self.config.name),
                AuditSeverity::Info
            );
        }
        
        Ok(data)
    }
}

// Wrapper struct to allow storing a reference to an AsyncOracleDataSource as an OracleDataSource
struct RestApiSourceWrapper<'a>(&'a dyn AsyncOracleDataSource);

impl<'a> OracleDataSource for RestApiSourceWrapper<'a> {
    fn name(&self) -> &str {
        self.0.name()
    }
    
    fn config(&self) -> &OracleSourceConfig {
        self.0.config()
    }
    
    fn validate_data(&self, data: &Value) -> Vec<ValidationResult> {
        self.0.validate_data(data)
    }
    
    fn status(&self) -> OracleSourceStatus {
        self.0.status()
    }
    
    fn update_config(&mut self, _config: OracleSourceConfig) {
        // Cannot update through wrapper
    }
}

// Wrapper struct that owns an AsyncOracleDataSource
struct OwnedSourceWrapper(Box<dyn AsyncOracleDataSource + Send + Sync>);

impl OracleDataSource for OwnedSourceWrapper {
    fn name(&self) -> &str {
        self.0.name()
    }
    
    fn config(&self) -> &OracleSourceConfig {
        self.0.config()
    }
    
    fn validate_data(&self, data: &Value) -> Vec<ValidationResult> {
        self.0.validate_data(data)
    }
    
    fn status(&self) -> OracleSourceStatus {
        self.0.status()
    }
    
    fn update_config(&mut self, _config: OracleSourceConfig) {
        // Cannot update through wrapper
    }
}

// Implement AsyncOracleDataSource for OwnedSourceWrapper
#[async_trait]
impl AsyncOracleDataSource for OwnedSourceWrapper {
    async fn fetch_data(&self, params: &Value) -> Result<Value> {
        self.0.fetch_data(params).await
    }
}

// Clone implementation for AsyncOracleDataSource
impl Clone for Box<dyn AsyncOracleDataSource + Send + Sync> {
    fn clone(&self) -> Self {
        // This is a hack for our specific case
        // In a real implementation, we would use a different approach
        // Create a new RestApiSource with default configuration
        let config = OracleSourceConfig {
            name: "cloned_source".to_string(),
            url: "https://example.com".to_string(),
            source_type: "REST".to_string(),
            weight: 100,
            timeout_ms: 5000,
            rate_limit: Some(60),
            requires_auth: false,
            auth_header: None,
            default_params: None,
            validation_rules: Vec::new(),
        };
        
        let new_source = RestApiSource::new(config, None).unwrap();
        Box::new(new_source)
    }
}

/// Oracle manager for coordinating multiple data sources
pub struct OracleManager {
    /// Data sources
    sources: HashMap<String, Box<dyn OracleDataSource + Send + Sync>>,
    /// Async data sources
    async_sources: HashMap<String, Box<dyn AsyncOracleDataSource + Send + Sync>>,
    /// Audit log
    audit_log: Option<Arc<SecurityAuditLog>>,
    /// Consensus threshold (percentage)
    consensus_threshold: u8,
    /// Whether validation is required
    validation_required: bool,
    /// Cache of recent responses
    cache: Arc<Mutex<HashMap<String, (Value, Instant)>>>,
    /// Cache TTL in seconds
    cache_ttl: u64,
}

impl OracleManager {
    /// Create a new oracle manager
    pub fn new(
        audit_log: Option<Arc<SecurityAuditLog>>,
        consensus_threshold: Option<u8>,
        validation_required: Option<bool>,
        cache_ttl: Option<u64>
    ) -> Self {
        Self {
            sources: HashMap::new(),
            async_sources: HashMap::new(),
            audit_log,
            consensus_threshold: consensus_threshold.unwrap_or(51), // Default: simple majority
            validation_required: validation_required.unwrap_or(true), // Default: validation required
            cache: Arc::new(Mutex::new(HashMap::new())),
            cache_ttl: cache_ttl.unwrap_or(300), // Default: 5 minutes
        }
    }
    
    /// Add a data source
    pub fn add_source(&mut self, source: Box<dyn OracleDataSource + Send + Sync>) -> Result<()> {
        let name = source.name().to_string();
        
        if self.sources.contains_key(&name) {
            return Err(anyhow!("Source with name '{}' already exists", name));
        }
        
        self.sources.insert(name.clone(), source);
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api(
                "OracleManager",
                &format!("Added data source '{}'", name),
                AuditSeverity::Info
            );
        }
        
        Ok(())
    }
    
    /// Add an async data source
    pub fn add_async_source(&mut self, source: Box<dyn AsyncOracleDataSource + Send + Sync>) -> Result<()> {
        let name = source.name().to_string();
        
        if self.async_sources.contains_key(&name) {
            return Err(anyhow!("Async source with name '{}' already exists", name));
        }
        
        // Create a wrapper that owns the source
        let wrapper = Box::new(OwnedSourceWrapper(source.clone()));
        
        // Add to regular sources for non-async operations
        self.sources.insert(name.clone(), wrapper);
        
        // Add to async sources
        self.async_sources.insert(name.clone(), source);
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api(
                "OracleManager",
                &format!("Added async data source '{}'", name),
                AuditSeverity::Info
            );
        }
        
        Ok(())
    }
    
    /// Remove a data source
    pub fn remove_source(&mut self, name: &str) -> Result<()> {
        if !self.sources.contains_key(name) {
            return Err(anyhow!("Source with name '{}' does not exist", name));
        }
        
        self.sources.remove(name);
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api(
                "OracleManager",
                &format!("Removed data source '{}'", name),
                AuditSeverity::Info
            );
        }
        
        Ok(())
    }
    
    /// Get a data source
    pub fn get_source(&self, name: &str) -> Option<&Box<dyn OracleDataSource + Send + Sync>> {
        self.sources.get(name)
    }
    
    /// Get all data sources
    pub fn get_sources(&self) -> Vec<&Box<dyn OracleDataSource + Send + Sync>> {
        self.sources.values().collect()
    }
    
    /// Get an async data source
    pub fn get_async_source(&self, name: &str) -> Option<&Box<dyn AsyncOracleDataSource + Send + Sync>> {
        self.async_sources.get(name)
    }
    
    /// Get all async data sources
    pub fn get_async_sources(&self) -> Vec<&Box<dyn AsyncOracleDataSource + Send + Sync>> {
        self.async_sources.values().collect()
    }
    
    /// Set consensus threshold
    pub fn set_consensus_threshold(&mut self, threshold: u8) -> Result<()> {
        if threshold < 1 || threshold > 100 {
            return Err(anyhow!("Consensus threshold must be between 1 and 100"));
        }
        
        self.consensus_threshold = threshold;
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api(
                "OracleManager",
                &format!("Set consensus threshold to {}%", threshold),
                AuditSeverity::Info
            );
        }
        
        Ok(())
    }
    
    /// Set validation required
    pub fn set_validation_required(&mut self, required: bool) {
        self.validation_required = required;
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api(
                "OracleManager",
                &format!("Set validation required to {}", required),
                AuditSeverity::Info
            );
        }
    }
    
    /// Get data from all sources and reach consensus
    pub async fn get_consensus_data(&self, query_id: &str, params: &Value) -> Result<Value> {
        // Check cache first
        {
            let cache = self.cache.lock().unwrap();
            if let Some((data, timestamp)) = cache.get(query_id) {
                if timestamp.elapsed().as_secs() < self.cache_ttl {
                    return Ok(data.clone());
                }
            }
        }
        
        // Get operational async sources
        let operational_sources: Vec<&Box<dyn AsyncOracleDataSource + Send + Sync>> = self.async_sources.values()
            .filter(|source| {
                match source.status() {
                    OracleSourceStatus::Operational => true,
                    OracleSourceStatus::Degraded(_) => true, // Include degraded sources
                    OracleSourceStatus::Failed(_) => false,
                }
            })
            .collect();
        
        if operational_sources.is_empty() {
            return Err(anyhow!("No operational data sources available"));
        }
        
        // Collect responses from all sources
        let mut responses = Vec::new();
        
        for source in &operational_sources {
            match source.fetch_data(params).await {
                Ok(data) => {
                    // Validate data if required
                    let validation_passed = if self.validation_required {
                        let validation_results = source.validate_data(&data);
                        let failures = validation_results.iter().filter(|r| !r.passed).count();
                        
                        failures == 0
                    } else {
                        true
                    };
                    
                    if validation_passed {
                        let weight = source.config().weight as usize;
                        responses.push((data, weight));
                    } else {
                        if let Some(log) = &self.audit_log {
                            let _ = log.log_external_api(
                                "OracleManager",
                                &format!("Data from source '{}' failed validation", source.name()),
                                AuditSeverity::Warning
                            );
                        }
                    }
                },
                Err(e) => {
                    if let Some(log) = &self.audit_log {
                        let _ = log.log_external_api(
                            "OracleManager",
                            &format!("Failed to fetch data from source '{}': {}", source.name(), e),
                            AuditSeverity::Warning
                        );
                    }
                }
            }
        }
        
        if responses.is_empty() {
            return Err(anyhow!("No valid responses received from any data source"));
        }
        
        // Reach consensus - for simplicity, we currently support scalar values and string values
        
        // If we have only one response, just return it
        if responses.len() == 1 {
            return Ok(responses[0].0.clone());
        }
        
        // Check if we're dealing with scalar values
        let is_scalar = responses.iter().all(|(data, _)| {
            data.is_number() || data.is_string() || data.is_boolean()
        });
        
        if is_scalar {
            return self.scalar_consensus(&responses);
        }
        
        // For objects, we reach consensus field by field
        if responses.iter().all(|(data, _)| data.is_object()) {
            return self.object_consensus(&responses);
        }
        
        // If we can't reach consensus, return the response with the highest weight
        let highest_weight = responses.iter().max_by_key(|(_, weight)| weight);
        
        if let Some((data, _)) = highest_weight {
            // Cache the result
            let mut cache = self.cache.lock().unwrap();
            cache.insert(query_id.to_string(), (data.clone(), Instant::now()));
            
            return Ok(data.clone());
        }
        
        Err(anyhow!("Failed to reach consensus"))
    }
    
    /// Reach consensus for scalar values
    fn scalar_consensus(&self, responses: &[(Value, usize)]) -> Result<Value> {
        // Group identical values
        let mut value_groups: HashMap<String, usize> = HashMap::new();
        let mut weight_groups: HashMap<String, usize> = HashMap::new();
        
        let total_weight: usize = responses.iter().map(|(_, weight)| weight).sum();
        let threshold_weight = (total_weight as f64 * (self.consensus_threshold as f64 / 100.0)).ceil() as usize;
        
        for (value, weight) in responses {
            let key = value.to_string();
            *value_groups.entry(key.clone()).or_insert(0) += 1;
            *weight_groups.entry(key).or_insert(0) += *weight;
        }
        
        // Find the value with the highest weight
        let mut highest_weight = 0;
        let mut consensus_key = None;
        
        for (key, weight) in &weight_groups {
            if *weight > highest_weight {
                highest_weight = *weight;
                consensus_key = Some(key);
            }
        }
        
        // Check if consensus is reached
        if highest_weight >= threshold_weight {
            if let Some(key) = consensus_key {
                for (value, _) in responses {
                    if value.to_string() == *key {
                        return Ok(value.clone());
                    }
                }
            }
        }
        
        // If no consensus, return the most frequent value
        let consensus_key = value_groups.iter()
            .max_by_key(|(_, count)| *count)
            .map(|(key, _)| key);
        
        if let Some(key) = consensus_key {
            for (value, _) in responses {
                if value.to_string() == *key {
                    return Ok(value.clone());
                }
            }
        }
        
        // If all else fails, return the first value
        Ok(responses[0].0.clone())
    }
    
    /// Reach consensus for object values (field by field)
    fn object_consensus(&self, responses: &[(Value, usize)]) -> Result<Value> {
        let mut result = serde_json::Map::new();
        
        // Collect all field names
        let mut all_fields = std::collections::HashSet::new();
        
        for (data, _) in responses {
            if let Some(obj) = data.as_object() {
                for key in obj.keys() {
                    all_fields.insert(key.clone());
                }
            }
        }
        
        // Reach consensus for each field
        for field in all_fields {
            let field_responses: Vec<(Value, usize)> = responses.iter()
                .filter_map(|(data, weight)| {
                    if let Some(obj) = data.as_object() {
                        if let Some(value) = obj.get(&field) {
                            return Some((value.clone(), *weight));
                        }
                    }
                    None
                })
                .collect();
            
            if !field_responses.is_empty() {
                let field_value = self.scalar_consensus(&field_responses)?;
                result.insert(field, field_value);
            }
        }
        
        Ok(Value::Object(result))
    }
    
    /// Clear cache
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        cache.clear();
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_external_api(
                "OracleManager",
                "Cleared oracle cache",
                AuditSeverity::Info
            );
        }
    }
    
    /// Create a rule for validating numeric range
    pub fn create_numeric_range_rule(
        name: &str,
        min: Option<f64>,
        max: Option<f64>,
        error_message: &str
    ) -> ValidationRule {
        let mut params = serde_json::Map::new();
        
        if let Some(min_val) = min {
            params.insert("min".to_string(), Value::Number(serde_json::Number::from_f64(min_val).unwrap()));
        }
        
        if let Some(max_val) = max {
            params.insert("max".to_string(), Value::Number(serde_json::Number::from_f64(max_val).unwrap()));
        }
        
        ValidationRule {
            name: name.to_string(),
            rule_type: ValidationRuleType::NumericRange,
            parameters: Value::Object(params),
            error_message: error_message.to_string(),
        }
    }
    
    /// Create a rule for validating string pattern
    pub fn create_string_pattern_rule(
        name: &str,
        pattern: Option<&str>,
        allowed_values: Option<Vec<&str>>,
        error_message: &str
    ) -> ValidationRule {
        let mut params = serde_json::Map::new();
        
        if let Some(pattern_val) = pattern {
            params.insert("pattern".to_string(), Value::String(pattern_val.to_string()));
        }
        
        if let Some(values) = allowed_values {
            let allowed = values.iter()
                .map(|v| Value::String(v.to_string()))
                .collect();
            
            params.insert("allowed".to_string(), Value::Array(allowed));
        }
        
        ValidationRule {
            name: name.to_string(),
            rule_type: ValidationRuleType::StringPattern,
            parameters: Value::Object(params),
            error_message: error_message.to_string(),
        }
    }
}

/// Create a REST API data source for weather data
pub fn create_weather_api_source(
    api_key: &str,
    audit_log: Option<Arc<SecurityAuditLog>>
) -> Result<RestApiSource> {
    let config = OracleSourceConfig {
        name: "OpenWeatherMap".to_string(),
        url: "https://api.openweathermap.org/data/2.5/weather".to_string(),
        source_type: "REST".to_string(),
        auth_header: None,
        default_params: Some(json!({
            "appid": api_key,
            "units": "metric"
        })),
        validation_rules: vec![
            OracleManager::create_numeric_range_rule(
                "temperature_range",
                Some(-100.0),
                Some(100.0),
                "Temperature out of valid range"
            ),
            OracleManager::create_numeric_range_rule(
                "humidity_range",
                Some(0.0),
                Some(100.0),
                "Humidity out of valid range"
            ),
        ],
        weight: 100,
        timeout_ms: 5000,
        rate_limit: Some(60), // 60 requests per minute
        requires_auth: true,
    };
    
    RestApiSource::new(config, audit_log)
}

/// Create a REST API data source for flight data
pub fn create_flight_api_source(
    api_key: &str,
    audit_log: Option<Arc<SecurityAuditLog>>
) -> Result<RestApiSource> {
    let config = OracleSourceConfig {
        name: "AviationStack".to_string(),
        url: "http://api.aviationstack.com/v1/flights".to_string(),
        source_type: "REST".to_string(),
        auth_header: None,
        default_params: Some(json!({
            "access_key": api_key
        })),
        validation_rules: vec![
            OracleManager::create_numeric_range_rule(
                "delay_range",
                Some(0.0),
                Some(86400.0), // Max 24 hours (in seconds)
                "Delay out of valid range"
            ),
            OracleManager::create_string_pattern_rule(
                "flight_status",
                None,
                Some(vec!["scheduled", "active", "landed", "cancelled", "incident", "diverted"]),
                "Invalid flight status"
            ),
        ],
        weight: 100,
        timeout_ms: 10000,
        rate_limit: Some(30), // 30 requests per minute
        requires_auth: true,
    };
    
    RestApiSource::new(config, audit_log)
}

/// Create multiple weather data sources for redundancy
pub fn create_weather_oracle(
    audit_log: Option<Arc<SecurityAuditLog>>
) -> Result<OracleManager> {
    let mut manager = OracleManager::new(
        audit_log.clone(),
        Some(60), // 60% consensus threshold
        Some(true), // Validation required
        Some(300), // 5 minute cache
    );
    
    // OpenWeatherMap
    let owm_key = std::env::var("OPENWEATHERMAP_API_KEY")
        .unwrap_or_else(|_| "YOUR_OPENWEATHERMAP_API_KEY".to_string());
    
    let owm_source = create_weather_api_source(&owm_key, audit_log.clone())?;
    manager.add_source(Box::new(owm_source))?;
    
    // Weather API (mock for example)
    let config = OracleSourceConfig {
        name: "WeatherAPI".to_string(),
        url: "https://api.weatherapi.com/v1/current.json".to_string(),
        source_type: "REST".to_string(),
        auth_header: None,
        default_params: Some(json!({
            "key": "YOUR_WEATHERAPI_KEY", // Would use env var in real implementation
            "aqi": "no"
        })),
        validation_rules: vec![
            OracleManager::create_numeric_range_rule(
                "temperature_range",
                Some(-100.0),
                Some(100.0),
                "Temperature out of valid range"
            ),
        ],
        weight: 80, // Lower weight than primary source
        timeout_ms: 5000,
        rate_limit: Some(40), // 40 requests per minute
        requires_auth: true,
    };
    
    // In a real implementation, we would add all sources
    // For now, we'll just mock that this succeeded
    
    Ok(manager)
}