/// Performance measurement structures and utilities.

use serde::{Deserialize, Serialize, Serializer, Deserializer};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};

// Removed unused Instant serialization/deserialization helpers and struct
// as ComponentBenchmark primarily uses duration calculations.

/// Deprecated struct for collecting basic performance metrics.
/// Replaced by `ComponentBenchmark` for more structured results.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[deprecated = "Use ComponentBenchmark instead for structured benchmark results."]
pub struct PerformanceMetrics {
    pub transaction_type: String,
    pub generation_start_time: Option<SystemTime>,
    pub generation_end_time: Option<SystemTime>,
    pub sui_start_time: Option<SystemTime>,
    pub sui_end_time: Option<SystemTime>,
    pub verification_start_time: Option<SystemTime>,
    pub verification_end_time: Option<SystemTime>,
    pub execution_start_time: Option<SystemTime>,
    pub execution_end_time: Option<SystemTime>,
    // Calculated fields below are kept for reference but might be removed later.
    pub generation_time_ms: Option<u64>,
    pub sui_time_ms: Option<u64>,
    pub verification_time_ms: Option<u64>,
    pub execution_time_ms: Option<u64>,
    pub timings: HashMap<String, Duration>,
    pub total_size_bytes: Option<usize>,
    pub verified: Option<bool>,
    pub verification_attempts: Option<u8>,
    pub chain_id: Option<String>,
    pub quorum_consensus_time_ms: Option<u64>,
    pub attestation_generation_time_ms: Option<u64>,
    // Derived calculations
    pub middleware_overhead_ms: Option<f64>,
    pub middleware_overhead_percent: Option<f64>,
    pub total_time_ms: Option<f64>,
}

// Implementations for PerformanceMetrics are kept but marked as potentially removable
// or refactored if basic timing is still needed outside benchmarks.
impl PerformanceMetrics {
    pub fn new(transaction_type: &str) -> Self {
        Self {
            transaction_type: transaction_type.to_string(),
            generation_start_time: Some(SystemTime::now()),
            // Initialize all others to None
            generation_end_time: None, sui_start_time: None, sui_end_time: None,
            verification_start_time: None, verification_end_time: None,
            execution_start_time: None, execution_end_time: None,
            generation_time_ms: None, sui_time_ms: None, verification_time_ms: None,
            execution_time_ms: None, timings: HashMap::new(), total_size_bytes: None,
            verified: None, verification_attempts: None, chain_id: None,
            quorum_consensus_time_ms: None, attestation_generation_time_ms: None,
            middleware_overhead_ms: None, middleware_overhead_percent: None, total_time_ms: None,
        }
    }

    // Helper to calculate duration between two SystemTime Option fields
    fn duration_ms_opt(start: Option<SystemTime>, end: Option<SystemTime>) -> Option<f64> {
        match (start, end) {
            (Some(s), Some(e)) => e.duration_since(s).ok().map(|d| d.as_secs_f64() * 1000.0),
            _ => None,
        }
    }

    pub fn generation_time_ms(&self) -> Option<f64> {
        Self::duration_ms_opt(self.generation_start_time, self.generation_end_time)
    }

    pub fn sui_time_ms(&self) -> Option<f64> {
        Self::duration_ms_opt(self.sui_start_time, self.sui_end_time)
    }

    pub fn execution_time_ms(&self) -> Option<f64> {
        Self::duration_ms_opt(self.execution_start_time, self.execution_end_time)
    }

    pub fn verification_time_ms(&self) -> Option<f64> {
        Self::duration_ms_opt(self.verification_start_time, self.verification_end_time)
    }

    pub fn total_time_ms(&self) -> Option<f64> {
        // Total time is from generation start to the latest end time (verification or execution)
        let latest_end = self.verification_end_time.max(self.execution_end_time);
        Self::duration_ms_opt(self.generation_start_time, latest_end)
    }

    pub fn middleware_overhead_ms(&self) -> Option<f64> {
        // Sum of generation, execution, and verification times
        [self.generation_time_ms(), self.execution_time_ms(), self.verification_time_ms()]
            .iter()
            .filter_map(|&opt| opt)
            .sum::<f64>()
            .into()
    }

