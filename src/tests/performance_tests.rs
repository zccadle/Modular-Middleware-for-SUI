#[tokio::test]
async fn test_javascript_transaction_performance() {
    // Setup metrics
    let metrics_storage = MetricsStorage::new();
    
    // Run multiple iterations
    for i in 0..10 {
        let mut metrics = PerformanceMetrics::new("javascript");
        
        // Create JavaScript transaction
        let js_transaction = create_js_transaction();
        
        // Process transaction with metrics tracking
        process_transaction(js_transaction, &mut metrics).await;
        
        // Store metrics
        metrics_storage.add_metrics(metrics);
    }
    
    // Save results
    metrics_storage.save_to_json_file("js_performance.json").unwrap();
}

#[tokio::test]
async fn test_python_transaction_performance() {
    // Similar structure to JavaScript test
}

#[tokio::test]
async fn test_weather_transaction_performance() {
    // Similar structure to JavaScript test
}