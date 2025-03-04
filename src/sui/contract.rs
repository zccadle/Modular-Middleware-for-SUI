use anyhow::{Result, anyhow};
use reqwest;
use serde_json::{json, Value};
use std::env;
use crate::SUI_TESTNET_RPC;
use crate::external::flight_api::FlightStatus;

// Flight insurance contract interaction
pub struct FlightInsuranceContract {
    client: reqwest::Client,
    package_id: String,
    treasury_id: String,
    oracle_address: String,
    private_key: String,
}

impl FlightInsuranceContract {
    pub fn new() -> Self {
        // In production, these would be read from environment variables
        let package_id = env::var("SUI_INSURANCE_PACKAGE_ID")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000000000000000000000000000".to_string());
        
        let treasury_id = env::var("SUI_INSURANCE_TREASURY_ID")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000000000000000000000000000".to_string());
        
        let oracle_address = env::var("SUI_MIDDLEWARE_ORACLE_ADDRESS")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000000000000000000000000000".to_string());
        
        let private_key = env::var("SUI_ORACLE_PRIVATE_KEY")
            .unwrap_or_else(|_| "".to_string());
        
        Self {
            client: reqwest::Client::new(),
            package_id,
            treasury_id,
            oracle_address,
            private_key,
        }
    }
    
    // Process a claim for a delayed or cancelled flight
    pub async fn process_claim(
        &self, 
        policy_id: &str, 
        flight_status: &FlightStatus,
        policy_type: &str
    ) -> Result<String> {
        println!("Processing insurance claim for policy {} on flight {}", 
                 policy_id, flight_status.flight_number);
        
        // Real production code would sign and execute a transaction to the SUI blockchain
        // For this prototype, we'll demonstrate the API construction but skip actual execution
        // if we don't have real credentials
        
        let compensation_amount = flight_status.get_compensation_amount(policy_type);
        
        // Skip if no compensation is due
        if compensation_amount == 0 {
            return Err(anyhow!("No compensation due for flight {}", flight_status.flight_number));
        }
        
        // Check if we have actual credentials
        if self.private_key.is_empty() || self.package_id.starts_with("0x00000") {
            println!("TEST MODE: Would process claim with these parameters:");
            println!("  Policy ID: {}", policy_id);
            println!("  Flight: {}", flight_status.flight_number);
            println!("  Delay: {} minutes", flight_status.delay_minutes);
            println!("  Cancelled: {}", flight_status.is_cancelled());
            println!("  Compensation: {}", compensation_amount);
            println!("  Policy Type: {}", policy_type);
            
            // Return a mock transaction hash
            return Ok(format!("simulation_tx_{}_{}_{}", 
                            policy_id, 
                            flight_status.flight_number,
                            compensation_amount));
        }
        
        // In production, construct the actual SUI transaction
        let move_call = json!({
            "packageObjectId": self.package_id,
            "module": "policy",
            "function": "process_claim",
            "typeArguments": [],
            "arguments": [
                self.treasury_id,
                policy_id,
                flight_status.flight_number,
                flight_status.delay_minutes,
                compensation_amount,
                flight_status.is_cancelled(),
            ],
            "gasBudget": 10000
        });
        
        // Call SUI RPC to execute transaction
        let response = self.client
            .post(SUI_TESTNET_RPC)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sui_executeTransactionBlock",
                "params": [move_call, self.oracle_address, "WaitForLocalExecution", "Ed25519", self.private_key]
            }))
            .send()
            .await?;
        
        // Parse the response
        let result: Value = response.json().await?;
        
        if let Some(error) = result.get("error") {
            return Err(anyhow!("SUI transaction error: {:?}", error));
        }
        
        if let Some(digest) = result.get("result").and_then(|r| r.get("digest")).and_then(|d| d.as_str()) {
            Ok(digest.to_string())
        } else {
            Err(anyhow!("Failed to get transaction digest"))
        }
    }
    
    // Get policy details (would be used in a full implementation)
    pub async fn get_policy_details(&self, policy_id: &str) -> Result<Value> {
        // In a real implementation, this would query the SUI blockchain for policy details
        // For this prototype, we'll just return a simulated response
        
        if self.private_key.is_empty() || self.package_id.starts_with("0x00000") {
            return Ok(json!({
                "policy_id": policy_id,
                "owner": "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6",
                "flight_number": "BA1326",
                "policy_type": "premium",
                "premium_paid": 50,
                "is_claimed": false,
                "expiration_time": (chrono::Utc::now() + chrono::Duration::days(30)).timestamp()
            }));
        }
        
        // Real blockchain query would go here
        let response = self.client
            .post(SUI_TESTNET_RPC)
            .json(&json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "sui_getObject",
                "params": [policy_id, {"showContent": true}]
            }))
            .send()
            .await?;
        
        let result: Value = response.json().await?;
        
        if let Some(error) = result.get("error") {
            return Err(anyhow!("SUI query error: {:?}", error));
        }
        
        Ok(result["result"]["data"]["content"].clone())
    }
}