use anyhow::{Result, anyhow};
use reqwest;
use std::sync::Arc;
use serde_json::{Value, json};
use std::time::SystemTime;

use crate::transaction::types::{Transaction, ExternalQuery, MiddlewareAttestation, VerificationInput};
use crate::languages::python::PythonExecutor;
use crate::languages::javascript::JavaScriptExecutor;
use crate::external::websocket::WebSocketClient;
use crate::conditions::time::TimeBasedEvaluator;
use crate::metrics::performance::PerformanceMetrics;
use crate::sui::verification::VerificationManager;
use crate::sui::network::NetworkManager;
use crate::security::audit::{SecurityAuditLog, AuditSeverity};

pub struct ExecutionManager {
    pub client: reqwest::Client,
    verification_manager: Option<Arc<VerificationManager>>,
    network_manager: Option<Arc<NetworkManager>>,
    security_audit_log: Option<Arc<SecurityAuditLog>>,
    pub client_manager: ClientManager,
}

impl ExecutionManager {
    pub fn new(
        verification_manager: Option<VerificationManager>,
        network_manager: Option<Arc<NetworkManager>>,
        security_audit_log: Option<Arc<SecurityAuditLog>>
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            verification_manager: verification_manager.map(Arc::new),
            network_manager: network_manager,
            security_audit_log: security_audit_log,
            client_manager: ClientManager::new(),
        }
    }

    pub async fn fetch_external_data(&self, query: &ExternalQuery, _metrics: Option<&mut PerformanceMetrics>) -> Result<f64> {
        let response = self.client.get(&query.url)
            .send()
            .await?
            .json::<Value>()
            .await?;
        
        let mut current = &response;
        for key in &query.path {
            current = &current[key];
        }
        
        current.as_f64()
            .ok_or_else(|| anyhow!("Invalid response format"))
    }

    pub async fn process_transaction_and_attest(
        &self,
        tx: &Transaction,
        mut metrics: Option<&mut PerformanceMetrics>
    ) -> Result<Option<MiddlewareAttestation>, anyhow::Error> {
        if let Some(audit_log) = &self.security_audit_log {
            audit_log.log_execution(
                "ExecutionManager",
                &format!("Processing transaction from {} to {}", tx.sender, tx.receiver),
                None,
                AuditSeverity::Info
            )?;
        }
        if let Some(m) = metrics.as_mut() {
            if let Some(network_manager) = &self.network_manager {
                let config = network_manager.get_active_config();
                if let Some(chain_id) = config.get_chain_id() {
                    m.set_chain_id(&chain_id);
                }
            }
        }
        if let Some(ref mut m) = metrics {
            m.execution_start_time = Some(SystemTime::now());
        }

        if let Some(time_condition) = &tx.time_condition {
            match TimeBasedEvaluator::evaluate(time_condition) {
                Ok(true) => println!("Time condition satisfied"),
                Ok(false) => {
                    println!("Time condition not satisfied, skipping middleware processing");
                    if let Some(m) = metrics.as_mut() { m.execution_end_time = Some(SystemTime::now()); }
                    return Ok(None);
                },
                Err(e) => {
                    println!("Error evaluating time condition: {}", e);
                    if let Some(m) = metrics.as_mut() { m.execution_end_time = Some(SystemTime::now()); }
                    return Err(anyhow!("Time condition evaluation error: {}", e));
                }
            }
        }

        let mut continue_execution = true;
        if let Some(query) = &tx.external_query {
            if let Some(condition) = &query.condition {
                println!("External query condition present: {:?}", query);
                let external_value_result = self.fetch_external_data(query, metrics.as_deref_mut()).await;
                match external_value_result {
                    Ok(external_value) => {
                        println!("External value from query {:?}", external_value);
                        let threshold = condition.threshold as f64;
                        continue_execution = match condition.operator.as_str() {
                            "gt" => external_value > threshold,
                            "lt" => external_value < threshold,
                            "eq" => (external_value - threshold).abs() < f64::EPSILON,
                            _ => false,
                        };
                        if !continue_execution {
                            println!("External query condition NOT met ({:?} {} {} = {}). Skipping middleware processing.",
                                external_value, condition.operator, threshold, continue_execution);
                        }
                    },
                    Err(e) => {
                        println!("Error fetching external data for condition: {}", e);
                        if let Some(m) = metrics.as_mut() { m.execution_end_time = Some(SystemTime::now()); }
                        return Err(anyhow!("External query failed: {}", e));
                    }
                }
            } else {
                let _ = self.fetch_external_data(query, metrics.as_deref_mut()).await;
            }
        }

        if !continue_execution {
            if let Some(m) = metrics.as_mut() { m.execution_end_time = Some(SystemTime::now()); }
            return Ok(None);
        }

        let mut middleware_outcome: Value = json!({});
        let mut should_generate_attestation = true;

        let language = tx.language.as_deref().unwrap_or("native");

        match language {
            "python" => {
                if let Some(code) = &tx.python_code {
                    println!("Executing Python code: {:?}", code);
                    match PythonExecutor::execute(code, tx.python_params.clone()) {
                        Ok(result) => {
                            println!("Python execution successful: {:?}", result.output);
                            middleware_outcome = result.output;
                            if let Some(Value::Bool(execute)) = middleware_outcome.get("should_execute") {
                                if !execute {
                                    println!("Python script decided not to generate attestation");
                                    should_generate_attestation = false;
                                }
                            }
                        },
                        Err(e) => {
                            println!("Error executing Python: {}", e);
                            if let Some(m) = metrics.as_mut() { m.execution_end_time = Some(SystemTime::now()); }
                            return Err(anyhow!("Python execution error: {}", e));
                        }
                    }
                }
            },
            "javascript" => {
                if let Some(script) = &tx.script {
                    println!("Executing JavaScript code: {:?}", script);
                    match JavaScriptExecutor::execute(script, None) {
                        Ok(result) => {
                            println!("JavaScript execution successful: {:?}", result.output);
                            middleware_outcome = result.output;
                            if let Some(Value::Bool(execute)) = middleware_outcome.get("should_execute") {
                                if !execute {
                                    println!("JavaScript script decided not to generate attestation");
                                    should_generate_attestation = false;
                                }
                            }
                        },
                        Err(e) => {
                            println!("Error executing JavaScript: {}", e);
                            if let Some(m) = metrics.as_mut() { m.execution_end_time = Some(SystemTime::now()); }
                            return Err(anyhow!("JavaScript execution error: {}", e));
                        }
                    }
                }
            },
            "native" => {
                middleware_outcome = json!({ "executed_natively": true });
            },
            _ => {
                if let Some(m) = metrics.as_mut() { m.execution_end_time = Some(SystemTime::now()); }
                return Err(anyhow!("Unsupported language: {}", language));
            }
        }

        if let Some(ws_endpoint) = &tx.websocket_endpoint {
            let ws_client = WebSocketClient::new(ws_endpoint);
            ws_client.start_listening()?;
            if let Some(message) = &tx.websocket_message {
                ws_client.send_message(message)?;
            }
        }

        if let Some(m) = metrics.as_mut() {
            m.execution_end_time = Some(SystemTime::now());
        }

        if !should_generate_attestation {
            println!("Skipping attestation generation as per middleware logic.");
            return Ok(None);
        }

        let payload_hash = match tx.payload_digest() {
            Ok(hash) => hash,
            Err(e) => return Err(anyhow!("Failed to compute transaction payload digest: {}", e)),
        };

        let attestation = MiddlewareAttestation::new(payload_hash, middleware_outcome);

        if let Some(audit_log) = &self.security_audit_log {
            audit_log.log_execution(
                "ExecutionManager",
                &format!("Successfully processed transaction and generated attestation"),
                None,
                AuditSeverity::Info
            )?;
        }

        Ok(Some(attestation))
    }

    // Placeholder method - Replace with actual implementation!
    pub async fn prepare_verification_input(&self, tx: &Transaction) -> Result<Option<VerificationInput>> {
        println!("[WARN] Using placeholder prepare_verification_input in ExecutionManager.");
        // TODO: Implement the actual logic based on tx type, script execution, etc.
        // This should involve:
        // 1. Potentially executing tx.script/tx.python_code if present.
        // 2. Determining the middleware_outcome (as a serde_json::Value).
        // 3. Calculating the original_payload_hash using tx.payload_digest().
        // 4. Constructing the MiddlewareAttestation.
        // 5. Serializing the MiddlewareAttestation to get the attestation_payload bytes for signing.
        // 6. Deciding if verification is needed (e.g., based on outcome or conditions).
        // 7. Returning Some(VerificationInput { attestation_payload, quorum_signatures: vec![] }) or Ok(None).

        // Example placeholder returning Some (replace with real logic):
        let dummy_payload_hash = tx.payload_digest().unwrap_or_default();
        let dummy_outcome = serde_json::json!({ "placeholder_outcome": true });
        let attestation = MiddlewareAttestation::new(dummy_payload_hash, dummy_outcome);
        let attestation_payload = attestation.to_bytes_for_signing()
            .map_err(|e| anyhow!("Failed to serialize placeholder attestation: {}", e))?;

        Ok(Some(VerificationInput {
            attestation_payload,
            quorum_signatures: Vec::new(), // Signatures added later by handler
        }))
    }
}

pub struct ClientManager {
    // Fields if needed
}

impl ClientManager {
    pub fn new() -> Self {
        Self {}
    }
    
    pub async fn execute(&self, tx: &crate::transaction::types::Transaction) -> Result<serde_json::Value, anyhow::Error> {
        // Simple implementation that returns a JSON object
        Ok(serde_json::json!({
            "status": "success",
            "transaction_id": format!("tx_{}", tx.timestamp),
            "amount": tx.amount
        }))
    }
}