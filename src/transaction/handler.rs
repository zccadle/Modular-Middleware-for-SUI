//! Handles middleware transaction processing, validation, signature collection, and L1 submission.

// Local Crate Imports
use super::types::{Transaction as MiddlewareTransaction, QuorumError, SignatureBytes, VerificationInput};
use crate::config; // Import top-level config module
use crate::metrics::performance::PerformanceMetrics;
use crate::quorum::simulation::QuorumSimulation;
use crate::security::audit::{AuditEvent, AuditEventType, AuditSeverity, SecurityAuditLog};
use crate::sui::verification::VerificationManager;

// External Crate Imports
use anyhow::{anyhow, Context, Result};
use bcs;
use shared_crypto::intent::{Intent, IntentMessage};
use std::{
    str::FromStr,
    sync::Arc,
    time::Instant,
};
use sui_sdk::{
    rpc_types::{ 
        SuiExecutionStatus,
        SuiMoveStruct,
        SuiMoveValue,
        SuiObjectDataOptions,
        SuiObjectResponseQuery, 
        SuiParsedData,
        SuiTransactionBlockEffectsAPI,
        SuiTransactionBlockResponseOptions,
    },
    types::{
        base_types::{ObjectID, SuiAddress},
        crypto::{Signature, SuiKeyPair},
        object::Owner,
        transaction::{CallArg, ObjectArg, Transaction, TransactionData},
        Identifier,
    },
    SuiClient,
};
use sui_types::{
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    quorum_driver_types::ExecuteTransactionRequestType,
};

/// Handles the lifecycle of middleware transactions.
#[derive(Clone)]
pub struct TransactionHandler {
    pub sui_client: Arc<SuiClient>,
    pub node_keypair: Arc<SuiKeyPair>,
    pub verification_manager: Option<Arc<VerificationManager>>,
    pub security_audit_log: Option<Arc<SecurityAuditLog>>,
    pub quorum_simulation: Arc<QuorumSimulation>,
}

// Implement Clone manually IF needed, otherwise remove if Arc makes it unnecessary
// impl Clone for TransactionHandler {
//     fn clone(&self) -> Self {
//         Self {
//             sui_client: self.sui_client.clone(),
//             node_keypair: self.node_keypair.clone(), // Arc clone is cheap
//             verification_manager: self.verification_manager.clone(),
//             security_audit_log: self.security_audit_log.clone(),
//             quorum_simulation: self.quorum_simulation.clone(),
//         }
//     }
// }

impl TransactionHandler {
    /// Creates a new `TransactionHandler`.
    pub async fn new(
        node_keypair: SuiKeyPair, // Take ownership
        verification_manager: Option<VerificationManager>,
        security_audit_log: Option<Arc<SecurityAuditLog>>,
        _byzantine_detector: Option<Arc<crate::sui::byzantine::ByzantineDetector>>, // Mark unused
        quorum_simulation: Arc<QuorumSimulation>,
        sui_client: Arc<SuiClient>,
    ) -> Result<Self> { // Correct Result usage
        let node_count = quorum_simulation.get_public_key_bytes().len();
        let quorum_threshold = quorum_simulation.get_threshold();

        println!(
            "Initializing TransactionHandler with {} nodes and quorum threshold of {}",
            node_count,
            quorum_threshold
        );

        Ok(Self {
            sui_client,
            node_keypair: Arc::new(node_keypair), // Create Arc here
            verification_manager: verification_manager.map(Arc::new),
            security_audit_log,
            quorum_simulation,
        })
    }

    /// Validates the basic structure and addresses of a transaction.
    pub async fn validate_transaction(
        &self,
        tx: &MiddlewareTransaction,
        metrics: Option<&mut PerformanceMetrics>,
    ) -> Result<bool> { // Correct Result usage
        let start = Instant::now();
        
        if !Self::is_valid_sui_address(&tx.sender) {
            self.log_audit(AuditSeverity::Warning, &format!("Invalid sender address: {}", tx.sender), None)?;
            return Ok(false);
        }
        
        if !Self::is_valid_sui_address(&tx.receiver) {
            self.log_audit(AuditSeverity::Warning, &format!("Invalid receiver address: {}", tx.receiver), None)?;
            return Ok(false);
        }
        
        if !self.validate_gas_object_ownership(&tx.sender, &tx.gas_payment).await? {
            self.log_audit(AuditSeverity::Warning, &format!("Gas object {} validation failed for sender {}", tx.gas_payment, tx.sender), None)?;
            return Ok(false);
        }
        
        if let Some(m) = metrics {
            if let Some(start_time) = m.generation_start_time {
                 // Use deprecated method if PerformanceMetrics is kept
                 m.set_timing("validation_time", start_time.elapsed().unwrap_or_default());
        }
        }

        Ok(true)
    }

