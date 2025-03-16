use anyhow::{Result, anyhow};
use ed25519_dalek::{Keypair, Signature, Signer};
use reqwest;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use crate::transaction::types::Transaction;
use crate::SUI_TESTNET_RPC;
use crate::metrics::performance::PerformanceMetrics;
use crate::sui::verification::{VerificationManager, VerificationStatus};
use crate::security::audit::{SecurityAuditLog, AuditSeverity};

#[derive(Debug)]
pub struct TransactionHandler {
    pub client: reqwest::Client,
    pub keypair: Keypair,
    verification_manager: Option<Arc<VerificationManager>>,
    security_audit_log: Option<Arc<SecurityAuditLog>>,
}

impl TransactionHandler {
    pub fn new(
        keypair: Keypair,
        verification_manager: Option<VerificationManager>,
        security_audit_log: Option<SecurityAuditLog>
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            keypair,
            verification_manager: verification_manager.map(Arc::new),
            security_audit_log: security_audit_log.map(Arc::new),
        }
    }

    pub async fn validate_transaction(&self, tx: &Transaction, metrics: Option<&mut PerformanceMetrics>) -> Result<bool> {
        if !self.validate_address(&tx.sender) || !self.validate_address(&tx.receiver) {
            return Ok(false);
        }
        if tx.sender == tx.receiver {
            return Ok(false);
        }

        // Track SUI interaction time if metrics are provided
        if let Some(m) = metrics {
            m.sui_start_time = Some(std::time::Instant::now());
            let result = self.validate_gas_payment(&tx.sender, &tx.gas_payment).await;
            m.sui_end_time = Some(std::time::Instant::now());
            return result;
        } else {
            return self.validate_gas_payment(&tx.sender, &tx.gas_payment).await;
        }
    }

    pub fn validate_address(&self, address: &str) -> bool {
        // Expect "0x" followed by exactly 64 hex digits (total length 66).
        address.starts_with("0x")
            && address.len() == 66
            && address[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    pub async fn validate_gas_payment(&self, sender: &str, gas_payment: &str) -> Result<bool> {
        // For demo purposes, we're simplifying to always return true
        // In a real implementation, this would make an RPC call to verify gas payment
        
        // Uncomment the following line in a demo environment
        // return Ok(true);
        
        // Comment for demo purposes:
        
        let params = serde_json::json!([
            sender,{
                "filter": { "ObjectId": gas_payment }
            }             
        ]);

        let response = self.client
            .post(SUI_TESTNET_RPC)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "suix_getOwnedObjects",
                "params": params
            }))
            .send()
            .await?;
        let result = response.json::<serde_json::Value>().await?;
        println!("validate_gas_payment RPC result: {:?}", result);
        let valid = result["result"]["data"].as_array().map_or(false, |arr| !arr.is_empty());
        Ok(valid)
    
    }

    pub fn wrap_transaction(&self, tx: Transaction, mut metrics: Option<&mut PerformanceMetrics>) -> Result<Vec<u8>> {
        // Create a copy of the transaction that doesn't include incompatible types
        let mut serializable_tx = tx.clone();
        
        // Remove fields that might contain floating-point values or complex objects
        serializable_tx.python_params = None;  // This likely contains the floating-point values
        
        // Add timestamp if missing
        if serializable_tx.timestamp == 0 {
            serializable_tx.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)?
                .as_secs();
        }
        
        let result = bcs::to_bytes(&serializable_tx).map_err(|e| anyhow!("Serialization error: {}", e));
        
        // Track metrics if provided
        if let Some(m) = metrics.as_mut() {
            m.generation_end_time = Some(std::time::Instant::now());
            if let Ok(bytes) = &result {
                m.total_size_bytes = Some(bytes.len());
            }
        }
        
        result
    }

    pub fn sign_transaction(&self, tx_bytes: &[u8]) -> Result<Signature> {
        Ok(self.keypair.sign(tx_bytes))
    }

    // Sign a transaction directly
    pub fn sign_transaction_object(&self, tx: &Transaction) -> Result<Signature> {
        // First wrap the transaction
        let tx_bytes = self.wrap_transaction(tx.clone(), None)?;
        // Then sign it
        self.sign_transaction(&tx_bytes)
    }

    // Register transaction for verification
    pub fn register_for_verification(&self, tx: &Transaction, digest: &str) -> Result<()> {
        if let Some(vm) = &self.verification_manager {
            // Register transaction for verification
            let register_result = vm.register_transaction(tx, digest);
            
            // Log registration
            if let Some(audit_log) = &self.security_audit_log {
                match &register_result {
                    Ok(_) => {
                        audit_log.log_validation(
                            "TransactionHandler",
                            "Transaction validation succeeded",
                            None,
                            AuditSeverity::Info
                        )?;
                    },
                    Err(e) => {
                        audit_log.log_verification(
                            "TransactionHandler",
                            &format!("Failed to register transaction for verification: {}", e),
                            Some(digest),
                            AuditSeverity::Error
                        )?;
                    }
                }
            }
            
            // Forward the result
            register_result?;
        }
        
        Ok(())
    }

    // Verify transaction
    pub async fn verify_transaction(&self, digest: &str, mut metrics: Option<&mut PerformanceMetrics>) -> Result<VerificationStatus> {
        if let Some(vm) = &self.verification_manager {
            // Verify the transaction
            let result = vm.verify_transaction(digest, metrics.as_deref_mut()).await;
            
            // Log verification result
            if let Some(audit_log) = &self.security_audit_log {
                match &result {
                    Ok(status) => {
                        match status {
                            VerificationStatus::Verified => {
                                audit_log.log_verification(
                                    "TransactionHandler",
                                    &format!("Transaction verified successfully: {}", digest),
                                    Some(digest),
                                    AuditSeverity::Info
                                )?;
                                
                                // Update metrics if provided - using as_deref_mut pattern
                                if let Some(m) = metrics.as_deref_mut() {
                                    m.set_verification_result(true, 1);
                                }
                            },
                            VerificationStatus::Failed(reason) => {
                                audit_log.log_verification(
                                    "TransactionHandler",
                                    &format!("Transaction verification failed: {} - {}", digest, reason),
                                    Some(digest),
                                    AuditSeverity::Warning
                                )?;
                                
                                // Update metrics if provided
                                if let Some(m) = metrics.as_deref_mut() {
                                    m.set_verification_result(false, 1);
                                }
                            },
                            VerificationStatus::Pending => {
                                audit_log.log_verification(
                                    "TransactionHandler",
                                    &format!("Transaction verification pending: {}", digest),
                                    Some(digest),
                                    AuditSeverity::Info
                                )?;
                            },
                            VerificationStatus::Unverifiable(reason) => {
                                audit_log.log_verification(
                                    "TransactionHandler",
                                    &format!("Transaction unverifiable: {} - {}", digest, reason),
                                    Some(digest),
                                    AuditSeverity::Error
                                )?;
                                
                                // Update metrics if provided
                                if let Some(m) = metrics.as_deref_mut() {
                                    m.set_verification_result(false, 1);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        audit_log.log_verification(
                            "TransactionHandler",
                            &format!("Transaction verification error: {}", e),
                            Some(digest),
                            AuditSeverity::Error
                        )?;
                    }
                }
            }
            
            return result;
        }
        
        // If verification manager is not available, return pending status
        Ok(VerificationStatus::Pending)
    }
}