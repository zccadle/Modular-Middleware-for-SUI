use std::sync::{Arc, Mutex};
use anyhow::{Result, anyhow};
use reqwest;
use serde::{Serialize, Deserialize};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::str::FromStr;
use sui_sdk::SuiClient;
use sui_sdk::SuiClientBuilder;
use async_trait::async_trait;
use crate::sui::SuiClientProvider;

/// Enum representing different network types that the system can connect to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NetworkType {
    /// Sui Testnet
    Testnet,
    /// Sui Devnet (for development and testing)
    Devnet,
    /// Sui Mainnet (production environment)
    Mainnet,
    /// Local network (for testing and development)
    Local,
    /// Custom network with a specified endpoint
    Custom(String),
}

impl std::fmt::Display for NetworkType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkType::Testnet => write!(f, "Testnet"),
            NetworkType::Devnet => write!(f, "Devnet"),
            NetworkType::Mainnet => write!(f, "Mainnet"),
            NetworkType::Local => write!(f, "Local"),
            NetworkType::Custom(url) => write!(f, "Custom({})", url),
        }
    }
}

impl FromStr for NetworkType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "testnet" => Ok(NetworkType::Testnet),
            "devnet" => Ok(NetworkType::Devnet),
            "mainnet" => Ok(NetworkType::Mainnet),
            "local" => Ok(NetworkType::Local),
            _ if s.starts_with("http://") || s.starts_with("https://") => {
                Ok(NetworkType::Custom(s.to_string()))
            }
            _ => Err(anyhow!("Unknown network type: {}", s)),
        }
    }
}

impl NetworkType {
    /// Get the RPC endpoint URL for the network type
    pub fn get_rpc_url(&self) -> String {
        match self {
            NetworkType::Testnet => "https://fullnode.testnet.sui.io:443".to_string(),
            NetworkType::Devnet => "https://fullnode.devnet.sui.io:443".to_string(),
            NetworkType::Mainnet => "https://fullnode.mainnet.sui.io:443".to_string(),
            NetworkType::Local => "http://localhost:9000".to_string(),
            NetworkType::Custom(url) => url.clone(),
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

/// Configuration for a specific blockchain/network
#[derive(Debug, Clone)]
pub struct ChainConfig {
    /// The type of network
    pub network_type: NetworkType,
    /// Additional configuration parameters specific to this chain
    pub params: HashMap<String, String>,
}

impl ChainConfig {
    pub fn new(network_type: NetworkType) -> Self {
        Self {
            network_type,
            params: HashMap::new(),
        }
    }

    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }
    
    // Helper methods to access common parameters
    pub fn get_chain_id(&self) -> Option<String> {
        self.params.get("chain_id").cloned()
    }
    
    pub fn get_rpc_endpoints(&self) -> Vec<String> {
        if let Some(endpoints) = self.params.get("rpc_endpoints") {
            endpoints.split(',').map(|s| s.trim().to_string()).collect()
        } else {
            vec![self.network_type.get_rpc_url()]
        }
    }
    
    pub fn get_explorer_url(&self) -> Option<String> {
        self.params.get("explorer_url").cloned()
    }
    
    pub fn get_min_gas_price(&self) -> u64 {
        self.params.get("min_gas_price")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1)
    }
    
    pub fn get_recommended_gas_price(&self) -> u64 {
        self.params.get("recommended_gas_price")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(100)
    }
    
    pub fn get_max_gas_price(&self) -> u64 {
        self.params.get("max_gas_price")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(10000)
    }
    
    pub fn get_block_time_ms(&self) -> u64 {
        self.params.get("block_time_ms")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(2000) // Default 2 seconds for Sui
    }
}

/// Manager for handling network connections and client instances
pub struct NetworkManager {
    /// Default network type to use when not specified
    default_network: NetworkType,
    /// Map of network type to SuiClient
    clients: HashMap<NetworkType, Arc<SuiClient>>,
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
    /// Create a new NetworkManager with the specified default network
    pub async fn new(default_network: NetworkType) -> Result<Self> {
        let config = ChainConfig::new(default_network.clone());
        
        let mut manager = Self {
            default_network: default_network.clone(),
            clients: HashMap::new(),
            active_config: Arc::new(Mutex::new(config)),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
            node_status_cache: Arc::new(Mutex::new(HashMap::new())),
            status_cache_ttl: 60, // Cache node status for 60 seconds
        };

        // Initialize the default client
        let client = manager.init_client(default_network).await?;
        manager.clients.insert(manager.default_network.clone(), client);

        Ok(manager)
    }

