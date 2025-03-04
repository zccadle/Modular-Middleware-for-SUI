use anyhow::{Result, anyhow};
use ed25519_dalek::{Keypair, Signature, Signer};
use reqwest;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::transaction::types::Transaction;
use crate::SUI_TESTNET_RPC;
use crate::metrics::performance::PerformanceMetrics;

#[derive(Debug)]
pub struct TransactionHandler {
    pub client: reqwest::Client,
    pub keypair: Keypair,
}

impl TransactionHandler {
    pub fn new(keypair: Keypair) -> Self {
        Self {
            client: reqwest::Client::new(),
            keypair,
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
}