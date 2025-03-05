use crate::metrics::storage::MetricsStorage;
use crate::metrics::performance::PerformanceMetrics;
use crate::transaction::types::{Transaction, TransactionType};
use crate::transaction::handler::TransactionHandler;
use crate::execution::manager::ExecutionManager;
use anyhow::Result;
use rand::thread_rng;
use ed25519_dalek::Keypair;

// Helper function to create a JS transaction
fn create_js_transaction() -> Transaction {
    Transaction {
        tx_type: TransactionType::Transfer,
        sender: "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(),
        receiver: "0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(),
        amount: 50,
        gas_payment: "0xb9fd6cfa2637273ca33dd3aef2f0b0296755889a0ef7f77c9cc95953a96a6302".to_string(),
        gas_budget: 50,
        commands: vec!["TransferObjects".to_string()],
        signatures: None,
        timestamp: 0,
        script: Some(r#"
            // Simple JS logic
            const result = {
                gasAdjustment: 1.2,
                gasBudget: Math.round(50 * 1.2),
                analysis: `Test transaction processed at ${new Date().toTimeString()}`
            };
            result;
        "#.to_string()),
        external_query: None,
        python_code: None,
        python_params: None,
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
        language: Some("javascript".to_string()),
    }
}

// Helper function to create a Python transaction
fn create_python_transaction() -> Transaction {
    Transaction {
        tx_type: TransactionType::Transfer,
        sender: "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(),
        receiver: "0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(),
        amount: 100,
        gas_payment: "0xb9fd6cfa2637273ca33dd3aef2f0b0296755889a0ef7f77c9cc95953a96a6302".to_string(),
        gas_budget: 50,
        commands: vec!["TransferObjects".to_string()],
        signatures: None,
        timestamp: 0,
        script: None,
        external_query: None,
        python_code: Some(r#"
# Simple Python logic
result = {
    "should_execute": True,
    "gas_budget": 75,
    "analysis": "Python test transaction"
}
        "#.to_string()),
        python_params: None,
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
        language: Some("python".to_string()),
    }
}

// Helper function to process a transaction with metrics
async fn process_transaction(tx: Transaction, metrics: &mut PerformanceMetrics) -> Result<bool> {
    // Create handlers for processing
    let keypair = Keypair::generate(&mut thread_rng());
    let transaction_handler = TransactionHandler::new(keypair, None, None);
    let execution_manager = ExecutionManager::new(None, None, None);
    
    // Validate transaction
    if transaction_handler.validate_transaction(&tx, Some(metrics)).await? {
        // Wrap transaction
        let _wrapped_txn = transaction_handler.wrap_transaction(tx.clone(), Some(metrics))?;
        
        // Execute transaction
        execution_manager.execute_transaction(&mut tx.clone(), Some(metrics)).await
    } else {
        Ok(false)
    }
}

#[tokio::test]
async fn test_javascript_transaction_performance() -> Result<()> {
    // Setup metrics
    let metrics_storage = MetricsStorage::new();
    
    // Run multiple iterations
    for i in 0..5 {
        println!("JS test iteration {}/5", i+1);
        let mut metrics = PerformanceMetrics::new("javascript");
        
        // Create JavaScript transaction
        let js_transaction = create_js_transaction();
        
        // Process transaction with metrics tracking
        let result = process_transaction(js_transaction, &mut metrics).await?;
        println!("Transaction result: {}", result);
        
        // Store metrics
        metrics_storage.add_metrics(metrics);
    }
    
    // Save results
    metrics_storage.save_to_json_file("js_performance.json")?;
    metrics_storage.print_summary();
    
    Ok(())
}

#[tokio::test]
async fn test_python_transaction_performance() -> Result<()> {
    // Setup metrics
    let metrics_storage = MetricsStorage::new();
    
    // Run multiple iterations
    for i in 0..5 {
        println!("Python test iteration {}/5", i+1);
        let mut metrics = PerformanceMetrics::new("python");
        
        // Create Python transaction
        let python_transaction = create_python_transaction();
        
        // Process transaction with metrics tracking
        let result = process_transaction(python_transaction, &mut metrics).await?;
        println!("Transaction result: {}", result);
        
        // Store metrics
        metrics_storage.add_metrics(metrics);
    }
    
    // Save results
    metrics_storage.save_to_json_file("python_performance.json")?;
    metrics_storage.print_summary();
    
    Ok(())
}