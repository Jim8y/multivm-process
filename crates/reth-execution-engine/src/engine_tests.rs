//! Comprehensive tests for Reth Execution Engine
//!
//! Tests transaction execution, state management, and RPC functionality

use crate::engine::{RethEngine, EngineConfig, EngineMetrics};
use crate::rpc::RethRpcClient;
use multivm_common::{BlockchainType, ProcessId, EngineState};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_config() -> EngineConfig {
        EngineConfig {
            chain_id: 1,
            network_id: 1,
            data_dir: "/tmp/reth-test".to_string(),
            rpc_port: 8545,
            p2p_port: 30303,
            enable_discovery: false,
            max_peers: 0, // Disable networking for tests
            gas_limit: 30_000_000,
            gas_price: 20_000_000_000, // 20 gwei
            coinbase: "0x0000000000000000000000000000000000000000".to_string(),
            enable_mining: false,
            verbosity: "info".to_string(),
            metrics_enabled: true,
            archive_mode: false,
            sync_mode: "fast".to_string(),
        }
    }
    
    #[tokio::test]
    async fn test_engine_initialization() {
        let config = create_test_config();
        let engine = RethEngine::new(config.clone());
        
        assert_eq!(engine.config.chain_id, 1);
        assert_eq!(engine.config.rpc_port, 8545);
        assert!(!engine.is_running().await);
    }
    
    #[tokio::test]
    async fn test_engine_start_stop() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        // Start engine
        let result = engine.start().await;
        assert!(result.is_ok(), "Engine should start successfully");
        
        // Give it time to initialize
        sleep(Duration::from_millis(100)).await;
        assert!(engine.is_running().await, "Engine should be running");
        
        // Stop engine
        let result = engine.stop().await;
        assert!(result.is_ok(), "Engine should stop successfully");
        
        sleep(Duration::from_millis(100)).await;
        assert!(!engine.is_running().await, "Engine should be stopped");
    }
    
    #[tokio::test]
    async fn test_engine_state_retrieval() {
        let config = create_test_config();
        let engine = RethEngine::new(config);
        
        let state = engine.get_state().await.unwrap();
        
        assert_eq!(state.process_id, ProcessId::Ethereum);
        assert_eq!(state.blockchain_type, BlockchainType::Ethereum);
        assert_eq!(state.chain_id, 1);
        assert!(!state.is_syncing);
        assert!(state.rpc_endpoints.contains(&"http://127.0.0.1:8545".to_string()));
    }
    
    #[tokio::test]
    async fn test_engine_metrics() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        // Start engine to begin collecting metrics
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(100)).await;
        
        let metrics = engine.get_metrics().await;
        
        // Check basic metrics structure
        assert!(metrics.uptime_seconds >= 0);
        assert_eq!(metrics.blocks_processed, 0); // No blocks processed yet
        assert_eq!(metrics.transactions_processed, 0);
        assert!(metrics.memory_usage_bytes > 0);
        
        engine.stop().await.expect("Engine should stop");
    }
    
    #[tokio::test]
    async fn test_block_processing() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(200)).await;
        
        // Create test block data
        let block_data = create_test_block_data();
        
        // Process block
        let result = engine.process_block(block_data, true).await;
        assert!(result.is_ok(), "Block processing should succeed");
        
        let response = result.unwrap();
        assert!(response.success, "Block should be processed successfully");
        assert!(response.block_hash.is_some(), "Block hash should be provided");
        assert!(response.gas_used > 0, "Some gas should be used");
        
        // Check metrics updated
        let metrics = engine.get_metrics().await;
        assert_eq!(metrics.blocks_processed, 1);
        assert!(metrics.transactions_processed > 0);
        
        engine.stop().await.expect("Engine should stop");
    }
    
    #[tokio::test]
    async fn test_invalid_block_handling() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(200)).await;
        
        // Create invalid block data
        let invalid_block = vec![0u8; 10]; // Too small to be valid
        
        // Process invalid block
        let result = engine.process_block(invalid_block, true).await;
        
        // Should handle gracefully
        match result {
            Ok(response) => assert!(!response.success, "Invalid block should fail"),
            Err(_) => {}, // Error is also acceptable
        }
        
        engine.stop().await.expect("Engine should stop");
    }
    
    #[tokio::test]
    async fn test_concurrent_block_processing() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(200)).await;
        
        // Process multiple blocks concurrently
        let mut handles = vec![];
        
        for i in 0..5 {
            let block_data = create_test_block_data_with_nonce(i);
            let engine_clone = engine.clone(); // This would need Arc<RwLock<>> wrapper
            
            let handle = tokio::spawn(async move {
                engine_clone.process_block(block_data, true).await
            });
            handles.push(handle);
        }
        
        // Wait for all to complete
        let mut success_count = 0;
        for handle in handles {
            if let Ok(Ok(response)) = handle.await {
                if response.success {
                    success_count += 1;
                }
            }
        }
        
        assert!(success_count > 0, "At least one block should process successfully");
        
        engine.stop().await.expect("Engine should stop");
    }
    
    #[tokio::test]
    async fn test_transaction_validation() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(200)).await;
        
        // Test various transaction scenarios
        let test_cases = vec![
            ("valid_transfer", create_valid_transfer_tx(), true),
            ("invalid_signature", create_invalid_signature_tx(), false),
            ("insufficient_gas", create_insufficient_gas_tx(), false),
            ("invalid_nonce", create_invalid_nonce_tx(), false),
        ];
        
        for (name, tx_data, should_succeed) in test_cases {
            let block_data = create_block_with_transaction(tx_data);
            let result = engine.process_block(block_data, true).await;
            
            match result {
                Ok(response) => {
                    if should_succeed {
                        assert!(response.success, "Transaction {} should succeed", name);
                    } else {
                        assert!(!response.success, "Transaction {} should fail", name);
                    }
                }
                Err(e) if !should_succeed => {
                    // Error is acceptable for invalid transactions
                    println!("Expected error for {}: {}", name, e);
                }
                Err(e) => panic!("Unexpected error for {}: {}", name, e),
            }
        }
        
        engine.stop().await.expect("Engine should stop");
    }
    
    #[tokio::test]
    async fn test_state_consistency() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(200)).await;
        
        // Get initial state
        let initial_state = engine.get_state().await.unwrap();
        let initial_block = initial_state.current_block.unwrap_or(0);
        
        // Process a few blocks
        for i in 0..3 {
            let block_data = create_test_block_data_with_nonce(i);
            let result = engine.process_block(block_data, true).await;
            assert!(result.is_ok(), "Block {} should process", i);
        }
        
        // Check state progression
        let final_state = engine.get_state().await.unwrap();
        let final_block = final_state.current_block.unwrap_or(0);
        
        assert!(final_block > initial_block, "Block height should increase");
        assert!(!final_state.state_root.is_empty(), "State root should be updated");
        
        engine.stop().await.expect("Engine should stop");
    }
    
    #[tokio::test]
    async fn test_engine_restart_recovery() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config.clone());
        
        // Start and process some blocks
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(200)).await;
        
        let block_data = create_test_block_data();
        engine.process_block(block_data, true).await.expect("Block should process");
        
        let state_before = engine.get_state().await.unwrap();
        
        // Stop engine
        engine.stop().await.expect("Engine should stop");
        sleep(Duration::from_millis(100)).await;
        
        // Create new engine instance (simulating restart)
        let mut engine2 = RethEngine::new(config);
        engine2.start().await.expect("Engine should restart");
        sleep(Duration::from_millis(200)).await;
        
        let state_after = engine2.get_state().await.unwrap();
        
        // State should be consistent after block processing
        assert_eq!(state_before.chain_id, state_after.chain_id);
        assert_eq!(state_before.blockchain_type, state_after.blockchain_type);
        
        engine2.stop().await.expect("Engine should stop");
    }
    
    #[tokio::test]
    async fn test_memory_management() {
        let config = create_test_config();
        let mut engine = RethEngine::new(config);
        
        engine.start().await.expect("Engine should start");
        sleep(Duration::from_millis(200)).await;
        
        let initial_metrics = engine.get_metrics().await;
        let initial_memory = initial_metrics.memory_usage_bytes;
        
        // Process many blocks to test memory usage
        for i in 0..50 {
            let block_data = create_test_block_data_with_nonce(i);
            let _ = engine.process_block(block_data, true).await;
        }
        
        let final_metrics = engine.get_metrics().await;
        let final_memory = final_metrics.memory_usage_bytes;
        
        // Memory should not grow unboundedly
        let memory_growth_ratio = final_memory as f64 / initial_memory as f64;
        assert!(memory_growth_ratio < 2.0, "Memory usage should not double: {:.2}x growth", memory_growth_ratio);
        
        engine.stop().await.expect("Engine should stop");
    }
    
    // Helper functions for creating test data
    
    fn create_test_block_data() -> Vec<u8> {
        // Create minimal valid block data for testing
        let block = serde_json::json!({
            "number": "0x1",
            "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "nonce": "0x0000000000000000",
            "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "stateRoot": "0xd7f8974fb5ac78d9ac099b9ad5018bedc2ce0a72dad1827a1709da30580f0544",
            "receiptsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
            "miner": "0x0000000000000000000000000000000000000000",
            "difficulty": "0x20000",
            "totalDifficulty": "0x20000",
            "extraData": "0x",
            "size": "0x218",
            "gasLimit": "0x1c9c380",
            "gasUsed": "0x0",
            "timestamp": "0x54e34e8e",
            "transactions": [],
            "uncles": []
        });
        
        serde_json::to_vec(&block).unwrap()
    }
    
    fn create_test_block_data_with_nonce(nonce: u64) -> Vec<u8> {
        let block = serde_json::json!({
            "number": format!("0x{:x}", nonce + 1),
            "hash": format!("0x{:064x}", nonce),
            "parentHash": if nonce == 0 { 
                "0x0000000000000000000000000000000000000000000000000000000000000000".to_string()
            } else { 
                format!("0x{:064x}", nonce - 1) 
            },
            "nonce": format!("0x{:016x}", nonce),
            "gasLimit": "0x1c9c380",
            "gasUsed": "0x5208",
            "timestamp": format!("0x{:x}", 1609459200 + nonce * 12), // 12 second blocks
            "transactions": [],
            "uncles": []
        });
        
        serde_json::to_vec(&block).unwrap()
    }
    
    fn create_valid_transfer_tx() -> Vec<u8> {
        let tx = serde_json::json!({
            "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66",
            "to": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "value": "0xde0b6b3a7640000", // 1 ETH
            "gas": "0x5208", // 21000
            "gasPrice": "0x4a817c800", // 20 gwei
            "nonce": "0x1",
            "data": "0x"
        });
        
        serde_json::to_vec(&tx).unwrap()
    }
    
    fn create_invalid_signature_tx() -> Vec<u8> {
        let tx = serde_json::json!({
            "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66",
            "to": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "value": "0xde0b6b3a7640000",
            "gas": "0x5208",
            "gasPrice": "0x4a817c800",
            "nonce": "0x1",
            "data": "0x",
            "v": "0x1b",
            "r": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "s": "0x0000000000000000000000000000000000000000000000000000000000000000"
        });
        
        serde_json::to_vec(&tx).unwrap()
    }
    
    fn create_insufficient_gas_tx() -> Vec<u8> {
        let tx = serde_json::json!({
            "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66",
            "to": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "value": "0xde0b6b3a7640000",
            "gas": "0x1000", // Too low
            "gasPrice": "0x4a817c800",
            "nonce": "0x1",
            "data": "0x"
        });
        
        serde_json::to_vec(&tx).unwrap()
    }
    
    fn create_invalid_nonce_tx() -> Vec<u8> {
        let tx = serde_json::json!({
            "from": "0x742d35Cc6634C0532925a3b844Bc9e7595f8fA66",
            "to": "0x5FbDB2315678afecb367f032d93F642f64180aa3",
            "value": "0xde0b6b3a7640000",
            "gas": "0x5208",
            "gasPrice": "0x4a817c800",
            "nonce": "0xffffffff", // Invalid high nonce
            "data": "0x"
        });
        
        serde_json::to_vec(&tx).unwrap()
    }
    
    fn create_block_with_transaction(tx_data: Vec<u8>) -> Vec<u8> {
        let tx: serde_json::Value = serde_json::from_slice(&tx_data).unwrap();
        
        let block = serde_json::json!({
            "number": "0x1",
            "hash": "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            "parentHash": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "gasLimit": "0x1c9c380",
            "gasUsed": "0x5208",
            "timestamp": "0x54e34e8e",
            "transactions": [tx],
            "uncles": []
        });
        
        serde_json::to_vec(&block).unwrap()
    }
}