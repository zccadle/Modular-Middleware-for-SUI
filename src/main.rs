mod transaction;
mod execution;
mod languages;
mod external;
mod conditions;
mod demo;
mod metrics;
mod examples;
mod sui;
mod security;

#[cfg(test)]
mod tests;

use anyhow::{Result, anyhow};
use ed25519_dalek::{Keypair, PublicKey};
use rand::rngs::OsRng; 
use std::error::Error;
use std::path::Path;
use std::fs;
use std::env;
use std::sync::Arc;

use transaction::types::{Transaction, TransactionType, ExternalQuery, QueryCondition};
use transaction::handler::TransactionHandler;
use transaction::sequencing::SequencingLayer;
use execution::manager::ExecutionManager;
use execution::fallback::FallbackManager;
use demo::weather::run_weather_based_transaction_demo;
use examples::flight_delay::run_flight_delay_demo;
use metrics::storage::MetricsStorage;
use metrics::performance::PerformanceMetrics;
use examples::flight_insurance::run_flight_insurance_demo;
use examples::enhanced_flight_insurance::run_enhanced_flight_insurance_demo;
use crate::sui::verification::{VerificationManager, VerificationStatus};
use crate::sui::network::{NetworkManager, NetworkType};
use crate::security::audit::{SecurityAuditLog, AuditEvent, AuditEventType, AuditSeverity};
use crate::security::model::{SecurityModel, generate_security_documentation};
use crate::security::verification::{create_verification_framework, demonstrate_security_verification};
use crate::sui::byzantine::ByzantineDetector;
use crate::sui::cross_chain::{create_chain_mapper, demonstrate_cross_chain_mapping};
use crate::external::oracle::{create_weather_oracle, OracleManager};


// For demo purposes, force gas payment validation to succeed.
const _DEMO_MODE: bool = true;
// Use the official SUI testnet endpoint.
const SUI_TESTNET_RPC: &str = "https://fullnode.testnet.sui.io:443";
// Provided public key; if only public key is available, a new keypair is generated.
const PUBLIC_KEY: &str = "AExF8y0MXp/Sl+UteSwmGoXwWC0L/tDt1U4Mq+EsrdD2";

