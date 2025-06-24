//! System integration tests for MultiVM
//!
//! These tests verify the complete end-to-end functionality of the MultiVM system.

use multivm_account_mapping::{AccountAddress, AccountMappingLayer, MemoryStorage, MultivmAccountId};
use multivm_common::*;
use multivm_consensus::{EvmTransaction, MalachiteConfig, MalachiteConsensus, MultiVMBlock, SvmTransaction};
use multivm_process_manager::{MultivmCoordinator, CoordinatorConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Test basic system initialization and configuration
#[tokio::test]
async fn test_system_initialization() {
    // Test coordinator creation with default config
    let config = CoordinatorConfig {
        consensus: MalachiteConfig::default(),
        health_check_interval: Duration::from_secs(30),
        block_timeout: Duration::from_secs(60),
        max_concurrent_blocks: 10,
        enable_recovery: true,
    };
    
    let coordinator = MultivmCoordinator::new(config).await;
    assert!(coordinator.is_ok(), "Failed to create coordinator: {:?}", coordinator.err());
    
    let coordinator = coordinator.unwrap();
    let state = coordinator.get_state().await;
    assert!(!state.is_running, "Coordinator should not be running initially");
    assert_eq!(state.blocks_processed, 0, "No blocks should be processed initially");
}

/// Test account mapping functionality
#[tokio::test]
async fn test_account_mapping_integration() {
    let storage = Arc::new(MemoryStorage::new());
    let mapping = AccountMappingLayer::new(storage);
    
    // Create test accounts
    let solana_account = AccountAddress::Solana([1u8; 32]);
    let ethereum_account = AccountAddress::Ethereum([2u8; 20]);
    
    // Test auto-binding
    let multivm_id = mapping.add_auto_binding(solana_account.clone()).await
        .expect("Failed to create auto binding");
    
    // Verify the binding was created
    let bound_addresses = mapping.get_bound_addresses(&multivm_id).await
        .expect("Failed to get bound addresses");
    
    assert_eq!(bound_addresses.len(), 1, "Should have one bound address");
    assert_eq!(bound_addresses[0], solana_account, "Bound address should match");
    
    // Test cross-binding (simplified - in production would require cryptographic proof)
    let proof = multivm_account_mapping::BindingProof {
        account: ethereum_account.clone(),
        proof_type: multivm_account_mapping::ProofType::Ed25519Signature,
        proof_data: vec![0u8; 64], // Mock signature
        timestamp: std::time::SystemTime::now(),
    };
    
    let result = mapping.add_cross_binding(multivm_id.clone(), ethereum_account.clone(), proof).await;
    assert!(result.is_ok(), "Cross-binding should succeed: {:?}", result.err());
    
    // Verify both accounts are now bound
    let bound_addresses = mapping.get_bound_addresses(&multivm_id).await
        .expect("Failed to get bound addresses after cross-binding");
    
    assert_eq!(bound_addresses.len(), 2, "Should have two bound addresses");
    assert!(bound_addresses.contains(&solana_account), "Should contain Solana address");
    assert!(bound_addresses.contains(&ethereum_account), "Should contain Ethereum address");
}

/// Test consensus block creation and processing
#[tokio::test]
async fn test_consensus_block_processing() {
    let consensus = MalachiteConsensus::new(MalachiteConfig::default())
        .expect("Failed to create consensus");
    
    // Create a test block with mixed transactions
    let evm_tx = EvmTransaction {
        hash: vec![1, 2, 3, 4],
        from: "0x1234567890123456789012345678901234567890".to_string(),
        to: Some("0x0987654321098765432109876543210987654321".to_string()),
        value: 1000000000000000000u64, // 1 ETH in wei
        gas_limit: 21000,
        gas_price: 20000000000u64, // 20 gwei
        nonce: 1,
        data: vec![],
        v: 27,
        r: vec![0u8; 32],
        s: vec![0u8; 32],
    };
    
    let svm_tx = SvmTransaction {
        signatures: vec![vec![0u8; 64]],
        accounts: vec!["11111111111111111111111111111112".to_string()],
        recent_blockhash: [0u8; 32],
        instructions: vec![],
        data: vec![],
    };
    
    let block = MultiVMBlock {
        header: multivm_consensus::BlockHeader {
            height: 1,
            timestamp: std::time::SystemTime::now(),
            previous_hash: vec![0u8; 32],
            merkle_root: vec![0u8; 32],
            state_root: vec![0u8; 32],
            validator_signature: vec![0u8; 64],
        },
        evm_transactions: vec![evm_tx],
        svm_transactions: vec![svm_tx],
        special_transactions: vec![],
    };
    
    // Test block validation
    let validation_result = consensus.validate_block(&block).await;
    assert!(validation_result.is_ok(), "Block validation should succeed: {:?}", validation_result.err());
    
    // Test block proposal
    let proposal_result = consensus.propose_block(block).await;
    assert!(proposal_result.is_ok(), "Block proposal should succeed: {:?}", proposal_result.err());
}

/// Test system health monitoring
#[tokio::test]
async fn test_health_monitoring() {
    let config = CoordinatorConfig {
        consensus: MalachiteConfig::default(),
        health_check_interval: Duration::from_millis(100), // Fast for testing
        block_timeout: Duration::from_secs(5),
        max_concurrent_blocks: 5,
        enable_recovery: true,
    };
    
    let coordinator = MultivmCoordinator::new(config).await
        .expect("Failed to create coordinator");
    
    // Start the coordinator
    coordinator.start().await
        .expect("Failed to start coordinator");
    
    // Wait a short time for health checks to run
    tokio::time::sleep(Duration::from_millis(200)).await;
    
    // Check that the system is running and healthy
    let state = coordinator.get_state().await;
    assert!(state.is_running, "Coordinator should be running");
    
    // Check health status
    let health = coordinator.get_health_status().await
        .expect("Failed to get health status");
    assert!(health.is_healthy, "System should be healthy");
    
    // Stop the coordinator
    coordinator.stop().await
        .expect("Failed to stop coordinator");
    
    let final_state = coordinator.get_state().await;
    assert!(!final_state.is_running, "Coordinator should be stopped");
}

/// Test cross-VM transfer workflow
#[tokio::test]
async fn test_cross_vm_transfer_workflow() {
    let storage = Arc::new(MemoryStorage::new());
    let mapping = AccountMappingLayer::new(storage);
    
    // Create source and destination accounts
    let source_solana = AccountAddress::Solana([1u8; 32]);
    let dest_ethereum = AccountAddress::Ethereum([2u8; 20]);
    
    // Create MultiVM accounts
    let source_multivm = mapping.add_auto_binding(source_solana.clone()).await
        .expect("Failed to create source account");
    
    let dest_multivm = mapping.add_auto_binding(dest_ethereum.clone()).await
        .expect("Failed to create destination account");
    
    // Create a cross-VM transfer
    let transfer = multivm_account_mapping::CrossVmTransfer {
        id: "test-transfer-001".to_string(),
        from: source_multivm.clone(),
        to: dest_multivm.clone(),
        amount: 1000000, // 1 SOL in lamports
        asset_type: multivm_account_mapping::AssetType::Native,
        status: multivm_account_mapping::TransferStatus::Pending,
        source_tx_hash: None,
        target_tx_hash: None,
        created_at: std::time::SystemTime::now(),
        expires_at: std::time::SystemTime::now() + Duration::from_secs(3600),
    };
    
    // Initiate the transfer
    let result = mapping.initiate_cross_vm_transfer(transfer).await;
    assert!(result.is_ok(), "Transfer initiation should succeed: {:?}", result.err());
    
    let transfer_id = result.unwrap();
    
    // Check transfer status
    let transfer_status = mapping.get_transfer_status(&transfer_id).await;
    assert!(transfer_status.is_ok(), "Should be able to get transfer status");
    
    let status = transfer_status.unwrap();
    assert_eq!(status, multivm_account_mapping::TransferStatus::Pending, "Transfer should be pending");
}

/// Test system recovery capabilities
#[tokio::test]
async fn test_system_recovery() {
    let config = CoordinatorConfig {
        consensus: MalachiteConfig::default(),
        health_check_interval: Duration::from_millis(50),
        block_timeout: Duration::from_secs(1),
        max_concurrent_blocks: 2,
        enable_recovery: true,
    };
    
    let coordinator = MultivmCoordinator::new(config).await
        .expect("Failed to create coordinator");
    
    // Start the system
    coordinator.start().await
        .expect("Failed to start coordinator");
    
    // Verify system is running
    let state = coordinator.get_state().await;
    assert!(state.is_running, "System should be running");
    
    // Test graceful shutdown
    let shutdown_result = timeout(Duration::from_secs(5), coordinator.stop()).await;
    assert!(shutdown_result.is_ok(), "Shutdown should complete within timeout");
    assert!(shutdown_result.unwrap().is_ok(), "Shutdown should succeed");
    
    // Verify system is stopped
    let final_state = coordinator.get_state().await;
    assert!(!final_state.is_running, "System should be stopped after shutdown");
}

/// Test performance under load (basic stress test)
#[tokio::test]
async fn test_basic_performance() {
    let storage = Arc::new(MemoryStorage::new());
    let mapping = AccountMappingLayer::new(storage);
    
    // Create multiple accounts rapidly
    let start_time = std::time::Instant::now();
    let mut account_ids = Vec::new();
    
    for i in 0..100 {
        let mut account_bytes = [0u8; 32];
        account_bytes[0] = i as u8;
        account_bytes[1] = (i >> 8) as u8;
        
        let account = AccountAddress::Solana(account_bytes);
        let multivm_id = mapping.add_auto_binding(account).await
            .expect("Failed to create account binding");
        
        account_ids.push(multivm_id);
    }
    
    let creation_time = start_time.elapsed();
    println!("Created 100 accounts in {:?}", creation_time);
    
    // Verify all accounts were created
    assert_eq!(account_ids.len(), 100, "Should have created 100 accounts");
    
    // Test retrieval performance
    let start_time = std::time::Instant::now();
    
    for account_id in &account_ids {
        let addresses = mapping.get_bound_addresses(account_id).await
            .expect("Failed to get bound addresses");
        assert_eq!(addresses.len(), 1, "Each account should have one bound address");
    }
    
    let retrieval_time = start_time.elapsed();
    println!("Retrieved 100 account bindings in {:?}", retrieval_time);
    
    // Performance assertions (reasonable thresholds)
    assert!(creation_time < Duration::from_secs(5), "Account creation should be fast");
    assert!(retrieval_time < Duration::from_secs(2), "Account retrieval should be fast");
}