    /// Checks if a string is a potentially valid Sui address format (0x... length 66).
    fn is_valid_sui_address(address: &str) -> bool {
        address.starts_with("0x")
            && address.len() == 66
            && address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Validates that the specified gas object exists and is owned by the expected owner.
    async fn validate_gas_object_ownership(&self, owner_address_str: &str, gas_object_id_str: &str) -> Result<bool> { // Correct Result
        let owner_address = SuiAddress::from_str(owner_address_str).context("Invalid owner address format")?;
        let gas_object_id = ObjectID::from_str(gas_object_id_str).context("Invalid gas object ID format")?;

        match self.sui_client.read_api().get_object_with_options(
            gas_object_id,
            SuiObjectDataOptions::new().with_owner(),
        ).await {
            Ok(obj_resp) => {
                if let Some(data) = obj_resp.data {
                    match data.owner {
                        Some(Owner::AddressOwner(addr)) if addr == owner_address => Ok(true),
                        Some(owner) => {
                            println!(
                                "WARN: Gas object {} owner ({:?}) does not match expected owner {}",
                                gas_object_id, owner, owner_address
                            );
                            Ok(false)
                        }
                        None => {
                             println!("WARN: Gas object {} has no owner info.", gas_object_id);
                             Ok(false)
                        }
                    }
                } else {
                    println!("WARN: Gas object {} not found.", gas_object_id);
                    Ok(false)
                }
            }
            Err(e) => {
                eprintln!("ERROR: Failed to fetch gas object {}: {}", gas_object_id, e);
                Ok(false)
            }
        }
    }

    /// Collects signatures from the simulated quorum for a given payload.
    pub async fn collect_quorum_signatures(
        &self,
        attestation_payload: &[u8],
    ) -> Result<Vec<SignatureBytes>, QuorumError> {
        let quorum_threshold = self.quorum_simulation.get_threshold();
        let node_count = self.quorum_simulation.keypairs.len();

        if node_count < quorum_threshold || quorum_threshold == 0 {
            return Err(QuorumError::InsufficientSignatures {
                got: node_count,
                needed: quorum_threshold,
            });
        }

        let signatures_with_validity = self.quorum_simulation.request_signatures(attestation_payload.to_vec()).await
            .map_err(|e| QuorumError::SigningError(format!("Simulation signing failed: {}", e)))?;

        if signatures_with_validity.len() < quorum_threshold {
            return Err(QuorumError::InsufficientSignatures {
                got: signatures_with_validity.len(),
                needed: quorum_threshold,
            });
        }

        let quorum_signatures: Vec<Vec<u8>> = signatures_with_validity
            .into_iter()
            .take(quorum_threshold)
            .map(|(bytes, _is_valid)| bytes)
            .collect();

        Ok(quorum_signatures)
    }

    /// Submits the attestation and signatures to the on-chain verification contract.
    pub async fn submit_for_onchain_verification(
        &self,
        verification_input: VerificationInput,
        l1_gas_budget: u64,
    ) -> Result<String> { // Correct Result
        println!("Submitting transaction for on-chain verification...");
        
        let submitter_keypair = &self.node_keypair;
        let submitter_address = SuiAddress::from(&submitter_keypair.public());
        println!("  Submitter Address: {}", submitter_address);
        
        let gas_object_ref = match self.select_best_gas_object_ref(submitter_address).await {
             Ok(obj_ref) => obj_ref,
             Err(e) => {
                 self.log_audit(AuditSeverity::Error, &format!("Failed to find usable gas object for {}: {}", submitter_address, e), None)?;
                 return Err(e.context("Failed to select gas object for L1 submission"));
             }
         };
        println!("  Using Gas Object: {} (Version: {})", gas_object_ref.0, gas_object_ref.1);

        let reference_gas_price = self.sui_client.read_api().get_reference_gas_price().await
            .context("Failed to get reference gas price")?;
        let package_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_PACKAGE_ID)
            .context("Invalid package ID in config")?;
        let module_name = Identifier::from_str(config::VERIFICATION_CONTRACT_MODULE)
            .context("Invalid module name in config")?;
        let function_name = Identifier::from_str(config::VERIFICATION_CONTRACT_FUNCTION)
            .context("Invalid function name in config")?;
        let config_obj_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_CONFIG_OBJECT_ID)
            .context("Invalid config object ID in config")?;

        let config_obj_resp = self.sui_client.read_api().get_object_with_options(
            config_obj_id,
            SuiObjectDataOptions::new().with_owner()
        ).await.context(format!("Failed to fetch config object {}", config_obj_id))?;

        let initial_shared_version = match config_obj_resp.data {
            Some(ref data) => match data.owner {
                Some(Owner::Shared { initial_shared_version }) => initial_shared_version,
                _ => return Err(anyhow!("Config object {} is not a shared object", config_obj_id)),
            },
            None => return Err(anyhow!("Config object {} not found", config_obj_id)),
        };
        println!("  Using Config Object: {} (InitialSharedVersion: {})", config_obj_id, initial_shared_version);
    
