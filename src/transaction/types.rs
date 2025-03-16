use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::conditions::time::TimeCondition;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Transfer,
    Invoke,
    Custom(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryCondition {
    pub threshold: u64,
    pub operator: String,  // "gt", "lt", "eq"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalQuery {
    pub url: String,
    pub path: Vec<String>, 
    pub condition: Option<QueryCondition>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub tx_type: TransactionType,
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub gas_payment: String,
    pub gas_budget: u64,
    pub commands: Vec<String>,
    pub signatures: Option<Vec<String>>,
    pub timestamp: u64,
    #[serde(default)]
    pub script: Option<String>,
    #[serde(default)]
    pub external_query: Option<ExternalQuery>,
    // New fields for enhanced capabilities
    #[serde(default)]
    pub python_code: Option<String>,
    #[serde(skip_serializing)]
    pub python_params: Option<Value>,
    #[serde(default)]
    pub websocket_endpoint: Option<String>,
    #[serde(default)]
    pub websocket_message: Option<String>,
    #[serde(default)]
    pub time_condition: Option<TimeCondition>,
    #[serde(default)]
    pub language: Option<String>,  // "javascript", "python", etc.
}

impl Transaction {
    pub fn new(
        tx_type: TransactionType,
        sender: String,
        receiver: String,
        amount: u64,
        gas_payment: String,
        gas_budget: u64,
        commands: Vec<String>,
    ) -> Self {
        Self {
            tx_type,
            sender,
            receiver,
            amount,
            gas_payment,
            gas_budget,
            commands,
            signatures: None,
            timestamp: chrono::Utc::now().timestamp() as u64,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResponse {
    pub digest: String,
    pub transaction: Transaction,
}