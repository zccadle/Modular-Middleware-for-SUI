use anyhow::{Result, anyhow};
use reqwest;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde_json::Value;

use crate::transaction::types::{Transaction, TransactionType, ExternalQuery};
use crate::languages::python::PythonExecutor;
use crate::languages::javascript::JavaScriptExecutor;
use crate::external::websocket::WebSocketClient;
use crate::conditions::time::TimeBasedEvaluator;
use crate::metrics::performance::PerformanceMetrics;
use crate::sui::verification::VerificationManager;
use crate::sui::network::NetworkManager;
use crate::security::audit::{SecurityAuditLog, AuditSeverity};

#[derive(Debug)]
pub struct ExecutionManager {
    pub client: reqwest::Client,
    pub state: Arc<Mutex<HashMap<String, u64>>>,
    verification_manager: Option<Arc<VerificationManager>>,
    network_manager: Option<Arc<NetworkManager>>,
    security_audit_log: Option<Arc<SecurityAuditLog>>,
}

impl ExecutionManager {
    pub fn reset_account_balances(&self) {
        let mut state = self.state.lock().expect("Failed to acquire lock");
        // Reset all account balances
        for (_, balance) in state.iter_mut() {
            *balance = 1000; // Reset to initial balance
        }
        // Ensure the default accounts are properly funded
        state.insert("0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(), 1000);
        state.insert("0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(), 0);
        println!("Account balances have been reset for the next test iteration.");
    }

    pub fn new(
        verification_manager: Option<VerificationManager>,
        network_manager: Option<Arc<NetworkManager>>,
        security_audit_log: Option<Arc<SecurityAuditLog>>
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            state: Arc::new(Mutex::new(HashMap::new())),
            verification_manager: verification_manager.map(Arc::new),
            network_manager: network_manager,
            security_audit_log: security_audit_log,
        }
    }

    pub async fn fetch_external_data(&self, query: &ExternalQuery, _metrics: Option<&mut PerformanceMetrics>) -> Result<f64> {
        // Track external API call as part of execution time (not SUI time)
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

    pub async fn execute_transaction(&self, tx: &mut Transaction, mut metrics: Option<&mut PerformanceMetrics>) -> Result<bool> {
        // Log execution start
        if let Some(audit_log) = &self.security_audit_log {
            audit_log.log_execution(
                "ExecutionManager",
                &format!("Executing transaction from {} to {}", tx.sender, tx.receiver),
                None,
                AuditSeverity::Info
            )?;
        }
        // Set chain ID in metrics if available
        if let Some(m) = metrics.as_mut() {
            if let Some(network_manager) = &self.network_manager {
                let config = network_manager.get_active_config();
                m.set_chain_id(&config.chain_id);
            }
        }
        // Start execution time tracking if metrics provided
        if let Some(m) = metrics.as_mut() {
            m.execution_start_time = Some(std::time::Instant::now());
        }
        
        // Check time-based conditions if present
        if let Some(time_condition) = &tx.time_condition {
            match TimeBasedEvaluator::evaluate(time_condition) {
                Ok(true) => {
                    println!("Time condition satisfied");
                },
                Ok(false) => {
                    println!("Time condition not satisfied, skipping transaction");
                    
                    // End execution time tracking if metrics provided
                    if let Some(m) = metrics.as_mut() {
                        m.execution_end_time = Some(std::time::Instant::now());
                    }
                    
                    return Ok(false);
                },
                Err(e) => {
                    println!("Error evaluating time condition: {}", e);
                    
                    // End execution time tracking if metrics provided
                    if let Some(m) = metrics.as_mut() {
                        m.execution_end_time = Some(std::time::Instant::now());
                    }
                    
                    return Err(anyhow!("Time condition evaluation error: {}", e));
                }
            }
        }

        // Handle execution based on language preference
        let language = tx.language.as_deref().unwrap_or("javascript");
        
        match language {
            "python" => {
                if let Some(code) = &tx.python_code {
                    println!("Executing Python code: {:?}", code);
                    
                    match PythonExecutor::execute(code, tx.python_params.clone()) {
                        Ok(result) => {
                            if result.success {
                                println!("Python execution successful: {:?}", result.output);
                                
                                // Update transaction parameters based on Python output
                                if let Value::Object(map) = &result.output {
                                    if let Some(Value::Number(new_gas)) = map.get("gas_budget") {
                                        if let Some(gas) = new_gas.as_u64() {
                                            tx.gas_budget = gas;
                                            println!("Updated gas budget to: {}", gas);
                                        }
                                    }
                                    
                                    // Check for calculated amount (for compensation scenarios)
                                    if let Some(Value::Number(amount)) = map.get("compensation_amount") {
                                        if let Some(amount_value) = amount.as_u64() {
                                            tx.amount = amount_value;
                                            println!("Set transaction amount to: {}", amount_value);
                                        }
                                    }
                                    
                                    if let Some(Value::Bool(should_execute)) = map.get("should_execute") {
                                        if !should_execute {
                                            println!("Python code decided not to execute transaction");
                                            
                                            // End execution time tracking if metrics provided
                                            if let Some(m) = metrics.as_mut() {
                                                m.execution_end_time = Some(std::time::Instant::now());
                                            }
                                            
                                            return Ok(false);
                                        }
                                    }
                                }
                            } else {
                                println!("Python execution failed: {:?}", result.error);
                                
                                // End execution time tracking if metrics provided
                                if let Some(m) = metrics.as_mut() {
                                    m.execution_end_time = Some(std::time::Instant::now());
                                }
                                
                                return Err(anyhow!("Python execution error: {:?}", result.error));
                            }
                        },
                        Err(e) => {
                            println!("Error executing Python: {}", e);
                            
                            // End execution time tracking if metrics provided
                            if let Some(m) = metrics.as_mut() {
                                m.execution_end_time = Some(std::time::Instant::now());
                            }
                            
                            return Err(anyhow!("Python execution error: {}", e));
                        }
                    }
                }
            },
            "javascript" => {
                // Execute existing JavaScript code using Boa engine instead of QuickJS
                if let Some(script) = &tx.script {
                    println!("Executing JavaScript code: {:?}", script);
                    
                    match JavaScriptExecutor::execute(script, None) {
                        Ok(result) => {
                            if result.success {
                                println!("JavaScript execution successful: {:?}", result.output);
                                
                                // Update transaction parameters based on JavaScript output
                                if let Value::Object(map) = &result.output {
                                    if let Some(Value::Number(new_gas)) = map.get("gas_budget") {
                                        if let Some(gas) = new_gas.as_u64() {
                                            tx.gas_budget = gas;
                                            println!("Updated gas budget to: {}", gas);
                                        }
                                    }
                                    
                                    if let Some(Value::Bool(should_execute)) = map.get("should_execute") {
                                        if !should_execute {
                                            println!("JavaScript code decided not to execute transaction");
                                            
                                            // End execution time tracking if metrics provided
                                            if let Some(m) = metrics.as_mut() {
                                                m.execution_end_time = Some(std::time::Instant::now());
                                            }
                                            
                                            return Ok(false);
                                        }
                                    }
                                }
                            } else {
                                println!("JavaScript execution failed: {:?}", result.error);
                                
                                // End execution time tracking if metrics provided
                                if let Some(m) = metrics.as_mut() {
                                    m.execution_end_time = Some(std::time::Instant::now());
                                }
                                
                                return Err(anyhow!("JavaScript execution error: {:?}", result.error));
                            }
                        },
                        Err(e) => {
                            println!("Error executing JavaScript: {}", e);
                            
                            // End execution time tracking if metrics provided
                            if let Some(m) = metrics.as_mut() {
                                m.execution_end_time = Some(std::time::Instant::now());
                            }
                            
                            return Err(anyhow!("JavaScript execution error: {}", e));
                        }
                    }
                }
            },
            _ => {
                // End execution time tracking if metrics provided
                if let Some(m) = metrics.as_mut() {
                    m.execution_end_time = Some(std::time::Instant::now());
                }
                
                return Err(anyhow!("Unsupported language: {}", language));
            }
        }
        
        // Handle WebSocket operations
        if let Some(ws_endpoint) = &tx.websocket_endpoint {
            let ws_client = WebSocketClient::new(ws_endpoint);
            
            // Start the WebSocket client if it's a new connection
            ws_client.start_listening()?;
            
            // Send a message if provided
            if let Some(message) = &tx.websocket_message {
                ws_client.send_message(message)?;
            }
            
            // Wait for a response (with timeout)
            let start = std::time::Instant::now();
            let timeout = Duration::from_secs(5);
            
            while start.elapsed() < timeout {
                if let Some(response) = ws_client.get_last_message() {
                    println!("Received WebSocket response: {:?}", response);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        
        // Run existing external query logic
        if let Some(query) = &tx.external_query {
            println!("External query {:?}", query);
        
            let external_value = self.fetch_external_data(query, None).await?;
            println!("External value from query {:?}", external_value);
        
            if let Some(condition) = &query.condition {
                let threshold_f64 = condition.threshold as f64 / 100.0;
    
                match condition.operator.as_str() {
                    "gt" => {
                        if external_value > threshold_f64 {
                            tx.gas_budget = (tx.gas_budget as f64 * 1.1) as u64;
                            println!("Gas budget increased to {:?}", tx.gas_budget);
                        } else {
                            // End execution time tracking if metrics provided
                            if let Some(m) = metrics.as_mut() {
                                m.execution_end_time = Some(std::time::Instant::now());
                            }
                            
                            return Ok(false);
                        }
                    },
                    "lt" => {
                        if external_value >= threshold_f64 {
                            // End execution time tracking if metrics provided
                            if let Some(m) = metrics.as_mut() {
                                m.execution_end_time = Some(std::time::Instant::now());
                            }
                            
                            return Ok(false);
                        }
                    },
                    "eq" => {
                        if (external_value - threshold_f64).abs() > f64::EPSILON {
                            // End execution time tracking if metrics provided
                            if let Some(m) = metrics.as_mut() {
                                m.execution_end_time = Some(std::time::Instant::now());
                            }
                            
                            return Ok(false);
                        }
                    },
                    _ => {
                        // End execution time tracking if metrics provided
                        if let Some(m) = metrics.as_mut() {
                            m.execution_end_time = Some(std::time::Instant::now());
                        }
                        
                        return Ok(false);
                    },
                }
            }
        };

        // Execute the transaction based on type
        match tx.tx_type {
            TransactionType::Transfer => {
                let mut state = self.state.lock().expect("Failed to acquire lock");
                {
                    // Get sender's balance
                    let sender_balance_option = state.get(&tx.sender).cloned();
                    let sender_balance = sender_balance_option.unwrap_or(1000);
                    
                    if sender_balance < tx.amount {
                        println!("Insufficient balance for sender: {}", tx.sender);
                        
                        // End execution time tracking if metrics provided
                        if let Some(m) = metrics.as_mut() {
                            m.execution_end_time = Some(std::time::Instant::now());
                        }
                        
                        return Err(anyhow!("Insufficient balance"));
                    }
                    
                    // Get receiver's balance
                    let receiver_balance_option = state.get(&tx.receiver).cloned();
                    let receiver_balance = receiver_balance_option.unwrap_or(0);
                    
                    // Update balances
                    state.insert(tx.sender.clone(), sender_balance - tx.amount);
                    state.insert(tx.receiver.clone(), receiver_balance + tx.amount);
                    
                    println!("Transferred {} from {} to {}", tx.amount, tx.sender, tx.receiver);
                    println!("New balances: {} = {}, {} = {}", 
                             tx.sender, sender_balance - tx.amount, 
                             tx.receiver, receiver_balance + tx.amount);
                }
            },
            TransactionType::Invoke | TransactionType::Custom(_) => {
                // For Invoke and Custom transaction types, we'll just log the execution
                println!("Executing {} transaction with commands: {:?}", 
                         format!("{:?}", tx.tx_type), tx.commands);
                
                // In a real implementation, this would invoke a smart contract or custom logic
                // For now, we'll just simulate success
                println!("Transaction executed successfully");
            }
        }
        
        // End execution time tracking if metrics provided
        if let Some(m) = metrics.as_mut() {
            m.execution_end_time = Some(std::time::Instant::now());
        }
        
        return Ok(true);
    }
}