use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::transaction::types::Transaction;
use crate::security::audit::{SecurityAuditLog, AuditSeverity};
use crate::sui::network::{NetworkManager, NetworkType, ChainConfig};

/// Cross-chain transaction mapping status
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CrossChainStatus {
    /// Transaction mapping is available
    Available,
    /// Transaction mapping is being prepared
    Preparing,
    /// Transaction mapping failed
    Failed(String),
    /// Transaction mapping is not supported
    Unsupported(String),
}

/// Cross-chain transaction format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainTransaction {
    /// Original chain ID
    pub origin_chain_id: String,
    /// Target chain ID
    pub target_chain_id: String,
    /// Original transaction ID/hash
    pub origin_tx_id: String,
    /// Target transaction ID/hash (if executed)
    pub target_tx_id: Option<String>,
    /// Transaction data in target chain format
    pub target_tx_data: Value,
    /// Mapping status
    pub status: CrossChainStatus,
    /// Error message (if any)
    pub error: Option<String>,
    /// Timestamp of creation (UNIX seconds)
    pub created_at: u64,
    /// Timestamp of last update (UNIX seconds)
    pub updated_at: u64,
}

/// Cross-chain mapping trait
#[async_trait]
pub trait CrossChainMapper: Send + Sync {
    /// Check if a transaction can be mapped to a target chain
    async fn can_map(&self, tx: &Transaction, target_chain: &str) -> Result<bool>;
    
    /// Map a transaction to a target chain format
    async fn map_transaction(&self, tx: &Transaction, target_chain: &str) -> Result<CrossChainTransaction>;
    
    /// Execute a mapped transaction on the target chain
    async fn execute_mapped(&self, mapped_tx: &CrossChainTransaction) -> Result<String>;
    
    /// Verify a mapped transaction was executed correctly
    async fn verify_mapped(&self, mapped_tx: &CrossChainTransaction) -> Result<bool>;
}

/// Chain adapter for specific blockchains
#[async_trait]
pub trait ChainAdapter: Send + Sync {
    /// Get the chain ID
    fn chain_id(&self) -> &str;
    
    /// Format a transaction for this chain
    async fn format_transaction(&self, tx: &Transaction) -> Result<Value>;
    
    /// Execute a transaction on this chain
    async fn execute_transaction(&self, tx_data: &Value) -> Result<String>;
    
    /// Get transaction status
    async fn get_transaction_status(&self, tx_hash: &str) -> Result<Value>;
    
    /// Check if a transaction type is supported
    fn supports_transaction_type(&self, tx_type: &str) -> bool;
    
    /// Get the chain's config
    fn get_config(&self) -> ChainConfig;
}

/// SUI chain adapter
pub struct SuiAdapter {
    /// Network manager
    network_manager: Arc<NetworkManager>,
    /// Chain ID
    chain_id: String,
    /// Client for API calls
    client: reqwest::Client,
    /// Audit log
    audit_log: Option<Arc<SecurityAuditLog>>,
}

impl SuiAdapter {
    /// Create a new SUI adapter
    pub fn new(
        network_manager: Arc<NetworkManager>,
        audit_log: Option<Arc<SecurityAuditLog>>,
    ) -> Self {
        let chain_id = network_manager.get_active_config().chain_id.clone();
        Self {
            network_manager,
            chain_id,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            audit_log,
        }
    }
}

#[async_trait]
impl ChainAdapter for SuiAdapter {
    fn chain_id(&self) -> &str {
        &self.chain_id
    }
    
    async fn format_transaction(&self, tx: &Transaction) -> Result<Value> {
        // For simplicity, we'll just convert the transaction to JSON
        let tx_json = serde_json::to_value(tx)?;
        
        // In a real implementation, we would format this for SUI's specific requirements
        // For example, converting to Move call parameters
        
        Ok(tx_json)
    }
    
    async fn execute_transaction(&self, tx_data: &Value) -> Result<String> {
        // In a real implementation, this would make an RPC call to SUI
        // For now, we'll just return a mock transaction hash
        
        // Log the operation
        if let Some(log) = &self.audit_log {
            let _ = log.log_network(
                "SuiAdapter",
                "Executing transaction on SUI",
                Some(self.chain_id()),
                AuditSeverity::Info
            );
        }
        
        // Mock transaction hash
        let tx_hash = format!("sui_tx_{}", rand::random::<u64>());
        
        Ok(tx_hash)
    }
    
