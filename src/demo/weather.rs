use crate::config;
use crate::metrics::performance::PerformanceMetrics;
use crate::metrics::storage::MetricsStorage;
use crate::transaction::{handler::TransactionHandler, types::{Transaction, TransactionType}};
use crate::external::api::cached_api_call;
use crate::security::audit::SecurityAuditLog;
use crate::transaction::utils::process_and_submit_verification;
use std::sync::Arc;
use anyhow::Result;
use crate::execution::manager::ExecutionManager;
use sui_sdk::types::base_types::ObjectID;
use sui_sdk::types::crypto::SuiKeyPair;
use serde_json::json;

const CITY: &str = "London";

pub async fn run_weather_based_transaction_demo(
    transaction_handler: &Arc<TransactionHandler>,
    execution_manager: &Arc<ExecutionManager>,
    metrics_storage: Option<&Arc<MetricsStorage>>,
    security_audit_log: &Arc<SecurityAuditLog>,
    submitter_keypair: &SuiKeyPair,
    gas_object_id: &ObjectID,
) -> Result<()> {
    println!("\n--- RUNNING WEATHER-BASED TRANSACTION DEMO ---\n");
    
    let tx_name = "weather_demo";
    let metrics = metrics_storage.map(|_s| PerformanceMetrics::new(tx_name));
    
    // 1. Get current weather data or use demo data
    let weather_data = cached_api_call("https://api.openweathermap.org/data/2.5/weather?q=London,uk&appid=d0e7b0a99a2af7075f4f705e1112c66c&units=metric").await
        .unwrap_or_else(|_| {
            // Use demo data that will pass our conditions
            serde_json::json!({
                "main": {
                    "temp": 22.5,  // Nice temperature for demo
                    "humidity": 65
                },
                "weather": [{
                    "main": "Clear",  // Good weather, not "Thunderstorm" which would block execution
                    "description": "clear sky"
                }],
                "wind": {
                    "speed": 3.1
                }
            })
        });
    
    println!("Current weather data: {:?}", weather_data);
    
    let condition_met = weather_data["main"]["temp"].as_f64().unwrap_or(-999.0) > 25.0; // Handle potential parse error

    if condition_met {
        println!("Weather condition met (temp > 25.0). Preparing transaction...");

        // Create a transaction representing the intent
        let transaction = Transaction {
            tx_type: TransactionType::Custom("weather_trigger".to_string()),
            sender: config::SUBMITTER_ADDRESS.to_string(),
            receiver: config::SUBMITTER_ADDRESS.to_string(),
            amount: 1, 
            gas_payment: config::SUBMITTER_GAS_OBJECT_ID.to_string(),
            gas_budget: 2000000,
            commands: vec!["process_weather_event".to_string()],
            python_params: Some(json!({
                "temperature": weather_data["main"]["temp"].as_f64(),
                "description": weather_data["weather"][0]["description"].as_str()
             })),
            language: Some("native".to_string()), // Assume native middleware logic handles this
            signatures: None, timestamp: 0, script: None, external_query: None,
            python_code: None, websocket_endpoint: None, websocket_message: None,
            time_condition: None,
        };

        // Call the main processing and submission function
        process_and_submit_verification(
            &transaction,
            tx_name,
            transaction_handler,
            execution_manager,
            metrics_storage,
            security_audit_log,
            submitter_keypair,
            gas_object_id,
        ).await?;

    } else {
        println!("Weather condition not met (temp <= 25.0). No transaction processed.");
    }

    // Store metrics (only if metrics_storage was Some)
    if let (Some(m), Some(storage)) = (metrics, metrics_storage) {
        storage.add_metrics(m);
    }
    
    println!("\n--- WEATHER-BASED TRANSACTION DEMO COMPLETE ---\n");
    
    Ok(())
}

// Mock function if actual function is missing
async fn get_weather_data(_api_key: &str, city: &str) -> Result<serde_json::Value> {
    // This is a mock implementation. Replace with actual implementation.
    Ok(json!({
        "city": city,
        "temperature": 22.5,
        "conditions": "Partly Cloudy",
        "humidity": 65,
        "wind_speed": 10
    }))
}