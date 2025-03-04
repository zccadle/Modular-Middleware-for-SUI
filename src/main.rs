mod transaction;
mod execution;
mod languages;
mod external;
mod conditions;
mod demo;
mod metrics;
mod examples;

use anyhow::{Result, anyhow};
use ed25519_dalek::{Keypair, PublicKey};
use rand::rngs::OsRng; 
use std::error::Error;
use std::path::Path;
use std::fs;

use transaction::types::{Transaction, TransactionType, ExternalQuery, QueryCondition};
use transaction::handler::TransactionHandler;
use transaction::sequencing::SequencingLayer;
use execution::manager::ExecutionManager;
use execution::fallback::FallbackManager;
use demo::weather::run_weather_based_transaction_demo;
use examples::flight_delay::run_flight_delay_demo;
use metrics::storage::MetricsStorage;
use metrics::performance::PerformanceMetrics;

// For demo purposes, force gas payment validation to succeed.
const _DEMO_MODE: bool = true;
// Use the official SUI testnet endpoint.
const SUI_TESTNET_RPC: &str = "https://fullnode.testnet.sui.io:443";
// Provided public key; if only public key is available, a new keypair is generated.
const PUBLIC_KEY: &str = "AExF8y0MXp/Sl+UteSwmGoXwWC0L/tDt1U4Mq+EsrdD2";

async fn run_js_transaction_demo(
    transaction_handler: &TransactionHandler,
    execution_manager: &ExecutionManager,
    metrics_storage: Option<&MetricsStorage>
) -> Result<()> {
    println!("\n--- RUNNING JAVASCRIPT TRANSACTION DEMO ---\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("javascript"))
    } else {
        None
    };
    
    // Create a simple JavaScript transaction
    let js_script = r#"
// Simple JavaScript transaction logic
const currentHour = new Date().getHours();
const currentDay = new Date().getDay(); // 0 = Sunday, 6 = Saturday

// Calculate gas based on time of day
let optimalGas = 50;
if (currentHour < 12) {
    optimalGas = 75; // Morning
} else if (currentHour < 18) {
    optimalGas = 90; // Afternoon
} else {
    optimalGas = 60; // Evening
}

// Demo mode - always execute
({
    shouldExecute: true,
    gasAdjustment: 1.2,
    gasBudget: Math.round(optimalGas * 1.2),
    analysis: `JavaScript transaction processed at ${new Date().toTimeString()}`
})
"#;

    let mut js_txn = Transaction {
        tx_type: TransactionType::Transfer,
        sender: "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(),
        receiver: "0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(),
        amount: 50,
        gas_payment: "0xb9fd6cfa2637273ca33dd3aef2f0b0296755889a0ef7f77c9cc95953a96a6302".to_string(),
        gas_budget: 50,
        commands: vec!["TransferObjects".to_string()],
        signatures: None,
        timestamp: 0,
        script: Some(js_script.to_string()),
        external_query: None,
        python_code: None,
        python_params: None,
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
        language: Some("javascript".to_string()),
    };
    
    match transaction_handler.validate_transaction(&js_txn, metrics.as_mut()).await {
        Ok(true) => {
            println!("JavaScript transaction validated successfully.");
            
            let wrapped_txn = transaction_handler.wrap_transaction(js_txn.clone(), metrics.as_mut())?;
            
            match execution_manager.execute_transaction(&mut js_txn, metrics.as_mut()).await {
                Ok(true) => println!("✅ JavaScript transaction executed successfully with gas budget: {}", js_txn.gas_budget),
                Ok(false) => println!("❌ JavaScript transaction condition failed - this should not happen in demo mode."),
                Err(e) => {
                    println!("❌ JavaScript execution error: {}", e);
                }
            }
        },
        Ok(false) => println!("❌ Validation Error: JavaScript transaction did not pass validation."),
        Err(e) => println!("❌ Validation Error: {}", e),
    }
    
    // Store metrics if provided
    if let (Some(m), Some(storage)) = (metrics, metrics_storage) {
        storage.add_metrics(m);
    }
    
    println!("\n--- JAVASCRIPT TRANSACTION DEMO COMPLETE ---\n");
    
    Ok(())
}