    async fn get_transaction_status(&self, tx_hash: &str) -> Result<Value> {
        // In a real implementation, this would query the transaction status from SUI
        // For now, we'll just return a mock status
        
        Ok(serde_json::json!({
            "digest": tx_hash,
            "status": "success",
            "confirmed": true,
            "timestamp_ms": chrono::Utc::now().timestamp_millis()
        }))
    }
    
    fn supports_transaction_type(&self, tx_type: &str) -> bool {
        // SUI supports basic transfer transactions
        matches!(tx_type, "Transfer")
    }
    
    fn get_config(&self) -> ChainConfig {
        self.network_manager.get_active_config()
    }
}

/// Ethereum chain adapter
pub struct EthereumAdapter {
    /// Chain config
    config: ChainConfig,
    /// Client for API calls
    client: reqwest::Client,
    /// Audit log
    audit_log: Option<Arc<SecurityAuditLog>>,
}

impl EthereumAdapter {
    /// Create a new Ethereum adapter
    pub fn new(
        network_type: NetworkType,
        audit_log: Option<Arc<SecurityAuditLog>>,
    ) -> Self {
        let config = match network_type {
            NetworkType::Mainnet => Self::mainnet_config(),
            NetworkType::Testnet => Self::testnet_config(),
            NetworkType::Devnet => Self::devnet_config(),
            NetworkType::Local => Self::local_config(),
        };
        
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            audit_log,
        }
    }
    
    /// Create Ethereum mainnet config
    fn mainnet_config() -> ChainConfig {
        ChainConfig {
            network_type: NetworkType::Mainnet,
            chain_id: "ethereum-mainnet".to_string(),
            rpc_endpoints: vec![
                "https://mainnet.infura.io/v3/YOUR_INFURA_KEY".to_string(),
                "https://eth-mainnet.alchemyapi.io/v2/YOUR_ALCHEMY_KEY".to_string(),
            ],
            explorer_url: Some("https://etherscan.io/tx".to_string()),
            min_gas_price: 1,
            recommended_gas_price: 50,
            max_gas_price: 500,
            block_time_ms: 12000, // ~12 seconds
        }
    }
    
    /// Create Ethereum testnet (Sepolia) config
    fn testnet_config() -> ChainConfig {
        ChainConfig {
            network_type: NetworkType::Testnet,
            chain_id: "ethereum-sepolia".to_string(),
            rpc_endpoints: vec![
                "https://sepolia.infura.io/v3/YOUR_INFURA_KEY".to_string(),
                "https://eth-sepolia.g.alchemy.com/v2/YOUR_ALCHEMY_KEY".to_string(),
            ],
            explorer_url: Some("https://sepolia.etherscan.io/tx".to_string()),
            min_gas_price: 1,
            recommended_gas_price: 20,
            max_gas_price: 100,
            block_time_ms: 12000, // ~12 seconds
        }
    }
    
    /// Create Ethereum devnet config
    fn devnet_config() -> ChainConfig {
        ChainConfig {
            network_type: NetworkType::Devnet,
            chain_id: "ethereum-goerli".to_string(),
            rpc_endpoints: vec![
                "https://goerli.infura.io/v3/YOUR_INFURA_KEY".to_string(),
            ],
            explorer_url: Some("https://goerli.etherscan.io/tx".to_string()),
            min_gas_price: 1,
            recommended_gas_price: 10,
            max_gas_price: 50,
            block_time_ms: 12000, // ~12 seconds
        }
    }
    
    /// Create Ethereum local config
    fn local_config() -> ChainConfig {
        ChainConfig {
            network_type: NetworkType::Local,
            chain_id: "ethereum-local".to_string(),
            rpc_endpoints: vec![
                "http://localhost:8545".to_string(),
            ],
            explorer_url: None,
            min_gas_price: 1,
            recommended_gas_price: 10,
            max_gas_price: 100,
            block_time_ms: 12000, // ~12 seconds
        }
    }
}

#[async_trait]
impl ChainAdapter for EthereumAdapter {
    fn chain_id(&self) -> &str {
        &self.config.chain_id
    }
    
    async fn format_transaction(&self, tx: &Transaction) -> Result<Value> {
        // Convert SUI transaction to Ethereum transaction format
        // In a real implementation, this would map SUI concepts to Ethereum
        
        // For now, we'll create a simple Ethereum transfer transaction
        let eth_tx = serde_json::json!({
            "from": tx.sender,
            "to": tx.receiver,
            "value": format!("0x{:x}", tx.amount),
            "gasPrice": format!("0x{:x}", tx.gas_budget * 1_000_000_000), // Convert to wei
            "gasLimit": "0x5208", // 21000 for standard transfer
            "data": "0x", // Empty data for standard transfer
            "nonce": "0x0", // Would be fetched from the network in real implementation
        });
        
        Ok(eth_tx)
    }
    
