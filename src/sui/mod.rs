pub mod tracker;
pub mod contract;
pub mod verification;  // Add this
pub mod network;      // Add this
pub mod byzantine;    // Add this
pub mod cross_chain;  // Add this

// Re-export tracker function
pub use tracker::track_sui_interaction;
// Re-export verification and network types
pub use verification::{VerificationManager, VerificationStatus};
pub use network::{NetworkManager, NetworkType, ChainConfig};
pub use contract::{SuiContract, SuiContractType, SuiContractState};
pub use byzantine::{ByzantineDetector, NodeResponse, NodeResponseStatus};

