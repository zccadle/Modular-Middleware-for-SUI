use serde::{Serialize, Deserialize};
use std::sync::{Arc, Mutex};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Security audit event types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditEventType {
    /// Transaction validation
    TransactionValidation,
    /// Transaction execution
    TransactionExecution,
    /// Transaction verification
    TransactionVerification,
    /// Network operation
    NetworkOperation,
    /// External API call
    ExternalAPI,
    /// Authentication event
    Authentication,
    /// Authorization event
    Authorization,
    /// Configuration change
    ConfigChange,
    /// Security error
    SecurityError,
}

/// Security audit event severity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditSeverity {
    /// Informational event
    Info,
    /// Warning event
    Warning,
    /// Error event
    Error,
    /// Critical security event
    Critical,
}

/// Security audit event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: AuditEventType,
    /// Event severity
    pub severity: AuditSeverity,
    /// User or component that initiated the event
    pub source: String,
    /// Transaction ID if applicable
    pub transaction_id: Option<String>,
    /// Chain ID if applicable
    pub chain_id: Option<String>,
    /// Detailed event message
    pub message: String,
    /// Additional context data
    pub context: serde_json::Value,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(
        event_type: AuditEventType,
        severity: AuditSeverity,
        source: &str,
        message: &str,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            severity,
            source: source.to_string(),
            transaction_id: None,
            chain_id: None,
            message: message.to_string(),
            context: serde_json::json!({}),
        }
    }
    
    /// Set the transaction ID
    pub fn with_transaction_id(mut self, tx_id: &str) -> Self {
        self.transaction_id = Some(tx_id.to_string());
        self
    }
    
    /// Set the chain ID
    pub fn with_chain_id(mut self, chain_id: &str) -> Self {
        self.chain_id = Some(chain_id.to_string());
        self
    }
    
    /// Add context data
    pub fn with_context(mut self, key: &str, value: impl Serialize) -> Self {
        if let Ok(value_json) = serde_json::to_value(value) {
            if let Some(obj) = self.context.as_object_mut() {
                obj.insert(key.to_string(), value_json);
            }
        }
        self
    }
    
    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "timestamp": self.timestamp.to_rfc3339(),
            "event_type": format!("{:?}", self.event_type),
            "severity": format!("{:?}", self.severity),
            "source": self.source,
            "transaction_id": self.transaction_id,
            "chain_id": self.chain_id,
            "message": self.message,
            "context": self.context,
        })
    }
    
    /// Convert to a formatted log string
    pub fn to_log_string(&self) -> String {
        format!(
            "[{}] [{}] [{}] {}: {} (tx_id: {:?}, chain: {:?})",
            self.timestamp.to_rfc3339(),
            format!("{:?}", self.severity),
            format!("{:?}", self.event_type),
            self.source,
            self.message,
            self.transaction_id,
            self.chain_id
        )
    }
}

/// Security audit log configuration
#[derive(Debug, Clone)]
pub struct AuditLogConfig {
    /// Whether to enable console logging
    pub console_enabled: bool,
    /// Whether to enable file logging
    pub file_enabled: bool,
    /// Path to the log file
    pub log_file_path: Option<PathBuf>,
    /// Minimum severity level to log
    pub min_severity: AuditSeverity,
}

impl Default for AuditLogConfig {
    fn default() -> Self {
        Self {
            console_enabled: true,
            file_enabled: true,
            log_file_path: Some(PathBuf::from("security_audit.log")),
            min_severity: AuditSeverity::Info,
        }
    }
}

/// Security audit logger
#[derive(Debug, Clone)]
pub struct SecurityAuditLog {
    /// Configuration for the audit log
    config: Arc<Mutex<AuditLogConfig>>,
    /// In-memory record of recent audit events
    events: Arc<Mutex<Vec<AuditEvent>>>,
    /// Maximum number of events to keep in memory
    max_events: usize,
}

