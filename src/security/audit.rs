//! Security Audit Logging Module
//!
//! Provides capabilities for logging security-relevant events.
//! Supports configurable destinations (console, file) and severity levels.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Types of events recorded by the audit log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AuditEventType {
    TransactionValidation,
    TransactionExecution,
    TransactionVerification,
    NetworkOperation,
    ExternalAPI,
    Authentication,
    Authorization,
    ConfigChange,
    SecurityError,
    // Add more specific types if needed
}

/// Severity levels for audit events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuditSeverity {
    Info,    // Informational messages
    Warning, // Potential issues
    Error,   // Recoverable errors or significant issues
    Critical, // Critical security events, potential system compromise
}

/// Represents a single security audit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    #[serde(with = "chrono::serde::ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub event_type: AuditEventType,
    pub severity: AuditSeverity,
    pub source: String, // Component or module originating the event
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    pub message: String, // Detailed description of the event
    #[serde(skip_serializing_if = "Value::is_null")]
    pub context: Value, // Additional structured data (JSON)
}

impl AuditEvent {
    /// Creates a new audit event.
    pub fn new(event_type: AuditEventType, severity: AuditSeverity, source: &str, message: &str) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            severity,
            source: source.to_string(),
            transaction_id: None,
            chain_id: None,
            message: message.to_string(),
            context: Value::Null,
        }
    }

    /// Associates a transaction ID with the event.
    pub fn with_transaction_id(mut self, tx_id: &str) -> Self {
        self.transaction_id = Some(tx_id.to_string());
        self
    }

    /// Associates a chain ID with the event.
    pub fn with_chain_id(mut self, chain_id: &str) -> Self {
        self.chain_id = Some(chain_id.to_string());
        self
    }

    /// Adds structured context data (key-value) to the event.
    pub fn with_context(mut self, key: &str, value: impl Serialize) -> Self {
        if self.context.is_null() {
            self.context = serde_json::json!({});
        }
        if let Some(obj) = self.context.as_object_mut() {
            if let Ok(value_json) = serde_json::to_value(value) {
                obj.insert(key.to_string(), value_json);
            }
        }
        self
    }

    /// Converts the event to a structured JSON value.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({ "error": "Failed to serialize AuditEvent" }))
    }

    /// Formats the event into a single log string.
    pub fn to_log_string(&self) -> String {
        format!(
            "[{}] [{:<8}] [{:<25}] {}: {} {}",
            self.timestamp.to_rfc3339(),
            format!("{:?}", self.severity).to_uppercase(),
            format!("{:?}", self.event_type),
            self.source,
            self.message,
            self.format_context()
        )
    }

    /// Helper to format context for the log string.
    fn format_context(&self) -> String {
        let mut parts = Vec::new();
        if let Some(tx_id) = &self.transaction_id {
            parts.push(format!("tx_id={}", tx_id));
        }
        if let Some(chain_id) = &self.chain_id {
            parts.push(format!("chain={}", chain_id));
        }
        if !self.context.is_null() {
            parts.push(format!("ctx={}", serde_json::to_string(&self.context).unwrap_or_default()));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("({})", parts.join(", "))
        }
    }
}

/// Configuration for the `SecurityAuditLog`.
#[derive(Debug, Clone)]
pub struct AuditLogConfig {
    pub console_enabled: bool,
    pub file_enabled: bool,
    pub log_file_path: Option<PathBuf>,
    pub min_severity: AuditSeverity,
}

impl Default for AuditLogConfig {
    fn default() -> Self {
        Self {
            console_enabled: true,
            file_enabled: true,
            log_file_path: Some("security_audit.log".into()),
            min_severity: AuditSeverity::Info,
        }
    }
}

/// Thread-safe system for recording security audit events.
#[derive(Debug, Clone)]
pub struct SecurityAuditLog {
    config: Arc<Mutex<AuditLogConfig>>,
    events: Arc<Mutex<Vec<AuditEvent>>>,
    max_events: usize,
}

impl SecurityAuditLog {
    /// Creates a new `SecurityAuditLog` with default configuration.
    pub fn new() -> Self {
        Self::with_config(AuditLogConfig::default())
    }

