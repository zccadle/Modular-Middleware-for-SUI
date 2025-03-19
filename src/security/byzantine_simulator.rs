use anyhow::{Result, anyhow};
use rand::{thread_rng, Rng};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::sui::byzantine::{NodeResponse, NodeResponseStatus};
use crate::security::audit::{SecurityAuditLog, AuditSeverity, AuditEventType};

/// Simulates Byzantine behavior for testing and benchmarking
pub struct ByzantineSimulator {
    normal_nodes: Vec<String>, // RPC endpoints of honest nodes
    byzantine_nodes: Vec<ByzantineNode>, // Simulated Byzantine nodes
    client: reqwest::Client,
    audit_log: Option<Arc<SecurityAuditLog>>,
}

/// Simulated Byzantine node for testing
pub struct ByzantineNode {
    pub endpoint: String,
    pub behavior: ByzantineBehavior,
}

/// Types of Byzantine behavior to simulate
#[derive(Debug, Clone)]
pub enum ByzantineBehavior {
    /// Manipulates response data with given probability
    DataManipulation(f64),
    /// Introduces timing delays (in ms)
    TimingAttack(u64),
    /// Node randomly becomes unavailable with given probability
    Unavailability(f64),
    /// Node gives inconsistent responses with given probability
    Inconsistency(f64),
}

