use anyhow::{Result, anyhow};
use reqwest;
use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use tokio::time::sleep;

/// Network type for SUI
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NetworkType {
    /// Local development environment
    Local,
    /// SUI devnet for development
    Devnet,
    /// SUI testnet for testing
    Testnet,
    /// SUI mainnet for production
    Mainnet,
}

impl std::fmt::Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkType::Local => write!(f, "local"),
            NetworkType::Devnet => write!(f, "devnet"),
            NetworkType::Testnet => write!(f, "testnet"),
            NetworkType::Mainnet => write!(f, "mainnet"),
        }
    }
}

/// Network node status
#[derive(Debug, Clone, PartialEq)]
pub enum NodeStatus {
    /// Node is operational
    Healthy,
    /// Node is experiencing issues but may still work
    Degraded(String),
    /// Node is not operational
    Down(String),
}

/// Chain details including network parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    /// Network type
    pub network_type: NetworkType,
    /// Chain ID string
    pub chain_id: String,
    /// RPC endpoints for this network
    pub rpc_endpoints: Vec<String>,
    /// Explorer URL for this network
    pub explorer_url: Option<String>,
    /// Minimum gas price for this network
    pub min_gas_price: u64,
    /// Recommended gas price for this network
    pub recommended_gas_price: u64,
    /// Maximum gas price for this network
    pub max_gas_price: u64,
    /// Block confirmation time in milliseconds
    pub block_time_ms: u64,
}

impl ChainConfig {
    /// Create a new chain configuration for the specified network type
    pub fn new(network_type: NetworkType) -> Self {
        match network_type {
            NetworkType::Local => Self::local_network(),
            NetworkType::Devnet => Self::devnet(),
            NetworkType::Testnet => Self::testnet(),
            NetworkType::Mainnet => Self::mainnet(),
        }
    }
    
    /// Create a local network configuration
    fn local_network() -> Self {
        Self {
            network_type: NetworkType::Local,
            chain_id: "local".to_string(),
            rpc_endpoints: vec!["http://localhost:9000".to_string()],
            explorer_url: None,
            min_gas_price: 1,
            recommended_gas_price: 10,
            max_gas_price: 1000,
            block_time_ms: 400,
        }
    }
    
    /// Create a devnet configuration
    fn devnet() -> Self {
        Self {
            network_type: NetworkType::Devnet,
            chain_id: "sui-devnet".to_string(),
            rpc_endpoints: vec![
                "https://fullnode.devnet.sui.io:443".to_string(),
            ],
            explorer_url: Some("https://explorer.sui.io/txblock".to_string()),
            min_gas_price: 1,
            recommended_gas_price: 50,
            max_gas_price: 2000,
            block_time_ms: 400,
        }
    }
    
    /// Create a testnet configuration
    fn testnet() -> Self {
        Self {
            network_type: NetworkType::Testnet,
            chain_id: "sui-testnet".to_string(),
            rpc_endpoints: vec![
                "https://fullnode.testnet.sui.io:443".to_string(),
                "https://sui-testnet.public.blastapi.io".to_string(),
                "https://sui-testnet-rpc.allthatnode.com".to_string(),
            ],
            explorer_url: Some("https://explorer.sui.io/txblock".to_string()),
            min_gas_price: 1,
            recommended_gas_price: 75,
            max_gas_price: 10000,
            block_time_ms: 400,
        }
    }
    
    /// Create a mainnet configuration
    fn mainnet() -> Self {
        Self {
            network_type: NetworkType::Mainnet,
            chain_id: "sui-mainnet".to_string(),
            rpc_endpoints: vec![
                "https://fullnode.mainnet.sui.io:443".to_string(),
                "https://sui-mainnet.public.blastapi.io".to_string(),
                "https://sui-mainnet-rpc.allthatnode.com".to_string(),
            ],
            explorer_url: Some("https://explorer.sui.io/txblock".to_string()),
            min_gas_price: 50,
            recommended_gas_price: 1000,
            max_gas_price: 50000,
            block_time_ms: 400,
        }
    }
    
    /// Generate a transaction explorer URL
    pub fn get_transaction_url(&self, tx_digest: &str) -> Option<String> {
        self.explorer_url.as_ref().map(|url| {
            format!("{}/{}", url, tx_digest)
        })
    }
}

/// Network manager to handle connections to different networks
#[derive(Debug, Clone)]
pub struct NetworkManager {
    /// Current active network configuration
    active_config: Arc<Mutex<ChainConfig>>,
    /// HTTP client for making RPC requests
    client: reqwest::Client,
    /// Cache of node statuses to reduce health check frequency
    node_status_cache: Arc<Mutex<HashMap<String, (NodeStatus, Instant)>>>,
    /// How long to cache node status (in seconds)
    status_cache_ttl: u64,
}

impl NetworkManager {
    /// Create a new network manager with the specified network type
    pub fn new(network_type: NetworkType) -> Self {
        let config = ChainConfig::new(network_type);
        
        Self {
            active_config: Arc::new(Mutex::new(config)),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
            node_status_cache: Arc::new(Mutex::new(HashMap::new())),
            status_cache_ttl: 60, // Cache node status for 60 seconds
        }
    }
    
    /// Switch to a different network
    pub fn switch_network(&self, network_type: NetworkType) -> Result<ChainConfig> {
        let config = ChainConfig::new(network_type);
        
        // Update active configuration
        let mut active_config = self.active_config.lock().unwrap();
        *active_config = config.clone();
        
        // Clear node status cache
        let mut cache = self.node_status_cache.lock().unwrap();
        cache.clear();
        
        Ok(config)
    }
    
