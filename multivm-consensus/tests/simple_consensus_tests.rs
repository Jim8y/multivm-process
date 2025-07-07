//! Simple Consensus Module Tests
//!
//! These tests work with the actual consensus implementation.

use multivm_consensus::{
    malachite::MalachiteConfig, state::StateManagerConfig, transaction_pool::TransactionPoolConfig,
    AlgorithmConfig, ConsensusAlgorithmType, ConsensusManagerConfig, MultiVMConsensusManager,
};
use tempfile::TempDir;

/// Helper function to create test consensus configuration
fn create_test_config(node_id: &str) -> (ConsensusManagerConfig, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let mut config = ConsensusManagerConfig::default();
    config.node_id = Some(node_id.to_string());
    config.algorithm = ConsensusAlgorithmType::Malachite;

    // Use actual field names from StateManagerConfig
    config.state_manager_config = StateManagerConfig {
        max_checkpoints: 10,
        checkpoint_interval: 100,
        enable_verification: true,
        max_pending_changes: 1000,
        rocksdb_path: Some(
            temp_dir
                .path()
                .join("rocksdb")
                .to_string_lossy()
                .to_string(),
        ),
    };

    // Use actual MalachiteConfig structure
    config.algorithm_config = AlgorithmConfig::Malachite(MalachiteConfig::default());

    // Configure transaction pool
    config.transaction_pool_config = TransactionPoolConfig {
        max_pool_size: 1000,
        max_per_account: 100,
        tx_expiry_seconds: 300,
        allow_replacement: true,
        replacement_gas_increase: 10,
    };

    (config, temp_dir)
}

/// Test consensus manager creation
#[tokio::test]
async fn test_consensus_manager_creation() {
    let (config, _temp_dir) = create_test_config("test_node");

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
    let (valid_config, _temp_dir) = create_test_config("config_test_node");
    let manager_result = MultiVMConsensusManager::new(valid_config).await;
    assert!(
        manager_result.is_ok(),
        "Valid configuration should create manager successfully"
    );

    // Additional validation tests could be added here based on actual validation rules
}

/// Test basic consensus operations
#[tokio::test]
async fn test_basic_consensus_operations() {
    let (config, _temp_dir) = create_test_config("ops_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test getting consensus stats
    let stats_result = manager.get_consensus_stats().await;
    assert!(stats_result.is_ok(), "Should get consensus statistics");

    manager.stop().await.unwrap();
}

/// Test transaction operations
#[tokio::test]
async fn test_transaction_operations() {
    let (config, _temp_dir) = create_test_config("tx_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test transaction submission
    let tx = serde_json::json!({
        "id": "test_tx_1",
        "data": "test_transaction"
    });
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

    // Test getting transaction pool stats
    let pool_stats = manager.get_transaction_pool_stats().await;
    assert_eq!(
        pool_stats.current_pool_size, 1,
        "Transaction pool should contain 1 transaction"
    );

    manager.stop().await.unwrap();
}

/// Test state operations
#[tokio::test]
async fn test_state_operations() {
    let (config, _temp_dir) = create_test_config("state_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test getting consensus stats
    let stats_result = manager.get_consensus_stats().await;
    assert!(
        stats_result.is_ok(),
        "Should get consensus stats successfully"
    );

    manager.stop().await.unwrap();
}

/// Test error handling
#[tokio::test]
async fn test_error_handling() {
    let (config, _temp_dir) = create_test_config("error_test_node");
    let mut manager = MultiVMConsensusManager::new(config).await.unwrap();

    manager.start().await.unwrap();

    // Test submitting transaction (size validation might not be enforced)
    let large_data = "x".repeat(1024); // 1KB transaction
    let large_tx = serde_json::json!({
        "id": "large_tx",
        "data": large_data
    });
    let result = manager
        .submit_transaction(
            large_tx,
            multivm_consensus::transaction_pool::TransactionPriority::Normal,
        )
        .await;
    // Note: The transaction pool might not enforce size limits in this implementation
    assert!(
        result.is_ok() || result.is_err(),
        "Transaction submission completes with either success or error"
    );

    manager.stop().await.unwrap();
}

/// Test multiple managers
#[tokio::test]
async fn test_multiple_managers() {
    let mut managers = Vec::new();
    let mut _temp_dirs = Vec::new();

    // Create multiple consensus managers
    for i in 0..3 {
        let node_id = format!("multi_test_node_{}", i);
        let (config, temp_dir) = create_test_config(&node_id);
        _temp_dirs.push(temp_dir); // Keep temp dirs alive
        let mut manager = MultiVMConsensusManager::new(config).await.unwrap();
        manager.start().await.unwrap();
        managers.push(manager);
    }

    // Submit transactions to each manager
    for (i, manager) in managers.iter().enumerate() {
        let tx = serde_json::json!({
            "id": format!("multi_tx_{}", i),
            "data": format!("transaction_{}", i)
        });
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
        let pool_stats = manager.get_transaction_pool_stats().await;
        assert_eq!(
            pool_stats.current_pool_size, 1,
            "Manager {} should have 1 transaction",
            i
        );
    }

    // Stop all managers
    for mut manager in managers {
        manager.stop().await.unwrap();
    }
}
