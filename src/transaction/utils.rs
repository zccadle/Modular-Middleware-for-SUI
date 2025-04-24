//! Utility functions for transaction processing orchestration.

use super::handler::TransactionHandler;
use super::types::{Transaction, VerificationInput}; // Removed VerifiableTransactionData
use crate::execution::manager::ExecutionManager;
use crate::metrics::storage::MetricsStorage;
use crate::metrics::performance::PerformanceMetrics; // Keep if used in metrics.as_mut()
use crate::security::audit::{AuditSeverity, SecurityAuditLog};
use crate::sui::verification::VerificationStatus;
use anyhow::{anyhow, Context, Result};
use std::sync::Arc;
use std::time::Duration;
use sui_sdk::types::{
    base_types::{ObjectID},
    crypto::SuiKeyPair,
};
use tokio; // Added import for sleep

/// Processes a transaction through the middleware and submits it for L1 verification.
///
/// Orchestrates the flow:
/// 1. Initial validation (via `TransactionHandler`).
/// 2. Off-chain processing and attestation preparation (via `ExecutionManager`).
/// 3. Quorum signature collection (via `TransactionHandler`).
/// 4. L1 submission for verification (via `TransactionHandler`).
/// 5. (Optional) L1 confirmation check.
///
/// # Arguments
/// * `tx`: The middleware transaction request.
/// * `tx_name`: A descriptive name for logging.
/// * `transaction_handler`: Shared reference to the transaction handler.
/// * `execution_manager`: Shared reference to the execution manager.
/// * `metrics_storage`: Optional shared storage for performance metrics (used for deprecated metrics).
/// * `security_audit_log`: Shared security audit logger.
/// * `submitter_keypair`: Keypair used to sign the L1 transaction.
/// * `gas_object_id`: ObjectID of the gas coin for the L1 transaction.
///
/// # Returns
/// `Ok(())` on successful processing and submission, `Err` otherwise.
pub async fn process_and_submit_verification(
    tx: &Transaction,
    tx_name: &str,
    transaction_handler: &Arc<TransactionHandler>,
    execution_manager: &Arc<ExecutionManager>,
    metrics_storage: Option<&Arc<MetricsStorage>>,
    security_audit_log: &Arc<SecurityAuditLog>,
    submitter_keypair: &SuiKeyPair, // Now passed directly
    gas_object_id: &ObjectID, // Now passed directly
) -> Result<()> {
    println!(
        "\n--- Running: {} ---",
        tx_name.to_uppercase()
    );

    // Reference to deprecated metrics struct
    let mut metrics = metrics_storage.map(|_storage| PerformanceMetrics::new(tx_name));

    // 1. Initial Validation
    if !transaction_handler.validate_transaction(tx, metrics.as_mut()).await? {
        // Validation failure already logged by handler
        println!("❌ Initial validation failed for {}", tx_name);
        return Err(anyhow!("Initial validation failed"));
    }
    println!("✅ Initial validation passed.");

    // 2. Process transaction off-chain & prepare attestation
    println!("Processing transaction off-chain...");
    // Assuming ExecutionManager now handles preparing the input needed for signing/verification
    let verification_input_opt = execution_manager.prepare_verification_input(tx).await; 

    let verification_input = match verification_input_opt {
        Ok(Some(input)) => {
            println!("✅ Middleware processing complete, verification input prepared.");
            input // Assuming this returns the VerificationInput struct directly
        }
        Ok(None) => {
            println!("✅ Middleware processing skipped (e.g., condition not met). No L1 verification needed.");
            return Ok(());
        }
        Err(e) => {
             println!("❌ Error during off-chain processing: {}", e);
             security_audit_log.log_execution(
                 tx_name,
                 &format!("Off-chain processing error: {}", e),
                 None,
                 AuditSeverity::Error,
             )?;
             // Use context for better error reporting
             return Err(e.context("Off-chain processing failed"));
        }
    };

    // 3. Collect Quorum Signatures
    println!(
        "Collecting {} signatures for attestation payload ({} bytes)...",
        transaction_handler.quorum_simulation.get_threshold(),
        verification_input.attestation_payload.len()
    );
    let quorum_signatures = match transaction_handler
        .collect_quorum_signatures(&verification_input.attestation_payload)
        .await
    {
        Ok(signatures) => {
            println!("✅ Successfully collected {} signatures.", signatures.len());
            signatures
        }
        Err(e) => {
            println!("❌ Failed to collect quorum signatures: {}", e);
            security_audit_log.log_network(
                tx_name,
                &format!("Failed to collect quorum signatures: {}", e),
                None,
                AuditSeverity::Error,
            )?;
            // Use context for better error reporting
            return Err(anyhow!(e).context("Quorum signature collection failed"));
        }
    };

    // Update VerificationInput with collected signatures
    let final_verification_input = VerificationInput {
        attestation_payload: verification_input.attestation_payload,
        quorum_signatures,
    };

    // 4. Submit for On-Chain Verification
    println!("Submitting for L1 verification...");
    let submission_result: Result<String> = transaction_handler // Explicit type for result
        .submit_for_onchain_verification(final_verification_input, tx.gas_budget)
        .await;

    match submission_result {
        Ok(l1_digest) => {
            println!(
                "✅ L1 verification transaction submitted successfully. Digest: {}",
                l1_digest
            );
            security_audit_log.log_network(
                tx_name,
                &format!("Submitted verification tx to SUI. L1 Digest: {}", l1_digest),
                None,
                AuditSeverity::Info,
            )?;

            // 5. Optional: Check L1 Confirmation
            println!("Waiting briefly before checking L1 status for digest: {}", l1_digest);
            tokio::time::sleep(Duration::from_secs(5)).await;

            if let Some(vm) = &transaction_handler.verification_manager {
                 // Pass metrics.as_mut() which correctly gives Option<&mut PerformanceMetrics>
                 match vm.verify_transaction(&l1_digest, metrics.as_mut()).await {
                     Ok(status) => {
                         println!("✅ L1 confirmation status for {}: {:?}", l1_digest, status);
                         security_audit_log.log_verification(
                             tx_name,
                             &format!("L1 confirmation status: {:?}", status),
                             Some(&l1_digest),
                             AuditSeverity::Info,
                         )?;
                         if status != VerificationStatus::Verified {
                             println!("WARN: L1 transaction {} not fully verified yet (status: {:?})", l1_digest, status);
                         }
                     }
                     Err(e) => {
                         println!(
                             "❌ Error checking L1 confirmation for {}: {}",
                             l1_digest, e
                         );
                         security_audit_log.log_verification(
                             tx_name,
                             &format!("Error checking L1 confirmation: {}", e),
                             Some(&l1_digest),
                             AuditSeverity::Error,
                         )?;
                     }
                 }
            } else {
                println!("Skipping L1 confirmation check (Verification Manager not available).");
            }

            // Handling deprecated metrics
            if let Some(mut m) = metrics {
                println!("Note: Old PerformanceMetrics system is deprecated.");
                m.generation_end_time = Some(std::time::SystemTime::now());
                // if let Some(storage) = metrics_storage {
                //     storage.add_metrics(m); // Deprecated call
                // }
            }

        }
        Err(e) => {
            println!("❌ L1 verification transaction submission failed: {:#}", e);
            security_audit_log.log_network(
                tx_name,
                &format!("L1 submission failed: {}", e),
                None,
                AuditSeverity::Error,
            )?;
            return Err(e.context("L1 submission failed"));
        }
    }

    println!("\n--- {} Demo Flow Complete ---", tx_name.to_uppercase());
    Ok(())
}