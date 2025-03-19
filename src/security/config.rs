use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Security level configuration for system benchmarking and operation
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// No security features enabled
    None,
    /// Basic security features
    Basic,
    /// Enhanced security with additional verification
    Enhanced,
    /// Maximum security with all features enabled
    Maximum
}

impl std::fmt::Display for SecurityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityLevel::None => write!(f, "none"),
            SecurityLevel::Basic => write!(f, "basic"),
            SecurityLevel::Enhanced => write!(f, "enhanced"),
            SecurityLevel::Maximum => write!(f, "maximum"),
        }
    }
}

/// Configuration for system security settings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityConfiguration {
    /// Overall security level
    pub level: SecurityLevel,
    /// Number of nodes for Byzantine detection
    pub byzantine_detection_nodes: u32,
    /// Number of sources for data verification
    pub data_verification_sources: u32,
    /// Number of retries for verification
    pub verification_retries: u32,
    /// Whether cross-chain support is enabled
    pub cross_chain_enabled: bool,
    /// Timeout for verification operations (ms)
    pub verification_timeout_ms: u64,
    /// Additional configuration parameters
    pub params: HashMap<String, String>,
}

impl SecurityConfiguration {
    /// Create a standard security configuration (minimal security)
    pub fn standard() -> Self {
        Self {
            level: SecurityLevel::Basic,
            byzantine_detection_nodes: 3,
            data_verification_sources: 1,
            verification_retries: 1,
            cross_chain_enabled: false,
            verification_timeout_ms: 5000,
            params: HashMap::new(),
        }
    }
    
    /// Create an enhanced security configuration (balanced)
    pub fn enhanced() -> Self {
        Self {
            level: SecurityLevel::Enhanced,
            byzantine_detection_nodes: 5,
            data_verification_sources: 3,
            verification_retries: 2,
            cross_chain_enabled: true,
            verification_timeout_ms: 10000,
            params: HashMap::new(),
        }
    }
    
    /// Create a maximum security configuration (highest security)
    pub fn maximum() -> Self {
        Self {
            level: SecurityLevel::Maximum,
            byzantine_detection_nodes: 10,
            data_verification_sources: 5,
            verification_retries: 3,
            cross_chain_enabled: true,
            verification_timeout_ms: 15000,
            params: HashMap::new(),
        }
    }
    
    /// Create a minimal security configuration (for testing, not recommended for production)
    pub fn minimal() -> Self {
        Self {
            level: SecurityLevel::None,
            byzantine_detection_nodes: 1,
            data_verification_sources: 1,
            verification_retries: 0,
            cross_chain_enabled: false,
            verification_timeout_ms: 3000,
            params: HashMap::new(),
        }
    }
    
    /// Create a custom security configuration
    pub fn custom(
        level: SecurityLevel,
        byzantine_detection_nodes: u32,
        data_verification_sources: u32,
        verification_retries: u32,
        cross_chain_enabled: bool,
        verification_timeout_ms: u64,
    ) -> Self {
        Self {
            level,
            byzantine_detection_nodes,
            data_verification_sources,
            verification_retries,
            cross_chain_enabled,
            verification_timeout_ms,
            params: HashMap::new(),
        }
    }
    
    /// Set a custom parameter
    pub fn set_param(&mut self, key: &str, value: &str) -> &mut Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Get security level as string
    pub fn level_str(&self) -> String {
        self.level.to_string()
    }
    
    /// Convert to JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "level": self.level_str(),
            "byzantine_detection_nodes": self.byzantine_detection_nodes,
            "data_verification_sources": self.data_verification_sources,
            "verification_retries": self.verification_retries,
            "cross_chain_enabled": self.cross_chain_enabled,
            "verification_timeout_ms": self.verification_timeout_ms,
            "params": self.params,
        })
    }
} 