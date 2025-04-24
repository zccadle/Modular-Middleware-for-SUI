//! Configuration Module for the SUI Modular Middleware
//!
//! This module defines constants, loads configuration (preferring environment variables),
//! and provides utility functions like test key generation.

use anyhow::{Result, anyhow};
use sui_sdk::types::base_types::SuiAddress;
use sui_sdk::types::crypto::{SuiKeyPair, EncodeDecodeBase64};
use base64;
use std::sync::atomic::{AtomicUsize, Ordering};
use sui_types::crypto::{SignatureScheme};

// --- TEMPORARY CONSTANTS FOR TESTING ---
// These values are from configbackup.rs and are being used temporarily
// to test the build and runtime after code cleanup.
// ***** REMEMBER TO REPLACE THESE WITH PLACEHOLDERS BEFORE COMMITTING TO GITHUB *****

/// Submitter address for transactions.
pub const SUBMITTER_ADDRESS: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

/// Submitter keypair encoded as a Base64 string.
pub const SUBMITTER_KEYPAIR_BASE64: &str = "PLACEHOLDER_BASE64_KEYPAIR_REPLACE_BEFORE_RUNNING";

/// Gas object ID owned by the submitter.
pub const SUBMITTER_GAS_OBJECT_ID: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";

/// Deployed verification contract package ID on Sui.
pub const VERIFICATION_CONTRACT_PACKAGE_ID: &str = "0x2f248352270781a3657cb7fa8df99ec32bd3f7b8c5dda1e9ab3f7369ffd7ea5d";

/// Verification contract module name.
pub const VERIFICATION_CONTRACT_MODULE: &str = "attestation_verifier";

/// Verification contract function name.
pub const VERIFICATION_CONTRACT_FUNCTION: &str = "verify_and_execute";

/// Quorum config object ID (for on-chain configuration).
pub const VERIFICATION_CONTRACT_CONFIG_OBJECT_ID: &str = "0x26f6a005684ff909c7a492104e232e9269132fbe1f519da47c54d0f3908b115a";

/// Admin capability object ID.
pub const VERIFICATION_CONTRACT_ADMIN_CAP_ID: &str = "0x1f3f247ebb9b303467c1ca98e3f136d0b7d2cea2b827f06478a82d6adfc226cc";

// --- Network Configuration ---

/// SUI Testnet fullnode RPC endpoint.
pub const SUI_TESTNET_RPC: &str = "https://fullnode.testnet.sui.io:443";

/// SUI RPC URL used for API calls within the application.
pub const SUI_RPC_URL: &str = SUI_TESTNET_RPC; // Defaulting to Testnet

// --- Keypair Loading Functions ---

/// Loads the submitter keypair, prioritizing environment variables.
///
/// Attempts to read SUBMITTER_KEYPAIR_BASE64 and SUBMITTER_ADDRESS from environment variables.
/// If found, validates the keypair against the address (if provided).
/// If environment variables are not set or invalid, falls back to the hardcoded constants above.
pub fn load_submitter_keypair() -> Result<SuiKeyPair> {
    // First try from environment variable
    if let Ok(base64_keypair) = std::env::var("SUBMITTER_KEYPAIR_BASE64") {
        println!("Attempting to load keypair from SUBMITTER_KEYPAIR_BASE64 env var.");
        let sui_keypair = SuiKeyPair::decode_base64(&base64_keypair)
            .map_err(|e| anyhow!("Failed to decode base64 keypair from env: {}", e))?;

        if let Ok(expected_address) = std::env::var("SUBMITTER_ADDRESS") {
            let derived_address = SuiAddress::from(&sui_keypair.public()).to_string();
            if derived_address != expected_address {
                return Err(anyhow!("Keypair mismatch: Derived address ({}) doesn't match SUBMITTER_ADDRESS env var ({})",
                         derived_address, expected_address));
            }
            println!("Address validation successful against env var: {}", derived_address);
        } else {
            let derived_address = SuiAddress::from(&sui_keypair.public()).to_string();
            println!("Using keypair from env var. Derived address: {}. (SUBMITTER_ADDRESS env var not set for verification)", derived_address);
        }
        return Ok(sui_keypair);
    }

    // Fallback to hardcoded constants (Now using actual values for testing)
    println!("[INFO] SUBMITTER_KEYPAIR_BASE64 env var not set. Falling back to hardcoded constants for testing.");
    let sui_keypair = SuiKeyPair::decode_base64(SUBMITTER_KEYPAIR_BASE64)
        .map_err(|e| anyhow!("Failed to decode hardcoded keypair constant: {}", e))?;

    let derived_address = SuiAddress::from(&sui_keypair.public()).to_string();
    if derived_address != SUBMITTER_ADDRESS {
        // This case should ideally not happen if constants match, but good to keep check.
        println!("[ERROR] Derived address ({}) from hardcoded keypair does not match hardcoded SUBMITTER_ADDRESS ({})",
                 derived_address, SUBMITTER_ADDRESS);
         return Err(anyhow!("Mismatch between hardcoded keypair and address constants."));
    } else {
         println!("Using keypair from hardcoded constant. Derived address: {}", derived_address);
    }
    Ok(sui_keypair)
}

