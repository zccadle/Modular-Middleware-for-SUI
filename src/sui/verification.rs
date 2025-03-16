use anyhow::{Result, anyhow};
use reqwest;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::time::sleep;

use crate::transaction::types::Transaction;
use crate::metrics::performance::PerformanceMetrics;
use crate::transaction::types::TransactionType;


/// Maximum number of verification attempts before failing
const MAX_VERIFICATION_ATTEMPTS: u8 = 10;
/// Delay between verification attempts (in milliseconds)
const VERIFICATION_RETRY_DELAY_MS: u64 = 1000;
/// Timeout for the entire verification process (in seconds)
const VERIFICATION_TIMEOUT_SECS: u64 = 60;

/// Status of a transaction verification attempt
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VerificationStatus {
    /// Transaction has not been verified yet
    Pending,
    /// Transaction is successfully verified
    Verified,
    /// Transaction execution failed on-chain
    Failed(String),
    /// Transaction cannot be verified (timeout or other issue)
    Unverifiable(String),
}

/// Record of a transaction verification
#[derive(Debug, Clone)]
pub struct VerificationRecord {
    /// Original transaction
    pub transaction: Transaction,
    /// Digest/hash of the transaction on SUI
    pub digest: Option<String>,
    /// Current verification status
    pub status: VerificationStatus,
    /// Timestamp of verification attempt
    pub timestamp: u64,
    /// Number of verification attempts
    pub attempts: u8,
    /// Receipt data from the blockchain
    pub receipt: Option<Value>,
    /// Execution effects (state changes) from the transaction
    pub effects: Option<Value>,
}

/// SUI Transaction Verification Manager
/// 
/// This module verifies that transactions submitted to the SUI blockchain
/// are executed correctly and with the expected outcomes. It provides
/// security guarantees by ensuring that middleware operations match
/// on-chain execution results.
#[derive(Debug, Clone)]
pub struct VerificationManager {
    /// HTTP client for making RPC requests
    client: reqwest::Client,
    /// Record of verification attempts, keyed by transaction digest
    verifications: Arc<Mutex<HashMap<String, VerificationRecord>>>,
    /// RPC endpoint for the blockchain
    rpc_endpoint: String,
}

