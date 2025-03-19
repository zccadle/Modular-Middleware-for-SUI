use anyhow::Result;
use reqwest;
use std::sync::{Arc, Mutex};
use crate::transaction::types::{Transaction, TransactionType};
use crate::metrics::performance::PerformanceMetrics;

// Use the official SUI testnet endpoint
const SUI_TESTNET_RPC: &str = "https://fullnode.testnet.sui.io:443";

#[derive(Debug)]
pub struct SequencingLayer {
    pub client: reqwest::Client,
    pub last_processed_tx: Arc<Mutex<Option<String>>>,
}

impl SequencingLayer {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            last_processed_tx: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn poll_transactions(&self) -> Result<Vec<Transaction>> {
        let last_tx = self.last_processed_tx.lock().unwrap().clone();
        let params = serde_json::json!({ "cursor": last_tx });
        println!("Polling for transactions with params: {:?}", params);
        
        let response = self.client
            .post(SUI_TESTNET_RPC)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "suix_queryTransactionBlocks",
                "params": [params]
            }))
            .send()
            .await?;
        
        let parsed_response = response.json::<serde_json::Value>().await?;
        println!("Polling response: {:?}", parsed_response);
        
        if let Some(error) = parsed_response["error"].as_object() {
            println!("RPC error: {:?}", error);
        }
    
        let digests: Vec<String> = if let Some(data) = parsed_response["result"]["data"].as_array() {
            data.iter()
                .filter_map(|tx| tx["digest"].as_str().map(|s| s.to_string()))
                .collect()
        } else {
            vec![]
        };

        println!("Found {} transaction digest(s).", digests.len());
    
        let mut transactions = Vec::new();
        for digest in digests {
            let params = serde_json::json!([digest, {
                "showInput": true,
                "showRawInput": false,
                "showEffects": true,
                "showEvents": true,
                "showObjectChanges": false,
                "showBalanceChanges": false,
                "showRawEffects": false
            }]);
            let detail_response = self.client
                .post(SUI_TESTNET_RPC)
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "sui_getTransactionBlock",
                    "params": params
                }))
                .send()
                .await?;
            let detail_parsed_response = detail_response.json::<serde_json::Value>().await?;
            if let Some(transaction_data) = detail_parsed_response["result"].as_object() {
                let transaction = self.parse_transaction(transaction_data);
                // Only include transactions with a non-default sender.
               if transaction.sender != "0x0000000000000000000000000000000000000000000000000000000000000000" {
                    transactions.push(transaction);
                }
            }
        }
        
        if let Some(last_tx) = transactions.last() {
            *self.last_processed_tx.lock().unwrap() = Some(last_tx.sender.clone());
        }
    
        Ok(transactions)
    }
    
    fn parse_transaction(&self, data: &serde_json::Map<String, serde_json::Value>) -> Transaction {
        let sender = data["transaction"]["data"]["sender"]
            .as_str().unwrap_or_default().to_string();
        let receiver = data["transaction"]["objectChanges"]
            .as_array()
            .and_then(|arr| arr.get(0))
            .and_then(|obj| obj["recipient"]["AddressOwner"].as_str())
            .unwrap_or_default().to_string();
        // For the MVP, amount is hardcoded as 0.
        let amount = 0;
        let gas_payment = data["transaction"]["data"]["gasData"]["payment"]
            .as_array().and_then(|arr| arr.get(0))
            .and_then(|obj| obj["objectId"].as_str())
            .unwrap_or_default().to_string();
        let gas_budget = data["transaction"]["data"]["gasData"]["budget"]
            .as_str().and_then(|s| s.parse::<u64>().ok())
            .unwrap_or_default();
        let commands = data["transaction"]["data"]["transaction"]["transactions"]
            .as_array().unwrap_or(&Vec::new())
            .iter()
            .filter_map(|cmd| {
                if cmd.get("TransferObjects").is_some() {
                    Some("TransferObjects".to_string())
                } else {
                    None
                }
            }).collect::<Vec<String>>();
        let signatures = data["transaction"]["txSignatures"]
            .as_array().map(|arr| arr.iter()
                .filter_map(|sig| sig.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        let timestamp = data["transaction"]["timestamp"]
            .as_u64().unwrap_or_default();
        Transaction {
            tx_type: TransactionType::Transfer,
            sender,
            receiver,
            amount,
            gas_payment,
            gas_budget,
            commands,
            signatures: Some(signatures),
            timestamp,
            script: None,
            external_query: None,
            python_code: None,
            python_params: None,
            websocket_endpoint: None,
            websocket_message: None,
            time_condition: None,
            language: None,
        }
    }
}