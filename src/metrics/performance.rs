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
    pub total_size_bytes: Option<usize>,
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
            total_size_bytes: None,
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
    
    pub fn total_time_ms(&self) -> Option<f64> {
        self.execution_end_time.map(|end| 
            end.duration_since(self.generation_start_time).as_secs_f64() * 1000.0
        )
    }
    
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "transaction_type": self.transaction_type,
            "generation_time_ms": self.generation_time_ms(),
            "sui_time_ms": self.sui_time_ms(),
            "execution_time_ms": self.execution_time_ms(),
            "total_time_ms": self.total_time_ms(),
            "total_size_bytes": self.total_size_bytes
        })
    }
}