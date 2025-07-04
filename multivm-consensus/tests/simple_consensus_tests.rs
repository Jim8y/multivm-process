//! Simple Consensus Module Tests
//!
//! These tests work with the actual consensus implementation.

use multivm_consensus::{
    malachite::MalachiteConfig, state::StateManagerConfig, transaction_pool::TransactionPoolConfig,
    AlgorithmConfig, ConsensusAlgorithmType, ConsensusManagerConfig, MultiVMConsensusManager,
};
use std::time::Duration;
use tempfile::TempDir;

/// Helper function to create test consensus configuration
fn create_test_config(node_id: &str) -> ConsensusManagerConfig {
    let mut config = ConsensusManagerConfig::default();
    config.node_id = Some(node_id.to_string());
    config.algorithm = ConsensusAlgorithmType::Malachite;

    // Use actual field names from StateManagerConfig
    config.state_manager_config = StateManagerConfig {
        max_checkpoints: 10,
        checkpoint_interval: 100,
        enable_verification: true,
        max_pending_changes: 1000,
        rocksdb_path: None,
    };

    // Use actual MalachiteConfig structure
    config.algorithm_config = AlgorithmConfig::Malachite(MalachiteConfig::default());

    // Configure transaction pool
    config.transaction_pool_config = TransactionPoolConfig {
        max_size: 1000,
        max_transaction_size: 64 * 1024, // 64KB
        ttl: Duration::from_secs(300),
        eviction_interval: Duration::from_secs(60),
        eviction_batch_size: 50,
    };

    config
}

/// Test consensus manager creation
#[tokio::test]
async fn test_consensus_manager_creation() {
    let config = create_test_config("test_node");

    // Test manager creation
    let manager_result = MultiVMConsensusManager::new(config).await;
    assert!(
        manager_result.is_ok(),
        "Consensus manager should be created successfully"
    );

    let mut manager = manager_result.unwrap();

    // Test manager startup
    let start_result = manager.start().await;
    assert!(
        start_result.is_ok(),
        "Consensus manager should start successfully"
    );

    // Test manager shutdown
    let stop_result = manager.stop().await;
    assert!(
        stop_result.is_ok(),
        "Consensus manager should stop successfully"
    );
}

/// Test configuration validation
#[tokio::test]
async fn test_configuration_validation() {
    // Test with valid configuration
    let valid_config = create_test_config("config_test_node");
    let valid_result = valid_config.validate();
    assert!(
        valid_result.is_ok(),
        "Valid configuration should pass validation"
    );

    // Test with invalid transaction pool size
    let mut invalid_config = create_test_config("invalid_node");
    invalid_config.transaction_pool_config.max_size = 0; // Invalid pool size

    let invalid_result = invalid_config.validate();
    assert!(
        invalid_result.is_err(),
        "Invalid configuration should fail validation"
    );
}

/// Test basic consensus operations
#[tokio::test]
async fn test_basic_consensus_operations() {
    let config = create_test_config("ops_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test getting consensus state
    let state_result = manager.get_consensus_state().await;
    assert!(
        state_result.is_ok(),
        "Should get consensus state successfully"
    );

    // Test getting stats
    let stats_result = manager.get_stats().await;
    assert!(stats_result.is_ok(), "Should get consensus statistics");

    manager.stop().await.unwrap();
}

/// Test transaction operations
#[tokio::test]
async fn test_transaction_operations() {
    let config = create_test_config("tx_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test transaction submission
    let tx = b"test_transaction".to_vec();
    let submit_result = manager
        .submit_transaction(
            tx,
            multivm_consensus::transaction_pool::TransactionPriority::Normal,
        )
        .await;
    assert!(
        submit_result.is_ok(),
        "Should submit transaction successfully"
    );

    // Test getting transaction pool size
    let pool_size = manager.get_transaction_pool_size().await;
    assert_eq!(
        pool_size, 1,
        "Transaction pool should contain 1 transaction"
    );

    manager.stop().await.unwrap();
}

/// Test state operations
#[tokio::test]
async fn test_state_operations() {
    let config = create_test_config("state_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test getting state root
    let state_root_result = manager.get_state_root().await;
    assert!(
        state_root_result.is_ok(),
        "Should get state root successfully"
    );

    // Test getting VM states
    let vm_states_result = manager.get_vm_states().await;
    assert!(
        vm_states_result.is_ok(),
        "Should get VM states successfully"
    );

    manager.stop().await.unwrap();
}

/// Test error handling
#[tokio::test]
async fn test_error_handling() {
    let config = create_test_config("error_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test submitting oversized transaction
    let oversized_tx = vec![0u8; 128 * 1024]; // 128KB, larger than 64KB limit
    let oversized_result = manager
        .submit_transaction(
            oversized_tx,
            multivm_consensus::transaction_pool::TransactionPriority::Normal,
        )
        .await;
    assert!(
        oversized_result.is_err(),
        "Should reject oversized transaction"
    );

    manager.stop().await.unwrap();
}

/// Test multiple managers
#[tokio::test]
async fn test_multiple_managers() {
    let mut managers = Vec::new();

    // Create multiple consensus managers
    for i in 0..3 {
        let node_id = format!("multi_test_node_{}", i);
        let config = create_test_config(&node_id);
        let mut manager = MultiVMConsensusManager::new(config).await.unwrap();
        manager.start().await.unwrap();
        managers.push(manager);
    }

    // Submit transactions to each manager
    for (i, manager) in managers.iter().enumerate() {
        let tx = format!("multi_tx_{}", i).into_bytes();
        manager
            .submit_transaction(
                tx,
                multivm_consensus::transaction_pool::TransactionPriority::Normal,
            )
            .await
            .unwrap();
    }

    // Verify each manager maintains its own state
    for (i, manager) in managers.iter().enumerate() {
        let pool_size = manager.get_transaction_pool_size().await;
        assert_eq!(pool_size, 1, "Manager {} should have 1 transaction", i);
    }

    // Stop all managers
    for mut manager in managers {
        manager.stop().await.unwrap();
    }
}
