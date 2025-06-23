//! Comprehensive tests for Solana Execution Engine
//!
//! Tests transaction execution, state management, and RPC functionality

use crate::engine::{SolanaBlockData, SolanaConfig, SolanaExecutionEngine, SolanaTransaction};
use multivm_common::{traits::execution::ExecutionEngine, BlockchainType, ProcessId};
use solana_sdk::hash::Hash;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> SolanaConfig {
        SolanaConfig {
            data_dir: "/tmp/solana-test".into(),
            rpc_addr: "127.0.0.1".to_string(),
            rpc_port: 8899,
            max_compute_units: 1_000_000,
        }
    }

    #[tokio::test]
    async fn test_engine_initialization() {
        let config = create_test_config();
        let engine = SolanaExecutionEngine::new_with_mode(config.clone(), true); // Mock mode

        // Test that the engine was created successfully
        assert!(!engine.is_ready().await);
    }

    #[tokio::test]
    async fn test_engine_start_stop() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        // Initialize engine
        let result = engine.initialize().await;
        assert!(result.is_ok(), "Engine should initialize successfully");

        // Give it time to initialize
        sleep(Duration::from_millis(100)).await;
        assert!(engine.is_ready().await, "Engine should be ready");

        // Shutdown engine
        let result = engine.shutdown(Some(Duration::from_secs(5))).await;
        assert!(result.is_ok(), "Engine should shutdown successfully");
    }

    #[tokio::test]
    async fn test_engine_state_retrieval() {
        let config = create_test_config();
        let engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        let state = engine.get_state().await.unwrap();

        assert_eq!(state.process_id, ProcessId::Solana);
        assert_eq!(state.blockchain_type, BlockchainType::Solana);
        assert_eq!(state.chain_id, 103); // Solana devnet
        assert!(!state.is_syncing);
        assert!(state
            .rpc_endpoints
            .contains(&"http://127.0.0.1:8899".to_string()));
    }

    #[tokio::test]
    async fn test_engine_health() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        // Initialize engine to get proper health status
        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(100)).await;

        let health = engine.get_health().await.unwrap();

        // Check basic health structure
        assert_eq!(health.process_id, ProcessId::Solana);
        assert!(health.is_healthy);
        assert!(health.uptime.as_millis() > 0);
        assert!(health.memory_usage > 0);
        assert!(health.rpc_active);
    }

    #[tokio::test]
    async fn test_block_processing() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(200)).await;

        // Create test block data
        let block_data = create_test_block_data();

        // Process block
        let result = engine.process_block(block_data).await;
        assert!(result.is_ok(), "Block processing should succeed");

        let response = result.unwrap();
        assert!(response.success, "Block should be processed successfully");
        assert_eq!(response.slot, 1);
        assert!(
            response.compute_units_used > 0,
            "Some compute units should be used"
        );

        // Check health updated
        let health = engine.get_health().await.unwrap();
        assert_eq!(health.blocks_processed_total, 1);
    }

    #[tokio::test]
    async fn test_invalid_block_handling() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(200)).await;

        // Create block with invalid transaction data
        let invalid_block = create_block_with_invalid_transaction();

        // Process invalid block - should still succeed in mock mode but show warnings
        let result = engine.process_block(invalid_block).await;

        // In mock mode, processing should succeed
        assert!(
            result.is_ok(),
            "Block processing should handle invalid data gracefully"
        );
    }

    #[tokio::test]
    async fn test_concurrent_block_processing() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(200)).await;

        // Process multiple blocks sequentially (engine is not Clone, so no concurrent access)
        let mut success_count = 0;

        for i in 0..5 {
            let block_data = create_test_block_data_with_slot(i + 1);
            let result = engine.process_block(block_data).await;

            if result.is_ok() && result.unwrap().success {
                success_count += 1;
            }
        }

        assert_eq!(success_count, 5, "All blocks should process successfully");

        // Check final state
        let health = engine.get_health().await.unwrap();
        assert_eq!(health.blocks_processed_total, 5);
    }

    #[tokio::test]
    async fn test_transaction_validation() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(200)).await;

        // Test various transaction scenarios
        let test_cases = vec![
            ("valid_transfer", create_valid_transfer_tx(), true),
            ("invalid_signature", create_invalid_signature_tx(), true), // Mock mode accepts all
            ("large_compute", create_large_compute_tx(), true),
            ("empty_data", create_empty_data_tx(), true),
        ];

        for (name, tx_data, should_succeed) in test_cases {
            let block_data = create_block_with_transaction(tx_data);
            let result = engine.process_block(block_data).await;

            match result {
                Ok(response) => {
                    if should_succeed {
                        assert!(response.success, "Transaction {} should succeed", name);
                    }
                }
                Err(e) if !should_succeed => {
                    // Error is acceptable for invalid transactions
                    println!("Expected error for {}: {}", name, e);
                }
                Err(e) => panic!("Unexpected error for {}: {}", name, e),
            }
        }
    }

    #[tokio::test]
    async fn test_state_consistency() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(200)).await;

        // Get initial state
        let initial_state = engine.get_state().await.unwrap();
        let initial_slot = initial_state.current_block.unwrap_or(0);

        // Process a few blocks
        for i in 1..=3 {
            let block_data = create_test_block_data_with_slot(i);
            let result = engine.process_block(block_data).await;
            assert!(result.is_ok(), "Block {} should process", i);
        }

        // Check state progression
        let final_state = engine.get_state().await.unwrap();
        let final_slot = final_state.current_block.unwrap_or(0);

        assert!(final_slot > initial_slot, "Slot should increase");
        assert!(
            !final_state.state_root.is_empty(),
            "State root should be updated"
        );
    }

    #[tokio::test]
    async fn test_engine_restart_recovery() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config.clone(), true); // Mock mode

        // Initialize and process some blocks
        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(200)).await;

        let block_data = create_test_block_data();
        engine
            .process_block(block_data)
            .await
            .expect("Block should process");

        let state_before = engine.get_state().await.unwrap();

        // Shutdown engine
        engine
            .shutdown(Some(Duration::from_secs(5)))
            .await
            .expect("Engine should shutdown");
        sleep(Duration::from_millis(100)).await;

        // Create new engine instance (simulating restart)
        let mut engine2 = SolanaExecutionEngine::new_with_mode(config, true);
        engine2.initialize().await.expect("Engine should restart");
        sleep(Duration::from_millis(200)).await;

        let state_after = engine2.get_state().await.unwrap();

        // State should be consistent (basic properties)
        assert_eq!(state_before.chain_id, state_after.chain_id);
        assert_eq!(state_before.blockchain_type, state_after.blockchain_type);
    }

    #[tokio::test]
    async fn test_compute_units_tracking() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(200)).await;

        // Create block with known compute units
        let mut block_data = create_test_block_data();
        block_data.transactions[0].compute_units = 10000;
        if block_data.transactions.len() > 1 {
            block_data.transactions[1].compute_units = 15000;
        }

        let result = engine.process_block(block_data).await.unwrap();

        // Should track compute units correctly
        assert!(
            result.compute_units_used >= 10000,
            "Should track compute units used"
        );

        // Check metrics
        let metrics = engine.get_metrics().await.unwrap();
        assert!(
            metrics.compute_units_used > 0,
            "Metrics should show compute units used"
        );
    }

    #[tokio::test]
    async fn test_blockchain_type() {
        let config = create_test_config();
        let engine = SolanaExecutionEngine::new_with_mode(config, true);

        assert_eq!(engine.blockchain_type(), BlockchainType::Solana);
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let config = create_test_config();
        let mut engine = SolanaExecutionEngine::new_with_mode(config, true); // Mock mode

        // Initialize engine to begin collecting metrics
        engine.initialize().await.expect("Engine should initialize");
        sleep(Duration::from_millis(100)).await;

        let metrics = engine.get_metrics().await.unwrap();

        // Check basic metrics structure
        assert!(metrics.memory_usage_bytes > 0);
        assert_eq!(metrics.transaction_count, 0); // No transactions processed yet
        assert_eq!(metrics.compute_units_used, 0);
        // CPU time is always non-negative for Duration
        assert!(metrics.cpu_time.as_millis() < u128::MAX);
    }

    // Helper functions for creating test data

    fn create_test_block_data() -> SolanaBlockData {
        SolanaBlockData {
            slot: 1,
            block_hash: Hash::new_from_array([1u8; 32]),
            parent_slot: 0,
            transactions: vec![
                SolanaTransaction {
                    signature: "test_signature_1".to_string(),
                    data: vec![0u8; 64],
                    compute_units: 5000,
                },
                SolanaTransaction {
                    signature: "test_signature_2".to_string(),
                    data: vec![1u8; 64],
                    compute_units: 7500,
                },
            ],
            block_time: Some(1609459200),
            previous_blockhash: Hash::new_from_array([0u8; 32]),
        }
    }

    fn create_test_block_data_with_slot(slot: u64) -> SolanaBlockData {
        SolanaBlockData {
            slot,
            block_hash: Hash::new_from_array([slot as u8; 32]),
            parent_slot: slot.saturating_sub(1),
            transactions: vec![SolanaTransaction {
                signature: format!("test_signature_{}", slot),
                data: vec![slot as u8; 64],
                compute_units: 5000 + slot * 100,
            }],
            block_time: Some(1609459200 + slot as i64 * 400), // 400ms slots
            previous_blockhash: Hash::new_from_array([(slot.saturating_sub(1)) as u8; 32]),
        }
    }

    fn create_block_with_invalid_transaction() -> SolanaBlockData {
        SolanaBlockData {
            slot: 1,
            block_hash: Hash::new_from_array([1u8; 32]),
            parent_slot: 0,
            transactions: vec![SolanaTransaction {
                signature: "invalid_signature".to_string(),
                data: vec![], // Empty data
                compute_units: 0,
            }],
            block_time: Some(1609459200),
            previous_blockhash: Hash::new_from_array([0u8; 32]),
        }
    }

    fn create_valid_transfer_tx() -> SolanaTransaction {
        SolanaTransaction {
            signature: "valid_transfer_signature".to_string(),
            data: vec![0x01, 0x02, 0x03, 0x04], // Mock valid transaction data
            compute_units: 5000,
        }
    }

    fn create_invalid_signature_tx() -> SolanaTransaction {
        SolanaTransaction {
            signature: "".to_string(), // Empty signature
            data: vec![0x01, 0x02, 0x03, 0x04],
            compute_units: 5000,
        }
    }

    fn create_large_compute_tx() -> SolanaTransaction {
        SolanaTransaction {
            signature: "large_compute_signature".to_string(),
            data: vec![0x01; 1024], // Larger transaction
            compute_units: 200_000, // High compute units
        }
    }

    fn create_empty_data_tx() -> SolanaTransaction {
        SolanaTransaction {
            signature: "empty_data_signature".to_string(),
            data: vec![],
            compute_units: 1000,
        }
    }

    fn create_block_with_transaction(tx: SolanaTransaction) -> SolanaBlockData {
        SolanaBlockData {
            slot: 1,
            block_hash: Hash::new_from_array([1u8; 32]),
            parent_slot: 0,
            transactions: vec![tx],
            block_time: Some(1609459200),
            previous_blockhash: Hash::new_from_array([0u8; 32]),
        }
    }
}
