use anyhow::Result;
use crate::metrics::performance::PerformanceMetrics;

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
        m.sui_start_time = Some(std::time::Instant::now());
    }
    
    let result = f().await;
    
    if let Some(m) = metrics.as_mut() {
        m.sui_end_time = Some(std::time::Instant::now());
    }
    
    result
}