    /// Get the current active network configuration
    pub fn get_active_config(&self) -> ChainConfig {
        let config = self.active_config.lock().unwrap();
        config.clone()
    }
    
    /// Get the current active RPC endpoint URL
    pub fn get_active_rpc_url(&self) -> Result<String> {
        let config = self.active_config.lock().unwrap();
        
        // Check if we have any endpoints
        if config.rpc_endpoints.is_empty() {
            return Err(anyhow!("No RPC endpoints available for {} network", config.network_type));
        }
        
        // Return the first endpoint for now
        // In a more advanced implementation, we'd select the best one
        // based on health checks, latency, etc.
        Ok(config.rpc_endpoints[0].clone())
    }
    
    /// Check if a node is healthy
    pub async fn is_node_healthy(&self, rpc_url: &str) -> Result<NodeStatus> {
        // Check cache first
        {
            let cache = self.node_status_cache.lock().unwrap();
            if let Some((status, timestamp)) = cache.get(rpc_url) {
                // If cache hasn't expired, return cached status
                if timestamp.elapsed().as_secs() < self.status_cache_ttl {
                    return Ok(status.clone());
                }
            }
        }
        
        // Perform health check
        match self.client
            .post(rpc_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "rpc.discover",
                "params": []
            }))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    match response.json::<serde_json::Value>().await {
                        Ok(json) => {
                            // Check if the response is valid
                            if json.get("result").is_some() {
                                let status = NodeStatus::Healthy;
                                
                                // Update cache
                                let mut cache = self.node_status_cache.lock().unwrap();
                                cache.insert(rpc_url.to_string(), (status.clone(), Instant::now()));
                                
                                Ok(status)
                            } else {
                                let error_msg = json["error"].to_string();
                                let status = NodeStatus::Degraded(error_msg);
                                
                                // Update cache
                                let mut cache = self.node_status_cache.lock().unwrap();
                                cache.insert(rpc_url.to_string(), (status.clone(), Instant::now()));
                                
                                Ok(status)
                            }
                        },
                        Err(e) => {
                            let status = NodeStatus::Degraded(format!("Invalid response: {}", e));
                            
                            // Update cache
                            let mut cache = self.node_status_cache.lock().unwrap();
                            cache.insert(rpc_url.to_string(), (status.clone(), Instant::now()));
                            
                            Ok(status)
                        }
                    }
                } else {
                    let status = NodeStatus::Down(format!("HTTP error: {}", response.status()));
                    
                    // Update cache
                    let mut cache = self.node_status_cache.lock().unwrap();
                    cache.insert(rpc_url.to_string(), (status.clone(), Instant::now()));
                    
                    Ok(status)
                }
            },
            Err(e) => {
                let status = NodeStatus::Down(format!("Connection error: {}", e));
                
                // Update cache
                let mut cache = self.node_status_cache.lock().unwrap();
                cache.insert(rpc_url.to_string(), (status.clone(), Instant::now()));
                
                Ok(status)
            }
        }
    }
    
    /// Get a healthy RPC endpoint, trying multiple endpoints if needed
    pub async fn get_healthy_rpc_url(&self) -> Result<String> {
        let config = self.active_config.lock().unwrap();
        
        // Try each endpoint until we find a healthy one
        for endpoint in &config.rpc_endpoints {
            match self.is_node_healthy(endpoint).await {
                Ok(NodeStatus::Healthy) => {
                    return Ok(endpoint.clone());
                },
                Ok(NodeStatus::Degraded(_)) => {
                    // If degraded, try the next endpoint
                    continue;
                },
                Ok(NodeStatus::Down(_)) => {
                    // If down, try the next endpoint
                    continue;
                },
                Err(_) => {
                    // If error, try the next endpoint
                    continue;
                }
            }
        }
        
        // If we couldn't find a healthy endpoint, return the first one as a fallback
        if !config.rpc_endpoints.is_empty() {
            println!("Warning: No healthy endpoints found. Using first endpoint as fallback.");
            return Ok(config.rpc_endpoints[0].clone());
        }
        
        Err(anyhow!("No RPC endpoints available for {} network", config.network_type))
    }
    
    /// Verify that a transaction is targeting the correct chain
    pub fn verify_chain_id(&self, chain_id: &str) -> Result<()> {
        let config = self.active_config.lock().unwrap();
        
        if config.chain_id != chain_id {
            return Err(anyhow!(
                "Chain ID mismatch. Transaction targets '{}' but active network is '{}'", 
                chain_id, 
                config.chain_id
            ));
        }
        
        Ok(())
    }
    
    /// Get the recommended gas price for the current network
    pub fn get_recommended_gas_price(&self) -> u64 {
        let config = self.active_config.lock().unwrap();
        config.recommended_gas_price
    }
    
    /// Check the health of all configured endpoints
    pub async fn health_check_all(&self) -> HashMap<String, NodeStatus> {
        let config = self.active_config.lock().unwrap();
        let mut results = HashMap::new();
        
        for endpoint in &config.rpc_endpoints {
            match self.is_node_healthy(endpoint).await {
                Ok(status) => {
                    results.insert(endpoint.clone(), status);
                },
                Err(_) => {
                    results.insert(endpoint.clone(), NodeStatus::Down("Health check failed".to_string()));
                }
            }
        }
        
        results
    }
    
    /// Calculate the estimated transaction finality time in milliseconds
    pub fn get_estimated_finality_ms(&self) -> u64 {
        let config = self.active_config.lock().unwrap();
        
        // For SUI, transactions are typically considered final after 1 block
        // Return the block time as the finality time
        config.block_time_ms
    }
}