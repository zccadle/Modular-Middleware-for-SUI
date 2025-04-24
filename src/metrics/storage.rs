/// Storage and aggregation for performance metrics and benchmarks.

use super::performance::{PerformanceMetrics, ComponentBenchmark};
use anyhow::Result;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Thread-safe storage for performance metrics and component benchmarks.
#[derive(Debug)]
pub struct MetricsStorage {
    metrics: Arc<Mutex<Vec<PerformanceMetrics>>>,
    benchmarks: Arc<Mutex<Vec<ComponentBenchmark>>>,
}

impl MetricsStorage {
    /// Creates a new, empty `MetricsStorage`.
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(Vec::new())),
            benchmarks: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Adds a single `PerformanceMetrics` record.
    pub fn add_metrics(&self, metrics: PerformanceMetrics) {
        match self.metrics.lock() {
            Ok(mut guard) => guard.push(metrics),
            Err(poisoned) => {
                eprintln!("ERROR: Metrics mutex poisoned. Metrics may be lost: {}", poisoned);
            }
        }
    }

    /// Retrieves a clone of all stored `PerformanceMetrics` records.
    pub fn get_all_metrics(&self) -> Vec<PerformanceMetrics> {
        self.metrics.lock().map_or_else(
            |poisoned| {
                eprintln!("ERROR: Metrics mutex poisoned while getting metrics: {}", poisoned);
                Vec::new() // Return empty vec on error
            },
            |guard| guard.clone(),
        )
    }

    /// Saves all stored `PerformanceMetrics` to a JSON file.
    /// Deprecated in favor of saving benchmark results.
    #[deprecated = "PerformanceMetrics are usually part of ComponentBenchmark now. Use save_benchmarks_to_json_file instead."]
    pub fn save_metrics_to_json_file(&self, filename: &str) -> Result<()> {
        let metrics = self.get_all_metrics();
        let json_metrics: Vec<serde_json::Value> = metrics.iter().map(|m| m.to_json()).collect();
        let json = serde_json::to_string_pretty(&json_metrics)?;

        let mut file = File::create(filename)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Calculates and returns average metrics, grouped by transaction type.
    /// Deprecated as metrics are now typically within benchmarks.
    #[deprecated = "Metrics are now typically part of benchmarks. Analyze benchmark results directly."]
    pub fn get_average_metrics_by_type(&self) -> HashMap<String, serde_json::Value> {
        let metrics_list = self.get_all_metrics();
        let mut type_groups: HashMap<String, Vec<&PerformanceMetrics>> = HashMap::new();

        for metric in &metrics_list {
            type_groups.entry(metric.transaction_type.clone()).or_default().push(metric);
        }

        let mut averages = HashMap::new();
        for (tx_type, metrics) in type_groups {
            if metrics.is_empty() {
                continue;
            }
            let count = metrics.len() as f64;

            let avg_gen_time: f64 = metrics.iter().filter_map(|m| m.generation_time_ms()).sum::<f64>() / count;
            let avg_sui_time: f64 = metrics.iter().filter_map(|m| m.sui_time_ms()).sum::<f64>() / count;
            let avg_exec_time: f64 = metrics.iter().filter_map(|m| m.execution_time_ms()).sum::<f64>() / count;
            let avg_total_time: f64 = metrics.iter().filter_map(|m| m.total_time_ms()).sum::<f64>() / count;
            let middleware_overhead = if avg_sui_time > 0.0 { ((avg_gen_time + avg_exec_time) / avg_sui_time) * 100.0 } else { 0.0 };

            averages.insert(tx_type, serde_json::json!({
                "sample_count": count,
                "avg_generation_time_ms": avg_gen_time,
                "avg_sui_time_ms": avg_sui_time,
                "avg_execution_time_ms": avg_exec_time,
                "avg_total_time_ms": avg_total_time,
                "middleware_overhead_percent": middleware_overhead
            }));
        }
        averages
    }

    /// Prints a summary of average metrics by transaction type to the console.
    /// Deprecated as metrics are now typically within benchmarks.
    #[deprecated = "Metrics are now typically part of benchmarks. Print benchmark summaries instead."]
    pub fn print_metrics_summary(&self) {
        let averages = self.get_average_metrics_by_type();

        println!("\n--- DEPRECATED Performance Metrics Summary ---");
        if averages.is_empty() {
            println!("No performance metrics recorded.");
        } else {
            for (tx_type, stats) in averages {
                println!("\nTransaction Type: {}", tx_type);
                println!("  Sample Count: {}", stats["sample_count"]);
                println!("  Avg Generation Time: {:.2} ms", stats["avg_generation_time_ms"]);
                println!("  Avg SUI Time: {:.2} ms", stats["avg_sui_time_ms"]);
                println!("  Avg Execution Time: {:.2} ms", stats["avg_execution_time_ms"]);
                println!("  Avg Total Time: {:.2} ms", stats["avg_total_time_ms"]);
                println!("  Middleware Overhead: {:.2}%", stats["middleware_overhead_percent"]);
            }
        }
        println!("--- End Deprecated Summary ---");
    }

    // --- Component Benchmark Storage ---

    /// Adds a `ComponentBenchmark` result to storage.
    pub fn add_benchmark(&self, benchmark: ComponentBenchmark) {
        match self.benchmarks.lock() {
            Ok(mut guard) => guard.push(benchmark),
            Err(poisoned) => {
                eprintln!("ERROR: Benchmarks mutex poisoned. Benchmark may be lost: {}", poisoned);
            }
        }
    }

    /// Retrieves a clone of all stored `ComponentBenchmark` results.
    pub fn get_all_benchmarks(&self) -> Vec<ComponentBenchmark> {
        self.benchmarks.lock().map_or_else(
            |poisoned| {
                eprintln!("ERROR: Benchmarks mutex poisoned while getting benchmarks: {}", poisoned);
                Vec::new()
            },
            |guard| guard.clone(),
        )
    }

    /// Saves all stored component benchmarks to a JSON file.
    pub fn save_benchmarks_to_json_file(&self, filename: &str) -> Result<()> {
        let benchmarks = self.get_all_benchmarks();
        let json_benchmarks: Vec<serde_json::Value> = benchmarks.iter().map(|b| b.to_json()).collect();
        let json = serde_json::to_string_pretty(&json_benchmarks)?;

        let mut file = File::create(filename)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Groups benchmarks by component name and then by security level.
    /// Returns a nested HashMap: `ComponentName -> SecurityLevel -> Vec<ComponentBenchmark>`.
    pub fn get_benchmarks_by_component_and_level(&self) -> HashMap<String, HashMap<String, Vec<ComponentBenchmark>>> {
        let benchmarks = self.get_all_benchmarks();
        let mut result: HashMap<String, HashMap<String, Vec<ComponentBenchmark>>> = HashMap::new();

        for benchmark in benchmarks {
            result.entry(benchmark.component_name.clone())
                .or_default()
                .entry(benchmark.security_level.clone())
                .or_default()
                .push(benchmark);
        }
        result
    }

    /// Prints a summary of the stored component benchmarks to the console.
    pub fn print_benchmark_summary(&self) {
        let grouped = self.get_benchmarks_by_component_and_level();

        println!("\n=== BENCHMARK SUMMARY ===");
        if grouped.is_empty() {
            println!("No benchmark results recorded.");
        } else {
            for (component, levels) in grouped {
                println!("\nComponent: {}", component);
                for (level, benchmarks) in levels {
                    if benchmarks.is_empty() { continue; }

                    // Calculate overall stats for this component + level combo
                    let total_iterations: u32 = benchmarks.iter().map(|b| b.iterations).sum();
                    let total_duration_ms: f64 = benchmarks.iter()
                        .filter_map(|b| b.duration_ms())
                        .map(|d| d as f64)
                        .sum();
                    let avg_duration_per_iter_ms = if total_iterations > 0 {
                        total_duration_ms / total_iterations as f64
                    } else {
                        0.0
                    };

                    println!("  Security Level: {}", level);
                    println!("    Benchmarks Run: {}", benchmarks.len());
                    println!("    Total Iterations: {}", total_iterations);
                    println!("    Avg Duration/Iteration: {:.3} ms", avg_duration_per_iter_ms);

                    // Optionally print average for specific operations if desired
                    // Example: Print average 'l1_submission' time
                    let mut op_times = Vec::new();
                    let mut op_counts = 0;
                    for b in &benchmarks {
                        if let Some(stats) = b.operation_stats.get("l1_submission") {
                            op_times.push(stats.average_duration_ms());
                            op_counts += stats.count;
                        }
                    }
                    if !op_times.is_empty() && op_counts > 0{
                        let avg_op_time: f64 = op_times.iter().sum::<f64>() / op_times.len() as f64;
                        println!("    Avg 'l1_submission' time (across benchmarks): {:.3} ms (total calls: {})", avg_op_time, op_counts);
                    }
                     // Example: Print average 'total_iteration' time
                     let mut total_iter_times = Vec::new();
                     let mut total_iter_counts = 0;
                     for b in &benchmarks {
                         if let Some(stats) = b.operation_stats.get("total_iteration") {
                             total_iter_times.push(stats.average_duration_ms());
                             total_iter_counts += stats.count;
                         }
                     }
                     if !total_iter_times.is_empty() && total_iter_counts > 0{
                         let avg_total_iter_time: f64 = total_iter_times.iter().sum::<f64>() / total_iter_times.len() as f64;
                         println!("    Avg 'total_iteration' time (across benchmarks): {:.3} ms (total calls: {})", avg_total_iter_time, total_iter_counts);
                     }
                }
            }
        }
        println!("===========================");
    }
}

impl Default for MetricsStorage {
    fn default() -> Self {
        Self::new()
    }
}