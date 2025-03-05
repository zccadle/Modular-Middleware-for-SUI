use serde::{Serialize, Deserialize};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub transaction_type: String,
    pub generation_start_time: Instant,
    pub generation_end_time: Option<Instant>,
    pub sui_start_time: Option<Instant>,
    pub sui_end_time: Option<Instant>,
    pub execution_start_time: Option<Instant>,
    pub execution_end_time: Option<Instant>,
    /// Time when verification process started
    pub verification_start_time: Option<Instant>,
    /// Time when verification process completed
    pub verification_end_time: Option<Instant>,
    pub total_size_bytes: Option<usize>,
    /// Whether the transaction was successfully verified
    pub verified: Option<bool>,
    /// Number of verification attempts
    pub verification_attempts: Option<u8>,
    /// Chain ID this transaction targets
    pub chain_id: Option<String>,
}

impl PerformanceMetrics {
    pub fn new(transaction_type: &str) -> Self {
        Self {
            transaction_type: transaction_type.to_string(),
            generation_start_time: Instant::now(),
            generation_end_time: None,
            sui_start_time: None,
            sui_end_time: None,
            execution_start_time: None,
            execution_end_time: None,
            verification_start_time: None,
            verification_end_time: None,
            total_size_bytes: None,
            verified: None,
            verification_attempts: None,
            chain_id: None,
        }
    }
    
    pub fn generation_time_ms(&self) -> Option<f64> {
        self.generation_end_time.map(|end| 
            end.duration_since(self.generation_start_time).as_secs_f64() * 1000.0
        )
    }
    
    pub fn sui_time_ms(&self) -> Option<f64> {
        match (self.sui_start_time, self.sui_end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start).as_secs_f64() * 1000.0),
            _ => None
        }
    }
    
    pub fn execution_time_ms(&self) -> Option<f64> {
        match (self.execution_start_time, self.execution_end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start).as_secs_f64() * 1000.0),
            _ => None
        }
    }
    
    pub fn verification_time_ms(&self) -> Option<f64> {
        match (self.verification_start_time, self.verification_end_time) {
            (Some(start), Some(end)) => Some(end.duration_since(start).as_secs_f64() * 1000.0),
            _ => None
        }
    }
    
    pub fn total_time_ms(&self) -> Option<f64> {
        self.verification_end_time.or(self.execution_end_time).map(|end| 
            end.duration_since(self.generation_start_time).as_secs_f64() * 1000.0
        )
    }
    
    pub fn middleware_overhead_ms(&self) -> Option<f64> {
        match (self.generation_time_ms(), self.execution_time_ms(), self.verification_time_ms(), self.sui_time_ms()) {
            (Some(gen), Some(exec), Some(verif), Some(sui)) => Some(gen + exec + verif),
            (Some(gen), Some(exec), None, Some(sui)) => Some(gen + exec),
            (Some(gen), None, None, Some(sui)) => Some(gen),
            _ => None
        }
    }
    
    pub fn middleware_overhead_percent(&self) -> Option<f64> {
        match (self.middleware_overhead_ms(), self.sui_time_ms()) {
            (Some(overhead), Some(sui)) if sui > 0.0 => Some((overhead / sui) * 100.0),
            _ => None
        }
    }
    
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "transaction_type": self.transaction_type,
            "generation_time_ms": self.generation_time_ms(),
            "sui_time_ms": self.sui_time_ms(),
            "execution_time_ms": self.execution_time_ms(),
            "verification_time_ms": self.verification_time_ms(),
            "middleware_overhead_ms": self.middleware_overhead_ms(),
            "middleware_overhead_percent": self.middleware_overhead_percent(),
            "total_time_ms": self.total_time_ms(),
            "total_size_bytes": self.total_size_bytes,
            "verified": self.verified,
            "verification_attempts": self.verification_attempts,
            "chain_id": self.chain_id
        })
    }
    
    /// Set the verification result
    pub fn set_verification_result(&mut self, verified: bool, attempts: u8) {
        self.verified = Some(verified);
        self.verification_attempts = Some(attempts);
    }
    
    /// Set the chain ID
    pub fn set_chain_id(&mut self, chain_id: &str) {
        self.chain_id = Some(chain_id.to_string());
    }
    
    /// Print a summary of the metrics
    pub fn print_summary(&self) {
        println!("Performance Summary for {} Transaction:", self.transaction_type);
        
        if let Some(gen_time) = self.generation_time_ms() {
            println!("  Generation Time: {:.2} ms", gen_time);
        }
        
        if let Some(sui_time) = self.sui_time_ms() {
            println!("  SUI Blockchain Time: {:.2} ms", sui_time);
        }
        
        if let Some(exec_time) = self.execution_time_ms() {
            println!("  Execution Time: {:.2} ms", exec_time);
        }
        
        if let Some(verif_time) = self.verification_time_ms() {
            println!("  Verification Time: {:.2} ms", verif_time);
        }
        
        if let Some(overhead) = self.middleware_overhead_ms() {
            println!("  Middleware Overhead: {:.2} ms", overhead);
        }
        
        if let Some(overhead_pct) = self.middleware_overhead_percent() {
            println!("  Middleware Overhead: {:.2}%", overhead_pct);
        }
        
        if let Some(total) = self.total_time_ms() {
            println!("  Total Processing Time: {:.2} ms", total);
        }
        
        if let Some(verified) = self.verified {
            println!("  Verified: {}", if verified { "Yes" } else { "No" });
        }
        
        if let Some(attempts) = self.verification_attempts {
            println!("  Verification Attempts: {}", attempts);
        }
        
        if let Some(chain_id) = &self.chain_id {
            println!("  Chain ID: {}", chain_id);
        }
    }
}