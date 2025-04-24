/// Defines core data structures used throughout the middleware.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::conditions::time::TimeCondition;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::Result;
// use tokio::sync::oneshot; // Unused
use std::fmt;
// use sui_types::transaction::TransactionData; // Unused

/// Types of transactions the middleware can handle.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Transfer, // Simple value transfer
    Invoke,   // Generic contract invocation
    Custom(String), // Custom types for specific middleware logic
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionType::Transfer => write!(f, "transfer"),
            TransactionType::Invoke => write!(f, "invoke"),
            TransactionType::Custom(s) => write!(f, "custom_{}", s),
        }
    }
}

/// Condition based on external query results.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QueryCondition {
    pub threshold: u64,
    pub operator: String, // e.g., "gt", "lt", "eq"
}

/// Represents a query to an external data source.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExternalQuery {
    pub url: String,      // Endpoint URL
    pub path: Vec<String>, // JSON path to extract data from the response
    pub condition: Option<QueryCondition>, // Optional condition to evaluate against the result
}

/// The primary structure representing a middleware transaction request.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub tx_type: TransactionType,
    pub sender: String,
    pub receiver: String,
    pub amount: u64,
    pub gas_payment: String, // Gas object ID on the L1 chain
    pub gas_budget: u64,
    pub commands: Vec<String>, // Optional commands for middleware execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signatures: Option<Vec<String>>, // Signatures if pre-signed (rarely used)
    pub timestamp: u64, // Timestamp of transaction creation (Unix epoch seconds)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<String>, // JavaScript code for execution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>, // Language of the script ("javascript", "python")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub python_code: Option<String>, // Python code for execution
    #[serde(skip_serializing, default)] // Don't serialize params, default to None
    pub python_params: Option<Value>, // Parameters for Python script
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_query: Option<ExternalQuery>, // External data query details
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub websocket_endpoint: Option<String>, // WebSocket endpoint for data streaming
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub websocket_message: Option<String>, // Message to send over WebSocket
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_condition: Option<TimeCondition>, // Time-based execution condition
}

impl Transaction {
    /// Creates a basic transaction with default optional fields.
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
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            script: None,
            language: None,
            python_code: None,
            python_params: None,
            external_query: None,
            websocket_endpoint: None,
            websocket_message: None,
            time_condition: None,
        }
    }

    /// Calculates a hashable digest of the transaction's core payload.
    ///
    /// This digest excludes volatile fields like signatures to ensure that the
    /// same core transaction data always produces the same hash for verification.
    /// The exact fields included here are critical for consistent attestation.
    pub fn payload_digest(&self) -> Result<Vec<u8>, bcs::Error> {
        // Create a temporary struct containing only the fields to be hashed.
        // This ensures that adding new fields to Transaction doesn't accidentally change the digest.
        #[derive(Serialize)]
        struct DigestPayload<'a> {
            tx_type: &'a TransactionType,
            sender: &'a str,
            receiver: &'a str,
            amount: u64,
            gas_payment: &'a str,
            gas_budget: u64,
            commands: &'a [String],
            timestamp: u64,
            script: &'a Option<String>,
            language: &'a Option<String>,
            python_code: &'a Option<String>,
            // python_params: &'a Option<Value>, // Skipping potentially non-deterministic Value
            external_query: &'a Option<ExternalQuery>,
            websocket_endpoint: &'a Option<String>,
            websocket_message: &'a Option<String>,
            time_condition: &'a Option<TimeCondition>,
        }

        let digest_payload = DigestPayload {
            tx_type: &self.tx_type,
            sender: &self.sender,
            receiver: &self.receiver,
            amount: self.amount,
            gas_payment: &self.gas_payment,
            gas_budget: self.gas_budget,
            commands: &self.commands,
            timestamp: self.timestamp,
            script: &self.script,
            language: &self.language,
            python_code: &self.python_code,
            // python_params: &self.python_params,
            external_query: &self.external_query,
            websocket_endpoint: &self.websocket_endpoint,
            websocket_message: &self.websocket_message,
            time_condition: &self.time_condition,
        };

        bcs::to_bytes(&digest_payload)
    }
}

/// Attestation generated by the middleware quorum.
/// Contains the outcome and links back to the original transaction.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MiddlewareAttestation {
    /// Hash of the original Transaction payload (`payload_digest`).
    pub original_payload_hash: Vec<u8>,
    /// Outcome determined by the middleware (e.g., calculated value, decision).
    pub middleware_outcome: Value, // Flexible JSON value for outcome
    /// Timestamp when the attestation was generated (Unix epoch seconds).
    pub timestamp: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub middleware_node_id: Option<String>, // Optional identifier of the attesting node
}

impl MiddlewareAttestation {
     /// Creates a new attestation with the current timestamp.
     pub fn new(original_payload_hash: Vec<u8>, middleware_outcome: Value) -> Self {
         Self {
             original_payload_hash,
             middleware_outcome,
             timestamp: SystemTime::now()
                 .duration_since(UNIX_EPOCH)
                 .unwrap_or_default()
                 .as_secs(),
            middleware_node_id: None,
         }
     }

     /// Serializes the attestation into bytes suitable for signing by quorum nodes.
     pub fn to_bytes_for_signing(&self) -> Result<Vec<u8>, bcs::Error> {
         bcs::to_bytes(self)
     }
 }

/// Raw bytes of a cryptographic signature.
pub type SignatureBytes = Vec<u8>;

/// Errors that can occur during quorum operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum QuorumError {
    #[error("Internal signing error: {0}")]
    SigningError(String),
    #[error("Attestation serialization error: {0}")]
    SerializationError(String),
    #[error("Signature collection timed out")]
    Timeout,
    #[error("Not enough signatures collected: got {got}, needed {needed}")]
    InsufficientSignatures { got: usize, needed: usize },
    // Removed ResponseSendError as oneshot channel is no longer used here
    // Removed InsufficientSignaturesWithMessage as the structured one is preferred
}

/// Data required for submitting a verification transaction to the L1 contract.
/// This struct was previously defined in handler.rs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationInput {
    // Removed original_tx_payload as it's not directly needed for the L1 call
    /// The payload (attestation or derived data) that was signed by the quorum.
    pub attestation_payload: Vec<u8>,
    /// The collected signatures from the quorum.
    pub quorum_signatures: Vec<Vec<u8>>,
    // Removed tx_data as it's constructed dynamically during submission
}

// Removed VerifiableTransactionData as it seemed redundant with Transaction/MiddlewareAttestation
// Removed TransactionResponse as it wasn't used
// Removed QuorumMessage as inter-node communication isn't simulated here