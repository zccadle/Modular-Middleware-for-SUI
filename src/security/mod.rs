pub mod audit;
pub mod model;
pub mod verification;
pub mod property_tests;

// Re-export security types
pub use audit::{SecurityAuditLog, AuditEvent, AuditEventType, AuditSeverity};
pub use model::{SecurityModel, TrustAssumption, SecurityThreat, SecurityGuarantee, SecurityDelegationWithVerification};
pub use verification::{FormalProperty, PropertyType, VerificationStatus, VerificationResult, VerificationTechnique};