impl SecurityAuditLog {
    /// Create a new security audit logger with default configuration
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(AuditLogConfig::default())),
            events: Arc::new(Mutex::new(Vec::new())),
            max_events: 1000, // Keep the last 1000 events in memory
        }
    }
    
    /// Create a new security audit logger with custom configuration
    pub fn with_config(config: AuditLogConfig) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            events: Arc::new(Mutex::new(Vec::new())),
            max_events: 1000,
        }
    }
    
    /// Update the configuration
    pub fn update_config(&self, config: AuditLogConfig) {
        let mut current_config = self.config.lock().unwrap();
        *current_config = config;
    }
    
    /// Log a security audit event
    pub fn log_event(&self, event: AuditEvent) -> Result<()> {
        // Check severity threshold
        let config = self.config.lock().unwrap();
        let should_log = match (&config.min_severity, &event.severity) {
            (AuditSeverity::Info, _) => true,
            (AuditSeverity::Warning, AuditSeverity::Warning | AuditSeverity::Error | AuditSeverity::Critical) => true,
            (AuditSeverity::Error, AuditSeverity::Error | AuditSeverity::Critical) => true,
            (AuditSeverity::Critical, AuditSeverity::Critical) => true,
            _ => false,
        };
        
        if !should_log {
            return Ok(());
        }
        
        // Add to in-memory events
        {
            let mut events = self.events.lock().unwrap();
            events.push(event.clone());
            
            // Trim if over max size
            if events.len() > self.max_events {
                events.remove(0);
            }
        }
        
        // Log to console if enabled
        if config.console_enabled {
            println!("{}", event.to_log_string());
        }
        
        // Log to file if enabled
        if config.file_enabled {
            if let Some(path) = &config.log_file_path {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)?;
                
                let mut file = std::io::BufWriter::new(file);
                writeln!(file, "{}", event.to_log_string())?;
            }
        }
        
        Ok(())
    }
    
    /// Get all audit events in memory
    pub fn get_events(&self) -> Vec<AuditEvent> {
        let events = self.events.lock().unwrap();
        events.clone()
    }
    
    /// Get events filtered by severity
    pub fn get_events_by_severity(&self, severity: AuditSeverity) -> Vec<AuditEvent> {
        let events = self.events.lock().unwrap();
        events.iter()
            .filter(|e| match (&e.severity, &severity) {
                (AuditSeverity::Info, AuditSeverity::Info) => true,
                (AuditSeverity::Warning, AuditSeverity::Warning) => true,
                (AuditSeverity::Error, AuditSeverity::Error) => true,
                (AuditSeverity::Critical, AuditSeverity::Critical) => true,
                _ => false,
            })
            .cloned()
            .collect()
    }
    
    /// Get events filtered by event type
    pub fn get_events_by_type(&self, event_type: AuditEventType) -> Vec<AuditEvent> {
        let events = self.events.lock().unwrap();
        events.iter()
            .filter(|e| std::mem::discriminant(&e.event_type) == std::mem::discriminant(&event_type))
            .cloned()
            .collect()
    }
    
    /// Get events related to a specific transaction
    pub fn get_events_by_transaction(&self, transaction_id: &str) -> Vec<AuditEvent> {
        let events = self.events.lock().unwrap();
        events.iter()
            .filter(|e| e.transaction_id.as_deref() == Some(transaction_id))
            .cloned()
            .collect()
    }
    
    /// Export events to a JSON file
    pub fn export_events_to_json(&self, path: &str) -> Result<()> {
        let events = self.events.lock().unwrap();
        let json = serde_json::to_string_pretty(&*events)?;
        
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;
        
        Ok(())
    }
    
    /// Clear all events from memory
    pub fn clear_events(&self) {
        let mut events = self.events.lock().unwrap();
        events.clear();
    }
    
    /// Create convenience logging methods for different event types
    
    /// Log a transaction validation event
    pub fn log_validation(&self, source: &str, message: &str, tx_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::TransactionValidation,
            severity,
            source,
            message,
        );
        
        if let Some(tx_id) = tx_id {
            event = event.with_transaction_id(tx_id);
        }
        
        self.log_event(event)
    }
    
    /// Log a transaction execution event
    pub fn log_execution(&self, source: &str, message: &str, tx_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::TransactionExecution,
            severity,
            source,
            message,
        );
        
        if let Some(tx_id) = tx_id {
            event = event.with_transaction_id(tx_id);
        }
        
        self.log_event(event)
    }
    
    /// Log a transaction verification event
    pub fn log_verification(&self, source: &str, message: &str, tx_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::TransactionVerification,
            severity,
            source,
            message,
        );
        
        if let Some(tx_id) = tx_id {
            event = event.with_transaction_id(tx_id);
        }
        
        self.log_event(event)
    }
    
    /// Log a network operation event
    pub fn log_network(&self, source: &str, message: &str, chain_id: Option<&str>, severity: AuditSeverity) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::NetworkOperation,
            severity,
            source,
            message,
        );
        
        if let Some(chain_id) = chain_id {
            event = event.with_chain_id(chain_id);
        }
        
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
    
    /// Log a security error
    pub fn log_security_error(&self, source: &str, message: &str, context: Option<serde_json::Value>) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::SecurityError,
            AuditSeverity::Error,
            source,
            message,
        );
        
        if let Some(context) = context {
            event = event.with_context("error_details", context);
        }
        
        self.log_event(event)
    }
    
    /// Log a critical security error
    pub fn log_critical_security_error(&self, source: &str, message: &str, context: Option<serde_json::Value>) -> Result<()> {
        let mut event = AuditEvent::new(
            AuditEventType::SecurityError,
            AuditSeverity::Critical,
            source,
            message,
        );
        
        if let Some(context) = context {
            event = event.with_context("error_details", context);
        }
        
        self.log_event(event)
    }
    
    /// Add an audit event with source, type, severity, and message
    pub fn add_event(&self, source: &str, event_type: AuditEventType, severity: AuditSeverity, message: &str) {
        let event = AuditEvent::new(event_type, severity, source, message);
        let _ = self.log_event(event);
    }
    
    /// Add an audit event with additional data
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