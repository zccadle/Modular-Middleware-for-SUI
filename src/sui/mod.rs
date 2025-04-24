pub mod tracker;
pub mod contract;
pub mod verification;
pub mod network;
pub mod byzantine;
pub mod cross_chain;
pub mod types;

// Re-export tracker function
pub use tracker::track_sui_interaction;
// Re-export verification and network types
pub use verification::{VerificationManager, VerificationStatus};
pub use network::{NetworkManager, NetworkType, ChainConfig};
pub use contract::{SuiContract, SuiContractType, SuiContractState};
pub use byzantine::{ByzantineDetector, NodeResponse, NodeResponseStatus};

use anyhow::Result;
use sui_sdk::SuiClient;
use std::sync::Arc;

/// Provider for SuiClient - to be implemented by components that can supply a SuiClient
#[async_trait::async_trait]
pub trait SuiClientProvider {
    /// Get a reference to a SuiClient
    async fn get_client(&self) -> Result<Arc<SuiClient>>;
    
    /// Get a reference to a specific SuiClient for a given endpoint
    async fn get_client_for_endpoint(&self, endpoint: &str) -> Result<Arc<SuiClient>>;
}