    /// Creates a new `SecurityAuditLog` with the specified configuration.
    pub fn with_config(config: AuditLogConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            events: Arc::new(Mutex::new(Vec::new())),
            max_events: 1000,
        }
    }

    /// Updates the logger configuration dynamically.
    pub fn update_config(&self, new_config: AuditLogConfig) -> Result<(), String> {
        self.config.lock().map(|mut guard| *guard = new_config)
             .map_err(|e| format!("Failed to acquire lock for config update: {}", e))
    }

    /// Logs an `AuditEvent` if its severity meets the configured minimum.
    pub fn log_event(&self, event: AuditEvent) -> Result<()> {
        let config = self.config.lock().map_err(|e| anyhow!("Config lock poisoned: {}", e))?;

        if event.severity < config.min_severity {
            return Ok(());
        }

        if config.console_enabled {
            println!("{}", event.to_log_string());
        }

        if config.file_enabled {
            if let Some(path) = &config.log_file_path {
                match OpenOptions::new().create(true).append(true).open(path) {
                    Ok(file) => {
                        let mut writer = BufWriter::new(file);
                        if let Err(e) = writeln!(writer, "{}", event.to_log_string()) {
                            eprintln!("ERROR: Failed to write audit event to file {:?}: {}", path, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("ERROR: Failed to open audit log file {:?}: {}", path, e);
                    }
                }
            }
        }

        if let Ok(mut events_guard) = self.events.lock() {
            events_guard.push(event);
            if events_guard.len() > self.max_events {
                events_guard.remove(0);
            }
        } else {
            eprintln!("ERROR: Events mutex poisoned. Event not added to in-memory buffer.");
        }

        Ok(())
    }

    /// Retrieves a clone of all audit events currently held in the in-memory buffer.
    pub fn get_events(&self) -> Vec<AuditEvent> {
        self.events.lock().map_or_else(
            |poisoned| {
                eprintln!("ERROR: Events mutex poisoned while getting events: {}", poisoned);
                Vec::new()
            },
            |guard| guard.clone(),
        )
    }

    /// Filters in-memory events by severity.
    pub fn get_events_by_severity(&self, severity: AuditSeverity) -> Vec<AuditEvent> {
        self.get_events().into_iter().filter(|e| e.severity == severity).collect()
    }

    /// Filters in-memory events by type.
    pub fn get_events_by_type(&self, event_type: AuditEventType) -> Vec<AuditEvent> {
        self.get_events().into_iter().filter(|e| e.event_type == event_type).collect()
    }

    /// Filters in-memory events by transaction ID.
    pub fn get_events_by_transaction(&self, transaction_id: &str) -> Vec<AuditEvent> {
        self.get_events().into_iter()
            .filter(|e| e.transaction_id.as_deref() == Some(transaction_id))
            .collect()
    }

    /// Exports all events from the in-memory buffer to a JSON file.
    pub fn export_events_to_json(&self, path: &str) -> Result<()> {
        let events = self.get_events();
        let json_value = serde_json::to_value(&events)?;
        let json_string = serde_json::to_string_pretty(&json_value)?;

        let mut file = File::create(path)?;
        file.write_all(json_string.as_bytes())?;
        Ok(())
    }

    /// Clears all events from the in-memory buffer.
    pub fn clear_events(&self) {
        if let Ok(mut events_guard) = self.events.lock() {
            events_guard.clear();
        } else {
            eprintln!("ERROR: Events mutex poisoned while clearing events.");
        }
    }

    /// Convenience logging methods for different event types
    
    /// Log a transaction validation event
    pub fn log_validation(&self, source: &str, message: &str, tx_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let event = AuditEvent::new(
            AuditEventType::TransactionValidation,
            severity,
            source,
            message,
        ).with_transaction_id(tx_id.unwrap_or(""));
        self.log_event(event)
    }
    
    /// Log a transaction execution event
    pub fn log_execution(&self, source: &str, message: &str, tx_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let event = AuditEvent::new(
            AuditEventType::TransactionExecution,
            severity,
            source,
            message,
        ).with_transaction_id(tx_id.unwrap_or(""));
        self.log_event(event)
    }
    
    /// Log a transaction verification event
    pub fn log_verification(&self, source: &str, message: &str, tx_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let event = AuditEvent::new(
            AuditEventType::TransactionVerification,
            severity,
            source,
            message,
        ).with_transaction_id(tx_id.unwrap_or(""));
        self.log_event(event)
    }
    
    /// Log a network operation event
    pub fn log_network(&self, source: &str, message: &str, chain_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let event = AuditEvent::new(
            AuditEventType::NetworkOperation,
            severity,
            source,
            message,
        ).with_chain_id(chain_id.unwrap_or(""));
        self.log_event(event)
    }
    
    /// Log an external API call event
    pub fn log_external_api(&self, source: &str, message: &str, severity: AuditSeverity) -> Result<()> {
        let event = AuditEvent::new(
            AuditEventType::ExternalAPI,
            severity,
            source,
            message,
        );
        self.log_event(event)
    }
    
    /// Log a security error (Error severity)
    pub fn log_security_error(&self, source: &str, message: &str, context: Option<Value>) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::SecurityError,
            AuditSeverity::Error,
            source,
            message,
        );
        if let Some(ctx) = context {
            event.context = ctx;
        }
        self.log_event(event)
    }
    
    /// Log a critical security error (Critical severity)
    pub fn log_critical_security_error(&self, source: &str, message: &str, context: Option<Value>) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::SecurityError,
            AuditSeverity::Critical,
            source,
            message,
        );
        if let Some(ctx) = context {
            event.context = ctx;
        }
        self.log_event(event)
    }
    
    /// Add a simple audit event with source, type, severity, and message
    pub fn add_event(&self, source: &str, event_type: AuditEventType, severity: AuditSeverity, message: &str) {
        let event = AuditEvent::new(event_type, severity, source, message);
        let _ = self.log_event(event);
    }
    
    /// Add an audit event with additional structured data
    pub fn add_event_with_data(&self, source: &str, event_type: AuditEventType, severity: AuditSeverity, 
                               message: &str, data: HashMap<String, String>) {
        let mut event = AuditEvent::new(event_type, severity, source, message);
        
        // Add all data as context
        let mut json_data = serde_json::Map::new();
        for (key, value) in data {
            json_data.insert(key, serde_json::Value::String(value));
        }
        
        event.context = serde_json::Value::Object(json_data);
        let _ = self.log_event(event);
    }
}

impl Default for SecurityAuditLog {
    fn default() -> Self {
        Self::new()
    }
}