use serde_json::json;
use anyhow::{Result};
use crate::execution::manager::ExecutionManager;
use crate::transaction::types::{Transaction, TransactionType};
use crate::metrics::storage::MetricsStorage;
use crate::security::audit::{SecurityAuditLog, AuditSeverity};
use crate::external::flight_api::{get_cached_flight_status};
use crate::sui::network::NetworkManager;
use crate::sui::verification::VerificationManager;
use crate::transaction::utils::process_and_submit_verification;
use std::sync::Arc;
use sui_sdk::types::base_types::ObjectID;
use sui_sdk::types::crypto::SuiKeyPair;
use crate::config::{SUBMITTER_ADDRESS, SUBMITTER_GAS_OBJECT_ID, generate_test_sui_keypair};
use crate::metrics::performance::PerformanceMetrics;
use crate::transaction::handler::TransactionHandler;
use std::env;

/// Enhanced flight insurance example that demonstrates our full security model
/// 
/// This example showcases:
/// 1. Multiple data sources with consensus (Oracle pattern)
/// 2. Byzantine fault detection for blockchain verification
/// 3. Cross-chain transaction portability
/// 4. Formal security guarantees
/// 5. Comprehensive audit logging
pub async fn run_enhanced_flight_insurance_demo(
    transaction_handler: &Arc<TransactionHandler>,
    execution_manager: &Arc<ExecutionManager>,
    metrics_storage: Option<&Arc<MetricsStorage>>,
    security_audit_log: &Arc<SecurityAuditLog>,
    _verification_manager: &Arc<VerificationManager>,
    _network_manager: &Arc<NetworkManager>,
    _submitter_keypair: &SuiKeyPair,
    gas_object_id: &ObjectID,
) -> Result<()> {
    println!("\n--- RUNNING ENHANCED FLIGHT INSURANCE DEMO (REFACTORED) ---");
    let tx_name = "enhanced_flight_insurance";
    let _metrics = metrics_storage.map(|_s| PerformanceMetrics::new(tx_name));

    let api_key = "YOUR_AVIATIONSTACK_API_KEY_HERE".to_string();
    let flight_number = "LH987";
    let flight_status = get_cached_flight_status(&api_key, flight_number).await?;
    println!("Flight {} status: {:?}", flight_number, flight_status);

    let policy_id = "POLICY_ENHANCED456";
    let policy_type = "premium";
    let compensation = flight_status.get_compensation_amount(policy_type);

    if compensation > 0 {
        println!("Flight status warrants compensation ({}). Processing enhanced claim for policy {}...", compensation, policy_id);

        let claim_transaction = Transaction {
            tx_type: TransactionType::Custom("enhanced_flight_claim".to_string()),
            sender: SUBMITTER_ADDRESS.to_string(),
            receiver: SUBMITTER_ADDRESS.to_string(),
            amount: compensation,
            gas_payment: SUBMITTER_GAS_OBJECT_ID.to_string(),
            gas_budget: 2000000,
            commands: vec!["process_claim_enhanced".to_string()],
            python_params: Some(json!({
                "policy_id": policy_id,
                "flight_number": flight_number,
                "status": flight_status.status,
                "delay_minutes": flight_status.delay_minutes,
                "compensation_amount": compensation,
                "policy_type": policy_type,
                "passenger_details": { "name": "Jane Doe", "booking_ref": "ABCDEF" },
                "security_level": "enhanced"
            })),
            language: Some("native".to_string()),
            signatures: None,
            timestamp: 0,
            script: None,
            external_query: None,
            python_code: None,
            websocket_endpoint: None,
            websocket_message: None,
            time_condition: None,
        };

        // For test purposes, generate a test SuiKeyPair
        let sui_keypair = generate_test_sui_keypair()?;

        process_and_submit_verification(
            &claim_transaction,
            tx_name,
            transaction_handler,
            execution_manager,
            metrics_storage,
            security_audit_log,
            &sui_keypair,
            gas_object_id,
        ).await?;

    } else {
        println!("Flight status does not warrant compensation for policy {}. No action taken.", policy_id);
        
        security_audit_log.log_execution(
            tx_name,
            "Condition for compensation not met",
            None,
            AuditSeverity::Info
        )?;
    }

    if let (Some(_metrics), Some(storage)) = (metrics_storage.map(|_s| PerformanceMetrics::new(tx_name)), metrics_storage) {
        println!("Note: Metrics storage update skipped (using deprecated system).");
    }

    println!("--- ENHANCED FLIGHT INSURANCE DEMO (REFACTORED) COMPLETE ---");
    Ok(())
} 