//! Quorum Simulation Module
//!
//! Provides simulated quorum signing for testing. Maintains node keypairs
//! and simulates Byzantine behavior for fault tolerance checks.

use crate::transaction::types::SignatureBytes;
use sui_sdk::types::crypto::SuiKeyPair;
use sui_types::crypto::{Signer, EncodeDecodeBase64, SignatureScheme};
use crate::config::generate_test_sui_keypair;
use rand::{Rng, rngs::ThreadRng};
use std::sync::{Arc, Mutex};
use anyhow::{Result, anyhow};
use hex;

/// Manages simulated quorum nodes and their signing behavior.
///
/// Includes capabilities for:
/// - Generating deterministic test keypairs for nodes.
/// - Simulating Byzantine faults (non-response, invalid signatures) at a set rate.
/// - Collecting signatures (potentially faulty) for attestation payloads.
#[derive(Debug)]
pub struct QuorumSimulation {
    /// Keypairs representing each simulated quorum node.
    pub keypairs: Vec<SuiKeyPair>,
    /// Probability (0.0 to 1.0) that a node acts Byzantine during signing.
    byzantine_percentage: f64,
    /// Thread-safe random number generator.
    rng: Arc<Mutex<ThreadRng>>,
}

/// Represents a signature produced by the simulation and whether it's valid.
/// Format: (signature_bytes, is_valid_flag)
pub type SimulatedSignature = (SignatureBytes, bool);

impl QuorumSimulation {
    /// Creates a new simulation instance with a given set of keypairs.
    pub fn new(keypairs: Vec<SuiKeyPair>) -> Self {
        Self {
            keypairs,
            byzantine_percentage: 0.0, // Default: all nodes behave honestly.
            rng: Arc::new(Mutex::new(rand::thread_rng())),
        }
    }

    /// Creates a simulation with a specified number of nodes using deterministically
    /// generated test keypairs.
    ///
    /// Ensures unique keys for each node.
    pub fn create_with_random_nodes(num_nodes: usize) -> Result<Self> {
        if num_nodes == 0 {
            return Err(anyhow!("Cannot create a quorum simulation with zero nodes."));
        }
        let mut keypairs = Vec::with_capacity(num_nodes);
        let mut used_pubkeys = std::collections::HashSet::new();

        // Use generate_test_sui_keypair for deterministic keys
        while keypairs.len() < num_nodes {
            let keypair = generate_test_sui_keypair()?;
            let pubkey_bytes = keypair.public().as_ref().to_vec();

            // Ensure uniqueness using public key bytes
            if used_pubkeys.insert(pubkey_bytes) {
                keypairs.push(keypair);
            } else {
                // This should ideally not happen frequently with the deterministic generator
                // if num_nodes is less than the pool size, but handles potential collisions.
                println!("[Warning] Collision detected during test keypair generation. Retrying...");
            }
        }

        Ok(Self::new(keypairs))
    }

    /// Sets the probability (0.0 to 1.0) of a node acting Byzantine.
    pub fn set_byzantine_percentage(&mut self, percentage: f64) {
        self.byzantine_percentage = percentage.clamp(0.0, 1.0);
    }

    /// Simulates requesting signatures from all quorum nodes for given bytes.
    ///
    /// Depending on `byzantine_percentage`, nodes might:
    /// - Sign honestly.
    /// - Not respond (no signature returned).
    /// - Return an invalid signature (random bytes).
    ///
    /// # Arguments
    /// * `attestation_bytes` - The data to be signed by the simulated quorum.
    ///
    /// # Returns
    /// A `Result` containing a vector of `SimulatedSignature` tuples, or an error
    /// if an honest node fails unexpectedly.
    pub async fn request_signatures(&self, attestation_bytes: Vec<u8>) -> Result<Vec<SimulatedSignature>> {
        let mut simulated_signatures: Vec<SimulatedSignature> = Vec::with_capacity(self.keypairs.len());
        let mut rng = self.rng.lock().expect("Failed to lock RNG mutex"); // Use expect for clearer panic

        for node_keypair in &self.keypairs {
            // Determine if this node acts Byzantine for this request.
            if rng.gen::<f64>() < self.byzantine_percentage {
                // Simulate Byzantine behavior: 50/50 chance of non-response vs invalid signature.
                if rng.gen::<bool>() {
                    // Simulate Non-Responsive / Timeout: Skip adding a signature.
                    continue;
                } else {
                    // Simulate Invalid Signature: Generate random bytes of the expected length.
                    let mut invalid_sig = vec![0u8; 64]; // Ed25519 signature size expected by contract.
                    rng.fill(&mut invalid_sig[..]);
                    simulated_signatures.push((invalid_sig, false)); // Mark as invalid.
                }
            } else {
                // --- Honest Node Behavior ---
                // Sign the raw bytes using the node's keypair.
                let signature = node_keypair.sign(&attestation_bytes);

                // Extract the 64-byte signature needed for sui::ed25519::ed25519_verify.
                // Sui signatures often have a scheme flag prepended.
                let sig_bytes = signature.as_ref();
                if sig_bytes.len() >= 65 && sig_bytes[0] == SignatureScheme::ED25519.flag() {
                    // Standard Ed25519 case: Extract bytes 1 through 64.
                    let ed25519_sig = sig_bytes[1..65].to_vec();
                    simulated_signatures.push((ed25519_sig, true)); // Mark as valid.
                } else {
                    // Handle cases where signature format is unexpected (e.g., different scheme).
                    // This indicates a potential issue with key generation or signing logic.
                    eprintln!(
                        "[ERROR] Node generated signature with unexpected format. Length: {}, Scheme Byte: 0x{:02x}",
                        sig_bytes.len(),
                        sig_bytes.get(0).cloned().unwrap_or(0xff) // Safely get the scheme byte
                    );
                    // Decide how to handle: return error or skip signature? Returning error for now.
                    return Err(anyhow!("Node generated unexpected signature format"));
                }
            }
        }

        Ok(simulated_signatures)
    }

    /// Calculates the minimum number of signatures required for quorum (BFT threshold).
    /// Formula: floor(2n/3) + 1
    pub fn get_threshold(&self) -> usize {
        let n = self.keypairs.len();
        if n == 0 {
            0
        } else {
            (n * 2) / 3 + 1
        }
    }

    /// Returns the public keys (as byte vectors) of all simulated quorum members.
    pub fn get_public_keys(&self) -> Vec<Vec<u8>> {
        self.keypairs.iter()
            .map(|kp| kp.public().as_ref().to_vec())
            .collect()
    }

    /// Alias for `get_public_keys`, returning raw public key bytes.
    pub fn get_public_key_bytes(&self) -> Vec<Vec<u8>> {
        self.get_public_keys()
    }

    /// Alias for `get_public_keys`, used for compatibility where `get_nodes` is expected.
    pub fn get_nodes(&self) -> Vec<Vec<u8>> {
        self.get_public_keys()
    }
}