async fn run_python_transaction_demo(
    transaction_handler: &TransactionHandler,
    execution_manager: &ExecutionManager,
    metrics_storage: Option<&MetricsStorage>
) -> Result<()> {
    println!("\n--- RUNNING PYTHON TRANSACTION DEMO ---\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("python"))
    } else {
        None
    };
    
    // Python script that adjusts gas fees based on time of day
    let python_script = r#"
# Demo Python transaction logic
import datetime

current_hour = datetime.datetime.now().hour
current_day = datetime.datetime.now().weekday()  # 0 = Monday, 6 = Sunday

# Calculate optimal gas based on time of day
if current_hour < 9:  # Early morning
    optimal_gas = 60
elif current_hour < 17:  # Business hours
    optimal_gas = 85
else:  # Evening
    optimal_gas = 70

# DEMO MODE: Always execute
result = {
    "should_execute": True,  # Always true for the demo
    "gas_budget": optimal_gas,
    "analysis": f"Transaction analyzed at {datetime.datetime.now().strftime('%H:%M:%S')} on day {current_day}"
}
"#;

    // Create the Python transaction
    let mut txn = Transaction {
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
        // External query that will adjust gas based on ETH price
        external_query: Some(ExternalQuery {
            url: "https://api.coingecko.com/api/v3/simple/price?ids=ethereum&vs_currencies=usd".to_string(),
            path: vec!["ethereum".to_string(), "usd".to_string()],
            // Set a condition that will always be true (any ETH price will be > 0)
            condition: Some(QueryCondition {
                threshold: 1000,
                operator: "gt".to_string(),
            }),
        }),
        python_code: Some(python_script.to_string()),
        python_params: None,
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
        language: Some("python".to_string()),
    };

    println!("Processing Python-based transaction...");
    match transaction_handler.validate_transaction(&txn, metrics.as_mut()).await {
        Ok(true) => {
            println!("Python transaction validated successfully.");
            
            let wrapped_txn = transaction_handler.wrap_transaction(txn.clone(), metrics.as_mut())?;
            
            match execution_manager.execute_transaction(&mut txn, metrics.as_mut()).await {
                Ok(true) => println!("✅ Python transaction executed successfully with gas budget: {}", txn.gas_budget),
                Ok(false) => println!("❌ Python transaction condition failed - this should not happen in demo mode."),
                Err(e) => {
                    println!("❌ Execution error: {}", e);
                }
            }
        },
        Ok(false) => println!("❌ Validation Error: Python transaction did not pass validation."),
        Err(e) => println!("❌ Validation Error: {}", e),
    }
    
    // Store metrics if provided
    if let (Some(m), Some(storage)) = (metrics, metrics_storage) {
        storage.add_metrics(m);
    }
    
    println!("\n--- PYTHON TRANSACTION DEMO COMPLETE ---\n");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load environment variables if present
    dotenv::dotenv().ok();
    
    // Initialize metrics storage
    let metrics_storage = MetricsStorage::new();

    // Generate keypair
    let keypair_bytes = base64::decode(PUBLIC_KEY)
        .map_err(|e| anyhow!("Failed to decode base64 key: {}", e))?;
    
    let mut rng = OsRng;
    let keypair = if keypair_bytes.len() == 32 {
        PublicKey::from_bytes(&keypair_bytes)
            .map_err(|e| anyhow!("Failed to create public key: {}", e))?;
        println!("Warning: Only public key available. Generating new keypair for testing.");
        Keypair::generate(&mut rng)
    } else {
        Keypair::generate(&mut rng)
    };

    // Initialize system components
    let transaction_handler = TransactionHandler::new(keypair);
    let sequencing_layer = SequencingLayer::new();
    let execution_manager = ExecutionManager::new();
    let fallback_manager = FallbackManager::new();

    println!("SUI Modular Middleware started with enhanced capabilities...");
    println!("- Multi-language Support (JavaScript, Python)");
    println!("- External API Integration with WebSockets");
    println!("- Time-based Transaction Processing");
    println!("- Dynamic Transaction Parameters");
    println!("- Performance Measurement Framework");

    // Create output directory for metrics and charts
    let output_dir = "performance_results";
    if !Path::new(output_dir).exists() {
        fs::create_dir(output_dir)?;
    }
    
    // Run performance tests
    println!("\n=== RUNNING PERFORMANCE TESTS ===");
    println!("Each transaction type will be run multiple times to gather performance metrics.");
    
    // Run each demo multiple times with performance tracking
    for i in 1..=5 {
        println!("\nTest Iteration {} of 5", i);
        
        // Reset account balances at the start of each iteration
        execution_manager.reset_account_balances();

        // Weather-based demo
        run_weather_based_transaction_demo(&transaction_handler, &execution_manager, Some(&metrics_storage)).await?;
        
        // Python transaction demo
        run_python_transaction_demo(&transaction_handler, &execution_manager, Some(&metrics_storage)).await?;
        
        // JavaScript transaction demo
        run_js_transaction_demo(&transaction_handler, &execution_manager, Some(&metrics_storage)).await?;
        
        // Flight delay insurance demo
        run_flight_delay_demo(&transaction_handler, &execution_manager, Some(&metrics_storage)).await?;
    }
    
    // Save performance metrics to file
    let metrics_file = format!("{}/performance_metrics.json", output_dir);
    metrics_storage.save_to_json_file(&metrics_file)?;
    
    // Print summary statistics
    metrics_storage.print_summary();
    
    println!("\nPerformance metrics saved to {}", metrics_file);
    println!("To generate visualization, run:");
    println!("python tools/visualize_performance.py {} {}/performance_chart.png", metrics_file, output_dir);

    // Poll transactions from the blockchain as before
    println!("\nPolling for ordered transactions...");
    match sequencing_layer.poll_transactions().await {
        Ok(transactions) => {
            println!("Polled {} transaction(s).", transactions.len());
            for mut tx in transactions {
                println!("Processing polled transaction: {:?}", tx);
                if let Ok(true) = transaction_handler.validate_transaction(&tx, None).await {
                    println!("Validated polled transaction");
                    if let Ok(wrapped_tx) = transaction_handler.wrap_transaction(tx.clone(), None) {
                        if let Ok(_signature) = transaction_handler.sign_transaction(&wrapped_tx) {
                            println!("Polled transaction signed: {:?}", _signature);
                            match execution_manager.execute_transaction(&mut tx, None).await {
                                Ok(true) => println!("Transaction executed successfully."),
                                Ok(false) => println!("Transaction condition failed."),
                                Err(e) => {
                                    println!("Execution error: {}", e);
                                    fallback_manager.log_error();
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("Error polling transactions: {}", e);
            fallback_manager.log_error();
        }
    }
    
    // Print final execution state
    {
        let state = execution_manager.state.lock().unwrap();
        println!("\nFinal execution state (account balances):");
        for (address, balance) in state.iter() {
            println!("  {}: {}", address, balance);
        }
    }
    
    Ok(())
}