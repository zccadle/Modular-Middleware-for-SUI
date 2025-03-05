#[cfg(test)]
mod security_tests {
    use std::time::Duration;
    use anyhow::Result;
    use tokio::test;
    
    use crate::transaction::types::{Transaction, TransactionType};
    use crate::sui::verification::{VerificationManager, VerificationStatus};
    use crate::sui::network::{NetworkManager, NetworkType, NodeStatus};
    use crate::security::audit::{SecurityAuditLog, AuditSeverity};
    use crate::transaction::handler::TransactionHandler;
    use crate::metrics::performance::PerformanceMetrics;
    use crate::metrics::storage::MetricsStorage;
    use crate::execution::manager::ExecutionManager;
    
    // Helper function to create a test transaction
    fn create_test_transaction() -> Transaction {
        Transaction {
            tx_type: TransactionType::Transfer,
            sender: "0x4c45f32d0c5e9fd297e52d792c261a85f0582d0bfed0edd54e0cabe12cadd0f6".to_string(),
            receiver: "0x02a212de6a9dfa3a69e22387acfbafbb1a9e591bd9d636e7895dcfc8de05f331".to_string(),
            amount: 100,
            gas_payment: "0xb9fd6cfa2637273ca33dd3aef2f0b0296755889a0ef7f77c9cc95953a96a6302".to_string(),
            gas_budget: 50,
            commands: vec!["TransferObjects".to_string()],
            signatures: None,
            timestamp: 0,
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
    
    #[test]
    async fn test_verification_system() -> Result<()> {
        println!("Testing verification system...");
        
        // Initialize components
        let network_manager = NetworkManager::new(NetworkType::Testnet);
        let rpc_url = network_manager.get_active_rpc_url()?;
        let verification_manager = VerificationManager::new(&rpc_url);
        
        // Create mock transaction and digest
        let tx = create_test_transaction();
        let mock_digest = "test_verification_digest_123";
        
        // Test transaction registration
        println!("Testing transaction registration...");
        verification_manager.register_transaction(&tx, mock_digest)?;
        
        // Test verification
        println!("Testing transaction verification...");
        let mut metrics = PerformanceMetrics::new("verification_test");
        let verification_result = verification_manager.verify_transaction(mock_digest, Some(&mut metrics)).await?;
        
        println!("Verification result: {:?}", verification_result);
        println!("Verification metrics: {:?}", metrics.verification_time_ms());
        
        // Since we're using a mock digest, we expect the status to be Unverifiable or Pending
        assert!(matches!(verification_result, 
            VerificationStatus::Unverifiable(_) | VerificationStatus::Pending));
        
        Ok(())
    }
    
    #[test]
    async fn test_network_management() -> Result<()> {
        println!("Testing network management...");
        
        // Initialize network manager
        let network_manager = NetworkManager::new(NetworkType::Testnet);
        
        // Test getting active configuration
        let config = network_manager.get_active_config();
        println!("Active network: {}", config.network_type);
        println!("Chain ID: {}", config.chain_id);
        println!("RPC endpoints: {:?}", config.rpc_endpoints);
        
        // Test network health check
        println!("Testing network health...");
        let health_results = network_manager.health_check_all().await;
        println!("Network health status: {:?}", health_results);
        
        // At least one endpoint should be responding
        assert!(!health_results.is_empty());
        
        // Test network switching
        println!("Testing network switching...");
        let new_config = network_manager.switch_network(NetworkType::Devnet)?;
        println!("Switched to network: {}", new_config.network_type);
        assert_eq!(new_config.network_type, NetworkType::Devnet);
        
        // Test getting a healthy endpoint
        println!("Testing healthy endpoint selection...");
        match network_manager.get_healthy_rpc_url().await {
            Ok(url) => println!("Found healthy endpoint: {}", url),
            Err(e) => println!("No healthy endpoints found: {}", e),
        }
        
        Ok(())
    }
    
    #[test]
    async fn test_security_audit_logging() -> Result<()> {
        println!("Testing security audit logging...");
        
        // Initialize audit log
        let security_audit_log = SecurityAuditLog::new();
        
        // Log different types of events
        println!("Testing various log types...");
        
        security_audit_log.log_network(
            "test_network",
            "Network initialized",
            Some("sui-testnet"),
            AuditSeverity::Info
        )?;
        
        security_audit_log.log_validation(
            "test_validation",
            "Transaction validation succeeded",
            Some("tx123"),
            AuditSeverity::Info
        )?;
        
        security_audit_log.log_verification(
            "test_verification",
            "Transaction verification failed",
            Some("tx456"),
            AuditSeverity::Warning
        )?;
        
        security_audit_log.log_execution(
            "test_execution",
            "Transaction execution completed",
            Some("tx789"),
            AuditSeverity::Info
        )?;
        
        security_audit_log.log_security_error(
            "test_error",
            "Security protocol violation detected",
            Some(serde_json::json!({"source_ip": "192.168.1.1", "attempt_count": 3}))
        )?;
        
        // Get and verify logs
        let events = security_audit_log.get_events();
        println!("Log events count: {}", events.len());
        assert_eq!(events.len(), 5);
        
        // Test filtering by severity
        let warnings = security_audit_log.get_events_by_severity(AuditSeverity::Warning);
        println!("Warning events: {}", warnings.len());
        assert_eq!(warnings.len(), 1);
        
        // Test filtering by transaction
        let tx_events = security_audit_log.get_events_by_transaction("tx123");
        println!("Events for transaction tx123: {}", tx_events.len());
        assert_eq!(tx_events.len(), 1);
        
        // Print some log examples
        println!("Sample log event:");
        if !events.is_empty() {
            println!("{}", events[0].to_log_string());
        }
        
        Ok(())
    }
    
    #[test]
    async fn test_integrated_security() -> Result<()> {
        println!("Testing integrated security components...");
        
        // Initialize all components
        let network_manager = NetworkManager::new(NetworkType::Testnet);
        let rpc_url = network_manager.get_active_rpc_url()?;
        let verification_manager = VerificationManager::new(&rpc_url);
        let security_audit_log = SecurityAuditLog::new();
        let metrics_storage = MetricsStorage::new();
        
        // Create mock keypair for TransactionHandler
        let keypair = ed25519_dalek::Keypair::generate(&mut rand::thread_rng());
        
        // Initialize transaction handler and execution manager with security components
        let transaction_handler = TransactionHandler::new(
            keypair,
            Some(verification_manager.clone()),
            Some(security_audit_log.clone())
        );
        
        let execution_manager = ExecutionManager::new(
            Some(verification_manager.clone()),
            Some(network_manager.clone()),
            Some(security_audit_log.clone())
        );
        
        // Create and process a test transaction
        let tx = create_test_transaction();
        let mock_digest = format!("test_digest_{}", tx.timestamp);
        
        // Create metrics for tracking
        let mut metrics = PerformanceMetrics::new("integrated_test");
        
        // Register the transaction for verification
        println!("Registering transaction for verification...");
        transaction_handler.register_for_verification(&tx, &mock_digest)?;
        
        // Wait briefly to allow registration to complete
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify transaction
        println!("Verifying transaction...");
        let verification_status = transaction_handler.verify_transaction(&mock_digest, Some(&mut metrics)).await?;
        println!("Verification status: {:?}", verification_status);
        
        // Check the metrics for verification time
        if let Some(verif_time) = metrics.verification_time_ms() {
            println!("Verification time: {} ms", verif_time);
        }
        
        // Add metrics to storage
        metrics_storage.add_metrics(metrics);
        
        // Print summary statistics
        metrics_storage.print_summary();
        
        // Check audit logs for verification events
        let verification_events = security_audit_log.get_events_by_transaction(&mock_digest);
        println!("Verification audit events: {}", verification_events.len());
        
        for event in &verification_events {
            println!("Audit event: {}", event.to_log_string());
        }
        
        // Assert that we got at least one verification event
        assert!(!verification_events.is_empty());
        
        Ok(())
    }
    
    #[test]
    async fn test_chain_switching_security() -> Result<()> {
        println!("Testing chain switching security...");
        
        // Initialize components
        let network_manager = NetworkManager::new(NetworkType::Testnet);
        let verification_manager = VerificationManager::new(&network_manager.get_active_rpc_url()?);
        let security_audit_log = SecurityAuditLog::new();
        
        // Get the initial chain ID
        let initial_chain_id = network_manager.get_active_config().chain_id.clone();
        println!("Initial chain ID: {}", initial_chain_id);
        
        // Create a transaction
        let tx = create_test_transaction();
        let mock_digest = "test_chain_switch_digest";
        
        // Register transaction for the current chain
        verification_manager.register_transaction(&tx, mock_digest)?;
        
        // Log the operation
        security_audit_log.log_network(
            "chain_switch_test",
            &format!("Transaction registered on chain {}", initial_chain_id),
            Some(&initial_chain_id),
            AuditSeverity::Info
        )?;
        
        // Switch to a different network
        println!("Switching chains...");
        network_manager.switch_network(NetworkType::Devnet)?;
        let new_chain_id = network_manager.get_active_config().chain_id.clone();
        println!("New chain ID: {}", new_chain_id);
        
        // Log the chain switch
        security_audit_log.log_network(
            "chain_switch_test",
            &format!("Switched from chain {} to {}", initial_chain_id, new_chain_id),
            Some(&new_chain_id),
            AuditSeverity::Info
        )?;
        
        // Verify that chain IDs are different
        assert_ne!(initial_chain_id, new_chain_id);
        
        // Get and check audit logs
        let network_events = security_audit_log.get_events();
        println!("Total audit events: {}", network_events.len());
        
        // Print chain-related events
        for event in &network_events {
            if event.chain_id.is_some() {
                println!("Chain event: {}", event.to_log_string());
            }
        }
        
        // Assert we have events for both chains
        let events_initial_chain = network_events.iter()
            .filter(|e| e.chain_id.as_deref() == Some(&initial_chain_id))
            .count();
        
        let events_new_chain = network_events.iter()
            .filter(|e| e.chain_id.as_deref() == Some(&new_chain_id))
            .count();
        
        println!("Events for initial chain: {}", events_initial_chain);
        println!("Events for new chain: {}", events_new_chain);
        
        assert!(events_initial_chain > 0);
        assert!(events_new_chain > 0);
        
        Ok(())
    }
}