        let config_obj_arg = CallArg::Object(ObjectArg::SharedObject {
                id: config_obj_id,
                initial_shared_version,
                mutable: true,
        });
        let attestation_payload_arg = CallArg::Pure(verification_input.attestation_payload);
        let encoded_signatures = bcs::to_bytes(&verification_input.quorum_signatures)
            .context("Failed to BCS encode signatures")?;
        let signatures_arg = CallArg::Pure(encoded_signatures);
        
        let pt = {
            let mut builder = ProgrammableTransactionBuilder::new();
            builder.move_call(
            package_id,
            module_name,
            function_name,
                vec![],
                vec![config_obj_arg, attestation_payload_arg, signatures_arg],
            )?;
            builder.finish()
        };

        let tx_data = TransactionData::new_programmable(
            submitter_address,
            vec![gas_object_ref],
            pt,
            l1_gas_budget,
            reference_gas_price,
        );
    
        let intent_msg = IntentMessage::new(Intent::sui_transaction(), tx_data.clone());
        // Use as_ref() to pass &SuiKeyPair which implements Signer
        let signature = Signature::new_secure(&intent_msg, self.node_keypair.as_ref());
    
        println!("Submitting verification transaction to Sui network...");
        let options = SuiTransactionBlockResponseOptions::new().with_effects().with_object_changes();

        let response = self.sui_client
            .quorum_driver_api()
            .execute_transaction_block(
                Transaction::from_data(tx_data, vec![signature.into()]),
                options,
                Some(ExecuteTransactionRequestType::WaitForLocalExecution),
            )
            .await
            .context("Failed to execute L1 verification transaction")?;
    
        println!("L1 Transaction executed.");
        let digest_str = response.digest.to_string();
        println!("  Digest: {}", digest_str);
        
        let effects = response.effects.context("Missing effects in L1 response")?;
        println!("  Status: {:?}", effects.status());
    
        match effects.status() {
            SuiExecutionStatus::Success => {
                self.log_audit(
                    AuditSeverity::Info,
                    "L1 verification transaction executed successfully.",
                    Some(&digest_str),
                )?;
                println!("L1 verification successful based on execution status.");
        Ok(digest_str)
    }
                SuiExecutionStatus::Failure { error } => {
                let error_msg = format!("L1 verification transaction failed: {}", error);
                eprintln!("ERROR: {}", error_msg);
                self.log_audit(AuditSeverity::Error, &error_msg, Some(&digest_str))?;
                Err(anyhow!(error_msg))
            }
        }
    }

    /// Selects a suitable gas object owned by the address.
    async fn select_best_gas_object_ref(
        &self,
        owner: SuiAddress,
    ) -> Result<sui_sdk::types::base_types::ObjectRef> { // Correct Result
        let gas_objects_resp = self.sui_client.read_api().get_owned_objects(
                owner, 
            Some(SuiObjectResponseQuery::new_with_options(
                SuiObjectDataOptions::new().with_type().with_owner().with_content(),
            )),
            None,
            None,
        ).await.context("Failed to fetch owned objects for gas selection")?;

        let gas_objects = gas_objects_resp.data;
        if gas_objects.is_empty() {
            return Err(anyhow!("No objects found for address {}", owner));
        }
        
        for obj_resp in &gas_objects {
            if let Some(data) = &obj_resp.data {
                if data.is_gas_coin() {
                    if let Some(SuiParsedData::MoveObject(move_obj)) = &data.content {
                         if let SuiMoveStruct::WithFields(fields) = &move_obj.fields {
                            if let Some(SuiMoveValue::Number(bal)) = fields.get("balance") {
                                if (*bal as u64) > 0 {
                                    println!("Selected SUI gas coin: {} (Balance: {})", data.object_id, bal);
                            return Ok(data.object_ref());
                        }
                    }
                }
            }
                     println!("Selected potential SUI gas coin (balance check failed/skipped): {}", data.object_id);
                     return Ok(data.object_ref());
                }
            }
        }

        if let Some(first_obj_resp) = gas_objects.first() {
            if let Some(data) = &first_obj_resp.data {
                println!(
                    "WARN: No SUI Coin found for gas. Falling back to first owned object: {}",
                    data.object_id
                );
                return Ok(data.object_ref());
            }
        }
        
        Err(anyhow!("No suitable gas object found for address {}", owner))
    }

     /// Helper to log audit events if the logger is configured.
     fn log_audit(&self, severity: AuditSeverity, message: &str, tx_id: Option<&str>) -> Result<()> { // Correct Result
        if let Some(log) = &self.security_audit_log {
            // Use TransactionExecution as a general handler type
            let event = AuditEvent::new(AuditEventType::TransactionExecution, severity, "TransactionHandler", message);
            let event_with_id = if let Some(id) = tx_id {
                event.with_transaction_id(id)
        } else {
                event
            };
            log.log_event(event_with_id)?;
        }
        Ok(())
    }
}

// Removed placeholder AuditEventType impl