/// Generates a deterministic test SuiKeyPair for simulations.
///
/// Uses a predefined list of valid Base64 encoded keypairs and cycles through them.
/// This ensures that simulated quorum nodes have consistent keys across different
/// test runs, aiding reproducibility.
///
/// # Returns
/// A Result containing the selected SuiKeyPair or an error if decoding fails.
pub fn generate_test_sui_keypair() -> Result<SuiKeyPair> {
    // Predefined list of valid Base64 keypairs for deterministic test key generation.
    // Ensures simulated nodes use consistent keys.
    let valid_keypairs = [
        "ANfSvMs3AQzpfNfJk8m1T2Q7HxSp5YqhmHkS2L1ET8Gg",
        "AMIJsNHHiBO8WGkTnpzh1YKN+TZWEZQYW8EwFQ9maDN/",
        "AOrr4jVwWxUDNTQXnHBkD9z27S/tL/0fvfPNUaYUblRX",
        "AP0Zicc3/srX63FO4ZLLFUc+ZOE4mKCGTf/NB5zWeHvz",
        "AMnaSUiUd2BvWQZkUrG9hTzdJhfQyaYDQo7veu3Ei8Hp",
        "ARLtoE6phJWSqtq4gk1hB18iANDwN1yvBs18W4aPECUs",
        "APPdCnEmR8X4cUoyKW9xpztPQwF6eA6uPS5yYvL7D4DR",
        "ASm3fbh1anwuGx+NzY7YMzS6Zy+NE2imXNJCVkF/1mJM",
        "ALwqJ0EUFnIECPxgQXk8QyoUDh7NKUUJmEe7/aBQZKCf",
        "AEEsqKmhmbGbOX9BK6KY0wLDyp4ZOUh0ZFQQeMUdKHW4",
        "ALsYyBUJpsoA53Czu9wVFkafnQmGMdvFKXmTZ1hdVEJM",
        "AHq1UUBrWTJphO0GfwusMjmWUZNyVX1rE0oy+Wpzs0SV",
        "AL1rUIW6A2+lsqVAzqW8i+7YQzRUzUkKf1/HLm1USBVu",
        "AHxgXVe3MgSQWd+0UeT+gj97k8s41JIgPRkWUuDYATM1",
        "AOe2aO5J5SQIFhXbO0xO2rJuIZNZSlbPJGpH+veB+LCQ",
    ];

    // Static atomic counter to cycle through the predefined keys deterministically.
    static KEY_INDEX: AtomicUsize = AtomicUsize::new(0);
    let index = KEY_INDEX.fetch_add(1, Ordering::SeqCst) % valid_keypairs.len();

    // Decode and return the selected keypair.
    SuiKeyPair::decode_base64(valid_keypairs[index])
        .map_err(|e| anyhow!("Failed to decode predefined test SuiKeyPair from base64 at index {}: {}", index, e))
}

/// Legacy function, primarily for maintaining API compatibility where expected.
/// Generates a deterministic test keypair instead of attempting conversion.
pub fn convert_dalek_to_sui_keypair(_dalek_keypair: &[u8]) -> Result<SuiKeyPair> {
    // This function name might be misleading given its current implementation.
    // It now calls `generate_test_sui_keypair` for deterministic test keys.
    eprintln!("NOTICE: `convert_dalek_to_sui_keypair` generating standard test keypair.");
    generate_test_sui_keypair()
}

/// Loads the keypair intended for node operations (e.g., submitting L1 transactions).
/// In this setup, it uses the same logic as loading the primary submitter keypair.
pub fn load_node_keypair() -> Result<SuiKeyPair> {
    println!("Loading node keypair (using submitter keypair logic)...");
    load_submitter_keypair()
}