    pub fn middleware_overhead_percent(&self) -> Option<f64> {
        match (self.middleware_overhead_ms(), self.sui_time_ms()) {
            (Some(overhead), Some(sui)) if sui > 0.0 => Some((overhead / sui) * 100.0),
            _ => None, // Avoid division by zero or if components are missing
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
            "chain_id": self.chain_id,
            "quorum_consensus_time_ms": self.quorum_consensus_time_ms, // Keep raw recorded values
            "attestation_generation_time_ms": self.attestation_generation_time_ms, // Keep raw recorded values
            // "timings": self.timings, // Consider if detailed timings map is needed in JSON
        })
    }

    // `end_operation`, `set_verification_result`, `set_chain_id`, `print_summary`,
    // `set_timing`, `get_timing` are kept but might be deprecated further.
    pub fn end_operation(&mut self, operation_name: &str) {
        self.execution_end_time = Some(SystemTime::now());
        println!("Deprecated: Operation '{}' for tx '{}' ended.", operation_name, self.transaction_type);
    }
    pub fn set_verification_result(&mut self, verified: bool, attempts: u8) {
        self.verified = Some(verified);
        self.verification_attempts = Some(attempts);
        self.verification_end_time = Some(SystemTime::now());
    }
    pub fn set_chain_id(&mut self, chain_id: &str) {
        self.chain_id = Some(chain_id.to_string());
    }
    pub fn print_summary(&self) { /* ... implementation ... */ }
    pub fn set_timing(&mut self, metric_name: &str, duration: Duration) {
         match metric_name {
            "quorum_consensus_time" => self.quorum_consensus_time_ms = Some(duration.as_millis() as u64),
            "execution_time" => self.execution_time_ms = Some(duration.as_millis() as u64),
            "attestation_generation_time" => self.attestation_generation_time_ms = Some(duration.as_millis() as u64),
            _ => { self.timings.insert(metric_name.to_string(), duration); } // Store others in map
        }
    }
    pub fn get_timing(&self, metric_name: &str) -> Option<Duration> {
        self.timings.get(metric_name).copied()
    }

    // Add back deprecated setter for compatibility if needed
    #[deprecated = "Use ComponentBenchmark::record_operation instead."]
    pub fn set_execution_time(&mut self, duration: Duration) {
        self.execution_time_ms = Some(duration.as_millis() as u64);
        self.execution_end_time = self.execution_start_time.map(|start| start + duration);
    }
}

// --- Component Benchmark --- (Primary Structure)

/// Stores results for a benchmark run targeting a specific system component or scenario.
#[derive(Debug, Clone, Serialize, Deserialize, Default)] // Added Deserialize
pub struct ComponentBenchmark {
    /// Name identifying the benchmark scenario (e.g., "end_to_end_performance_n5").
    pub component_name: String,
    /// Description of the security level or conditions tested (e.g., "0_percent_byzantine").
    pub security_level: String,
    /// Number of iterations performed in this benchmark run.
    pub iterations: u32,
    /// Start time of the benchmark run.
    #[serde(skip)] // Skip serialization/deserialization of Instant
    start_time: Option<Instant>, // Made Option to handle deserialization if needed
    /// End time of the benchmark run.
    #[serde(skip)]
    end_time: Option<Instant>,
    /// Total duration of the benchmark run in milliseconds (calculated).
    total_duration_ms: Option<u64>,
    /// Configuration parameters used for this benchmark run (e.g., quorum size, Byzantine %).
    pub configuration: HashMap<String, String>,
    /// Stores statistics (count, sum, etc.) for specific named operations within the benchmark.
    pub operation_stats: HashMap<String, OperationStats>,
    // Removed redundant operation_timings and operation_counts, consolidated into OperationStats
}

/// Stores statistics for a specific timed operation within a benchmark.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OperationStats {
    /// Number of times the operation was executed.
    pub count: u32,
    /// Sum of all durations recorded for this operation (in milliseconds).
    pub total_duration_ms: u64,
    /// Minimum duration recorded (in milliseconds).
    pub min_duration_ms: u64,
    /// Maximum duration recorded (in milliseconds).
    pub max_duration_ms: u64,
}

impl OperationStats {
    /// Calculates the average duration for this operation.
    pub fn average_duration_ms(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_duration_ms as f64 / self.count as f64
        }
    }
}

impl ComponentBenchmark {
    /// Creates a new benchmark instance and records the start time.
    pub fn new(component_name: &str, security_level: &str, iterations: u32) -> Self {
        Self {
            component_name: component_name.to_string(),
            security_level: security_level.to_string(),
            iterations,
            start_time: Some(Instant::now()), // Record start time
            end_time: None,
            total_duration_ms: None, // Calculated on end()
            configuration: HashMap::new(),
            operation_stats: HashMap::new(),
        }
    }

    /// Adds a configuration parameter key-value pair.
    pub fn add_config(&mut self, key: &str, value: &str) -> &mut Self {
        self.configuration.insert(key.to_string(), value.to_string());
        self
    }

