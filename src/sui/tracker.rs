pub async fn track_sui_interaction<F, Fut, T>(
    metrics: Option<&mut PerformanceMetrics>,
    f: F
) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    if let Some(m) = metrics {
        m.sui_start_time = Some(std::time::Instant::now());
    }
    
    let result = f().await;
    
    if let Some(m) = metrics {
        m.sui_end_time = Some(std::time::Instant::now());
    }
    
    result
}