    async fn execute_transaction(&self, tx_data: &Value) -> Result<String> {
        // In a real implementation, this would send the transaction to Ethereum
        // For now, we'll just return a mock transaction hash
        
        // Log the operation
        if let Some(log) = &self.audit_log {
            let _ = log.log_network(
                "EthereumAdapter",
                "Executing transaction on Ethereum",
                Some(self.chain_id()),
                AuditSeverity::Info
            );
        }
        
        // Mock transaction hash (Ethereum uses 0x-prefixed hex)
        let tx_hash = format!("0x{:064x}", rand::random::<u64>());
        
        Ok(tx_hash)
    }
    
    async fn get_transaction_status(&self, tx_hash: &str) -> Result<Value> {
        // In a real implementation, this would query the transaction status from Ethereum
        // For now, we'll just return a mock status
        
        Ok(serde_json::json!({
            "hash": tx_hash,
            "status": "0x1", // 0x1 = success, 0x0 = failure
            "blockNumber": format!("0x{:x}", rand::random::<u32>()),
            "confirmations": 10,
            "timestamp": chrono::Utc::now().timestamp()
        }))
    }
    
    fn supports_transaction_type(&self, tx_type: &str) -> bool {
        // Ethereum supports basic transfer transactions
        matches!(tx_type, "Transfer")
    }
    
    fn get_config(&self) -> ChainConfig {
        self.config.clone()
    }
}

/// Cross-chain transaction mapper implementation
pub struct CrossChainMapperImpl {
    /// Map of chain adapters by chain ID
    adapters: Arc<Mutex<HashMap<String, Box<dyn ChainAdapter>>>>,
    /// Map of mapped transactions
    mappings: Arc<Mutex<HashMap<String, CrossChainTransaction>>>,
    /// Network manager for primary chain
    network_manager: Arc<NetworkManager>,
    /// Audit log
    audit_log: Option<Arc<SecurityAuditLog>>,
}

impl CrossChainMapperImpl {
    /// Create a new cross-chain mapper
    pub fn new(
        network_manager: Arc<NetworkManager>,
        audit_log: Option<Arc<SecurityAuditLog>>,
    ) -> Self {
        Self {
            adapters: Arc::new(Mutex::new(HashMap::new())),
            mappings: Arc::new(Mutex::new(HashMap::new())),
            network_manager,
            audit_log,
        }
    }
    
    /// Add a chain adapter
    pub fn add_adapter(&self, adapter: Box<dyn ChainAdapter>) -> Result<()> {
        let chain_id = adapter.chain_id().to_string();
        
        let mut adapters = self.adapters.lock().unwrap();
        
        if adapters.contains_key(&chain_id) {
            return Err(anyhow!("Adapter for chain '{}' already exists", chain_id));
        }
        
        adapters.insert(chain_id.clone(), adapter);
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_network(
                "CrossChainMapper",
                &format!("Added chain adapter for '{}'", chain_id),
                Some(&chain_id),
                AuditSeverity::Info
            );
        }
        
        Ok(())
    }
    
    /// Get a chain adapter by chain ID
    fn get_adapter(&self, chain_id: &str) -> Result<Box<dyn ChainAdapter + Send + Sync>> {
        let adapters = self.adapters.lock().unwrap();
        
        if let Some(adapter) = adapters.get(chain_id) {
            // We can't return a reference to the adapter due to lifetime issues,
            // so we'll create a new adapter of the same type
            if chain_id.contains("sui") {
                // Create a new SUI adapter
                return Ok(Box::new(SuiAdapter::new(
                    self.network_manager.clone(),
                    self.audit_log.clone(),
                )));
            } else if chain_id.contains("ethereum") {
                // Create a new Ethereum adapter with the same network type
                let config = adapter.get_config();
                return Ok(Box::new(EthereumAdapter::new(
                    config.network_type,
                    self.audit_log.clone(),
                )));
            } else {
                return Err(anyhow!("Unsupported chain type for cloning: {}", chain_id));
            }
        } else {
            Err(anyhow!("No adapter found for chain '{}'", chain_id))
        }
    }
    
    /// Create a mapping key from origin and transaction IDs
    fn mapping_key(origin_chain_id: &str, origin_tx_id: &str) -> String {
        format!("{}:{}", origin_chain_id, origin_tx_id)
    }
    
    /// Get a mapped transaction
    pub fn get_mapping(&self, origin_chain_id: &str, origin_tx_id: &str) -> Option<CrossChainTransaction> {
        let key = Self::mapping_key(origin_chain_id, origin_tx_id);
        let mappings = self.mappings.lock().unwrap();
        
        mappings.get(&key).cloned()
    }
    
    /// Initialize common adapters
    pub fn initialize_common_adapters(&self) -> Result<()> {
        // Add SUI adapter
        let sui_adapter = SuiAdapter::new(
            self.network_manager.clone(),
            self.audit_log.clone(),
        );
        
        self.add_adapter(Box::new(sui_adapter))?;
        
        // Add Ethereum adapter (testnet)
        let eth_adapter = EthereumAdapter::new(
            NetworkType::Testnet,
            self.audit_log.clone(),
        );
        
        self.add_adapter(Box::new(eth_adapter))?;
        
        if let Some(log) = &self.audit_log {
            let _ = log.log_network(
                "CrossChainMapper",
                "Initialized common chain adapters",
                None,
                AuditSeverity::Info
            );
        }
        
        Ok(())
    }
    
    /// Get all supported chain IDs
    pub fn get_supported_chains(&self) -> Vec<String> {
        let adapters = self.adapters.lock().unwrap();
        adapters.keys().cloned().collect()
    }
}

