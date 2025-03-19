use serde::{Serialize, Deserialize};
use std::time::Instant;
use std::collections::HashMap;

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
    
    pub fn end_operation(&mut self, operation_name: &str) {
        // Mark the end of the operation
        self.execution_end_time = Some(Instant::now());
        
        // Log the operation completion
        println!("Operation '{}' for transaction type '{}' completed", 
                 operation_name, self.transaction_type);
    }
    
    /// Set the verification result
    pub fn set_verification_result(&mut self, verified: bool, attempts: u8) {
        self.verified = Some(verified);
        self.verification_attempts = Some(attempts);
        self.verification_end_time = Some(Instant::now());
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

/// Component-level benchmark for measuring performance of specific system components
#[derive(Debug, Clone)]
pub struct ComponentBenchmark {
    pub component_name: String,
    pub start_time: Instant,
    pub end_time: Option<Instant>,
    pub iterations: u32,
    pub security_level: String, // "none", "basic", "enhanced", "maximum"
    pub configuration: HashMap<String, String>, // Store test parameters
    pub operation_timings: HashMap<String, Vec<u64>>, // Operation-level timing data
    pub operation_counts: HashMap<String, u32>, // Count of operations performed
}

impl ComponentBenchmark {
    /// Create a new component benchmark
    pub fn new(component_name: &str, security_level: &str, iterations: u32) -> Self {
        Self {
            component_name: component_name.to_string(),
            start_time: Instant::now(),
            end_time: None,
            iterations,
            security_level: security_level.to_string(),
            configuration: HashMap::new(),
            operation_timings: HashMap::new(),
            operation_counts: HashMap::new(),
        }
    }
    
    /// Add a configuration parameter
    pub fn add_config(&mut self, key: &str, value: &str) -> &mut Self {
        self.configuration.insert(key.to_string(), value.to_string());
        self
    }
    
    /// Record timing for a specific operation
    pub fn record_operation(&mut self, operation: &str, duration_ms: u64) -> &mut Self {
        self.operation_timings.entry(operation.to_string())
            .or_insert_with(Vec::new)
            .push(duration_ms);
        
        let count = self.operation_counts.entry(operation.to_string())
            .or_insert(0);
        *count += 1;
        
        self
    }
    
    /// End the benchmark and calculate elapsed time
    pub fn end(&mut self) -> &mut Self {
        self.end_time = Some(Instant::now());
        self
    }
    
    /// Calculate the total duration in milliseconds
    pub fn duration_ms(&self) -> Option<u64> {
        self.end_time.map(|end| 
            end.duration_since(self.start_time).as_millis() as u64
        )
    }
    
    /// Calculate average duration per iteration in milliseconds
    pub fn avg_duration_per_iteration_ms(&self) -> Option<f64> {
        self.duration_ms().map(|duration| 
            duration as f64 / self.iterations as f64
        )
    }
    
    /// Get average timing for a specific operation
    pub fn avg_operation_time(&self, operation: &str) -> Option<f64> {
        self.operation_timings.get(operation).map(|times| {
            if times.is_empty() {
                0.0
            } else {
                times.iter().sum::<u64>() as f64 / times.len() as f64
            }
        })
    }
    
    /// Get median timing for a specific operation
    pub fn median_operation_time(&self, operation: &str) -> Option<f64> {
        self.operation_timings.get(operation).map(|times| {
            if times.is_empty() {
                0.0
            } else {
                let mut sorted = times.clone();
                sorted.sort();
                let mid = sorted.len() / 2;
                if sorted.len() % 2 == 0 {
                    (sorted[mid-1] + sorted[mid]) as f64 / 2.0
                } else {
                    sorted[mid] as f64
                }
            }
        })
    }
    
    /// Convert benchmark to JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        let mut operation_avg = HashMap::new();
        let mut operation_median = HashMap::new();
        
        for op in self.operation_timings.keys() {
            if let Some(avg) = self.avg_operation_time(op) {
                operation_avg.insert(op, avg);
            }
            if let Some(median) = self.median_operation_time(op) {
                operation_median.insert(op, median);
            }
        }
        
        serde_json::json!({
            "component_name": self.component_name,
            "security_level": self.security_level,
            "iterations": self.iterations,
            "duration_ms": self.duration_ms(),
            "avg_duration_per_iteration_ms": self.avg_duration_per_iteration_ms(),
            "configuration": self.configuration,
            "operation_counts": self.operation_counts,
            "operation_avg_times": operation_avg,
            "operation_median_times": operation_median
        })
    }
    
    /// Print a summary of the benchmark results
    pub fn print_summary(&self) {
        println!("\n=== COMPONENT BENCHMARK SUMMARY ===");
        println!("Component: {}", self.component_name);
        println!("Security Level: {}", self.security_level);
        println!("Iterations: {}", self.iterations);
        
        if let Some(duration) = self.duration_ms() {
            println!("Total Duration: {} ms", duration);
        }
        
        if let Some(avg) = self.avg_duration_per_iteration_ms() {
            println!("Avg Duration Per Iteration: {:.2} ms", avg);
        }
        
        if !self.operation_timings.is_empty() {
            println!("\nOperation Timings:");
            for (op, _) in &self.operation_timings {
                if let Some(avg) = self.avg_operation_time(op) {
                    println!("  {} - Avg: {:.2} ms, Count: {}", 
                             op, avg, self.operation_counts.get(op).unwrap_or(&0));
                }
            }
        }
        
        if !self.configuration.is_empty() {
            println!("\nConfiguration:");
            for (key, value) in &self.configuration {
                println!("  {}: {}", key, value);
            }
        }
        
        println!("==============================\n");
    }
}

// Wrapper functions for benchmarking specific components
pub fn benchmark_transaction_verification(security_level: &str, iterations: u32) -> ComponentBenchmark {
    ComponentBenchmark::new("transaction_verification", security_level, iterations)
}

pub fn benchmark_byzantine_detection(nodes: u32, security_level: &str, iterations: u32) -> ComponentBenchmark {
    let mut benchmark = ComponentBenchmark::new("byzantine_detection", security_level, iterations);
    benchmark.add_config("nodes", &nodes.to_string());
    benchmark
}

pub fn benchmark_external_data_verification(sources: u32, security_level: &str, iterations: u32) -> ComponentBenchmark {
    let mut benchmark = ComponentBenchmark::new("external_data_verification", security_level, iterations);
    benchmark.add_config("sources", &sources.to_string());
    benchmark
}

pub fn benchmark_cross_chain_support(chains: &[&str], security_level: &str, iterations: u32) -> ComponentBenchmark {
    let mut benchmark = ComponentBenchmark::new("cross_chain_support", security_level, iterations);
    benchmark.add_config("chains", &chains.join(","));
    benchmark
}