// use std::sync::Arc;
// use serde_json::json;
use crate::transaction::types::{Transaction, TransactionType};
use crate::transaction::handler::TransactionHandler;
use crate::execution::manager::ExecutionManager;
use crate::external::flight_api::{get_cached_flight_status};
use crate::metrics::storage::MetricsStorage;
use crate::metrics::performance::PerformanceMetrics;
// use crate::sui::verification::VerificationStatus;
use anyhow::{Result};
// use rand::Rng;
// use tokio::time;
use std::env; // Needed for API key
use serde_json::json;
use std::sync::Arc;
use crate::config::*; // Import config
use crate::security::audit::{AuditSeverity, SecurityAuditLog};
use sui_sdk::types::base_types::{ObjectID};
use crate::transaction::utils::process_and_submit_verification;
use crate::config::{self, SUBMITTER_ADDRESS, SUBMITTER_GAS_OBJECT_ID};
use sui_sdk::types::crypto::SuiKeyPair;

pub async fn run_flight_delay_demo(
    transaction_handler: &Arc<TransactionHandler>,
    execution_manager: &Arc<ExecutionManager>,
    metrics_storage: Option<&Arc<MetricsStorage>>,
    security_audit_log: &Arc<SecurityAuditLog>,
    submitter_keypair: &SuiKeyPair,
    gas_object_id: &ObjectID,
) -> Result<()> {
    println!("\n--- RUNNING FLIGHT DELAY DEMO ---");
    let tx_name = "flight_delay_demo";
    let _metrics = metrics_storage.map(|_s| PerformanceMetrics::new(tx_name));

    // Replace environment variable lookup with hardcoded API key
    let api_key = "YOUR_AVIATIONSTACK_API_KEY_HERE".to_string();
    let flight_number = "BA123";
    let flight_status = get_cached_flight_status(&api_key, flight_number).await?;
    println!("Flight {} status: {:?}", flight_number, flight_status);

    let delay_threshold = 60; 
    if flight_status.delay_minutes >= delay_threshold {
        println!("Flight {} delayed by {} minutes. Preparing compensation transaction...",
                 flight_number, flight_status.delay_minutes);

        let policy_type = "standard";
        let transaction = Transaction {
            tx_type: TransactionType::Custom("flight_delay_claim".to_string()),
            sender: SUBMITTER_ADDRESS.to_string(), // Use config constant
            receiver: SUBMITTER_ADDRESS.to_string(), // Use config constant (send to self for demo)
            amount: 0, 
            gas_payment: SUBMITTER_GAS_OBJECT_ID.to_string(), // Use config constant
            gas_budget: 2000000, // Increased from 10000 to 2000000 to meet minimum requirement
            commands: vec!["process_delay_claim".to_string()],
            python_params: Some(json!({
                "flight_number": flight_status.flight_number,
                "delay_minutes": flight_status.delay_minutes,
                "policy_type": policy_type
            })),
            language: Some("native".to_string()), // Assume native logic calculates payout based on params
            // ... other fields null/None ...
            signatures: None, timestamp: 0, script: None, external_query: None,
            python_code: None, websocket_endpoint: None, websocket_message: None,
            time_condition: None,
        };

        // Call the main processing and submission function
        process_and_submit_verification(
            &transaction,
            tx_name,
            transaction_handler,
            execution_manager,
            metrics_storage,
            security_audit_log,
            submitter_keypair,
            gas_object_id,
        ).await?;

    } else {
        println!("Flight {} not significantly delayed. No action taken.", flight_number);
    }

    if let (Some(m), Some(storage)) = (metrics_storage.map(|_s| PerformanceMetrics::new(tx_name)), metrics_storage) {
        storage.add_metrics(m);
    }

    println!("--- FLIGHT DELAY DEMO COMPLETE ---");
    Ok(())
}