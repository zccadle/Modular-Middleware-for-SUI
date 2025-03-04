use crate::{
    transaction::types::{Transaction, TransactionType},
    transaction::handler::TransactionHandler,
    execution::manager::ExecutionManager,
    conditions::time::{TimeCondition, TimeConditionType},
    external::api::cached_api_call,
    metrics::performance::PerformanceMetrics,
    metrics::storage::MetricsStorage
};
use anyhow::Result;

pub async fn run_weather_based_transaction_demo(
    transaction_handler: &TransactionHandler,
    execution_manager: &ExecutionManager,
    metrics_storage: Option<&MetricsStorage>
) -> Result<()> {
    println!("\n--- RUNNING WEATHER-BASED TRANSACTION DEMO ---\n");
    
    // Create metrics if storage is provided
    let mut metrics = if metrics_storage.is_some() {
        Some(PerformanceMetrics::new("weather"))
    } else {
        None
    };
    
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
    
    // 2. Create a Python script that will definitely execute
    let python_code = r#"
import datetime
    
# Weather data is provided as 'params'
# First, we need to check that params exists and print it to debug
print(f"Weather data received: {params}")
    
# Extract key information or use defaults
try:
    temperature = params["main"]["temp"]
    humidity = params["main"]["humidity"]
    weather_condition = params["weather"][0]["main"]
    wind_speed = params["wind"]["speed"]
        
    print(f"Successfully extracted: temp={temperature}, humidity={humidity}, condition={weather_condition}")
except Exception as e:
    print(f"Error extracting weather data: {e}")
    # Set default values for demo
    temperature = 22.5
    humidity = 65
    weather_condition = "Clear"
    wind_speed = 3.1
    print("Using default values instead")
    
# Current time
current_hour = datetime.datetime.now().hour
    
# Logic to determine transaction parameters based on weather
def calculate_transaction_amount(temp, weather, hour):
    # Base amount
    amount = 100
        
    # Adjust for temperature (hotter = higher amount)
    temp_factor = temp / 20.0  # normalize around 20°C
    amount *= temp_factor
        
    # Adjust for time of day (higher during business hours)
    if 9 <= hour <= 17:
        amount *= 1.5
    elif hour < 6 or hour > 22:
        amount *= 0.7
        
    # Special case for rain
    if weather in ["Rain", "Thunderstorm"]:
        amount *= 0.8  # Reduce amounts in bad weather
            
    # Cap at reasonable values
    return max(50, min(500, round(amount)))
    
# Calculate gas based on network conditions
def calculate_gas_budget(wind, humid):
    # Base gas
    gas = 50
        
    # Higher gas when windy (metaphor for network congestion)
    gas += round(wind * 5)
        
    # Higher gas during high humidity (another metaphor)
    humidity_factor = humid / 50.0
    gas = gas * humidity_factor
        
    return max(50, min(200, round(gas)))
    
# DEMO MODE: Force execution to true for demo purposes
should_execute = True
    
# Results to be used by the middleware
result = {
    "calculated_amount": calculate_transaction_amount(temperature, weather_condition, current_hour),
    "gas_budget": calculate_gas_budget(wind_speed, humidity),
    "should_execute": should_execute,  # Always true for demo
    "weather_condition": weather_condition,
    "analysis_time": datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
}
"#;

    // 3. Create a time window condition THAT WILL ALWAYS PASS
    // This is a 24/7 condition to ensure the demo works
    let time_condition = TimeCondition {
        condition_type: TimeConditionType::BetweenTimes,
        timestamp: None,
        datetime: None,
        timezone: Some("UTC".to_string()),
        // Setting time window to cover full 24 hours
        start_time: Some("00:00:00".to_string()),
        end_time: Some("23:59:59".to_string()),
        // Allow any day of the week
        weekdays: Some(vec![1, 2, 3, 4, 5, 6, 7]),
        days: None,
        months: None,
    };
    
    // 4. Create our transaction with multi-language, time-based, and external data capabilities
    let mut transaction = Transaction {
        tx_type: TransactionType::Transfer,
        sender: "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(),
        receiver: "0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(),
        amount: 100, // Will be dynamically updated by Python code
        gas_payment: "0xb9fd6cfa2637273ca33dd3aef2f0b0296755889a0ef7f77c9cc95953a96a6302".to_string(),
        gas_budget: 50, // Will be dynamically updated by Python code
        commands: vec!["TransferObjects".to_string()],
        signatures: None,
        timestamp: 0,
        script: None, // Not using JavaScript in this demo
        external_query: None, // Handling external data differently in this demo
        // New fields for enhanced capabilities
        python_code: Some(python_code.to_string()),
        python_params: Some(weather_data.clone()),
        websocket_endpoint: None, //Diasble WebSocket for demo
        websocket_message: None, //Disable WebSocket message
        time_condition: Some(time_condition),
        language: Some("python".to_string()),
    };
    
    // 5. Process the transaction with performance tracking
    println!("Validating weather-based transaction...");
    
    // Validate transaction
    match transaction_handler.validate_transaction(&transaction, metrics.as_mut()).await {
        Ok(true) => {
            println!("Transaction validated successfully.");
            
            // Wrap transaction (marks the end of generation time)
            let wrapped_txn = transaction_handler.wrap_transaction(transaction.clone(), metrics.as_mut())?;
            
            // Execute transaction
            match execution_manager.execute_transaction(&mut transaction, metrics.as_mut()).await {
                Ok(true) => {
                    println!("✅ Weather-based transaction executed successfully!");
                    println!("Final transaction parameters:");
                    println!("  Amount: {}", transaction.amount);
                    println!("  Gas budget: {}", transaction.gas_budget);
                },
                Ok(false) => {
                    println!("❌ Transaction skipped due to conditions not being met.");
                    println!("This is unexpected in demo mode - check your code.");
                },
                Err(e) => {
                    println!("❌ Error during execution: {}", e);
                }
            }
        },
        Ok(false) => println!("❌ Transaction validation failed."),
        Err(e) => println!("❌ Error during validation: {}", e),
    }
    
    // Store metrics if provided
    if let (Some(m), Some(storage)) = (metrics, metrics_storage) {
        storage.add_metrics(m);
    }
    
    println!("\n--- WEATHER-BASED TRANSACTION DEMO COMPLETE ---\n");
    
    Ok(())
}