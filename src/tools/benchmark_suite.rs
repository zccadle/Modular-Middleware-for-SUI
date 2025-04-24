// tools/benchmark_suite.rs

//! Benchmark Suite for SUI Modular Middleware
//!
//! This module provides comprehensive benchmarking functionality for the
//! middleware framework, focusing on end-to-end performance and Byzantine resilience.
//! 
//! The suite simulates a middleware quorum, tests signature collection under varying
//! Byzantine fault conditions, and measures performance of L1 verification transactions.
//!
//! # Environment Setup
//! 
//! Before running benchmarks, ensure proper configuration in `config.rs`
//! (or via environment variables):
//! - Set SUBMITTER_ADDRESS and SUBMITTER_KEYPAIR_BASE64 environment variables.
//! - Configure deployed contract addresses (VERIFICATION_CONTRACT_PACKAGE_ID, etc.).
//!
//! # Usage Example
//!
//! ```bash
//! # Run benchmarks with output directory:
//! cargo run --release -- --benchmark --output-dir benchmark_results_100_iter
//! ```

// Standard library imports
use anyhow::{anyhow, Context, Result};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::fs;
use chrono;
use rand::Rng;

// Sui SDK imports
use sui_sdk::{
    rpc_types::{
        SuiObjectDataOptions,
        SuiTransactionBlockResponseOptions,
        SuiExecutionStatus,
        SuiTransactionBlockEffectsAPI,
    },
    SuiClient,
    SuiClientBuilder,
    types::{
        base_types::{ObjectID, SuiAddress},
        crypto::{SuiKeyPair, Signature as SdkSignature},
        object::Owner,
        Identifier,
    },
};

// Sui Types imports
use sui_types::{
    quorum_driver_types::ExecuteTransactionRequestType,
    programmable_transaction_builder::ProgrammableTransactionBuilder,
    transaction::{TransactionData, Transaction, CallArg, ObjectArg},
    crypto::EncodeDecodeBase64,
};

// Shared Crypto imports
use shared_crypto::intent::{Intent, IntentMessage};

// Crate specific imports
use crate::{
    config::{self, SUI_TESTNET_RPC},
    config::load_submitter_keypair as load_keypair,
    execution::manager::ExecutionManager,
    metrics::{
        performance::ComponentBenchmark,
        storage::MetricsStorage,
    },
    quorum::simulation::QuorumSimulation,
    security::audit::SecurityAuditLog,
    sui::{byzantine::ByzantineDetector, network::{NetworkManager, NetworkType}, verification::VerificationManager},
    transaction::types::Transaction as MiddlewareTransaction,
};

/// Number of iterations to run per benchmark scenario.
pub const BENCHMARK_ITERATIONS: usize = 100;

/// Byzantine percentages to test (as decimals).
pub const BYZANTINE_PERCENTAGES: [f64; 6] = [0.0, 0.1, 0.2, 0.33, 0.5, 0.75];