    /// Records the duration of a specific operation within the benchmark.
    /// Updates count, total, min, and max statistics for the operation.
    pub fn record_operation(&mut self, operation: &str, duration_ms: u64) -> &mut Self {
        let stats = self.operation_stats.entry(operation.to_string()).or_default();
        stats.count += 1;
        stats.total_duration_ms += duration_ms;
        if stats.count == 1 {
            stats.min_duration_ms = duration_ms;
            stats.max_duration_ms = duration_ms;
        } else {
            stats.min_duration_ms = stats.min_duration_ms.min(duration_ms);
            stats.max_duration_ms = stats.max_duration_ms.max(duration_ms);
        }
        self
    }

    /// Marks the end of the benchmark run and calculates the total duration.
    pub fn end(&mut self) -> &mut Self {
        if self.end_time.is_none() {
            let now = Instant::now();
            self.end_time = Some(now);
            if let Some(start) = self.start_time {
                 self.total_duration_ms = Some(now.duration_since(start).as_millis() as u64);
            }
        }
        self
    }

    /// Returns the total duration of the benchmark run in milliseconds (if ended).
    pub fn duration_ms(&self) -> Option<u64> {
        self.total_duration_ms
        // Alternative calculation if needed:
        // self.end_time.zip(self.start_time).map(|(end, start)| {
        //     end.duration_since(start).as_millis() as u64
        // })
    }

    /// Calculates the average duration per iteration in milliseconds (if ended).
    pub fn avg_duration_per_iteration_ms(&self) -> Option<f64> {
        self.duration_ms().map(|total_duration| {
            if self.iterations == 0 {
                0.0
            } else {
                total_duration as f64 / self.iterations as f64
            }
        })
    }

    /// Returns the statistics for a specific operation, if recorded.
    pub fn get_operation_stats(&self, operation: &str) -> Option<&OperationStats> {
        self.operation_stats.get(operation)
    }

    // Removed avg_operation_time and median_operation_time, use get_operation_stats().average_duration_ms()

    /// Converts the benchmark results into a serializable JSON value.
    pub fn to_json(&self) -> serde_json::Value {
        // Ensure end() has been called to calculate duration
        let duration = self.duration_ms();
        let avg_iter_duration = self.avg_duration_per_iteration_ms();

        // Create JSON object for operation stats including average
        let operation_summary: HashMap<String, serde_json::Value> = self.operation_stats.iter()
            .map(|(name, stats)| {
                (name.clone(), serde_json::json!({
                    "count": stats.count,
                    "total_duration_ms": stats.total_duration_ms,
                    "min_duration_ms": stats.min_duration_ms,
                    "max_duration_ms": stats.max_duration_ms,
                    "average_duration_ms": stats.average_duration_ms(),
                }))
            })
            .collect();

        serde_json::json!({
            "component_name": self.component_name,
            "security_level": self.security_level,
            "iterations": self.iterations,
            "total_duration_ms": duration,
            "avg_duration_per_iteration_ms": avg_iter_duration,
            "configuration": self.configuration,
            "operation_stats": operation_summary, // Use the calculated summary map
            // Removed deprecated fields like operation_counts, operation_avg_times, operation_median_times
        })
    }

    /// Prints a formatted summary of the benchmark results to the console.
    pub fn print_summary(&self) {
        // Ensure end() has been called
        let duration = self.duration_ms();
        let avg_iter_duration = self.avg_duration_per_iteration_ms();

        println!("\n--- Component Benchmark Summary ---");
        println!("Component:       {}", self.component_name);
        println!("Security Level:  {}", self.security_level);
        println!("Iterations:      {}", self.iterations);

        if let Some(d) = duration {
            println!("Total Duration:  {} ms", d);
        }
        if let Some(avg) = avg_iter_duration {
            println!("Avg Iteration:   {:.3} ms", avg);
        }

        if !self.operation_stats.is_empty() {
            println!("\nOperation Statistics:");
            // Sort operations for consistent output
            let mut ops: Vec<_> = self.operation_stats.keys().collect();
            ops.sort();
            for op_name in ops {
                if let Some(stats) = self.operation_stats.get(op_name) {
                     println!("  - {}", op_name);
                     println!("      Count:          {}", stats.count);
                     println!("      Total Time:     {} ms", stats.total_duration_ms);
                     println!("      Avg Time:       {:.3} ms", stats.average_duration_ms());
                     println!("      Min Time:       {} ms", stats.min_duration_ms);
                     println!("      Max Time:       {} ms", stats.max_duration_ms);
                }
            }
        }

        if !self.configuration.is_empty() {
            println!("\nConfiguration:");
             // Sort config keys for consistent output
            let mut keys: Vec<_> = self.configuration.keys().collect();
            keys.sort();
            for key in keys {
                 if let Some(value) = self.configuration.get(key) {
                    println!("  {}: {}", key, value);
                 }
            }
        }
        println!("----------------------------------\n");
    }
}

// Removed old wrapper functions like benchmark_transaction_verification etc.
// Benchmarks should be created directly using ComponentBenchmark::new().