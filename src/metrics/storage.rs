use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::Write;
use anyhow::Result;
use super::performance::PerformanceMetrics;
use std::collections::HashMap;

pub struct MetricsStorage {
    metrics: Arc<Mutex<Vec<PerformanceMetrics>>>,
}

impl MetricsStorage {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(Vec::new())),
        }
    }
    
    pub fn add_metrics(&self, metrics: PerformanceMetrics) {
        let mut metrics_guard = self.metrics.lock().unwrap();
        metrics_guard.push(metrics);
    }
    
    pub fn get_all_metrics(&self) -> Vec<PerformanceMetrics> {
        let metrics_guard = self.metrics.lock().unwrap();
        metrics_guard.clone()
    }
    
    pub fn save_to_json_file(&self, filename: &str) -> Result<()> {
        let metrics = self.get_all_metrics();
        
        // Convert metrics to JSON-friendly format
        let json_metrics: Vec<serde_json::Value> = metrics.iter()
            .map(|m| m.to_json())
            .collect();
        
        let json = serde_json::to_string_pretty(&json_metrics)?;
        
        let mut file = File::create(filename)?;
        file.write_all(json.as_bytes())?;
        Ok(())
    }
    
    // Calculate average metrics by transaction type
    pub fn get_average_metrics_by_type(&self) -> HashMap<String, serde_json::Value> {
        let metrics = self.get_all_metrics();
        let mut type_groups: HashMap<String, Vec<&PerformanceMetrics>> = HashMap::new();
        
        // Group metrics by transaction type
        for metric in metrics.iter() {
            type_groups.entry(metric.transaction_type.clone())
                .or_insert_with(Vec::new)
                .push(metric);
        }
        
        // Calculate averages for each type
        let mut averages = HashMap::new();
        for (tx_type, metrics) in type_groups {
            let count = metrics.len() as f64;
            
            // Calculate generation time average
            let gen_times: Vec<f64> = metrics.iter()
                .filter_map(|m| m.generation_time_ms())
                .collect();
            
            let avg_gen_time = if !gen_times.is_empty() {
                gen_times.iter().sum::<f64>() / gen_times.len() as f64
            } else {
                0.0
            };
            
            // Calculate SUI time average
            let sui_times: Vec<f64> = metrics.iter()
                .filter_map(|m| m.sui_time_ms())
                .collect();
            
            let avg_sui_time = if !sui_times.is_empty() {
                sui_times.iter().sum::<f64>() / sui_times.len() as f64
            } else {
                0.0
            };
            
            // Calculate execution time average
            let exec_times: Vec<f64> = metrics.iter()
                .filter_map(|m| m.execution_time_ms())
                .collect();
            
            let avg_exec_time = if !exec_times.is_empty() {
                exec_times.iter().sum::<f64>() / exec_times.len() as f64
            } else {
                0.0
            };
            
            // Calculate total time average
            let total_times: Vec<f64> = metrics.iter()
                .filter_map(|m| m.total_time_ms())
                .collect();
            
            let avg_total_time = if !total_times.is_empty() {
                total_times.iter().sum::<f64>() / total_times.len() as f64
            } else {
                0.0
            };
            
            // Store the averages
            averages.insert(tx_type, serde_json::json!({
                "sample_count": count,
                "avg_generation_time_ms": avg_gen_time,
                "avg_sui_time_ms": avg_sui_time,
                "avg_execution_time_ms": avg_exec_time,
                "avg_total_time_ms": avg_total_time,
                "middleware_overhead_percent": ((avg_gen_time + avg_exec_time) / avg_sui_time * 100.0)
            }));
        }
        
        averages
    }
    
    pub fn print_summary(&self) {
        let averages = self.get_average_metrics_by_type();
        
        println!("\n=== PERFORMANCE METRICS SUMMARY ===");
        for (tx_type, stats) in averages {
            println!("\nTransaction Type: {}", tx_type);
            println!("Sample Count: {}", stats["sample_count"]);
            println!("Average Generation Time: {:.2} ms", stats["avg_generation_time_ms"]);
            println!("Average SUI Time: {:.2} ms", stats["avg_sui_time_ms"]);
            println!("Average Execution Time: {:.2} ms", stats["avg_execution_time_ms"]);
            println!("Average Total Time: {:.2} ms", stats["avg_total_time_ms"]);
            println!("Middleware Overhead: {:.2}%", stats["middleware_overhead_percent"]);
        }
        println!("\n=================================");
    }
}