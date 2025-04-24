//! Main entry point for the SUI Modular Middleware.
//!
//! Handles command-line arguments for running benchmarks, setup tasks, or demos.

// Declare modules first
mod conditions;
mod config;
mod demo;
mod examples;
mod execution;
mod external;
mod languages;
mod metrics;
mod quorum;
mod security;
mod sui;
mod tests;
mod tools;
mod transaction;

// Use statements
use crate::{ // Use crate:: prefix for local modules
    config::{load_submitter_keypair}, // Removed self import
    demo::weather::run_weather_based_transaction_demo,
    examples::{enhanced_flight_insurance::run_enhanced_flight_insurance_demo, flight_delay::run_flight_delay_demo},
    execution::manager::ExecutionManager,
    external::oracle::create_weather_oracle,
    metrics::storage::MetricsStorage,
    quorum::simulation::QuorumSimulation,
    security::{audit::{AuditSeverity, SecurityAuditLog, AuditEventType}, model::generate_security_documentation, verification::create_verification_framework}, // Added AuditEventType
    sui::{byzantine::ByzantineDetector, cross_chain::create_chain_mapper, network::{NetworkManager, NetworkType}, verification::VerificationManager},
    tools::benchmark_suite,
    transaction::{handler::TransactionHandler, types::{Transaction, TransactionType}, utils::process_and_submit_verification},
};
use anyhow::{anyhow, Context, Result};
use clap::{App, Arg};
use std::{
    collections::HashMap, // Added HashMap import
    env,
    error::Error,
    fs,
    path::Path,
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use sui_sdk::{
    rpc_types::SuiObjectDataOptions, // Added import
    types::{
        base_types::{ObjectID, SuiAddress},
        // crypto::SuiKeyPair, // Removed unused import
    },
    SuiClient,
    SuiClientBuilder,
};

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // Parse command-line arguments using Clap
    let matches = App::new("SUI Modular Middleware")
        .version(env!("CARGO_PKG_VERSION")) // Use version from Cargo.toml
        .author("D. Lee <dongguk.lee@kcl.ac.uk>") // Replace with actual author info
        .about("A modular middleware framework for secure off-chain computation with Sui verification.")
        .arg(
            Arg::with_name("benchmark")
                .long("benchmark")
                .help("Run the comprehensive benchmark suite (End-to-End and Byzantine Resilience)."),
        )
        // Removed --security-benchmarks flag as --benchmark runs all
        .arg(
            Arg::with_name("verify-contract-objects")
                .long("verify-contract-objects")
                .help("Verify essential contract objects (Package, Config, AdminCap) exist on Testnet."),
        )
        .arg(
            Arg::with_name("setup-quorum")
                .long("setup-quorum")
                .help("Set up the initial quorum configuration on the Testnet contract (requires AdminCap owner). Requires 10 nodes simulation."),
        )
        .arg(
            Arg::with_name("output-dir")
                .long("output-dir")
                .takes_value(true)
                .default_value("benchmark_results_100_iter") // Default to the final results dir
                .help("Directory to save benchmark results."),
        )
        .arg(
            Arg::with_name("network")
                .long("network")
                .takes_value(true)
                .possible_values(&["testnet", "devnet", "local"]) // Add mainnet later if needed
                .default_value("testnet")
                .help("Specify the Sui network to connect to (testnet, devnet, local)."),
        )
        .get_matches();

    let output_dir = matches.value_of("output-dir").unwrap(); // Clap ensures default
    let network_arg = matches.value_of("network").unwrap();

    println!("--- SUI Modular Middleware --- Version: {} ---", env!("CARGO_PKG_VERSION"));

    // Handle special commands first (verify, setup)
    if matches.is_present("verify-contract-objects") {
        println!("Verifying essential contract objects on {}...", network_arg);
        let rpc_url = match network_arg {
            "testnet" => config::SUI_TESTNET_RPC,
            // Add URLs for devnet/local if needed, or use a NetworkManager approach
            _ => return Err(anyhow!("Network '{}' RPC URL not configured for verification.", network_arg).into()),
        };
        let client = SuiClientBuilder::default().build(rpc_url).await?;
        // Call a hypothetical verification function (needs implementation if TransactionHandler one removed)
        match verify_contract_setup(&client).await {
            Ok(_) => println!("✅ Contract objects verified successfully on {}!", network_arg),
            Err(e) => {
                eprintln!("❌ Contract object verification failed: {}", e);
                // Return error to indicate failure
                 return Err(e.into());
            }
        }
        return Ok(()); // Exit after verification
    }

    if matches.is_present("setup-quorum") {
         println!("Attempting to set up quorum configuration on {}...", network_arg);
         let rpc_url = match network_arg {
            "testnet" => config::SUI_TESTNET_RPC,
            _ => return Err(anyhow!("Network '{}' RPC URL not configured for quorum setup.", network_arg).into()),
         };
         let sui_client = SuiClientBuilder::default().build(rpc_url).await?;

         // Quorum setup requires interaction; consider moving this to a dedicated tool/script
         // or carefully implementing it here.
         match setup_onchain_quorum_config(&sui_client).await {
             Ok(_) => println!("✅ Quorum configuration set up successfully on {}!", network_arg),
             Err(e) => {
                 eprintln!("❌ Failed to set up quorum configuration: {:#}", e); // Detailed error
                  return Err(e.into());
             }
         }
         return Ok(()); // Exit after setup
     }

    // If benchmark flag is provided, run the benchmark suite.
    if matches.is_present("benchmark") {
        println!(
            "Running comprehensive benchmarks on {}. Output will be saved to: {}",
            network_arg, output_dir
        );
        // Pass network info if benchmarks need it, otherwise assume testnet focus
        return benchmark_suite::run_comprehensive_benchmarks(output_dir).await;
    }

    // --- Default Execution: Run Demos --- 
    println!(
        "Starting middleware in DEMO mode on {}...",
        network_arg
    );
    dotenv::dotenv().ok(); // Load .env file if present

    // Initialize shared components
    let security_audit_log = Arc::new(SecurityAuditLog::new());
    let network_type = match network_arg {
        "devnet" => NetworkType::Devnet,
        "local" => NetworkType::Local,
        _ => NetworkType::Testnet, // Default to testnet
    };

    println!("Initializing components for network: {:?}...", network_type);
    let network_manager = Arc::new(NetworkManager::new(network_type.clone()).await?);
    let rpc_url = network_manager.get_active_rpc_url()?;
    let verification_manager = VerificationManager::new(&rpc_url);
    let byzantine_detector = Arc::new(ByzantineDetector::new(
        network_manager.get_active_config().get_rpc_endpoints().clone(),
        Some(security_audit_log.clone()),
        None,
        None,
    ));
    // Unused variable warnings suppressed with `_`
    let _chain_mapper = create_chain_mapper(network_manager.clone(), Some(security_audit_log.clone()))?;
    let _weather_oracle = create_weather_oracle(
        Some(security_audit_log.clone()),
        Some(Duration::from_secs(300)), // Cache duration
        Some(Duration::from_secs(60)), // Update interval
    )?;
    let _verification_framework = create_verification_framework(Some(security_audit_log.clone()));

    security_audit_log.log_network(
        "main",
        &format!("Middleware demo mode started on network: {:?}", network_type),
        network_manager.get_active_config().get_chain_id().as_deref(),
        AuditSeverity::Info,
    )?;

    let metrics_storage = Arc::new(MetricsStorage::new());
    // Quorum simulation (e.g., 5 nodes for demos)
    let quorum_sim = Arc::new(QuorumSimulation::create_with_random_nodes(5)?);

    // Load keys and objects needed for demos
    // Use load_submitter_keypair which handles env vars and fallbacks
    let submitter_keypair = load_submitter_keypair().context("Failed to load submitter keypair for demos")?;
    let submitter_address = SuiAddress::from(&submitter_keypair.public());
    // Gas object ID also loaded via env var or config constant
    let gas_object_id = ObjectID::from_str(config::SUBMITTER_GAS_OBJECT_ID)
         .context("Invalid SUBMITTER_GAS_OBJECT_ID in config or env var")?;

    println!("Demo Submitter Address: {}", submitter_address);
    println!("Demo Gas Object ID: {}", gas_object_id);

    // Initialize core components
    let sui_client = Arc::new(SuiClientBuilder::default().build(&rpc_url).await?);
    let transaction_handler = Arc::new(
        TransactionHandler::new(
            load_submitter_keypair().context("Failed to load keypair for TransactionHandler")?, // Load fresh keypair
            Some(verification_manager.clone()), // Clone VM if needed
            Some(security_audit_log.clone()),
            Some(byzantine_detector.clone()),
            quorum_sim.clone(),
            sui_client.clone(),
        )
        .await?,
    );
    let execution_manager = Arc::new(ExecutionManager::new(
        Some(verification_manager.clone()), // Pass clone of VM
        Some(network_manager.clone()),
        Some(security_audit_log.clone()),
    ));
    // Unused fallback manager
    // let _fallback_manager = Arc::new(FallbackManager::new());
    // Unused sequencing layer
    // let _sequencing_layer = Arc::new(SequencingLayer::new());

    println!("\n--- Running Middleware Demos ---");
    println!("(These demos showcase different transaction types and execution paths)");

    // Generate and save security documentation (can be moved elsewhere)
    // let _security_model = security::model::SecurityModel::new(); // Removed unused var
    let security_docs = generate_security_documentation();
    let docs_dir = "docs";
    fs::create_dir_all(docs_dir)?;
    let security_docs_path = Path::new(docs_dir).join("security-model-generated.md");
    fs::write(&security_docs_path, security_docs)?;
    println!("Generated security model documentation: {}", security_docs_path.display());

    // --- Run Demos --- 
    // Note: The process_and_submit_verification utility now orchestrates the flow.
    // It needs the submitter keypair and gas object ID.

    // JS Demo
    let js_script = r#"({"shouldExecute": true, "outcome": "js_ok"})"#; // Use raw string literal
    let js_txn = Transaction {
        tx_type: TransactionType::Custom("js_demo".to_string()),
        sender: submitter_address.to_string(),
        receiver: submitter_address.to_string(),
        amount: 0,
        gas_payment: gas_object_id.to_string(),
        gas_budget: 100_000_000, // Use consistent high budget
        commands: vec![],
        signatures: None,
        timestamp: 0, // Timestamp handled by Transaction::new or digest
        script: Some(js_script.to_string()),
        language: Some("javascript".to_string()),
        external_query: None,
        python_code: None,
        python_params: None,
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
    };
    if let Err(e) = process_and_submit_verification(
        &js_txn,
        "JavaScript Demo",
        &transaction_handler,
        &execution_manager,
        Some(&metrics_storage),
        &security_audit_log,
        &load_submitter_keypair().context("Failed to load keypair for JS Demo")?,
        &gas_object_id,
    ).await {
        eprintln!("ERROR in JavaScript Demo: {:#}", e);
    }

    // Python Demo
    let python_script = r#"result = {"should_execute": True, "outcome": "python_ok"}"#;
    let python_txn = Transaction {
        tx_type: TransactionType::Custom("python_demo".to_string()),
        sender: submitter_address.to_string(),
        receiver: submitter_address.to_string(),
        amount: 0,
        gas_payment: gas_object_id.to_string(),
        gas_budget: 100_000_000,
        commands: vec![],
        signatures: None,
        timestamp: 0,
        script: None,
        language: Some("python".to_string()),
        python_code: Some(python_script.to_string()),
        python_params: None,
        external_query: None,
        websocket_endpoint: None,
        websocket_message: None,
        time_condition: None,
    };
     if let Err(e) = process_and_submit_verification(
        &python_txn,
        "Python Demo",
        &transaction_handler,
        &execution_manager,
        Some(&metrics_storage),
        &security_audit_log,
        &load_submitter_keypair().context("Failed to load keypair for Python Demo")?,
        &gas_object_id,
    ).await {
         eprintln!("ERROR in Python Demo: {:#}", e);
     }

    // Weather Demo
     if let Err(e) = run_weather_based_transaction_demo(
        &transaction_handler,
        &execution_manager,
        Some(&metrics_storage),
        &security_audit_log,
        &load_submitter_keypair().context("Failed to load keypair for Weather Demo")?,
        &gas_object_id,
    ).await {
         eprintln!("ERROR in Weather Demo: {:#}", e);
     }

    // Flight Delay Demo
    if let Err(e) = run_flight_delay_demo(
        &transaction_handler,
        &execution_manager,
        Some(&metrics_storage),
        &security_audit_log,
        &load_submitter_keypair().context("Failed to load keypair for Flight Delay Demo")?,
        &gas_object_id,
    ).await {
        eprintln!("ERROR in Flight Delay Demo: {:#}", e);
    }

    // Enhanced Flight Insurance Demo
    // Needs Arc<VerificationManager> and Arc<NetworkManager>
    let vm_arc = Arc::new(verification_manager); // Create Arc for this call
    let nm_arc = network_manager; // Already an Arc
    if let Err(e) = run_enhanced_flight_insurance_demo(
        &transaction_handler,
        &execution_manager,
        Some(&metrics_storage),
        &security_audit_log,
        &vm_arc, // Pass Arc
        &nm_arc, // Pass Arc
        &load_submitter_keypair().context("Failed to load keypair for Flight Insurance Demo")?,
        &gas_object_id,
    ).await {
        eprintln!("ERROR in Enhanced Flight Insurance Demo: {:#}", e);
    }

    // --- Deprecated Demo Calls --- 
    // demonstrate_security_verification(&_verification_framework)?;
    // demonstrate_cross_chain_mapping(&demo_tx, &_chain_mapper).await... ;
    // Removed deprecated metric saving/printing calls
    println!("\n--- DEMOS COMPLETE ---");
    println!("(Note: Old PerformanceMetrics are deprecated; use benchmark results for analysis.)");

    // Print final audit summary
    print_audit_summary(&security_audit_log);

    Ok(())
}

