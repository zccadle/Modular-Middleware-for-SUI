pub mod audit;
pub mod model;
pub mod verification;
pub mod property_tests;
pub mod config;
pub mod byzantine_simulator;

// Re-export security types
pub use audit::{SecurityAuditLog, AuditEvent, AuditEventType, AuditSeverity};
pub use model::{SecurityModel, TrustAssumption, SecurityThreat, SecurityGuarantee, SecurityDelegationWithVerification};
pub use verification::{FormalProperty, PropertyType, VerificationStatus, VerificationResult, VerificationTechnique};
pub use config::{SecurityConfiguration, SecurityLevel};
pub use byzantine_simulator::{ByzantineSimulator, ByzantineNode, ByzantineBehavior};
// Import Byzantine detector types directly from sui module
pub use crate::sui::byzantine::{ByzantineDetector, NodeResponse, NodeResponseStatus};
