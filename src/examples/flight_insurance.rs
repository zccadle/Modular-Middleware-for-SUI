use anyhow::Result;
use std::env;
use chrono::Utc;
use serde_json::json;

use crate::transaction::types::{Transaction, TransactionType};
use crate::execution::manager::ExecutionManager;
use crate::transaction::handler::TransactionHandler;
use crate::metrics::performance::PerformanceMetrics;
use crate::metrics::storage::MetricsStorage;
use crate::external::flight_api::{get_cached_flight_status, FlightStatus};
use crate::sui::contract::FlightInsuranceContract;

/// Enhanced flight insurance example that demonstrates a full end-to-end
/// implementation using real flight status APIs and interacting with a SUI
/// smart contract for insurance payouts.
///
/// This showcases how the middleware can enhance blockchain functionality by:
/// 1. Integrating external real-world data (flight status)
/// 2. Automating conditional payments (insurance claims)
/// 3. Providing a seamless user experience with minimal overhead
pub async fn run_flight_insurance_demo(
    transaction_handler: &TransactionHandler,
    execution_manager: &ExecutionManager,
    metrics_storage: Option<&MetricsStorage>
) -> Result<()> {
    println!("\n=== RUNNING ENHANCED FLIGHT INSURANCE DEMO ===\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("flight_insurance"))
    } else {
        None
    };
    
    // Get AviationStack API key (if available)
    // Check if .env file was loaded correctly
    println!("Loading environment variables...");
    for (key, value) in env::vars() {
        if key.contains("API") || key.contains("KEY") {
            println!("Found env var: {}={}", key, 
                     if value.len() > 4 { format!("{}...", &value[0..4]) } else { value.clone() });
        }
    }

    let api_key = env::var("AVIATION_STACK_API_KEY")
        .map(|key| {
            println!("Found API key in environment: {}", 
                     if key.len() > 4 { format!("{}...", &key[0..4]) } else { key.clone() });
            key
        })
        .unwrap_or_else(|e| {
            println!("Error loading API key: {}", e);
            println!("Using default API key");
            "YOUR_API_KEY".to_string()
        });

    // 1. Get flight status from real API
    let flight_number = "SKW5690"; // SkyWest Flight 5690 (for testing)
    
    println!("Checking status for flight {}...", flight_number);
    
    // Track the start time for full process
    let start_time = std::time::Instant::now();
    
    // Get flight status (cached to avoid hitting API limits during testing)
    let flight_status = match get_cached_flight_status(&api_key, flight_number).await {
        Ok(status) => {
            println!("✅ Flight status received:");
            println!("   Status: {}", status.status);
            println!("   Delay: {} minutes", status.delay_minutes);
            if status.is_cancelled() {
                println!("   Flight is CANCELLED");
            } else if status.is_delayed() {
                println!("   Flight is DELAYED");
            } else {
                println!("   Flight is ON TIME");
            }
            status
        }
        Err(e) => {
            println!("❌ Error getting flight status: {}", e);
            println!("Using simulated flight data for demo purposes");
            
            // Create simulated data for demo
            FlightStatus {
                flight_number: flight_number.to_string(),
                status: "delayed".to_string(),
                scheduled_departure: Some(Utc::now()),
                estimated_departure: Some(Utc::now() + chrono::Duration::minutes(75)),
                actual_departure: None,
                scheduled_arrival: Some(Utc::now() + chrono::Duration::hours(2)),
                estimated_arrival: Some(Utc::now() + chrono::Duration::hours(2) + chrono::Duration::minutes(75)),
                actual_arrival: None,
                delay_minutes: 70,
                raw_data: json!({"simulated": true}),
            }
        }
    };

    // 2. Create a Python script that will process the flight data and decide on compensation
    let python_code = r#"
# Flight insurance processor
import json

# Parse the flight data received from the API
try:
    flight_data = params["flight_status"]
    policy_data = params["policy"]
    print(f"Processing flight: {flight_data['flight_number']}")
    print(f"Flight status: {flight_data['status']}")
    print(f"Delay: {flight_data['delay_minutes']} minutes")
except Exception as e:
    print(f"Error parsing parameters: {e}")
    # We'll exit with an error result if we can't process
    result = {
        "success": False,
        "error": f"Parameter parsing error: {str(e)}",
        "should_execute": False
    }
    exit()

# Calculate compensation based on policy type and delay
def calculate_compensation(delay_minutes, policy_type, is_cancelled):
    # No compensation for short delays
    if delay_minutes < 30 and not is_cancelled:
        return 0
        
    # Standard policy calculations
    if policy_type == "standard":
        if is_cancelled:
            return 500
        elif delay_minutes >= 180:  # 3+ hours
            return 300
        elif delay_minutes >= 120:  # 2+ hours
            return 200
        elif delay_minutes >= 60:  # 1+ hour
            return 100
        elif delay_minutes >= 30:  # 30+ minutes
            return 50
    
    # Premium policy (higher payouts)
    elif policy_type == "premium":
        if is_cancelled:
            return 1000
        elif delay_minutes >= 180:
            return 600
        elif delay_minutes >= 120:
            return 400
        elif delay_minutes >= 60:
            return 200
        elif delay_minutes >= 30:
            return 100
            
    return 0

# Get policy details
policy_type = policy_data["policy_type"]
policy_id = policy_data["policy_id"]
is_cancelled = flight_data["status"].lower() == "cancelled"

# Calculate compensation
compensation_amount = calculate_compensation(
    flight_data["delay_minutes"], 
    policy_type,
    is_cancelled
)

# Determine if we should trigger a payout
should_execute = compensation_amount > 0

# Prepare result with all the necessary information
# for the smart contract call
result = {
    "success": True,
    "should_execute": should_execute,
    "compensation_amount": compensation_amount,
    "flight_number": flight_data["flight_number"],
    "delay_minutes": flight_data["delay_minutes"],
    "is_cancelled": is_cancelled,
    "policy_id": policy_id,
    "policy_type": policy_type,
    "gas_budget": 10000 if should_execute else 1000,
    "timestamp": params.get("timestamp", 0)
}
"#;

    // 3. Initialize the SUI contract interface
    let insurance_contract = FlightInsuranceContract::new();
    
    // 4. Create a user insurance policy (in a real implementation, this would
    // already exist in the blockchain)
    let policy_id = "POL123456789";
    let policy_type = "premium"; // premium or standard
    
    println!("Processing insurance claim for policy {} (type: {})", policy_id, policy_type);
    
    // 5. Create the transaction that will process the claim if needed
    let mut transaction = Transaction {
        tx_type: TransactionType::Transfer,
        // These are placeholder addresses
        sender: "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(),
        receiver: "0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(),
        amount: 0, // Will be set by Python code
        gas_payment: "0xb9fd6cfa2637273ca33dd3aef2f0b0296755889a0ef7f77c9cc95953a96a6302".to_string(),
        gas_budget: 2000,
        commands: vec!["TransferObjects".to_string()],
        signatures: None,
        timestamp: Utc::now().timestamp() as u64,
        script: None,
        external_query: None,
        python_code: Some(python_code.to_string()),
        python_params: Some(json!({
            "flight_status": {
                "flight_number": flight_status.flight_number,
                "status": flight_status.status,
                "delay_minutes": flight_status.delay_minutes,
                "scheduled_departure": flight_status.scheduled_departure,
                "actual_departure": flight_status.actual_departure,
                "scheduled_arrival": flight_status.scheduled_arrival,
                "actual_arrival": flight_status.actual_arrival
            },
            "policy": {
                "policy_id": policy_id,
                "policy_type": policy_type,
                "owner": "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6"
            },
            "timestamp": Utc::now().timestamp()
        })),
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
        language: Some("python".to_string()),
    };
    
    // 6. Process the transaction with performance tracking
    println!("Validating flight insurance transaction...");
    
    // Validate transaction
    match transaction_handler.validate_transaction(&transaction, metrics.as_mut()).await {
        Ok(true) => {
            println!("Transaction validated successfully.");
            
            // Wrap transaction (marks the end of generation time)
            let wrapped_txn = transaction_handler.wrap_transaction(transaction.clone(), metrics.as_mut())?;
            
            // Execute transaction
            match execution_manager.execute_transaction(&mut transaction, metrics.as_mut()).await {
                Ok(true) => {
                    // Transaction executed successfully, compensation is due
                    println!("✅ Flight delay/cancellation compensation calculated:");
                    println!("  Flight: {}", flight_status.flight_number);
                    println!("  Status: {}", flight_status.status);
                    println!("  Delay: {} minutes", flight_status.delay_minutes);
                    println!("  Policy Type: {}", policy_type);
                    println!("  Compensation amount: {}", transaction.amount);
                    
                    // Now trigger the smart contract to process the actual payment
                    println!("\nProcessing insurance claim on SUI blockchain...");
                    match insurance_contract.process_claim(policy_id, &flight_status, policy_type).await {
                        Ok(tx_digest) => {
                            println!("✅ Insurance claim processed successfully!");
                            println!("  Transaction hash: {}", tx_digest);
                            println!("  Compensation amount: {}", transaction.amount);
                            println!("  Recipient: 0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6");
                        },
                        Err(e) => {
                            println!("❌ Error processing insurance claim on blockchain: {}", e);
                        }
                    }
                },
                Ok(false) => {
                    println!("❌ No compensation due - flight on time or minimal delay.");
                    println!("  Flight: {}", flight_status.flight_number);
                    println!("  Status: {}", flight_status.status);
                    println!("  Delay: {} minutes", flight_status.delay_minutes);
                    println!("  Policy Type: {}", policy_type);
                },
                Err(e) => {
                    println!("❌ Error processing insurance claim: {}", e);
                }
            }
        },
        Ok(false) => println!("❌ Transaction validation failed."),
        Err(e) => println!("❌ Error during validation: {}", e),
    }
    
    // 7. Calculate and display performance metrics
    let elapsed = start_time.elapsed();
    println!("\n--- Performance Summary ---");
    println!("Total processing time: {:.2} ms", elapsed.as_secs_f64() * 1000.0);
    
    if let Some(ref m) = metrics {
        if let Some(gen_time) = m.generation_time_ms() {
            println!("Generation time: {:.2} ms", gen_time);
        }
        if let Some(sui_time) = m.sui_time_ms() {
            println!("SUI time: {:.2} ms", sui_time);
        }
        if let Some(exec_time) = m.execution_time_ms() {
            println!("Execution time: {:.2} ms", exec_time);
        }
    }
    
    // Store metrics if provided
    if let (Some(m), Some(storage)) = (metrics, metrics_storage) {
        storage.add_metrics(m.clone());
    }
    
    println!("\n=== FLIGHT INSURANCE DEMO COMPLETE ===\n");
    
    Ok(())
}