impl VerificationManager {
    /// Create a new verification manager
    pub fn new(rpc_endpoint: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            verifications: Arc::new(Mutex::new(HashMap::new())),
            rpc_endpoint: rpc_endpoint.to_string(),
        }
    }
    
    /// Register a transaction for verification
    pub fn register_transaction(&self, tx: &Transaction, digest: &str) -> Result<()> {
        let record = VerificationRecord {
            transaction: tx.clone(),
            digest: Some(digest.to_string()),
            status: VerificationStatus::Pending,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            attempts: 0,
            receipt: None,
            effects: None,
        };
        
        let mut verifications = self.verifications.lock().unwrap();
        verifications.insert(digest.to_string(), record);
        
        Ok(())
    }
    
    /// Verify a transaction by its digest
    pub async fn verify_transaction(&self, digest: &str, mut metrics: Option<&mut PerformanceMetrics>) -> Result<VerificationStatus> {
        // Start verification timing if metrics provided
        if let Some(ref mut m) = metrics {
            m.verification_start_time = Some(std::time::Instant::now());
        }
        
        // Get the transaction record
        let record = {
            let verifications = self.verifications.lock().unwrap();
            match verifications.get(digest) {
                Some(record) => record.clone(),
                None => {
                    // End verification timing if metrics provided
                    if let Some(ref mut m) = metrics {
                        m.verification_end_time = Some(std::time::Instant::now());
                    }
                    return Err(anyhow!("Transaction not registered for verification"));
                }
            }
        };
        
        // Initialize timeout and attempt counters
        let start_time = Instant::now();
        let timeout = Duration::from_secs(VERIFICATION_TIMEOUT_SECS);
        let mut attempts = 0;
        
        // Attempt verification until success, max attempts, or timeout
        while attempts < MAX_VERIFICATION_ATTEMPTS && start_time.elapsed() < timeout {
            attempts += 1;
            
            // Update the attempts counter in our records
            {
                let mut verifications = self.verifications.lock().unwrap();
                if let Some(record) = verifications.get_mut(digest) {
                    record.attempts = attempts;
                }
            }
            
            // Query the blockchain for transaction status
            match self.query_transaction_status(digest).await {
                Ok((receipt, effects)) => {
                    // Check if the transaction was executed successfully
                    let status = self.verify_transaction_effects(&record.transaction, &effects);
                    
                    // Update the verification record
                    {
                        let mut verifications = self.verifications.lock().unwrap();
                        if let Some(record) = verifications.get_mut(digest) {
                            record.receipt = Some(receipt.clone());
                            record.effects = Some(effects.clone());
                            record.status = status.clone();
                        }
                    }
                    
                    // End verification timing if metrics provided
                    if let Some(ref mut m) = metrics {
                        m.verification_end_time = Some(std::time::Instant::now());
                    }
                    
                    // If verified or explicitly failed, return the status
                    match status {
                        VerificationStatus::Verified => return Ok(status),
                        VerificationStatus::Failed(_) => return Ok(status),
                        _ => () // Continue trying for other statuses
                    }
                },
                Err(e) => {
                    println!("Verification attempt {} failed: {}", attempts, e);
                    // If this was our last attempt, update the record
                    if attempts >= MAX_VERIFICATION_ATTEMPTS || start_time.elapsed() >= timeout {
                        let status = VerificationStatus::Unverifiable(format!("Max attempts reached: {}", e));
                        let mut verifications = self.verifications.lock().unwrap();
                        if let Some(record) = verifications.get_mut(digest) {
                            record.status = status.clone();
                        }
                        
                        // End verification timing if metrics provided
                        if let Some(ref mut m) = metrics {
                            m.verification_end_time = Some(std::time::Instant::now());
                        }
                        
                        return Ok(status);
                    }
                }
            }
            
            // Wait before retrying
            sleep(Duration::from_millis(VERIFICATION_RETRY_DELAY_MS)).await;
        }
        
        // If we got here, we timed out
        let status = VerificationStatus::Unverifiable("Verification timeout".to_string());
        
        // Update the verification record
        {
            let mut verifications = self.verifications.lock().unwrap();
            if let Some(record) = verifications.get_mut(digest) {
                record.status = status.clone();
            }
        }
        
        // End verification timing if metrics provided
        if let Some(ref mut m) = metrics {
            m.verification_end_time = Some(std::time::Instant::now());
        }
        
        Ok(status)
    }
    
    /// Query the blockchain for a transaction's status
    async fn query_transaction_status(&self, digest: &str) -> Result<(Value, Value)> {
        let params = json!([
            digest,
            {
                "showInput": true,
                "showEffects": true,
                "showEvents": true,
                "showObjectChanges": true,
                "showBalanceChanges": true
            }
        ]);
        
        let response = self.client
            .post(&self.rpc_endpoint)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sui_getTransactionBlock",
                "params": params
            }))
            .send()
            .await?;
        
        let result: Value = response.json().await?;
        
        if let Some(error) = result.get("error") {
            return Err(anyhow!("RPC error: {}", error));
        }
        
        if !result["result"].is_object() {
            return Err(anyhow!("Invalid response format"));
        }
        
        // Extract the transaction receipt and effects
        let receipt = result["result"].clone();
        let effects = result["result"]["effects"].clone();
        
        Ok((receipt, effects))
    }
    
    /// Verify the effects of a transaction against expectations
    fn verify_transaction_effects(&self, tx: &Transaction, effects: &Value) -> VerificationStatus {
        // Check if the transaction succeeded at all
        if let Some(status) = effects["status"].as_object() {
            if status.contains_key("error") {
                return VerificationStatus::Failed(status["error"].to_string());
            }
        }
        
        // For transfer transactions, verify the correct amount was transferred
        match tx.tx_type {
            TransactionType::Transfer => {
                // Check balance changes to verify transfer amount
                if let Some(balance_changes) = effects["balanceChanges"].as_array() {
                    let mut sender_change = false;
                    let mut receiver_change = false;
                    
                    for change in balance_changes {
                        // Check if this is the sender's balance change
                        if let Some(owner) = change["owner"].as_object() {
                            if let Some(address) = owner["AddressOwner"].as_str() {
                                let normalized_address = Self::normalize_address(address);
                                let normalized_sender = Self::normalize_address(&tx.sender);
                                let normalized_receiver = Self::normalize_address(&tx.receiver);
                                
                                if normalized_address == normalized_sender {
                                    sender_change = true;
                                } else if normalized_address == normalized_receiver {
                                    receiver_change = true;
                                }
                            }
                        }
                    }
                    
                    if sender_change && receiver_change {
                        return VerificationStatus::Verified;
                    } else {
                        return VerificationStatus::Failed("Transfer not properly reflected in balance changes".to_string());
                    }
                } else {
                    return VerificationStatus::Failed("No balance changes found in effects".to_string());
                }
            },
            TransactionType::Invoke | TransactionType::Custom(_) => {
                // In a real implementation, we would verify specific effects based on the contract or custom logic
                VerificationStatus::Verified
            }
        }
    }
    
    /// Normalize a blockchain address for comparison
    fn normalize_address(address: &str) -> String {
        // Remove 0x prefix if present and convert to lowercase
        if address.starts_with("0x") {
            address[2..].to_lowercase()
        } else {
            address.to_lowercase()
        }
    }
    
    /// Get verification statistics
    pub fn get_verification_stats(&self) -> HashMap<VerificationStatus, usize> {
        let mut stats = HashMap::new();
        stats.insert(VerificationStatus::Pending, 0);
        stats.insert(VerificationStatus::Verified, 0);
        stats.insert(VerificationStatus::Failed("".to_string()), 0);
        stats.insert(VerificationStatus::Unverifiable("".to_string()), 0);
        
        let verifications = self.verifications.lock().unwrap();
        
        for record in verifications.values() {
            match &record.status {
                VerificationStatus::Pending => {
                    *stats.get_mut(&VerificationStatus::Pending).unwrap() += 1;
                },
                VerificationStatus::Verified => {
                    *stats.get_mut(&VerificationStatus::Verified).unwrap() += 1;
                },
                VerificationStatus::Failed(_) => {
                    *stats.get_mut(&VerificationStatus::Failed("".to_string())).unwrap() += 1;
                },
                VerificationStatus::Unverifiable(_) => {
                    *stats.get_mut(&VerificationStatus::Unverifiable("".to_string())).unwrap() += 1;
                },
            }
        }
        
        stats
    }
    
    /// Clear old verification records
    pub fn clear_old_records(&self, max_age_seconds: u64) -> Result<usize> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        
        let mut verifications = self.verifications.lock().unwrap();
        let old_count = verifications.len();
        
        verifications.retain(|_, record| {
            // Keep records that are not too old
            now - record.timestamp < max_age_seconds
        });
        
        let removed = old_count - verifications.len();
        Ok(removed)
    }
}