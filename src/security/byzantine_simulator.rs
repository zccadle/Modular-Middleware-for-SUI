use anyhow::Result;
use rand::{thread_rng, Rng};
use serde_json::json;
use std::time::{Duration, Instant};
use std::sync::Arc;
use crate::security::audit::{SecurityAuditLog, AuditSeverity, AuditEventType};
use std::collections::HashMap;
use tokio::sync::Mutex;

// Import Byzantine detector types with renamed imports to avoid conflicts
use crate::sui::byzantine::{NodeResponse, NodeResponseStatus, ByzantineDetector};

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

/// Result of a byzantine detection operation
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Timestamp of the detection
    pub timestamp: u64,
    /// Number of nodes analyzed
    pub nodes_analyzed: usize,
    /// Number of nodes flagged as byzantine
    pub byzantine_nodes: usize,
    /// List of byzantine node IDs
    pub byzantine_node_ids: Vec<String>,
    /// Detection details
    pub details: String,
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
    
    /// Simulate responses from nodes (for testing byzantine behavior)
    pub async fn simulate_responses(&self, _endpoint: &str, node_count: usize) -> Vec<NodeResponse> {
        let mut responses = Vec::new();
        let mut rng = thread_rng();
        
        let majority_data = format!("{}", rng.gen_range(1, 1000));
        
        for i in 0..node_count {
            let node_id = format!("node_{}", i);
            let is_byzantine = rng.gen::<f64>() < 0.2; // 20% chance of being byzantine
            
            let response = if is_byzantine {
                // Generate different data for byzantine nodes
                let byzantine_data = format!("{}", rng.gen_range(1001, 2000));
                NodeResponse {
                    node_url: node_id,
                    status: NodeResponseStatus::Inconsistent,
                    data: Some(serde_json::from_str(&byzantine_data).unwrap_or(serde_json::Value::Null)),
                    error: None,
                    response_time_ms: Some(rng.gen_range(100, 500)),
                    timestamp: Instant::now(),
                }
            } else {
                // Generate consistent data for honest nodes
                NodeResponse {
                    node_url: node_id,
                    status: NodeResponseStatus::Valid,
                    data: Some(serde_json::from_str(&majority_data).unwrap_or(serde_json::Value::Null)),
                    error: None,
                    response_time_ms: Some(rng.gen_range(50, 200)),
                    timestamp: Instant::now(),
                }
            };
            
            responses.push(response);
        }
        
        responses
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