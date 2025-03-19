use anyhow::{Result, anyhow};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use reqwest;
use serde_json::{json, Value};
use crate::transaction::types::Transaction;
use crate::security::audit::{SecurityAuditLog, AuditSeverity, AuditEventType};
use crate::sui::verification::VerificationStatus;

/// Maximum allowed discrepancy between node responses (in milliseconds)
const MAX_TIME_DISCREPANCY_MS: u64 = 5000;  // 5 seconds

/// Minimum number of nodes required for quorum
const MIN_QUORUM_SIZE: usize = 2;

/// Response status from a blockchain node
#[derive(Debug, Clone, PartialEq)]
pub enum NodeResponseStatus {
    /// Response was successful and valid
    Valid,
    /// Response had structural errors
    Malformed,
    /// Response did not match consensus
    Inconsistent,
    /// Node failed to respond
    Unavailable,
    /// Response was delayed beyond acceptable threshold
    Delayed,
}

/// Response from a blockchain node
#[derive(Debug, Clone, PartialEq)]
pub struct NodeResponse {
    /// URL of the node
    pub node_url: String,
    /// Status of the response
    pub status: NodeResponseStatus,
    /// Response data (if available)
    pub data: Option<Value>,
    /// Error message (if any)
    pub error: Option<String>,
    /// Response time (if measured)
    pub response_time_ms: Option<u64>,
    /// Timestamp when the response was received
    pub timestamp: Instant,
}

/// Byzantine fault detector for blockchain nodes
pub struct ByzantineDetector {
    /// HTTP client for making RPC requests
    client: reqwest::Client,
    /// RPC endpoints to query
    endpoints: Vec<String>,
    /// Recent response history
    response_history: Arc<Mutex<HashMap<String, Vec<NodeResponse>>>>,
    /// Audit log for security events
    audit_log: Option<Arc<SecurityAuditLog>>,
    /// Maximum response time in milliseconds
    max_response_time_ms: u64,
    /// Cache for transaction results to avoid repeated network calls
    response_cache: Arc<Mutex<HashMap<String, (NodeResponse, Instant)>>>,
    /// Cache TTL (time to live) in seconds
    cache_ttl_seconds: u64,
}