#[async_trait]
impl CrossChainMapper for self::CrossChainMapperImpl {
    async fn can_map(&self, tx: &Transaction, target_chain: &str) -> Result<bool> {
        // Get the origin chain ID
        let origin_chain_id = self.network_manager.get_active_config().chain_id.clone();
        
        // Get the target chain adapter
        let target_adapter = self.get_adapter(target_chain)?;
        
        // Check if the transaction type is supported by the target chain
        let tx_type = format!("{:?}", tx.tx_type);
        let supported = target_adapter.supports_transaction_type(&tx_type);
        
        if !supported && self.audit_log.is_some() {
            let log = self.audit_log.as_ref().unwrap();
            let _ = log.log_network(
                "CrossChainMapper",
                &format!("Transaction type '{}' not supported by chain '{}'", tx_type, target_chain),
                Some(target_chain),
                AuditSeverity::Warning
            );
        }
        
        Ok(supported)
    }
    
    async fn map_transaction(&self, tx: &Transaction, target_chain: &str) -> Result<CrossChainTransaction> {
        // First check if mapping is possible
        if !self.can_map(tx, target_chain).await? {
            return Err(anyhow!("Cannot map transaction to chain '{}'", target_chain));
        }
        
        // Get the origin chain ID
        let origin_chain_id = self.network_manager.get_active_config().chain_id.clone();
        
        // Get the target chain adapter
        let target_adapter = self.get_adapter(target_chain)?;
        
        // Format the transaction for the target chain
        let target_tx_data = target_adapter.format_transaction(tx).await?;
        
        // Create a mock origin transaction ID
        let origin_tx_id = format!("origin_tx_{}", rand::random::<u64>());
        
        // Create the mapping
        let now = chrono::Utc::now().timestamp() as u64;
        let mapping = CrossChainTransaction {
            origin_chain_id,
            target_chain_id: target_chain.to_string(),
            origin_tx_id: origin_tx_id.clone(),
            target_tx_id: None,
            target_tx_data,
            status: CrossChainStatus::Preparing,
            error: None,
            created_at: now,
            updated_at: now,
        };
        
        // Store the mapping
        let key = Self::mapping_key(&mapping.origin_chain_id, &mapping.origin_tx_id);
        let mut mappings = self.mappings.lock().unwrap();
        mappings.insert(key, mapping.clone());
        
        // Log the operation
        if let Some(log) = &self.audit_log {
            let _ = log.log_network(
                "CrossChainMapper",
                &format!("Mapped transaction from '{}' to '{}'", 
                    mapping.origin_chain_id, mapping.target_chain_id),
                Some(&mapping.target_chain_id),
                AuditSeverity::Info
            );
        }
        
        Ok(mapping)
    }
    
    async fn execute_mapped(&self, mapped_tx: &CrossChainTransaction) -> Result<String> {
        // Get the target chain adapter
        let target_adapter = self.get_adapter(&mapped_tx.target_chain_id)?;
        
        // Execute the transaction on the target chain
        let target_tx_id = target_adapter.execute_transaction(&mapped_tx.target_tx_data).await?;
        
        // Update the mapping
        let key = Self::mapping_key(&mapped_tx.origin_chain_id, &mapped_tx.origin_tx_id);
        let mut mappings = self.mappings.lock().unwrap();
        
        if let Some(mapping) = mappings.get_mut(&key) {
            mapping.target_tx_id = Some(target_tx_id.clone());
            mapping.status = CrossChainStatus::Available;
            mapping.updated_at = chrono::Utc::now().timestamp() as u64;
        }
        
        // Log the operation
        if let Some(log) = &self.audit_log {
            let _ = log.log_network(
                "CrossChainMapper",
                &format!("Executed mapped transaction on chain '{}': {}",
                    mapped_tx.target_chain_id, target_tx_id),
                Some(&mapped_tx.target_chain_id),
                AuditSeverity::Info
            );
        }
        
        Ok(target_tx_id)
    }
    