/// Main entry point for running comprehensive benchmarks.
/// 
/// Runs all benchmark scenarios (End-to-End, Byzantine Resilience)
/// and saves results to the specified directory.
///
/// # Parameters
/// * `output_dir` - Directory to save benchmark results (e.g., "benchmark_results_100_iter").
///
/// # Returns
/// Result indicating success or error.
pub async fn run_comprehensive_benchmarks(output_dir: &str) -> Result<(), Box<dyn std::error::Error>> {
    let start_time = Instant::now();
    println!("Running comprehensive middleware benchmarks...");
    println!("Output Directory: {}", output_dir);
    println!("Iterations per scenario: {}", BENCHMARK_ITERATIONS);
    
    // Create output directory if it doesn't exist.
    fs::create_dir_all(output_dir)?;
    let metrics_storage = Arc::new(MetricsStorage::new());

    // Initialize shared components.
    let security_audit_log = Arc::new(SecurityAuditLog::new());
    let network_manager = Arc::new(crate::sui::network::NetworkManager::new(NetworkType::Testnet).await?);
    
    let rpc_url = network_manager.get_active_rpc_url()?;
    // Create VerificationManager directly, no Arc needed for ExecutionManager::new
    let verification_manager = VerificationManager::new(&rpc_url);
    let byzantine_detector = Arc::new(ByzantineDetector::new(
        network_manager.get_active_config().get_rpc_endpoints().clone(),
        Some(security_audit_log.clone()),
        None,
        None,
    ));
    let execution_manager = Arc::new(ExecutionManager::new(
        Some(verification_manager.clone()), // Pass clone of VM
        Some(network_manager.clone()),
        Some(security_audit_log.clone()),
    ));

    // Create Quorum Simulation (fixed size n=5 for these benchmarks).
    let quorum_simulation = Arc::new(QuorumSimulation::create_with_random_nodes(5)?);

    // Load submitter keypair.
    let submitter_keypair = load_keypair()?;
    
    // --- Connect to Sui RPC ---
    let sui_client: Arc<SuiClient> = Arc::new(SuiClientBuilder::default().build(SUI_TESTNET_RPC).await?);
        
    // Parse gas object ID from config.
    let gas_object_id = ObjectID::from_str(config::SUBMITTER_GAS_OBJECT_ID)?;

    // --- Run Benchmark Scenarios (Fixed n=5) --- 

    println!("\nRunning End-to-End Performance Benchmark (n=5)...");
    run_end_to_end_performance(
        output_dir,
        metrics_storage.clone(),
        execution_manager.clone(),
        security_audit_log.clone(),
        sui_client.clone(),
        quorum_simulation.clone(),
        &submitter_keypair, // Pass reference to keypair
        &gas_object_id,
    ).await?;
    
    // Run Byzantine resilience testing.
    println!("\nRunning Byzantine Resilience Benchmarks (n=5)...");
        run_byzantine_resilience(
            output_dir,
            metrics_storage.clone(),
            execution_manager.clone(),
            security_audit_log.clone(),
            quorum_simulation.clone(),
        &submitter_keypair, // Pass reference to keypair
        &gas_object_id,
        ).await?;
    
    // --- Save Benchmark Results ---
    let results_file = format!("{}/refactored_benchmarks.json", output_dir);
    metrics_storage.save_benchmarks_to_json_file(&results_file)?;
    metrics_storage.print_benchmark_summary();
    
    // --- Generate Summary File --- 
    let summary_path = format!("{}/benchmark_summary.txt", output_dir);
    let mut summary = String::new();
    summary.push_str("Suimodular Comprehensive Benchmarks Summary\n");
    summary.push_str(&format!("Date: {}\n", chrono::Local::now().to_rfc2822()));
    summary.push_str(&format!("Results Directory: {}\n", output_dir));
    let total_duration = start_time.elapsed();
    summary.push_str(&format!("Total Duration: {:?}\n", total_duration));
    summary.push_str("Quorum Size: n=5, Threshold t=4 (2f+1 for f=1)\n");
    summary.push_str(&format!("Iterations per scenario: {}\n", BENCHMARK_ITERATIONS));
    summary.push_str(&format!("Byzantine percentages tested: {:?}\n", BYZANTINE_PERCENTAGES.iter().map(|p| format!("{:.1}%", p * 100.0)).collect::<Vec<_>>() ));
    fs::write(&summary_path, summary)?;
    println!("Benchmark summary written to {}", summary_path);
    
    println!("\nBenchmarks completed successfully!");
    println!("Results JSON written to {}", results_file);
    println!("Summary text written to {}", summary_path);
    
    Ok(())
}

