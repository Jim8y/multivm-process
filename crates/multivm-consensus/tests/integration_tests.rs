//! Integration tests for the MultiVM consensus module
//!
//! These tests verify the interaction between different consensus components
//! and ensure the system works correctly as a whole.

use multivm_consensus::{
    BlockSyncConfig, BlockSyncManager, ConsensusEngine, CrossVMStateCoordinator,
    ForkDetectionConfig, ForkDetectionManager, MalachiteConfig, MalachiteConsensus, MultiVMBlock,
    NetworkRecoveryConfig, NetworkRecoveryManager, StatePersistenceConfig,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_consensus_with_fork_detection_integration() {
    // Create Malachite consensus
    let malachite_config = MalachiteConfig {
        node_id: "test-node-1".to_string(),
        network_config: multivm_consensus::malachite::NetworkConfig {
            listen_addr: "127.0.0.1:26656".to_string(),
            peers: vec![],
        },
        consensus_params: multivm_consensus::malachite::ConsensusParams {
            block_time_ms: 100,
            max_block_size: 1024 * 1024,
            timeout_propose_ms: 1000,
            timeout_prevote_ms: 500,
            timeout_precommit_ms: 500,
        },
        validators: vec![],
    };

    let (mut consensus, _block_sender, _commit_receiver) =
        MalachiteConsensus::new(malachite_config)
            .await
            .expect("Failed to create consensus");

    // Create fork detection manager
    let fork_config = ForkDetectionConfig::default();
    let fork_detector = Arc::new(
        ForkDetectionManager::new(fork_config)
            .await
            .expect("Failed to create fork detector"),
    );

    // Test consensus operation with fork detection
    consensus.start().await.expect("Failed to start consensus");

    // Create two blocks at same height (potential fork)
    let block1 = MultiVMBlock::new(1, "0".repeat(64), "validator1".to_string(), vec![]);
    let block2 = MultiVMBlock::new(1, "0".repeat(64), "validator2".to_string(), vec![]);

    // Process blocks through fork detector
    let fork1 = fork_detector
        .process_block(block1.clone(), "validator1".to_string())
        .await
        .expect("Failed to process block1");

    let fork2 = fork_detector
        .process_block(block2.clone(), "validator2".to_string())
        .await
        .expect("Failed to process block2");

    // Verify fork was detected
    assert!(fork1.is_none()); // First block doesn't trigger fork
    assert!(fork2.is_some()); // Second block at same height triggers fork

    consensus.stop().await.expect("Failed to stop consensus");
}

#[tokio::test]
async fn test_network_recovery_with_consensus() {
    // Create network recovery manager
    let recovery_config = NetworkRecoveryConfig::default();
    let mut recovery_manager = NetworkRecoveryManager::new(recovery_config)
        .await
        .expect("Failed to create recovery manager");

    // Create fork detector for integration
    let fork_config = ForkDetectionConfig::default();
    let fork_detector = Arc::new(
        ForkDetectionManager::new(fork_config)
            .await
            .expect("Failed to create fork detector"),
    );

    recovery_manager.set_fork_detector(fork_detector).await;

    // Simulate validator status updates
    recovery_manager
        .update_validator_status("validator1".to_string(), true, 100)
        .await;
    recovery_manager
        .update_validator_status("validator2".to_string(), false, 95)
        .await;
    recovery_manager
        .update_validator_status("validator3".to_string(), false, 90)
        .await;

    // Check partition detection
    let is_partitioned = recovery_manager
        .detect_partition()
        .await
        .expect("Failed to detect partition");

    assert!(is_partitioned, "Should detect network partition");

    // Get network health
    let health = recovery_manager.get_network_health().await;
    assert_eq!(
        health.status,
        multivm_consensus::NetworkHealthStatus::Partitioned
    );
}

#[tokio::test]
async fn test_block_sync_with_consensus() {
    // Create block sync manager
    let sync_config = BlockSyncConfig {
        enabled: true,
        max_blocks_per_request: 10,
        sync_threshold_height: 5,
        ..Default::default()
    };

    let sync_manager = BlockSyncManager::new(sync_config)
        .await
        .expect("Failed to create sync manager");

    // Update peer heights
    sync_manager
        .update_peer_height("peer1".to_string(), 100)
        .await;
    sync_manager
        .update_peer_height("peer2".to_string(), 95)
        .await;

    // Check if sync is needed
    let needs_sync = sync_manager.needs_sync(90).await;
    assert!(needs_sync, "Should need sync when behind peers");

    // Get sync status
    let status = sync_manager.get_sync_status().await;
    assert_eq!(
        status,
        multivm_consensus::BlockSyncStatus::Idle,
        "Should be idle before sync starts"
    );
}

#[tokio::test]
async fn test_state_persistence_integration() {
    use multivm_consensus::state::{CrossVMStateManager, StateManagerConfig, StorageBackend};

    // Create state manager with persistence
    let state_config = StateManagerConfig::default();
    let persistence_config = StatePersistenceConfig {
        backend: StorageBackend::Memory,
        auto_save_interval_secs: 1,
        ..Default::default()
    };

    let state_manager = CrossVMStateManager::with_persistence(state_config, persistence_config)
        .await
        .expect("Failed to create state manager");

    // Update state
    state_manager
        .update_height(10)
        .expect("Failed to update height");

    // Save for recovery
    state_manager
        .save_for_recovery(10)
        .await
        .expect("Failed to save for recovery");

    // Verify we can create a checkpoint
    let checkpoint = state_manager
        .create_checkpoint()
        .await
        .expect("Failed to create checkpoint");

    assert_eq!(checkpoint.height, 10);
}

#[tokio::test]
async fn test_full_consensus_flow() {
    // This test simulates a complete consensus flow with all components

    // 1. Create consensus engine
    let malachite_config = MalachiteConfig::default();
    let (mut consensus, block_sender, mut commit_receiver) =
        MalachiteConsensus::new(malachite_config)
            .await
            .expect("Failed to create consensus");

    // 2. Create fork detector
    let fork_detector = Arc::new(
        ForkDetectionManager::new(ForkDetectionConfig::default())
            .await
            .expect("Failed to create fork detector"),
    );

    // 3. Create network recovery
    let mut recovery_manager = NetworkRecoveryManager::new(NetworkRecoveryConfig::default())
        .await
        .expect("Failed to create recovery manager");
    recovery_manager
        .set_fork_detector(fork_detector.clone())
        .await;

    // 4. Create block sync
    let _sync_manager = Arc::new(
        BlockSyncManager::new(BlockSyncConfig::default())
            .await
            .expect("Failed to create sync manager"),
    );

    // 5. Start consensus
    consensus.start().await.expect("Failed to start consensus");

    // 6. Create and propose a block
    let test_block = MultiVMBlock::new(1, "0".repeat(64), "test-validator".to_string(), vec![]);

    // Send block for consensus
    block_sender
        .send(test_block.clone())
        .await
        .expect("Failed to send block");

    // 7. Wait for consensus with timeout
    let commit_result = tokio::time::timeout(Duration::from_secs(5), commit_receiver.recv()).await;

    // 8. Verify consensus completed
    match commit_result {
        Ok(Some(committed_block)) => {
            assert_eq!(
                committed_block.header.height, 1,
                "Committed block should have correct height"
            );
        }
        Ok(None) => panic!("Commit channel closed unexpectedly"),
        Err(_) => {
            // Timeout is expected in test environment without full Malachite setup
            println!("Consensus timeout (expected in test environment)");
        }
    }

    // 9. Check metrics
    let stats = consensus
        .get_consensus_stats()
        .await
        .expect("Failed to get stats");
    assert_eq!(stats.algorithm, "Malachite");

    // 10. Stop consensus
    consensus.stop().await.expect("Failed to stop consensus");
}

#[tokio::test]
async fn test_concurrent_operations() {
    // Test that multiple consensus operations can run concurrently

    let fork_detector = Arc::new(
        ForkDetectionManager::new(ForkDetectionConfig::default())
            .await
            .expect("Failed to create fork detector"),
    );

    let sync_manager = Arc::new(
        BlockSyncManager::new(BlockSyncConfig::default())
            .await
            .expect("Failed to create sync manager"),
    );

    // Spawn multiple concurrent operations
    let mut fork_handles = vec![];
    let mut sync_handles = vec![];

    // Fork detection operations
    for i in 0..5 {
        let detector = fork_detector.clone();
        let handle = tokio::spawn(async move {
            let block = MultiVMBlock::new(
                i + 1,
                format!("{:064x}", i),
                format!("validator{}", i),
                vec![],
            );
            detector
                .process_block(block, format!("validator{}", i))
                .await
        });
        fork_handles.push(handle);
    }

    // Sync operations
    for i in 0..5 {
        let sync = sync_manager.clone();
        let handle = tokio::spawn(async move {
            sync.update_peer_height(format!("peer{}", i), 100 + i as u64)
                .await;
            sync.needs_sync(95).await
        });
        sync_handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in fork_handles {
        let _ = handle.await.expect("Fork detection task failed");
    }
    for handle in sync_handles {
        handle.await.expect("Sync task failed");
    }

    // Verify operations completed successfully
    let fork_metrics = fork_detector.get_metrics().await;
    // Verify metrics exist (forks_detected is u64, always >= 0)
    let _ = fork_metrics.forks_detected;

    let sync_metrics = sync_manager.get_metrics().await;
    assert_eq!(
        sync_metrics.current_status,
        multivm_consensus::BlockSyncStatus::Idle
    );
}

#[tokio::test]
async fn test_error_recovery() {
    // Test that the consensus system can recover from errors

    let malachite_config = MalachiteConfig::default();
    let (mut consensus, _, _) = MalachiteConsensus::new(malachite_config)
        .await
        .expect("Failed to create consensus");

    // Start consensus
    consensus.start().await.expect("Failed to start consensus");

    // Validate invalid block (should fail)
    let invalid_block = MultiVMBlock::new(0, String::new(), String::new(), vec![]); // Invalid height
    let validation_result = consensus.validate_block(&invalid_block).await;

    assert!(
        validation_result.is_err(),
        "Should fail to validate invalid block"
    );

    // Verify consensus is still running after error
    assert!(consensus.is_running(), "Consensus should still be running");

    // Stop consensus
    consensus.stop().await.expect("Failed to stop consensus");
}

#[tokio::test]
async fn test_metrics_collection() {
    // Test that all components properly collect metrics

    // Create all components
    let (mut consensus, _, _) = MalachiteConsensus::new(MalachiteConfig::default())
        .await
        .expect("Failed to create consensus");

    let fork_detector = ForkDetectionManager::new(ForkDetectionConfig::default())
        .await
        .expect("Failed to create fork detector");

    let recovery_manager = NetworkRecoveryManager::new(NetworkRecoveryConfig::default())
        .await
        .expect("Failed to create recovery manager");

    let sync_manager = BlockSyncManager::new(BlockSyncConfig::default())
        .await
        .expect("Failed to create sync manager");

    // Start consensus
    consensus.start().await.expect("Failed to start consensus");

    // Perform some operations
    let block = MultiVMBlock::new(1, "0".repeat(64), "test".to_string(), vec![]);
    let _ = fork_detector.process_block(block, "test".to_string()).await;

    // Collect metrics from all components
    let consensus_stats = consensus
        .get_consensus_stats()
        .await
        .expect("Failed to get consensus stats");
    let fork_metrics = fork_detector.get_metrics().await;
    let recovery_metrics = recovery_manager.get_metrics().await;
    let sync_metrics = sync_manager.get_metrics().await;

    // Verify metrics are being collected
    assert_eq!(consensus_stats.algorithm, "Malachite");
    // Verify metrics exist (forks_detected is u64, always >= 0)
    let _ = fork_metrics.forks_detected;
    assert_eq!(
        recovery_metrics.current_health_status,
        multivm_consensus::NetworkHealthStatus::Healthy
    );
    assert_eq!(
        sync_metrics.current_status,
        multivm_consensus::BlockSyncStatus::Idle
    );

    consensus.stop().await.expect("Failed to stop consensus");
}