    async fn verify_mapped(&self, mapped_tx: &CrossChainTransaction) -> Result<bool> {
        // Make sure we have a target transaction ID
        let target_tx_id = mapped_tx.target_tx_id.clone()
            .ok_or_else(|| anyhow!("No target transaction ID to verify"))?;
        
        // Get the target chain adapter
        let target_adapter = self.get_adapter(&mapped_tx.target_chain_id)?;
        
        // Get the transaction status from the target chain
        let status = target_adapter.get_transaction_status(&target_tx_id).await?;
        
        // Check if the transaction was successful
        let success = if mapped_tx.target_chain_id.contains("ethereum") {
            // Ethereum status: 0x1 = success, 0x0 = failure
            status["status"].as_str().map_or(false, |s| s == "0x1")
        } else if mapped_tx.target_chain_id.contains("sui") {
            // SUI status: check for success field
            status["status"].as_str().map_or(false, |s| s == "success")
        } else {
            // Default implementation for other chains
            status["status"].as_str().map_or(false, |s| s == "success" || s == "confirmed")
        };
        
        // Log the verification result
        if let Some(log) = &self.audit_log {
            if success {
                let _ = log.log_network(
                    "CrossChainMapper",
                    &format!("Verified mapped transaction on chain '{}': {}",
                        mapped_tx.target_chain_id, target_tx_id),
                    Some(&mapped_tx.target_chain_id),
                    AuditSeverity::Info
                );
            } else {
                let _ = log.log_network(
                    "CrossChainMapper",
                    &format!("Mapped transaction failed on chain '{}': {}",
                        mapped_tx.target_chain_id, target_tx_id),
                    Some(&mapped_tx.target_chain_id),
                    AuditSeverity::Error
                );
            }
        }
        
        Ok(success)
    }
}

/// Create a chain mapper with common adapters
pub fn create_chain_mapper(
    network_manager: Arc<NetworkManager>,
    audit_log: Option<Arc<SecurityAuditLog>>,
) -> Result<impl CrossChainMapper> {
    let mapper = CrossChainMapperImpl::new(network_manager, audit_log);
    mapper.initialize_common_adapters()?;
    
    Ok(mapper)
}

/// Example of using cross-chain transaction mapping
pub async fn demonstrate_cross_chain_mapping(
    tx: &Transaction,
    mapper: &(impl CrossChainMapper + 'static),
) -> Result<()> {
    println!("Demonstrating cross-chain transaction mapping");
    
    // Get supported chains
    if let Some(mapper_obj) = (mapper as &dyn std::any::Any).downcast_ref::<CrossChainMapperImpl>() {
        let chains = mapper_obj.get_supported_chains();
        println!("Supported chains: {:?}", chains);
        
        // Choose a target chain (different from origin)
        let origin_chain = mapper_obj.network_manager.get_active_config().chain_id.clone();
        let target_chain = chains.iter()
            .find(|&chain| chain != &origin_chain)
            .ok_or_else(|| anyhow!("No suitable target chain found"))?;
        
        println!("Mapping transaction from {} to {}", origin_chain, target_chain);
        
        // Check if mapping is possible
        if mapper.can_map(tx, target_chain).await? {
            println!("Transaction can be mapped to {}", target_chain);
            
            // Map the transaction
            let mapped_tx = mapper.map_transaction(tx, target_chain).await?;
            println!("Transaction mapped: {:?}", mapped_tx);
            
            // Execute the mapped transaction
            let target_tx_id = mapper.execute_mapped(&mapped_tx).await?;
            println!("Mapped transaction executed: {}", target_tx_id);
            
            // Verify the mapped transaction
            let verified = mapper.verify_mapped(&mapped_tx).await?;
            println!("Mapped transaction verified: {}", verified);
            
            return Ok(());
        } else {
            return Err(anyhow!("Transaction cannot be mapped to {}", target_chain));
        }
    }
    
    Err(anyhow!("Could not downcast mapper to CrossChainMapperImpl"))
}