impl ByzantineSimulator {
    /// Create a new Byzantine simulator
    pub fn new(normal_nodes: Vec<String>, audit_log: Option<Arc<SecurityAuditLog>>) -> Self {
        Self {
            normal_nodes,
            byzantine_nodes: Vec::new(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            audit_log,
        }
    }
    
    /// Add a Byzantine node with specified behavior
    pub fn add_byzantine_node(&mut self, endpoint: &str, behavior: ByzantineBehavior) {
        self.byzantine_nodes.push(ByzantineNode {
            endpoint: endpoint.to_string(),
            behavior: behavior.clone(),
        });
        
        if let Some(audit_log) = &self.audit_log {
            audit_log.add_event(
                "ByzantineSimulator",
                AuditEventType::ConfigChange, 
                AuditSeverity::Info,
                &format!("Added Byzantine node {} with {:?} behavior", endpoint, behavior)
            );
        }
    }
    
    /// Get all endpoints (normal + byzantine)
    pub fn get_all_endpoints(&self) -> Vec<String> {
        let mut endpoints = self.normal_nodes.clone();
        for node in &self.byzantine_nodes {
            endpoints.push(node.endpoint.clone());
        }
        endpoints
    }
    
    /// Query all nodes for a transaction digest
    pub async fn query_transaction(&self, digest: &str) -> Result<Vec<NodeResponse>> {
        let mut responses = Vec::new();
        
        // First query honest nodes
        for endpoint in &self.normal_nodes {
            match self.query_honest_node(endpoint, digest).await {
                Ok(data) => {
                    let response_time = thread_rng().gen_range(50, 150);
                    responses.push(NodeResponse {
                        node_url: endpoint.clone(),
                        status: NodeResponseStatus::Valid,
                        data: Some(data),
                        error: None,
                        response_time_ms: Some(response_time),
                        timestamp: Instant::now(),
                    });
                },
                Err(e) => {
                    responses.push(NodeResponse {
                        node_url: endpoint.clone(),
                        status: NodeResponseStatus::Unavailable,
                        data: None,
                        error: Some(e.to_string()),
                        response_time_ms: None,
                        timestamp: Instant::now(),
                    });
                }
            }
        }
        
        // Then query Byzantine nodes (with simulated behaviors)
        for node in &self.byzantine_nodes {
            match self.query_byzantine_node(node, digest).await {
                Ok(response) => responses.push(response),
                Err(e) => {
                    if let Some(audit_log) = &self.audit_log {
                        audit_log.add_event(
                            "ByzantineSimulator",
                            AuditEventType::SecurityError, 
                            AuditSeverity::Warning,
                            &format!("Error from Byzantine node {}: {}", node.endpoint, e)
                        );
                    }
                }
            }
        }
        
        Ok(responses)
    }
    
    /// Query an honest node for transaction data
    async fn query_honest_node(&self, endpoint: &str, digest: &str) -> Result<Value> {
        // This is a simulation - in reality this would make an actual RPC call
        // Create a simulated honest response
        let mut rng = thread_rng();
        
        // Simulate different transaction types
        let transaction_type = match rng.gen_range(0, 3) {
            0 => "transfer",
            1 => "invoke",
            _ => "custom",
        };
        
        // Create a fake valid response
        let response = json!({
            "digest": digest,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "transaction": {
                "type": transaction_type,
                "sender": format!("0x{:x}", rng.gen::<u64>()),
                "receiver": format!("0x{:x}", rng.gen::<u64>()),
                "amount": rng.gen_range(1, 1000),
                "gas_fee": rng.gen_range(1, 50),
            },
            "status": "success",
            "block_height": rng.gen_range(1000, 10000),
            "confirmed": true
        });
        
        Ok(response)
    }
    
    /// Query a Byzantine node with simulated faulty behavior
    async fn query_byzantine_node(&self, node: &ByzantineNode, digest: &str) -> Result<NodeResponse> {
        let mut rng = thread_rng();
        let start_time = Instant::now();
        
        // First get a legitimate response
        let base_data = self.query_honest_node(&node.endpoint, digest).await?;
        
        // Apply Byzantine behavior based on node type
        match &node.behavior {
            ByzantineBehavior::DataManipulation(probability) => {
                if rng.gen::<f64>() < *probability {
                    // Manipulate the data
                    let mut data = base_data.clone();
                    
                    // Change transaction amount or other fields
                    if let Some(obj) = data.as_object_mut() {
                        if let Some(tx) = obj.get_mut("transaction") {
                            if let Some(tx_obj) = tx.as_object_mut() {
                                if let Some(amount) = tx_obj.get_mut("amount") {
                                    // Double the amount
                                    if let Some(n) = amount.as_u64() {
                                        *amount = json!(n * 2);
                                    }
                                }
                                
                                // Change the transaction type
                                if let Some(tx_type) = tx_obj.get_mut("type") {
                                    *tx_type = json!("manipulated");
                                }
                            }
                        }
                    }
                    
                    return Ok(NodeResponse {
                        node_url: node.endpoint.clone(),
                        status: NodeResponseStatus::Inconsistent,
                        data: Some(data),
                        error: None,
                        response_time_ms: Some(start_time.elapsed().as_millis() as u64),
                        timestamp: Instant::now(),
                    });
                }
            },
            ByzantineBehavior::TimingAttack(delay_ms) => {
                // Simulate a delay
                tokio::time::sleep(Duration::from_millis(*delay_ms)).await;
                
                return Ok(NodeResponse {
                    node_url: node.endpoint.clone(),
                    status: NodeResponseStatus::Delayed,
                    data: Some(base_data),
                    error: None,
                    response_time_ms: Some(*delay_ms + start_time.elapsed().as_millis() as u64),
                    timestamp: Instant::now(),
                });
            },
            ByzantineBehavior::Unavailability(probability) => {
                if rng.gen::<f64>() < *probability {
                    // Simulate node being unavailable
                    return Ok(NodeResponse {
                        node_url: node.endpoint.clone(),
                        status: NodeResponseStatus::Unavailable,
                        data: None,
                        error: Some("Node is unavailable".to_string()),
                        response_time_ms: Some(start_time.elapsed().as_millis() as u64),
                        timestamp: Instant::now(),
                    });
                }
            },
            ByzantineBehavior::Inconsistency(probability) => {
                if rng.gen::<f64>() < *probability {
                    // Return malformed data
                    return Ok(NodeResponse {
                        node_url: node.endpoint.clone(),
                        status: NodeResponseStatus::Malformed,
                        data: Some(json!({
                            "error": "Malformed response",
                            "status": "error",
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        })),
                        error: Some("Invalid response format".to_string()),
                        response_time_ms: Some(start_time.elapsed().as_millis() as u64),
                        timestamp: Instant::now(),
                    });
                }
            },
        }
        
        // Default case: return honest response
        Ok(NodeResponse {
            node_url: node.endpoint.clone(),
            status: NodeResponseStatus::Valid,
            data: Some(base_data),
            error: None,
            response_time_ms: Some(start_time.elapsed().as_millis() as u64),
            timestamp: Instant::now(),
        })
    }
}

// Helper functions to create different Byzantine node configurations
pub fn create_data_manipulation_node(endpoint: &str, probability: f64) -> ByzantineNode {
    ByzantineNode {
        endpoint: endpoint.to_string(),
        behavior: ByzantineBehavior::DataManipulation(probability),
    }
}

pub fn create_timing_attack_node(endpoint: &str, delay_ms: u64) -> ByzantineNode {
    ByzantineNode {
        endpoint: endpoint.to_string(),
        behavior: ByzantineBehavior::TimingAttack(delay_ms),
    }
}

pub fn create_unavailable_node(endpoint: &str, probability: f64) -> ByzantineNode {
    ByzantineNode {
        endpoint: endpoint.to_string(),
        behavior: ByzantineBehavior::Unavailability(probability),
    }
}

pub fn create_inconsistent_node(endpoint: &str, probability: f64) -> ByzantineNode {
    ByzantineNode {
        endpoint: endpoint.to_string(),
        behavior: ByzantineBehavior::Inconsistency(probability),
    }
} 