/// Runs the end-to-end performance benchmark.
///
/// Measures baseline performance with no Byzantine faults (0%).
/// Simulates payload generation, quorum signing, and L1 verification submission.
async fn run_end_to_end_performance(
    _output_dir: &str, // Parameter kept for consistency, but not used directly here
    metrics_storage: Arc<MetricsStorage>,
    _execution_manager: Arc<ExecutionManager>, // Not directly used for submission logic here
    _security_audit_log: Arc<SecurityAuditLog>, // Not directly used for submission logic here
    sui_client: Arc<SuiClient>,
    quorum_simulation: Arc<QuorumSimulation>,
    submitter_sui_keypair: &SuiKeyPair, // Take reference
    gas_object_id: &ObjectID,
) -> Result<(), anyhow::Error> {
    // Implementation largely unchanged, comments refined...
    println!("  Running End-to-End Performance Benchmark (0% Byzantine)...");
    let config_name = "end_to_end_performance_n5";
    let security_level = "0_percent_byzantine";
    let mut benchmark = ComponentBenchmark::new(config_name, security_level, BENCHMARK_ITERATIONS as u32);
    benchmark.add_config("num_transactions", &BENCHMARK_ITERATIONS.to_string());
    benchmark.add_config("quorum_size", &quorum_simulation.keypairs.len().to_string());
    benchmark.add_config("byzantine_percentage", "0.0");

    let mut successful_submissions = 0;
    let mut successful_confirmations = 0;

    let l1_submission_address = SuiAddress::from(&submitter_sui_keypair.public());
    // No need for Arc here as we take a reference
    // let submitter_keypair_arc = Arc::new(submitter_sui_keypair.clone()); 

    // Get contract details from config
    let package_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_PACKAGE_ID)?;
    let module_name = Identifier::from_str(config::VERIFICATION_CONTRACT_MODULE)?;
    let function_name = Identifier::from_str(config::VERIFICATION_CONTRACT_FUNCTION)?;
    let config_object_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_CONFIG_OBJECT_ID)?;

    println!("    Using Submitter Address: {}", l1_submission_address);
    println!("    Using Gas Object ID: {}", gas_object_id);
    println!("    Using Config Object ID: {}", config_object_id);

    for i in 0..BENCHMARK_ITERATIONS {
        let iteration_start = Instant::now();

        // 1. Generate unique payload for this iteration
        let processing_start = Instant::now();
        let mut unique_payload = vec![0u8; 32]; // Simulate a unique payload hash
        let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_millis();
        unique_payload[0..8].copy_from_slice(&i.to_le_bytes());
        unique_payload[8..24].copy_from_slice(&now_ms.to_le_bytes());
        rand::thread_rng().fill(&mut unique_payload[24..]);

        // 2. Request signatures from the quorum simulation (0% Byzantine here)
        let quorum_size = quorum_simulation.keypairs.len();
        let quorum_threshold = quorum_simulation.get_threshold(); // Use helper

        let all_signatures = match quorum_simulation.request_signatures(unique_payload.clone()).await {
             Ok(sigs) => sigs,
             Err(e) => { 
                 eprintln!("ERROR: Failed to get signatures in iteration {}: {}", i, e);
                 // Decide how to handle: skip iteration? record failure?
                 // Skipping for now, but might want failure counter.
                 continue;
             }
         };
        
        // Ensure enough signatures were obtained (should always pass with 0% Byzantine)
        if all_signatures.len() < quorum_threshold {
             eprintln!("ERROR: Not enough signatures ({}/{}) obtained for threshold ({}) in iteration {} (0% Byzantine)", 
                      all_signatures.len(), quorum_size, quorum_threshold, i);
             continue;
         }
        
        // Extract signatures (bytes) needed for the Move contract call
        let signatures_for_move: Vec<Vec<u8>> = all_signatures.into_iter()
            .take(quorum_threshold) // Take exactly threshold number of signatures
            .map(|(bytes, _is_valid)| bytes) // Extract bytes, ignore validity flag (all should be valid)
            .collect();
        
        let signing_duration = processing_start.elapsed();
        benchmark.record_operation("middleware_processing_and_prep", signing_duration.as_millis() as u64);
        benchmark.record_operation("quorum_signing", signing_duration.as_millis() as u64);

        // 3. Prepare and submit transaction to L1 for verification
                let l1_submit_start = Instant::now();

        // Serialize signatures for Move contract argument
        let signatures_bcs = match bcs::to_bytes(&signatures_for_move) {
             Ok(bytes) => bytes,
             Err(e) => { 
                 eprintln!("ERROR: Failed to serialize signatures in iteration {}: {}", i, e);
                 continue;
             }
         };

        // Fetch the latest gas object reference
        let gas_object_response = sui_client.read_api().get_object_with_options(
            *gas_object_id,
            SuiObjectDataOptions::new().with_owner().with_previous_transaction()
        ).await.context(format!("Failed to fetch gas object {}", gas_object_id))?;

        let gas_object_ref = gas_object_response.object_ref_if_exists()
            .ok_or_else(|| anyhow!("Gas object {} not found or deleted", gas_object_id))?;

        // Fetch the latest Config Object version (it's a shared object)
        let config_object_response = sui_client.read_api().get_object_with_options(
            config_object_id,
            SuiObjectDataOptions::new().with_owner()
        ).await.context(format!("Failed to fetch config object {}", config_object_id))?;

        let config_object_version = config_object_response.owner()
             .and_then(|owner_enum| match owner_enum {
                 Owner::Shared { initial_shared_version } => Some(initial_shared_version),
                 _ => None,
             })
            .ok_or_else(|| anyhow!("Could not get initial shared version for config object {}", config_object_id))?;

        // Fetch the current reference gas price
        let reference_gas_price = sui_client.read_api().get_reference_gas_price().await?;

        // Serialize the unique payload for Move contract argument
        let payload_bcs = bcs::to_bytes(&unique_payload)
            .context("Failed to serialize payload")?;

        // Build the Programmable Transaction Block (PTB)
        let pt = {
             let mut builder = ProgrammableTransactionBuilder::new();
             // Define arguments for the Move call
             let config_call_arg = CallArg::Object(ObjectArg::SharedObject { id: config_object_id, initial_shared_version: config_object_version, mutable: true });
             let payload_call_arg = CallArg::Pure(payload_bcs);
             let sigs_call_arg = CallArg::Pure(signatures_bcs);
             // Create the Move call to the verification function
             builder.move_call( package_id, module_name.clone(), function_name.clone(), vec![], vec![config_call_arg, payload_call_arg, sigs_call_arg] )?;
             builder.finish()
         };

        // Create the transaction data
        let tx_data = TransactionData::new_programmable(
             l1_submission_address,
             vec![gas_object_ref],
             pt,
             100_000_000, // Increased gas budget for safety
             reference_gas_price
         );

        // Sign the transaction data
        let intent = Intent::sui_transaction();
        let intent_message = IntentMessage::new(intent, tx_data.clone());
        // Pass the keypair reference directly
        let sdk_signature = SdkSignature::new_secure(&intent_message, submitter_sui_keypair);

        // Create the final transaction object
        let transaction = Transaction::from_data(tx_data, vec![sdk_signature.into()]);

        // Execute the transaction on the Sui network
        let transaction_response_result = sui_client.quorum_driver_api().execute_transaction_block(
            transaction,
            SuiTransactionBlockResponseOptions::new().with_effects(), // Request effects to check status
            Some(ExecuteTransactionRequestType::WaitForLocalExecution) // Wait for node execution
                ).await;

                let l1_submission_duration = l1_submit_start.elapsed();
                benchmark.record_operation("l1_submission", l1_submission_duration.as_millis() as u64);

        // Process the transaction response
        match transaction_response_result {
            Ok(response) => {
                 let l1_digest = response.digest;
                 successful_submissions += 1;

                 if let Some(effects) = response.effects {
                      match effects.status() {
                           SuiExecutionStatus::Success => {
                                // Simulate confirmation time (e.g., small delay)
                    let l1_confirm_start = Instant::now();
                                tokio::time::sleep(Duration::from_millis(50)).await; // Simulate network confirmation latency
                                let l1_confirmation_duration = l1_confirm_start.elapsed();
                                benchmark.record_operation("l1_confirmation", l1_confirmation_duration.as_millis() as u64);
                                successful_confirmations += 1;
                           },
                           SuiExecutionStatus::Failure { error } => {
                                eprintln!("ERROR: L1 transaction {} failed: {:?}", l1_digest, error);
                                benchmark.record_operation("l1_confirmation", 0); // Record 0 for failure
                           }
                    }
                } else {
                      eprintln!("WARNING: L1 transaction {} succeeded but had no effects reported.", l1_digest);
                      benchmark.record_operation("l1_confirmation", 0); // Treat as failure if effects missing
                 }
            },
            Err(e) => {
                 eprintln!("ERROR: L1 submission failed: {:?}", e);
                 benchmark.record_operation("l1_confirmation", 0); // Record 0 for submission error
            }
        }

        let total_iteration_time = iteration_start.elapsed();
        benchmark.record_operation("total_iteration", total_iteration_time.as_millis() as u64);
        
        // Progress indicator
        if (i + 1) % (BENCHMARK_ITERATIONS / 10).max(1) == 0 || i == BENCHMARK_ITERATIONS - 1 {
            println!("    Iteration {}/{} complete ({:?}), Current Success Rate: {:.1}%", 
                    i + 1, BENCHMARK_ITERATIONS, total_iteration_time, 
                    (successful_confirmations as f64 / (i + 1) as f64) * 100.0);
        }
    }

    // Finalize and record benchmark results
    let success_rate = if BENCHMARK_ITERATIONS > 0 { successful_confirmations as f64 / BENCHMARK_ITERATIONS as f64 } else { 0.0 };
    benchmark.add_config("verification_success_rate", &format!("{:.3}", success_rate));
    benchmark.end();
    benchmark.print_summary();
    metrics_storage.add_benchmark(benchmark);

    println!("  End-to-End Performance Benchmark completed.");
    Ok(())
}

