pub mod audit;

// Re-export security types
pub use audit::{SecurityAuditLog, AuditEvent, AuditEventType, AuditSeverity};