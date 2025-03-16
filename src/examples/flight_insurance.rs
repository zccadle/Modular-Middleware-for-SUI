use anyhow::Result;
use std::env;
use std::sync::Arc;
use chrono::Utc;
use serde_json::json;
use rand;
use hex;

use crate::transaction::types::{Transaction, TransactionType};
use crate::execution::manager::ExecutionManager;
use crate::transaction::handler::TransactionHandler;
use crate::metrics::performance::PerformanceMetrics;
use crate::metrics::storage::MetricsStorage;
use crate::external::flight_api::{get_cached_flight_status, FlightStatus};
use crate::sui::contract::FlightInsuranceContract;
use crate::sui::verification::{VerificationManager, VerificationStatus};
use crate::sui::network::NetworkManager;
use crate::security::audit::{SecurityAuditLog, AuditSeverity};

/// Flight insurance example that demonstrates basic insurance claim processing
/// 
/// This example showcases:
/// 1. Flight status checking
/// 2. Insurance policy validation
/// 3. Claim processing
/// 4. Transaction execution
pub async fn run_flight_insurance_demo(
    transaction_handler: &TransactionHandler,
    execution_manager: &ExecutionManager,
    metrics_storage: Option<&MetricsStorage>,
    security_audit_log: &SecurityAuditLog,
    verification_manager: &VerificationManager,
) -> Result<()> {
    println!("\n=== RUNNING FLIGHT INSURANCE DEMO ===\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("flight_insurance"))
    } else {
        None
    };
    
    // Log the operation
    security_audit_log.log_validation(
        "FlightInsurance",
        "Starting flight insurance demo",
        None,
        AuditSeverity::Info
    )?;
    
    // Get API key
    let aviation_api_key = env::var("AVIATION_STACK_API_KEY")
        .unwrap_or_else(|_| "YOUR_API_KEY".to_string());
    
    // 1. Create a flight number (using a real flight)
    let flight_number = "UA2402"; // United Airlines Flight 2402
    
    println!("Checking status for flight {}...", flight_number);
    
    // 2. Fetch flight status
    let flight_status = match get_cached_flight_status(&aviation_api_key, flight_number).await {
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
                estimated_departure: Some(Utc::now() + chrono::Duration::minutes(45)),
                actual_departure: None,
                scheduled_arrival: Some(Utc::now() + chrono::Duration::hours(2)),
                estimated_arrival: Some(Utc::now() + chrono::Duration::hours(2) + chrono::Duration::minutes(45)),
                actual_arrival: None,
                delay_minutes: 45,
                raw_data: json!({"simulated": true}),
            }
        }
    };
    
    // 3. Create a contract object
    let contract = FlightInsuranceContract::new();
    
    // 4. Create a simulated insurance claim transaction
    println!("Creating an insurance claim transaction...");
    
    let mut claim_transaction = Transaction::new(
        TransactionType::Invoke,
        format!("0x{}", hex::encode(rand::random::<[u8; 32]>())),
        format!("0x{}", hex::encode(rand::random::<[u8; 16]>())),
        100, // Example amount
        format!("0x{}", hex::encode(rand::random::<[u8; 32]>())),
        1000, // Gas budget
        vec![format!("flight_insurance_claim_{}", flight_number)],
    );
    
    // 5. Execute the transaction
    println!("Executing transaction...");
    
    let _signature = transaction_handler.sign_transaction_object(&claim_transaction)?;
    execution_manager.execute_transaction(&mut claim_transaction, None).await?;
    
    // 6. Check if claim is valid (flight delayed more than threshold)
    let claim_valid = flight_status.delay_minutes >= 30;
    
    println!("Claim validation result: {}", if claim_valid { "APPROVED" } else { "DENIED" });
    
    // 7. Verify the transaction (basic verification)
    println!("Verifying transaction...");
    
    let mock_tx_digest = "abcdef1234567890";
    let verification_result = verification_manager.verify_transaction(mock_tx_digest, None).await?;
    
    println!("Transaction verification result: {:?}", verification_result);
    
    // Log completion
    security_audit_log.log_validation(
        "FlightInsurance",
        "Flight insurance demo completed successfully",
        None,
        AuditSeverity::Info
    )?;
    
    if let Some(mut m) = metrics {
        m.end_operation("flight_insurance_demo");
        
        if let Some(ms) = metrics_storage {
            ms.add_metrics(m);
        }
    }
    
    println!("\n=== FLIGHT INSURANCE DEMO COMPLETED ===\n");
    
    Ok(())
} 