/// Runs the Byzantine resilience benchmark.
///
/// Tests middleware resilience by injecting Byzantine behavior (invalid signatures, non-responses)
/// into the simulated quorum at varying percentages.
async fn run_byzantine_resilience(
    _output_dir: &str, // Parameter kept for consistency
    metrics_storage: Arc<MetricsStorage>,
    _execution_manager: Arc<ExecutionManager>, // Not directly used
    _security_audit_log: Arc<SecurityAuditLog>, // Not directly used
    base_quorum_simulation: Arc<QuorumSimulation>,
    submitter_sui_keypair: &SuiKeyPair,
    gas_object_id: &ObjectID,
) -> Result<(), anyhow::Error> {
    println!("  Starting Byzantine Resilience Benchmarks (n=5) with percentages: {:?}", 
             BYZANTINE_PERCENTAGES.iter().map(|p| format!("{:.1}%", p * 100.0)).collect::<Vec<_>>());

    let sui_client: Arc<SuiClient> = Arc::new(SuiClientBuilder::default().build(SUI_TESTNET_RPC).await?);
    let l1_submission_address = SuiAddress::from(&submitter_sui_keypair.public());
    let submitter_keypair_arc = Arc::new(submitter_sui_keypair.clone());

    // Get contract details from config
    let package_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_PACKAGE_ID)?;
    let module_name = Identifier::from_str(config::VERIFICATION_CONTRACT_MODULE)?;
    let function_name = Identifier::from_str(config::VERIFICATION_CONTRACT_FUNCTION)?;
    let config_object_id = ObjectID::from_str(config::VERIFICATION_CONTRACT_CONFIG_OBJECT_ID)?;

    // Test each Byzantine percentage
    for &percentage in BYZANTINE_PERCENTAGES.iter() {
        println!("    Running Benchmark with {:.1}% Byzantine Nodes...", percentage * 100.0);

        // Create a new quorum simulation instance for this percentage, cloning base keys
        let keypairs_clone = base_quorum_simulation.keypairs.iter().map(|kp| {
            // Decoding/Encoding ensures a fresh copy, preventing potential issues with Arc references
            SuiKeyPair::decode_base64(&kp.encode_base64()).expect("Failed to re-encode/decode keypair")
        }).collect();
        let mut current_sim = QuorumSimulation::new(keypairs_clone);
        current_sim.set_byzantine_percentage(percentage); // Set the fault rate
        let current_sim_arc = Arc::new(current_sim);

        // Create benchmark component for this scenario
        let config_name = format!("byzantine_resilience_n5_{:.0}pct", percentage * 100.0);
        let security_level = format!("{:.1}%_byzantine", percentage * 100.0);
        let mut benchmark = ComponentBenchmark::new(&config_name, &security_level, BENCHMARK_ITERATIONS as u32);
        benchmark.add_config("num_transactions", &BENCHMARK_ITERATIONS.to_string());
        benchmark.add_config("quorum_size", &current_sim_arc.keypairs.len().to_string());
        benchmark.add_config("byzantine_percentage", &percentage.to_string());

        // Initialize counters for success and failure reasons
        let mut successful_confirmations = 0;
        let mut failure_not_enough_signatures = 0;
        let mut failure_l1_execution = 0;
        let mut failure_l1_rpc = 0;
        let mut failure_signing_error = 0;

        // Run iterations for this percentage
        for i in 0..BENCHMARK_ITERATIONS {
            let iteration_start = Instant::now();
            
            // 1. Generate unique payload
            let processing_start = Instant::now();
            let mut unique_payload = vec![0u8; 32];
            let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_millis();
            unique_payload[0..8].copy_from_slice(&i.to_le_bytes());
            unique_payload[8..24].copy_from_slice(&now_ms.to_le_bytes());
            rand::thread_rng().fill(&mut unique_payload[24..]);

            // 2. Request signatures from quorum (with simulated Byzantine behavior)
            let quorum_size = current_sim_arc.keypairs.len();
            let quorum_threshold = current_sim_arc.get_threshold();

            let all_signatures_with_validity_result = current_sim_arc.request_signatures(unique_payload.clone()).await;
            
            let all_signatures_with_validity = match all_signatures_with_validity_result {
                Ok(sigs) => sigs,
                Err(e) => {
                    eprintln!("ERROR: Signing payload failed in iteration {} ({}% Byzantine): {}", 
                             i, percentage * 100.0, e);
                    failure_signing_error += 1;
                    // Record zero timings for failed signing attempts
                    benchmark.record_operation("middleware_processing_and_prep", processing_start.elapsed().as_millis() as u64);
                    benchmark.record_operation("quorum_signing", processing_start.elapsed().as_millis() as u64);
                    benchmark.record_operation("l1_submission", 0);
                    benchmark.record_operation("l1_confirmation", 0);
                    benchmark.record_operation("total_iteration", iteration_start.elapsed().as_millis() as u64);
                    continue; // Skip to next iteration
                }
            };

            let num_signatures_obtained = all_signatures_with_validity.len();

            // Check if enough signatures were gathered (even if some are invalid)
            if num_signatures_obtained < quorum_threshold {
                 // This is expected when Byzantine % is high enough to prevent reaching threshold
                 println!("INFO: Not enough signatures ({}/{}) for threshold ({}) in iteration {} ({}% Byzantine). Recording failure.", 
                          num_signatures_obtained, quorum_size, quorum_threshold, i, percentage * 100.0);
                 failure_not_enough_signatures += 1;
                 // Record appropriate timings
                 benchmark.record_operation("middleware_processing_and_prep", processing_start.elapsed().as_millis() as u64);
                 benchmark.record_operation("quorum_signing", processing_start.elapsed().as_millis() as u64);
                 benchmark.record_operation("l1_submission", 0); // No submission attempted
                 benchmark.record_operation("l1_confirmation", 0);
                 benchmark.record_operation("total_iteration", iteration_start.elapsed().as_millis() as u64);
                 continue; // Skip to next iteration
             }

             // Extract signature bytes for L1 submission (take threshold amount)
             let signatures_for_move: Vec<Vec<u8>> = all_signatures_with_validity
                 .into_iter()
                 .take(quorum_threshold)
                 .map(|(bytes, _is_valid)| bytes) // Extract bytes, validity checked on-chain
                 .collect();

             // Record timings for middleware part
             let signing_duration = processing_start.elapsed();
             benchmark.record_operation("middleware_processing_and_prep", signing_duration.as_millis() as u64);
             benchmark.record_operation("quorum_signing", signing_duration.as_millis() as u64);

             // 3. Prepare and submit L1 transaction
             let l1_submit_start = Instant::now();

             // Serialize signatures for Move contract
             let signatures_bcs = match bcs::to_bytes(&signatures_for_move) {
                  Ok(bytes) => bytes,
                  Err(e) => { 
                      eprintln!("ERROR: Failed to serialize signatures in iteration {}: {}", i, e);
                      // Consider this a form of signing error for counting purposes?
                      failure_signing_error += 1; 
                      benchmark.record_operation("l1_submission", 0);
                      benchmark.record_operation("l1_confirmation", 0);
                      benchmark.record_operation("total_iteration", iteration_start.elapsed().as_millis() as u64);
                      continue;
                  }
              };

             // Fetch gas object reference
             let gas_object_response_res = sui_client.read_api().get_object_with_options(
                 *gas_object_id,
                 SuiObjectDataOptions::new().with_owner().with_previous_transaction()
             ).await;
              let gas_object_ref = match gas_object_response_res {
                 Ok(resp) => resp.object_ref_if_exists().ok_or_else(|| anyhow!("Gas object {} not found or deleted", gas_object_id))?,
                 Err(e) => {
                     eprintln!("ERROR: Failed to fetch gas object {} ({}% Byzantine): {}", gas_object_id, percentage * 100.0, e);
                     failure_l1_rpc += 1;
                     benchmark.record_operation("l1_submission", l1_submit_start.elapsed().as_millis() as u64);
                     benchmark.record_operation("l1_confirmation", 0);
                     benchmark.record_operation("total_iteration", iteration_start.elapsed().as_millis() as u64);
                     continue;
                 }
             };
             

             // Fetch config object version
             let config_object_response_res = sui_client.read_api().get_object_with_options(
                 config_object_id,
                 SuiObjectDataOptions::new().with_owner()
             ).await;
              let config_object_version = match config_object_response_res {
                 Ok(resp) => resp.owner()
                     .and_then(|owner_enum| match owner_enum { Owner::Shared { initial_shared_version } => Some(initial_shared_version), _ => None, })
                     .ok_or_else(|| anyhow!("Could not get initial shared version for config object {}", config_object_id))?,
                 Err(e) => {
                     eprintln!("ERROR: Failed to fetch config object {} ({}% Byzantine): {}", config_object_id, percentage * 100.0, e);
                     failure_l1_rpc += 1;
                     benchmark.record_operation("l1_submission", l1_submit_start.elapsed().as_millis() as u64);
                     benchmark.record_operation("l1_confirmation", 0);
                     benchmark.record_operation("total_iteration", iteration_start.elapsed().as_millis() as u64);
                     continue;
                 }
             };
             
             // Get reference gas price
             let reference_gas_price_res = sui_client.read_api().get_reference_gas_price().await;
             let reference_gas_price = match reference_gas_price_res {
                 Ok(price) => price,
                 Err(e) => {
                     eprintln!("ERROR: Failed to get reference gas price ({}% Byzantine): {}", percentage * 100.0, e);
                     failure_l1_rpc += 1;
                     benchmark.record_operation("l1_submission", l1_submit_start.elapsed().as_millis() as u64);
                     benchmark.record_operation("l1_confirmation", 0);
                     benchmark.record_operation("total_iteration", iteration_start.elapsed().as_millis() as u64);
                     continue;
                 }
             };
             

             // Serialize payload for Move contract
             let payload_bcs = bcs::to_bytes(&unique_payload)
                 .context("Failed to serialize payload")?; // Should not fail

             // Build Programmable Transaction Block (PTB)
             let pt = {
                  let mut builder = ProgrammableTransactionBuilder::new();
                  let config_call_arg = CallArg::Object(ObjectArg::SharedObject { id: config_object_id, initial_shared_version: config_object_version, mutable: true });
                  let payload_call_arg = CallArg::Pure(payload_bcs);
                  let sigs_call_arg = CallArg::Pure(signatures_bcs);
                  builder.move_call( package_id, module_name.clone(), function_name.clone(), vec![], vec![config_call_arg, payload_call_arg, sigs_call_arg] )?;
                  builder.finish()
              };

             // Create transaction data
             let tx_data = TransactionData::new_programmable(
                  l1_submission_address,
                  vec![gas_object_ref],
                  pt,
                  100_000_000, // Increased gas budget
                  reference_gas_price
              );

             // Sign transaction
             let intent = Intent::sui_transaction();
             let intent_message = IntentMessage::new(intent, tx_data.clone());
             // Pass the keypair reference directly without Arc deref
             let sdk_signature = SdkSignature::new_secure(&intent_message, submitter_sui_keypair);

             // Create transaction
             let transaction = Transaction::from_data(tx_data, vec![sdk_signature.into()]);

             // Execute transaction
             let transaction_response_result = sui_client.quorum_driver_api().execute_transaction_block(
                 transaction,
                 SuiTransactionBlockResponseOptions::new().with_effects(),
                 Some(ExecuteTransactionRequestType::WaitForLocalExecution)
             ).await;

             // Record L1 submission timing
             let l1_submission_duration = l1_submit_start.elapsed();
             benchmark.record_operation("l1_submission", l1_submission_duration.as_millis() as u64);

             // Process transaction result
             match transaction_response_result {
                 Ok(response) => {
                      if let Some(effects) = response.effects {
                           match effects.status() {
                                SuiExecutionStatus::Success => {
                                     // Record L1 confirmation timing (simulated)
                                     let l1_confirm_start = Instant::now();
                                     tokio::time::sleep(Duration::from_millis(50)).await;
                                     let l1_confirmation_duration = l1_confirm_start.elapsed();
                                     benchmark.record_operation("l1_confirmation", l1_confirmation_duration.as_millis() as u64);
                                     successful_confirmations += 1;
                                },
                                SuiExecutionStatus::Failure { error } => {
                                     // This is expected when enough invalid signatures are included
                                     println!("INFO: L1 transaction {} failed as expected ({}% Byzantine): {:?}", 
                                              response.digest, percentage * 100.0, error);
                                     failure_l1_execution += 1;
                                     benchmark.record_operation("l1_confirmation", 0);
                                }
                           }
                      } else {
                           eprintln!("WARNING: L1 transaction {} succeeded but had no effects ({}% Byzantine)", 
                                    response.digest, percentage * 100.0);
                           // Count as execution failure if effects are missing
                           failure_l1_execution += 1; 
                           benchmark.record_operation("l1_confirmation", 0);
                      }
                 },
                 Err(e) => {
                      eprintln!("ERROR: L1 submission RPC error ({}% Byzantine): {:?}", percentage * 100.0, e);
                      failure_l1_rpc += 1;
                      benchmark.record_operation("l1_confirmation", 0);
                 }
             }

             // Record total iteration timing
             let total_iteration_time = iteration_start.elapsed();
             benchmark.record_operation("total_iteration", total_iteration_time.as_millis() as u64);
             
             // Progress indicator
             if (i + 1) % (BENCHMARK_ITERATIONS / 10).max(1) == 0 || i == BENCHMARK_ITERATIONS - 1 {
                 println!("      Iteration {}/{} ({:.1}% Byzantine) complete ({:?}), Current Success Rate: {:.1}%",
                     i + 1, BENCHMARK_ITERATIONS, percentage * 100.0, total_iteration_time, 
                     (successful_confirmations as f64 / (i + 1) as f64) * 100.0);
             }
        } // End iterations loop

        // Record success rate and failure reasons for this percentage
        let success_rate = if BENCHMARK_ITERATIONS > 0 { successful_confirmations as f64 / BENCHMARK_ITERATIONS as f64 } else { 0.0 };
        benchmark.add_config("verification_success_rate", &format!("{:.3}", success_rate));
        benchmark.add_config("failure_reason_signing_error", &failure_signing_error.to_string());
        benchmark.add_config("failure_reason_not_enough_signatures", &failure_not_enough_signatures.to_string());
        benchmark.add_config("failure_reason_l1_execution", &failure_l1_execution.to_string());
        benchmark.add_config("failure_reason_l1_rpc", &failure_l1_rpc.to_string());

        // Finalize and store benchmark results
        benchmark.end();
        benchmark.print_summary();
        metrics_storage.add_benchmark(benchmark);

    } // End percentages loop
    
    println!("  Byzantine Resilience Benchmarks completed.");
    Ok(())
}