    /// Initialize a client for the specified network type
    pub async fn init_client(&self, network_type: NetworkType) -> Result<Arc<SuiClient>> {
        let rpc_url = network_type.get_rpc_url();
        println!("Initializing SuiClient for {}: {}", 
            match &network_type {
                NetworkType::Custom(url) => format!("Custom({})", url),
                _ => format!("{:?}", network_type),
            },
            rpc_url
        );

        let client = SuiClientBuilder::default()
            .build(&rpc_url)
            .await
            .map_err(|e| anyhow!("Failed to create SuiClient for {}: {}", rpc_url, e))?;

        Ok(Arc::new(client))
    }

    /// Get the client for the specified network type, initializing it if needed
    pub async fn get_client_for_network(&mut self, network_type: NetworkType) -> Result<Arc<SuiClient>> {
        if !self.clients.contains_key(&network_type) {
            let client = self.init_client(network_type.clone()).await?;
            self.clients.insert(network_type.clone(), client);
        }

        self.clients.get(&network_type)
            .cloned()
            .ok_or_else(|| anyhow!("Failed to get client for network type: {:?}", network_type))
    }

    /// Get the default client
    pub async fn get_default_client(&self) -> Result<Arc<SuiClient>> {
        self.clients.get(&self.default_network)
            .cloned()
            .ok_or_else(|| anyhow!("Default client not initialized"))
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
        let endpoints = config.get_rpc_endpoints();
        if endpoints.is_empty() {
            return Err(anyhow!("No RPC endpoints available for {:?} network", config.network_type));
        }
        
        // Return the first endpoint for now
        // In a more advanced implementation, we'd select the best one
        // based on health checks, latency, etc.
        Ok(endpoints[0].clone())
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
        let endpoints = config.get_rpc_endpoints();
        
        // Try each endpoint until we find a healthy one
        for endpoint in &endpoints {
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
        if !endpoints.is_empty() {
            println!("Warning: No healthy endpoints found. Using first endpoint as fallback.");
            return Ok(endpoints[0].clone());
        }
        
        Err(anyhow!("No RPC endpoints available for {:?} network", config.network_type))
    }
    
    /// Verify that a transaction is targeting the correct chain
    pub fn verify_chain_id(&self, chain_id: &str) -> Result<()> {
        let config = self.active_config.lock().unwrap();
        
        if let Some(config_chain_id) = config.get_chain_id() {
            if config_chain_id != chain_id {
                return Err(anyhow!(
                    "Chain ID mismatch. Transaction targets '{}' but active network is '{}'", 
                    chain_id, 
                    config_chain_id
                ));
            }
        }
        
        Ok(())
    }
    
    /// Get the recommended gas price for the current network
    pub fn get_recommended_gas_price(&self) -> u64 {
        let config = self.active_config.lock().unwrap();
        config.get_recommended_gas_price()
    }
    
    /// Check the health of all configured endpoints
    pub async fn health_check_all(&self) -> HashMap<String, NodeStatus> {
        let config = self.active_config.lock().unwrap();
        let endpoints = config.get_rpc_endpoints();
        let mut results = HashMap::new();
        
        for endpoint in &endpoints {
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
        config.get_block_time_ms()
    }
    
    /// Check if the network is currently available
    pub async fn check_network_status(&self) -> bool {
        match self.get_healthy_rpc_url().await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

#[async_trait]
impl SuiClientProvider for NetworkManager {
    async fn get_client(&self) -> Result<Arc<SuiClient>> {
        self.get_default_client().await
    }

    async fn get_client_for_endpoint(&self, endpoint: &str) -> Result<Arc<SuiClient>> {
        let network_type = NetworkType::from_str(endpoint)?;
        let client = SuiClientBuilder::default()
            .build(&network_type.get_rpc_url())
            .await
            .map_err(|e| anyhow!("Failed to create SuiClient for {}: {}", endpoint, e))?;

        Ok(Arc::new(client))
    }
}