async fn run_js_transaction_demo(
    transaction_handler: &TransactionHandler,
    execution_manager: &ExecutionManager,
    metrics_storage: Option<&MetricsStorage>,
    security_audit_log: &SecurityAuditLog,
) -> Result<()> {
    println!("\n--- RUNNING JAVASCRIPT TRANSACTION DEMO WITH SECURITY VALIDATION ---\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("javascript"))
    } else {
        None
    };
    
    // Log the operation
    security_audit_log.log_validation(
        "JavaScriptDemo",
        "Starting JavaScript transaction demo with security validation",
        None,
        AuditSeverity::Info
    )?;
    
    // Create a simple JavaScript transaction
    let js_script = r#"
// Simple JavaScript transaction logic with security validation
const currentHour = new Date().getHours();
const currentDay = new Date().getDay(); // 0 = Sunday, 6 = Saturday

// Security: Input validation
if (typeof currentHour !== 'number' || currentHour < 0 || currentHour > 23) {
    console.error("Security validation failed: Invalid hour");
    return {
        shouldExecute: false,
        securityValidated: false,
        error: "Invalid hour detected"
    };
}

// Calculate gas based on time of day with bounds checking
let optimalGas = 50;
if (currentHour < 12) {
    optimalGas = 75; // Morning
} else if (currentHour < 18) {
    optimalGas = 90; // Afternoon
} else {
    optimalGas = 60; // Evening
}

// Security: Ensure gas is within acceptable limits
const minGas = 50;
const maxGas = 200;
const gasFactor = 1.2;
const finalGas = Math.min(maxGas, Math.max(minGas, Math.round(optimalGas * gasFactor)));

// Security: Include validation in response
({
    shouldExecute: true,
    gasAdjustment: gasFactor,
    gasBudget: finalGas,
    analysis: `JavaScript transaction processed at ${new Date().toTimeString()}`,
    securityValidated: true,
    securityContext: {
        inputValidated: true,
        gasBoundsChecked: true,
        timestamp: Date.now()
    }
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
    
    // Create a mock transaction digest for verification
    let mock_digest = format!("js_tx_{}", rand::random::<u64>());
    
    match transaction_handler.validate_transaction(&js_txn, metrics.as_mut()).await {
        Ok(true) => {
            println!("JavaScript transaction validated successfully with security checks.");
            
            // Register for verification
            transaction_handler.register_for_verification(&js_txn, &mock_digest)?;
            
            let wrapped_txn = transaction_handler.wrap_transaction(js_txn.clone(), metrics.as_mut())?;
            
            match execution_manager.execute_transaction(&mut js_txn, metrics.as_mut()).await {
                Ok(true) => {
                    println!("✅ JavaScript transaction executed successfully with gas budget: {}", js_txn.gas_budget);
                    
                    // Verify transaction
                    match transaction_handler.verify_transaction(&mock_digest, metrics.as_mut()).await {
                        Ok(VerificationStatus::Verified) => {
                            println!("✅ Transaction verified successfully!");
                            
                            // Log successful verification
                            security_audit_log.log_verification(
                                "JavaScriptDemo",
                                "Transaction verified successfully",
                                Some(&mock_digest),
                                AuditSeverity::Info
                            )?;
                        },
                        Ok(status) => {
                            println!("⚠️ Transaction verification status: {:?}", status);
                            
                            // Log verification status
                            security_audit_log.log_verification(
                                "JavaScriptDemo",
                                &format!("Transaction verification status: {:?}", status),
                                Some(&mock_digest),
                                AuditSeverity::Warning
                            )?;
                        },
                        Err(e) => {
                            println!("❌ Error verifying transaction: {}", e);
                            
                            // Log verification error
                            security_audit_log.log_verification(
                                "JavaScriptDemo",
                                &format!("Error verifying transaction: {}", e),
                                Some(&mock_digest),
                                AuditSeverity::Error
                            )?;
                        }
                    }
                },
                Ok(false) => {
                    println!("❌ JavaScript transaction condition failed - this should not happen in demo mode.");
                    
                    // Log execution failure
                    security_audit_log.log_execution(
                        "JavaScriptDemo",
                        "JavaScript transaction condition failed",
                        Some(&mock_digest),
                        AuditSeverity::Warning
                    )?;
                },
                Err(e) => {
                    println!("❌ JavaScript execution error: {}", e);
                    
                    // Log execution error
                    security_audit_log.log_execution(
                        "JavaScriptDemo",
                        &format!("JavaScript execution error: {}", e),
                        Some(&mock_digest),
                        AuditSeverity::Error
                    )?;
                }
            }
        },
        Ok(false) => {
            println!("❌ Validation Error: JavaScript transaction did not pass validation.");
            
            // Log validation failure
            security_audit_log.log_validation(
                "JavaScriptDemo",
                "JavaScript transaction did not pass validation",
                None,
                AuditSeverity::Warning
            )?;
        },
        Err(e) => {
            println!("❌ Validation Error: {}", e);
            
            // Log validation error
            security_audit_log.log_validation(
                "JavaScriptDemo",
                &format!("Validation error: {}", e),
                None,
                AuditSeverity::Error
            )?;
        }
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
    metrics_storage: Option<&MetricsStorage>,
    security_audit_log: &SecurityAuditLog,
) -> Result<()> {
    println!("\n--- RUNNING PYTHON TRANSACTION DEMO WITH SECURITY VALIDATION ---\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("python"))
    } else {
        None
    };
    
    // Log the operation
    security_audit_log.log_validation(
        "PythonDemo",
        "Starting Python transaction demo with security validation",
        None,
        AuditSeverity::Info
    )?;
    
    // Python script that adjusts gas fees based on time of day with security validation
    let python_script = r#"
# Demo Python transaction logic with security validation
import datetime
import json

# Security: Input validation function
def validate_input():
    current_hour = datetime.datetime.now().hour
    current_day = datetime.datetime.now().weekday()  # 0 = Monday, 6 = Sunday
    
    if not isinstance(current_hour, int) or current_hour < 0 or current_hour > 23:
        print("Security validation failed: Invalid hour")
        return False, current_hour, current_day
        
    if not isinstance(current_day, int) or current_day < 0 or current_day > 6:
        print("Security validation failed: Invalid day")
        return False, current_hour, current_day
    
    return True, current_hour, current_day

# Perform input validation
is_valid, current_hour, current_day = validate_input()

if not is_valid:
    result = {
        "should_execute": False,
        "security_validated": False,
        "error": "Input validation failed"
    }
else:
    # Calculate optimal gas based on time of day
    if current_hour < 9:  # Early morning
        optimal_gas = 60
    elif current_hour < 17:  # Business hours
        optimal_gas = 85
    else:  # Evening
        optimal_gas = 70

    # Security: Gas price bounds checking
    min_gas = 50
    max_gas = 150
    gas_budget = max(min_gas, min(max_gas, optimal_gas))

    # Security: Record validation in result
    result = {
        "should_execute": True,  # Always true for the demo
        "gas_budget": gas_budget,
        "analysis": f"Transaction analyzed at {datetime.datetime.now().strftime('%H:%M:%S')} on day {current_day}",
        "security_validated": True,
        "security_context": {
            "input_validated": True,
            "gas_bounds_checked": True,
            "timestamp": datetime.datetime.now().timestamp()
        }
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

    // Create a mock transaction digest for verification
    let mock_digest = format!("python_tx_{}", rand::random::<u64>());

    println!("Processing Python-based transaction with security validation...");
    match transaction_handler.validate_transaction(&txn, metrics.as_mut()).await {
        Ok(true) => {
            println!("Python transaction validated successfully with security checks.");
            
            // Register for verification
            transaction_handler.register_for_verification(&txn, &mock_digest)?;
            
            let wrapped_txn = transaction_handler.wrap_transaction(txn.clone(), metrics.as_mut())?;
            
            match execution_manager.execute_transaction(&mut txn, metrics.as_mut()).await {
                Ok(true) => {
                    println!("✅ Python transaction executed successfully with gas budget: {}", txn.gas_budget);
                    
                    // Verify transaction
                    match transaction_handler.verify_transaction(&mock_digest, metrics.as_mut()).await {
                        Ok(VerificationStatus::Verified) => {
                            println!("✅ Transaction verified successfully!");
                            
                            // Log successful verification
                            security_audit_log.log_verification(
                                "PythonDemo",
                                "Transaction verified successfully",
                                Some(&mock_digest),
                                AuditSeverity::Info
                            )?;
                        },
                        Ok(status) => {
                            println!("⚠️ Transaction verification status: {:?}", status);
                            
                            // Log verification status
                            security_audit_log.log_verification(
                                "PythonDemo",
                                &format!("Transaction verification status: {:?}", status),
                                Some(&mock_digest),
                                AuditSeverity::Warning
                            )?;
                        },
                        Err(e) => {
                            println!("❌ Error verifying transaction: {}", e);
                            
                            // Log verification error
                            security_audit_log.log_verification(
                                "PythonDemo",
                                &format!("Error verifying transaction: {}", e),
                                Some(&mock_digest),
                                AuditSeverity::Error
                            )?;
                        }
                    }
                },
                Ok(false) => {
                    println!("❌ Python transaction condition failed - this should not happen in demo mode.");
                    
                    // Log execution failure
                    security_audit_log.log_execution(
                        "PythonDemo",
                        "Python transaction condition failed",
                        Some(&mock_digest),
                        AuditSeverity::Warning
                    )?;
                },
                Err(e) => {
                    println!("❌ Execution error: {}", e);
                    
                    // Log execution error
                    security_audit_log.log_execution(
                        "PythonDemo",
                        &format!("Python execution error: {}", e),
                        Some(&mock_digest),
                        AuditSeverity::Error
                    )?;
                }
            }
        },
        Ok(false) => {
            println!("❌ Validation Error: Python transaction did not pass validation.");
            
            // Log validation failure
            security_audit_log.log_validation(
                "PythonDemo",
                "Python transaction did not pass validation",
                None,
                AuditSeverity::Warning
            )?;
        },
        Err(e) => {
            println!("❌ Validation Error: {}", e);
            
            // Log validation error
            security_audit_log.log_validation(
                "PythonDemo",
                &format!("Validation error: {}", e),
                None,
                AuditSeverity::Error
            )?;
        }
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
    
    // Create output directory for metrics, charts, and documentation
    let output_dir = "performance_results";
    if !Path::new(output_dir).exists() {
        fs::create_dir(output_dir)?;
    }
    
    // Create docs directory for documentation
    let docs_dir = "docs";
    if !Path::new(docs_dir).exists() {
        fs::create_dir(docs_dir)?;
    }
    
    // Initialize security audit log
    let security_audit_log = SecurityAuditLog::new();
    
    // Initialize network manager
    let network_type = match env::var("SUI_NETWORK_TYPE").unwrap_or_else(|_| "testnet".to_string()).as_str() {
        "mainnet" => NetworkType::Mainnet,
        "devnet" => NetworkType::Devnet,
        "local" => NetworkType::Local,
        _ => NetworkType::Testnet,
    };

    let network_manager = NetworkManager::new(network_type);
    let rpc_url = network_manager.get_active_rpc_url()?;

    // Initialize verification manager
    let verification_manager = VerificationManager::new(&rpc_url);
    
    // Initialize Byzantine fault detector
    let byzantine_detector = ByzantineDetector::new(
        network_manager.get_active_config().rpc_endpoints.clone(),
        Some(Arc::new(security_audit_log.clone())),
        None,
        None,
    );
    
    // Initialize cross-chain mapper
    let chain_mapper = create_chain_mapper(
        Arc::new(network_manager.clone()),
        Some(Arc::new(security_audit_log.clone()))
    )?;
    
    // Initialize Oracle Manager
    let weather_oracle = create_weather_oracle(
        Some(Arc::new(security_audit_log.clone()))
    )?;
    
    // Initialize formal verification framework
    let verification_framework = create_verification_framework(
        Some(Arc::new(security_audit_log.clone()))
    );

    // Log startup security event
    security_audit_log.log_network(
        "main", 
        &format!("Middleware started with network type: {}", network_type),
        Some(&network_manager.get_active_config().chain_id),
        AuditSeverity::Info
    )?;

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
    let transaction_handler = TransactionHandler::new(
        keypair, 
        Some(verification_manager.clone()), 
        Some(security_audit_log.clone())
    );
    let sequencing_layer = SequencingLayer::new();
    let execution_manager = ExecutionManager::new(
        Some(verification_manager.clone()), 
        Some(Arc::new(network_manager.clone())), 
        Some(Arc::new(security_audit_log.clone()))
    );    
    let fallback_manager = FallbackManager::new();

    println!("SUI Modular Middleware started with enhanced security features:");
    println!("- Multi-language Support (JavaScript, Python)");
    println!("- Formal Security Model with Verification");
    println!("- Byzantine Fault Detection");
    println!("- External Data Oracle Framework");
    println!("- Cross-Chain Transaction Portability");
    println!("- Comprehensive Security Audit Logging");
    println!("- Performance Measurement Framework");

    // Generate security documentation
    let security_model = SecurityModel::new();
    let security_docs = generate_security_documentation();
    
    // Save security documentation to file
    let security_docs_path = format!("{}/security-model.md", docs_dir);
    fs::write(&security_docs_path, security_docs)?;
    println!("Security model documentation generated: {}", security_docs_path);
    
    // Run performance tests
    println!("\n=== RUNNING PERFORMANCE TESTS WITH SECURITY VALIDATION ===");
    println!("Each transaction type will be run multiple times to gather performance metrics.");
    
    // Run each demo multiple times with performance tracking
    for i in 1..=3 {
        println!("\nTest Iteration {} of 3", i);
        
        // Reset account balances at the start of each iteration
        execution_manager.reset_account_balances();

        // Weather-based demo
        run_weather_based_transaction_demo(&transaction_handler, &execution_manager, Some(&metrics_storage)).await?;
        
        // Python transaction demo
        run_python_transaction_demo(&transaction_handler, &execution_manager, Some(&metrics_storage), &security_audit_log).await?;
        
        // JavaScript transaction demo
        run_js_transaction_demo(&transaction_handler, &execution_manager, Some(&metrics_storage), &security_audit_log).await?;
        
        // Flight delay insurance demo
        run_flight_delay_demo(&transaction_handler, &execution_manager, Some(&metrics_storage)).await?;
        
        // Enhanced flight insurance demo with security features
        run_enhanced_flight_insurance_demo(
            &transaction_handler,
            &execution_manager,
            Some(&metrics_storage),
            &security_audit_log,
            &verification_manager,
            &network_manager
        ).await?;
    }
    
    // Demonstrate security verification
    demonstrate_security_verification(&verification_framework)?;
    
    // Demonstrate cross-chain mapping
    // Create a simple transaction for demonstration
    let demo_tx = Transaction {
        tx_type: TransactionType::Transfer,
        sender: "0x123".to_string(),
        receiver: "0x456".to_string(),
        amount: 100,
        gas_payment: "0x789".to_string(),
        gas_budget: 1000,
        commands: vec!["transfer".to_string()],
        signatures: None,
        timestamp: chrono::Utc::now().timestamp() as u64,
        script: None,
        external_query: None,
        python_code: None,
        python_params: None,
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
        language: None,
    };
    
    demonstrate_cross_chain_mapping(&demo_tx, &chain_mapper).await.unwrap_or_else(|e| {
        println!("Cross-chain mapping demonstration error: {}", e);
    });
    
    // Save performance metrics to file
    let metrics_file = format!("{}/performance_metrics.json", output_dir);
    metrics_storage.save_to_json_file(&metrics_file)?;
    
    // Print summary statistics
    metrics_storage.print_summary();
    
    println!("\nPerformance metrics saved to {}", metrics_file);
    println!("To generate visualization, run:");
    println!("python tools/visualize_performance.py {} {}/performance_chart.png", metrics_file, output_dir);

    // Poll transactions from the blockchain
    println!("\nPolling for ordered transactions with security validation...");
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
                            
                            // Register for verification
                            let mock_digest = format!("polled_tx_{}", rand::random::<u64>());
                            transaction_handler.register_for_verification(&tx, &mock_digest)?;
                            
                            match execution_manager.execute_transaction(&mut tx, None).await {
                                Ok(true) => {
                                    println!("Transaction executed successfully.");
                                    
                                    // Verify transaction
                                    match transaction_handler.verify_transaction(&mock_digest, None).await {
                                        Ok(VerificationStatus::Verified) => {
                                            println!("Transaction verified successfully.");
                                        },
                                        Ok(status) => {
                                            println!("Transaction verification status: {:?}", status);
                                        },
                                        Err(e) => {
                                            println!("Error verifying transaction: {}", e);
                                            fallback_manager.log_error();
                                        }
                                    }
                                },
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
    
    // Print security audit summary
    let events = security_audit_log.get_events();
    println!("\n=== SECURITY AUDIT SUMMARY ===");
    println!("Total security events: {}", events.len());
    
    // Group events by type
    let mut event_types = std::collections::HashMap::new();
    for event in &events {
        let count = event_types.entry(format!("{:?}", event.event_type)).or_insert(0);
        *count += 1;
    }
    
    // Print event type counts
    println!("\nEvent types:");
    for (event_type, count) in event_types {
        println!("  {}: {}", event_type, count);
    }
    
    // Group events by severity
    let mut event_severities = std::collections::HashMap::new();
    for event in &events {
        let count = event_severities.entry(format!("{:?}", event.severity)).or_insert(0);
        *count += 1;
    }
    
    // Print severity counts
    println!("\nEvent severities:");
    for (severity, count) in event_severities {
        println!("  {}: {}", severity, count);
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