impl ByzantineDetector {
    /// Create a new Byzantine fault detector
    pub fn new(
        endpoints: Vec<String>,
        audit_log: Option<Arc<SecurityAuditLog>>,
        max_response_time_ms: Option<u64>,
        cache_ttl_seconds: Option<u64>
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            endpoints,
            response_history: Arc::new(Mutex::new(HashMap::new())),
            audit_log,
            max_response_time_ms: max_response_time_ms.unwrap_or(10000), // Default 10 seconds
            response_cache: Arc::new(Mutex::new(HashMap::new())),
            cache_ttl_seconds: cache_ttl_seconds.unwrap_or(60), // Default 60 seconds
        }
    }
    
    /// Add endpoints to the detector
    pub fn add_endpoints(&mut self, endpoints: &[String]) {
        for endpoint in endpoints {
            if !self.endpoints.contains(endpoint) {
                self.endpoints.push(endpoint.clone());
            }
        }
    }
    
    /// Remove an endpoint from the detector
    pub fn remove_endpoint(&mut self, endpoint: &str) {
        self.endpoints.retain(|e| e != endpoint);
    }
    
    /// Set the maximum response time
    pub fn set_max_response_time(&mut self, max_response_time_ms: u64) {
        self.max_response_time_ms = max_response_time_ms;
    }
    
    /// Check if a transaction exists across multiple nodes
    pub async fn verify_transaction_existence(&self, digest: &str) -> Result<VerificationStatus> {
        // Check cache first
        {
            let cache = self.response_cache.lock().unwrap();
            if let Some((response, timestamp)) = cache.get(digest) {
                // If cache entry is still valid
                if timestamp.elapsed().as_secs() < self.cache_ttl_seconds {
                    return Ok(self.response_to_verification_status(&response));
                }
            }
        }
        
        // Not in cache or expired, query nodes
        let mut responses = Vec::new();
        
        for endpoint in &self.endpoints {
            let start_time = Instant::now();
            
            match self.query_transaction(endpoint, digest).await {
                Ok(data) => {
                    let elapsed = start_time.elapsed();
                    let elapsed_ms = elapsed.as_millis() as u64;
                    
                    // Check if response time is acceptable
                    let status = if elapsed_ms > self.max_response_time_ms {
                        NodeResponseStatus::Delayed
                    } else {
                        NodeResponseStatus::Valid
                    };
                    
                    let response = NodeResponse {
                        node_url: endpoint.clone(),
                        status,
                        data: Some(data),
                        error: None,
                        response_time_ms: Some(elapsed_ms),
                        timestamp: Instant::now(),
                    };
                    
                    responses.push(response);
                },
                Err(e) => {
                    let elapsed = start_time.elapsed();
                    let elapsed_ms = elapsed.as_millis() as u64;
                    
                    let status = if e.to_string().contains("timeout") {
                        NodeResponseStatus::Delayed
                    } else {
                        NodeResponseStatus::Unavailable
                    };
                    
                    let response = NodeResponse {
                        node_url: endpoint.clone(),
                        status,
                        data: None,
                        error: Some(e.to_string()),
                        response_time_ms: Some(elapsed_ms),
                        timestamp: Instant::now(),
                    };
                    
                    responses.push(response);
                    
                    // Log error
                    if let Some(log) = &self.audit_log {
                        let _ = log.log_validation(
                            "ByzantineDetector",
                            &format!("Node {} failed to respond: {}", endpoint, e),
                            Some(digest),
                            AuditSeverity::Warning
                        );
                    }
                }
            }
        }
        
        // Update response history
        {
            let mut history = self.response_history.lock().unwrap();
            history.insert(digest.to_string(), responses.clone());
        }
        
        // Check for Byzantine behavior
        let (consensus_reached, consensus_response) = self.check_consensus(&responses, digest)?;
        
        // Cache the consensus response
        {
            let mut cache = self.response_cache.lock().unwrap();
            cache.insert(digest.to_string(), (consensus_response.clone(), Instant::now()));
        }
        
        // Return verification status based on consensus
        Ok(self.response_to_verification_status(&consensus_response))
    }
    
    /// Query a transaction from a specific node
    async fn query_transaction(&self, endpoint: &str, digest: &str) -> Result<Value> {
        let params = json!([
            digest,
            {
                "showInput": true,
                "showEffects": true,
                "showEvents": true,
                "showObjectChanges": true,
                "showBalanceChanges": true
            }
        ]);
        
        let response = self.client
            .post(endpoint)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sui_getTransactionBlock",
                "params": params
            }))
            .send()
            .await?;
        
        let result: Value = response.json().await?;
        
        if let Some(error) = result.get("error") {
            return Err(anyhow!("RPC error: {}", error));
        }
        
        if !result["result"].is_object() {
            return Err(anyhow!("Invalid response format"));
        }
        
        Ok(result["result"].clone())
    }
    
    /// Check for consensus among node responses
    fn check_consensus(&self, responses: &[NodeResponse], digest: &str) -> Result<(bool, NodeResponse)> {
        let start_time = Instant::now();
        
        // Filter valid responses
        let valid_responses: Vec<&NodeResponse> = responses.iter()
            .filter(|r| r.status == NodeResponseStatus::Valid)
            .collect();
        let filter_time = start_time.elapsed();
        
        // Record timing information for metrics
        let mut metrics_data = HashMap::new();
        metrics_data.insert("filter_time_ms".to_string(), filter_time.as_millis().to_string());
        
        // If we don't have enough valid responses for quorum
        if valid_responses.len() < MIN_QUORUM_SIZE {
            if let Some(audit_log) = &self.audit_log {
                audit_log.add_event(
                    "ByzantineDetector",
                    AuditEventType::TransactionVerification,
                    AuditSeverity::Warning,
                    &format!("Insufficient valid nodes for consensus ({}/{})", 
                             valid_responses.len(), responses.len())
                );
            }
            return Err(anyhow!("Insufficient valid responses for consensus ({})", valid_responses.len()));
        }
        
        // Start consensus calculation
        let consensus_start = Instant::now();
        
        // Find the most common response data
        let mut data_frequency: HashMap<String, (usize, &NodeResponse)> = HashMap::new();
        
        for resp in &valid_responses {
            if let Some(data) = &resp.data {
                // Normalize data for comparison to handle irrelevant differences
                let normalized = Self::normalize_data_for_comparison(data);
                let data_str = normalized.to_string();
                
                let entry = data_frequency.entry(data_str).or_insert((0, *resp));
                entry.0 += 1;
            }
        }
        
        let data_comparison_time = consensus_start.elapsed();
        metrics_data.insert("data_comparison_time_ms".to_string(), data_comparison_time.as_millis().to_string());
        
        // Find the most frequent response
        let consensus_calculation_start = Instant::now();
        let mut max_frequency = 0;
        let mut consensus_response = None;
        
        for (_, (frequency, resp)) in data_frequency {
            if frequency > max_frequency {
                max_frequency = frequency;
                consensus_response = Some(resp);
            }
        }
        
        let consensus_calculation_time = consensus_calculation_start.elapsed();
        metrics_data.insert("consensus_calculation_time_ms".to_string(), 
                            consensus_calculation_time.as_millis().to_string());
        
        // Check if we have a majority consensus
        let quorum_size = (valid_responses.len() / 2) + 1;
        let has_consensus = max_frequency >= quorum_size;
        
        let total_time = start_time.elapsed();
        metrics_data.insert("total_consensus_check_time_ms".to_string(), 
                           total_time.as_millis().to_string());
        
        // Record operation in security audit log
        if let Some(audit_log) = &self.audit_log {
            audit_log.add_event_with_data(
                "ByzantineDetector",
                AuditEventType::TransactionVerification,
                if has_consensus { AuditSeverity::Info } else { AuditSeverity::Warning },
                &format!("Byzantine consensus check: {} (consensus: {}/{})", 
                        if has_consensus { "success" } else { "failed" },
                        max_frequency, valid_responses.len()),
                metrics_data
            );
        }
        
        match consensus_response {
            Some(resp) => Ok((has_consensus, resp.clone())),
            None => Err(anyhow!("Failed to establish consensus for transaction {}", digest))
        }
    }
    
    /// Normalize transaction data for comparison, removing volatile fields
    fn normalize_data_for_comparison(data: &Value) -> Value {
        if let Some(obj) = data.as_object() {
            let mut normalized = serde_json::Map::new();
            
            for (key, value) in obj {
                // Skip timestamps and other volatile fields
                if key == "timestamp" || key == "id" {
                    continue;
                }
                
                if value.is_object() {
                    normalized.insert(key.clone(), Self::normalize_data_for_comparison(value));
                } else if value.is_array() {
                    if let Some(arr) = value.as_array() {
                        let normalized_arr: Vec<Value> = arr.iter()
                            .map(Self::normalize_data_for_comparison)
                            .collect();
                        normalized.insert(key.clone(), Value::Array(normalized_arr));
                    }
                } else {
                    normalized.insert(key.clone(), value.clone());
                }
            }
            
            Value::Object(normalized)
        } else if let Some(arr) = data.as_array() {
            let normalized_arr: Vec<Value> = arr.iter()
                .map(Self::normalize_data_for_comparison)
                .collect();
            Value::Array(normalized_arr)
        } else {
            data.clone()
        }
    }
    
    /// Convert a node response to a verification status
    fn response_to_verification_status(&self, response: &NodeResponse) -> VerificationStatus {
        match &response.status {
            NodeResponseStatus::Valid => {
                // Check if the transaction was successful
                if let Some(data) = &response.data {
                    if let Some(status) = data.get("status") {
                        if status.get("error").is_some() {
                            return VerificationStatus::Failed(format!("Transaction failed: {}", status));
                        }
                    }
                    
                    VerificationStatus::Verified
                } else {
                    VerificationStatus::Unverifiable("Valid response without data".to_string())
                }
            },
            NodeResponseStatus::Malformed => {
                VerificationStatus::Unverifiable("Malformed response".to_string())
            },
            NodeResponseStatus::Inconsistent => {
                VerificationStatus::Unverifiable("Inconsistent response across nodes".to_string())
            },
            NodeResponseStatus::Unavailable => {
                VerificationStatus::Pending
            },
            NodeResponseStatus::Delayed => {
                VerificationStatus::Unverifiable("Response delayed beyond threshold".to_string())
            },
        }
    }
    
    /// Analyze response timings for potential time-based attacks
    pub fn analyze_timing_attacks(&self, digest: &str) -> Result<bool> {
        let history = self.response_history.lock().unwrap();
        
        if let Some(responses) = history.get(digest) {
            // Get response times
            let response_times: Vec<(String, u64)> = responses.iter()
                .filter_map(|r| {
                    if let Some(time) = r.response_time_ms {
                        Some((r.node_url.clone(), time))
                    } else {
                        None
                    }
                })
                .collect();
            
            // Calculate average and standard deviation
            if response_times.len() >= 2 {
                let sum: u64 = response_times.iter().map(|(_, time)| time).sum();
                let avg = sum as f64 / response_times.len() as f64;
                
                let variance = response_times.iter()
                    .map(|(_, time)| {
                        let diff = *time as f64 - avg;
                        diff * diff
                    })
                    .sum::<f64>() / response_times.len() as f64;
                
                let std_dev = variance.sqrt();
                
                // Check for outliers (more than 2 standard deviations)
                let outliers: Vec<(&String, &u64)> = response_times.iter()
                    .filter(|(_, time)| {
                        let diff = (*time as f64 - avg).abs();
                        diff > 2.0 * std_dev
                    })
                    .map(|(url, time)| (url, time))
                    .collect();
                
                if !outliers.is_empty() {
                    // Log potential timing attack
                    if let Some(log) = &self.audit_log {
                        let outlier_nodes = outliers.iter()
                            .map(|(url, time)| format!("{} ({}ms)", url, time))
                            .collect::<Vec<String>>()
                            .join(", ");
                        
                        let _ = log.log_security_error(
                            "ByzantineDetector",
                            &format!("Potential timing attack detected for {}: outliers [{}], avg={}ms, stddev={}ms",
                                digest, outlier_nodes, avg, std_dev),
                            Some(json!({
                                "outliers": outliers.iter().map(|(url, time)| json!({
                                    "node": url,
                                    "time_ms": time
                                })).collect::<Vec<Value>>(),
                                "avg_ms": avg,
                                "std_dev_ms": std_dev
                            }))
                        );
                    }
                    
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    /// Detect data inconsistencies across nodes
    pub fn detect_data_inconsistencies(&self, digest: &str) -> Result<Vec<String>> {
        let history = self.response_history.lock().unwrap();
        
        if let Some(responses) = history.get(digest) {
            // Filter valid responses with data
            let valid_responses: Vec<&NodeResponse> = responses.iter()
                .filter(|r| r.status == NodeResponseStatus::Valid && r.data.is_some())
                .collect();
            
            // If we don't have enough valid responses
            if valid_responses.len() < 2 {
                return Ok(Vec::new());
            }
            
            // Compare each pair of responses for key fields
            let mut inconsistencies = Vec::new();
            
            for i in 0..valid_responses.len() {
                for j in i+1..valid_responses.len() {
                    let response_i = valid_responses[i];
                    let response_j = valid_responses[j];
                    
                    if let (Some(data_i), Some(data_j)) = (&response_i.data, &response_j.data) {
                        // Check key fields for inconsistencies
                        self.compare_transaction_fields(data_i, data_j)
                            .into_iter()
                            .for_each(|field| {
                                let message = format!(
                                    "Inconsistency in {} between nodes {} and {}",
                                    field, response_i.node_url, response_j.node_url
                                );
                                
                                if !inconsistencies.contains(&message) {
                                    inconsistencies.push(message);
                                }
                            });
                    }
                }
            }
            
            // Log inconsistencies
            if !inconsistencies.is_empty() && self.audit_log.is_some() {
                let log = self.audit_log.as_ref().unwrap();
                
                let _ = log.log_security_error(
                    "ByzantineDetector",
                    &format!("Data inconsistencies detected for {}: {} issues found",
                        digest, inconsistencies.len()),
                    Some(json!({
                        "inconsistencies": inconsistencies
                    }))
                );
            }
            
            return Ok(inconsistencies);
        }
        
        Ok(Vec::new())
    }
    
    /// Compare transaction fields between two responses
    fn compare_transaction_fields(&self, data_i: &Value, data_j: &Value) -> Vec<String> {
        let mut inconsistencies = Vec::new();
        
        // Compare transaction digest
        if let (Some(digest_i), Some(digest_j)) = (
            data_i.get("digest").and_then(Value::as_str),
            data_j.get("digest").and_then(Value::as_str)
        ) {
            if digest_i != digest_j {
                inconsistencies.push("transaction digest".to_string());
            }
        }
        
        // Compare execution status
        if let (Some(status_i), Some(status_j)) = (
            data_i.get("status"),
            data_j.get("status")
        ) {
            let status_str_i = serde_json::to_string(status_i).unwrap_or_default();
            let status_str_j = serde_json::to_string(status_j).unwrap_or_default();
            
            if status_str_i != status_str_j {
                inconsistencies.push("execution status".to_string());
            }
        }
        
        // Compare gas used
        if let (Some(gas_i), Some(gas_j)) = (
            data_i.get("effects").and_then(|e| e.get("gasUsed")),
            data_j.get("effects").and_then(|e| e.get("gasUsed"))
        ) {
            let gas_str_i = serde_json::to_string(gas_i).unwrap_or_default();
            let gas_str_j = serde_json::to_string(gas_j).unwrap_or_default();
            
            if gas_str_i != gas_str_j {
                inconsistencies.push("gas used".to_string());
            }
        }
        
        // Compare balance changes
        if let (Some(changes_i), Some(changes_j)) = (
            data_i.get("balanceChanges"),
            data_j.get("balanceChanges")
        ) {
            let changes_str_i = serde_json::to_string(changes_i).unwrap_or_default();
            let changes_str_j = serde_json::to_string(changes_j).unwrap_or_default();
            
            if changes_str_i != changes_str_j {
                inconsistencies.push("balance changes".to_string());
            }
        }
        
        // Compare object changes
        if let (Some(obj_changes_i), Some(obj_changes_j)) = (
            data_i.get("objectChanges"),
            data_j.get("objectChanges")
        ) {
            let obj_changes_str_i = serde_json::to_string(obj_changes_i).unwrap_or_default();
            let obj_changes_str_j = serde_json::to_string(obj_changes_j).unwrap_or_default();
            
            if obj_changes_str_i != obj_changes_str_j {
                inconsistencies.push("object changes".to_string());
            }
        }
        
        inconsistencies
    }
}

/// Integration with VerificationManager
pub async fn verify_transaction_with_byzantine_detection(
    detector: &ByzantineDetector,
    digest: &str,
) -> Result<VerificationStatus> {
    // First, verify transaction existence across nodes
    let verification_status = detector.verify_transaction_existence(digest).await?;
    
    // If verified, perform additional Byzantine checks
    if verification_status == VerificationStatus::Verified {
        // Check for timing attacks
        let timing_suspicious = detector.analyze_timing_attacks(digest)?;
        
        // Check for data inconsistencies
        let inconsistencies = detector.detect_data_inconsistencies(digest)?;
        
        // If any Byzantine behavior detected, downgrade to Unverifiable
        if timing_suspicious || !inconsistencies.is_empty() {
            let reason = if timing_suspicious && !inconsistencies.is_empty() {
                format!("Byzantine behavior detected: timing anomalies and {} data inconsistencies",
                    inconsistencies.len())
            } else if timing_suspicious {
                "Byzantine behavior detected: timing anomalies".to_string()
            } else {
                format!("Byzantine behavior detected: {} data inconsistencies",
                    inconsistencies.len())
            };
            
            return Ok(VerificationStatus::Unverifiable(reason));
        }
    }
    
    Ok(verification_status)
}