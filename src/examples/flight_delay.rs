use crate::transaction::types::{Transaction, TransactionType};
use crate::execution::manager::ExecutionManager;
use crate::transaction::handler::TransactionHandler;
use crate::metrics::performance::PerformanceMetrics;
use crate::metrics::storage::MetricsStorage;
use anyhow::Result;

pub async fn run_flight_delay_demo(
    transaction_handler: &TransactionHandler,
    execution_manager: &ExecutionManager,
    metrics_storage: Option<&MetricsStorage>
) -> Result<()> {
    println!("\n--- RUNNING FLIGHT DELAY INSURANCE DEMO ---\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("flight_delay"))
    } else {
        None
    };
    
    // Create a Python script with NO external dependencies
    // Much simpler approach that will definitely work
    let python_code = r#"
# Ultra-simplified flight delay simulation
# No dependencies, no imports - just pure Python

def check_flight_status(flight_number):
    print(f"Checking status for flight {flight_number}")
    
    # Simple deterministic flight delay calculator
    # BA flights with even numbers tend to be delayed
    is_ba_flight = flight_number.startswith("BA")
    flight_num = 0
    for c in flight_number:
        if c.isdigit():
            flight_num = flight_num * 10 + int(c)
    
    # Determine if flight is delayed
    is_delayed = False
    delay_minutes = 0
    
    if is_ba_flight and flight_num % 2 == 0:
        # Even numbered BA flights have delays
        is_delayed = True
        delay_minutes = 75  # 1 hour 15 minutes
    elif is_ba_flight and flight_num % 3 == 0:
        # BA flights divisible by 3 have shorter delays
        is_delayed = True
        delay_minutes = 45  # 45 minutes
    elif not is_ba_flight and flight_num % 5 == 0:
        # Non-BA flights divisible by 5 have longer delays
        is_delayed = True
        delay_minutes = 150  # 2.5 hours
    
    return {
        "flight_number": flight_number,
        "status": "DELAYED" if is_delayed else "ON TIME",
        "delay_minutes": delay_minutes
    }

# Get flight information from parameters
flight_info = check_flight_status(params["flight_number"])
print(f"Flight status: {flight_info}")

# Calculate compensation based on delay
def calculate_compensation(delay_minutes, policy_type="standard"):
    # No compensation for short delays
    if delay_minutes < 30:
        return 0
        
    # Standard policy calculations
    if policy_type == "standard":
        if delay_minutes >= 180:  # 3+ hours
            return 300
        elif delay_minutes >= 120:  # 2+ hours
            return 200
        elif delay_minutes >= 60:  # 1+ hour
            return 100
        elif delay_minutes >= 30:  # 30+ minutes
            return 50
    
    # Premium policy (higher payouts)
    elif policy_type == "premium":
        if delay_minutes >= 180:
            return 600
        elif delay_minutes >= 120:
            return 400
        elif delay_minutes >= 60:
            return 200
        elif delay_minutes >= 30:
            return 100
            
    return 0

# Get policy type or use default
policy_type = "standard"
if "policy_type" in params:
    policy_type = params["policy_type"]

# Calculate compensation
compensation_amount = calculate_compensation(flight_info["delay_minutes"], policy_type)

# Return blockchain transaction parameters
result = {
    "flight_number": flight_info["flight_number"],
    "status": flight_info["status"],
    "delay_minutes": flight_info["delay_minutes"],
    "compensation_amount": compensation_amount,
    "policy_type": policy_type,
    "should_execute": compensation_amount > 0,
    "gas_budget": 70
}
"#;

    // Create a flight delay insurance transaction
    let mut transaction = Transaction {
        tx_type: TransactionType::Transfer,
        sender: "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(),
        receiver: "0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(),
        amount: 0, // Will be set based on compensation calculation
        gas_payment: "0xb9fd6cfa2637273ca33dd3aef2f0b0296755889a0ef7f77c9cc95953a96a6302".to_string(),
        gas_budget: 50,
        commands: vec!["TransferObjects".to_string()],
        signatures: None,
        timestamp: 0,
        script: None,
        external_query: None,
        python_code: Some(python_code.to_string()),
        python_params: Some(serde_json::json!({
            "flight_number": "BA1326", // Even number - should be delayed
            "insurance_policy_id": "POL123456",
            "customer_id": "CUST789012",
            "policy_type": "premium"
        })),
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
        language: Some("python".to_string()),
    };
    
    // Process the transaction with performance tracking
    println!("Processing flight delay insurance transaction...");
    
    // Validate transaction
    match transaction_handler.validate_transaction(&transaction, metrics.as_mut()).await {
        Ok(true) => {
            println!("Transaction validated successfully.");
            
            // Wrap transaction (marks the end of generation time)
            let _wrapped_txn = transaction_handler.wrap_transaction(transaction.clone(), metrics.as_mut())?;
            
            // Execute transaction
            match execution_manager.execute_transaction(&mut transaction, metrics.as_mut()).await {
                Ok(true) => {
                    println!("✅ Flight delay compensation processed:");
                    println!("  Flight: {}", transaction.python_params.as_ref().unwrap()["flight_number"]);
                    println!("  Policy Type: {}", transaction.python_params.as_ref().unwrap()["policy_type"]);
                    println!("  Compensation amount: {}", transaction.amount);
                },
                Ok(false) => {
                    println!("❌ No compensation due - flight on time or minimal delay.");
                },
                Err(e) => {
                    println!("❌ Error processing insurance claim: {}", e);
                }
            }
        },
        Ok(false) => println!("❌ Transaction validation failed."),
        Err(e) => println!("❌ Error during validation: {}", e),
    }
    
    // Store metrics if provided
    if let (Some(m), Some(storage)) = (metrics, metrics_storage) {
        storage.add_metrics(m);
    }
    
    println!("\n--- FLIGHT DELAY INSURANCE DEMO COMPLETE ---\n");
    
    Ok(())
}