/// Placeholder function to verify contract setup (replace with actual logic or remove)
async fn verify_contract_setup(client: &SuiClient) -> Result<()> {
    println!("Placeholder verification: Check config.rs for required object IDs.");
    let package_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_PACKAGE_ID)
        .context("Reading package ID from config")?;
    client.read_api().get_object_with_options(package_id, SuiObjectDataOptions::new()).await // Used import
        .map_err(|e| anyhow!("Failed to get package object {}: {}", package_id, e))
        .and_then(|resp| if resp.data.is_some() { Ok(()) } else { Err(anyhow!("Package object {} not found", package_id)) })?;

    let config_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_CONFIG_OBJECT_ID)
         .context("Reading config ID from config")?;
    client.read_api().get_object_with_options(config_id, SuiObjectDataOptions::new()).await // Used import
         .map_err(|e| anyhow!("Failed to get config object {}: {}", config_id, e))
         .and_then(|resp| if resp.data.is_some() { Ok(()) } else { Err(anyhow!("Config object {} not found", config_id)) })?;

    let admin_cap_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_ADMIN_CAP_ID)
         .context("Reading admin cap ID from config")?;
    client.read_api().get_object_with_options(admin_cap_id, SuiObjectDataOptions::new()).await // Used import
         .map_err(|e| anyhow!("Failed to get admin cap object {}: {}", admin_cap_id, e))
         .and_then(|resp| if resp.data.is_some() { Ok(()) } else { Err(anyhow!("Admin cap object {} not found", admin_cap_id)) })?;

    Ok(())
}

