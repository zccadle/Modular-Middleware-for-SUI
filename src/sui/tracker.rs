use anyhow::Result;
use crate::metrics::performance::PerformanceMetrics;
use std::time::SystemTime;

/// Track a SUI interaction asynchronously
pub async fn track_sui_interaction<F, Fut, T, E>(
    mut metrics: Option<&mut PerformanceMetrics>,
    f: F
) -> Result<T, E>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::error::Error + Send + Sync + 'static,
{
    if let Some(m) = metrics.as_mut() {
        m.sui_start_time = Some(SystemTime::now());
    }
    
    let result = f().await;
    
    if let Some(m) = metrics.as_mut() {
        m.sui_end_time = Some(SystemTime::now());
    }
    
    result
}

pub fn track_sui_operation<F, R>(metrics: &mut PerformanceMetrics, operation: F) -> R
where
    F: FnOnce() -> R
{
    // Start timing
    metrics.sui_start_time = Some(SystemTime::now());
    
    // Execute operation
    let result = operation();
    
    // End timing
    metrics.sui_end_time = Some(SystemTime::now());
    
    result
}