/// Placeholder function for setting up quorum config on-chain (replace or remove)
async fn setup_onchain_quorum_config(_client: &SuiClient) -> Result<()> { // Prefixed client with _
     println!("Placeholder setup: This action requires interaction and careful implementation.");
     println!("Ensure you own the AdminCap ({}) and have gas.", config::VERIFICATION_CONTRACT_ADMIN_CAP_ID);
     println!("Simulating 10 nodes with 2/3+1 threshold for setup.");
     // Placeholder logic: Load keys, build PTB, submit
     // This requires importing TransactionHandler and related types, or replicating the logic.
     // For now, just return an error indicating it's not implemented here.
     Err(anyhow!("On-chain quorum setup not fully implemented in main.rs. Use TransactionHandler method or dedicated script."))
}

/// Prints a summary of recorded security audit events.
fn print_audit_summary(security_audit_log: &Arc<SecurityAuditLog>) {
     let events = security_audit_log.get_events();
     println!("\n--- Security Audit Summary ---");
     println!("Total Events Recorded: {}", events.len());

     let mut event_types: HashMap<AuditEventType, usize> = HashMap::new(); // Used import
     let mut event_severities: HashMap<AuditSeverity, usize> = HashMap::new();

     for event in &events {
         *event_types.entry(event.event_type.clone()).or_insert(0) += 1;
         *event_severities.entry(event.severity.clone()).or_insert(0) += 1;
     }

     println!("\nEvents by Type:");
     if event_types.is_empty() {
         println!("  (No events)");
     } else {
         let mut sorted_types: Vec<_> = event_types.into_iter().collect();
         sorted_types.sort_by_key(|k| format!("{:?}", k.0));
         for (event_type, count) in sorted_types {
             println!("  - {:<25}: {}", format!("{:?}", event_type), count);
         }
     }

     println!("\nEvents by Severity:");
     if event_severities.is_empty() {
         println!("  (No events)");
     } else {
        let mut sorted_severities: Vec<_> = event_severities.into_iter().collect();
        sorted_severities.sort_by_key(|k| k.0.clone());
        for (severity, count) in sorted_severities {
            println!("  - {:<10}: {}", format!("{:?}", severity).to_uppercase(), count);
        }
     }
     println!("---------------------------");
}
