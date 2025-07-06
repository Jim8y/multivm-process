#[cfg(test)]
mod tests {
    use crate::engine::*;
    use crate::simple_engine::*;
    use multivm_common::{traits::ExecutionEngine, BlockchainType, EngineState, ProcessId};
    use multivm_common::{ExecutionResult, TransactionData};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_solana_engine_creation() {
        let data_dir = PathBuf::from("/tmp/test-solana");
        let rpc_port = 8899;
        let cluster = "devnet";
        
        let engine = SolanaExecutionEngine::new(data_dir.clone(), rpc_port, cluster).await.unwrap();
        
        assert_eq!(engine.cluster, cluster);
        assert_eq!(engine.rpc_port, rpc_port);
        assert_eq!(engine.data_dir, data_dir);
    }

    #[tokio::test]
    async fn test_engine_initialization() {
        let engine = SolanaExecutionEngine::new(
            PathBuf::from("/tmp/test-solana-init"),
            8900,
            "devnet"
        ).await.unwrap();
        
        // In mock mode, initialization should always succeed
        let result = engine.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_engine_ready_state() {
        let mut engine = SolanaExecutionEngine::new(
            PathBuf::from("/tmp/test-solana-ready"),
            8901,
            "devnet"
        ).await.unwrap();
        
        // Initially not ready
        assert!(!engine.is_ready().await);
        
        // After initialization, should be ready
        engine.initialize().await.unwrap();
        assert!(engine.is_ready().await);
    }

    #[tokio::test]
    async fn test_get_engine_state() {
        let mut engine = SolanaExecutionEngine::new(
            PathBuf::from("/tmp/test-solana-state"),
            8902,
            "devnet"
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        let state = engine.get_engine_state().await.unwrap();
        assert_eq!(state.process_id, ProcessId::Solana);
        assert_eq!(state.blockchain_type, BlockchainType::Solana);
        assert_eq!(state.chain_id, 103); // Solana devnet chain ID
        assert!(!state.is_syncing);
    }

    #[tokio::test]
    async fn test_execute_transaction_simple() {
        let mut engine = SolanaExecutionEngine::new(
            PathBuf::from("/tmp/test-solana-execute"),
            8903,
            "devnet"
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        let tx_data = TransactionData {
            from: "11111111111111111111111111111111".to_string(), // System program
            to: Some("22222222222222222222222222222222".to_string()),
            value: Some("1000000000".to_string()), // 1 SOL in lamports
            data: None,
            gas_limit: None, // Not used in Solana
            gas_price: None, // Not used in Solana
            nonce: None,
            chain_id: None,
        };
        
        let result = engine.execute_transaction(tx_data).await.unwrap();
        
        // In simple/mock mode, transactions always succeed
        assert!(result.success);
        assert!(!result.transaction_hash.is_empty());
        assert!(result.logs.is_some());
    }

    #[tokio::test]
    async fn test_execute_block_simple() {
        let mut engine = SolanaExecutionEngine::new(
            PathBuf::from("/tmp/test-solana-block"),
            8904,
            "devnet"
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        // Create a simple block with multiple transactions
        let transactions = vec![
            TransactionData {
                from: "11111111111111111111111111111111".to_string(),
                to: Some("22222222222222222222222222222222".to_string()),
                value: Some("1000000000".to_string()),
                data: None,
                gas_limit: None,
                gas_price: None,
                nonce: None,
                chain_id: None,
            },
            TransactionData {
                from: "33333333333333333333333333333333".to_string(),
                to: Some("44444444444444444444444444444444".to_string()),
                value: Some("2000000000".to_string()),
                data: None,
                gas_limit: None,
                gas_price: None,
                nonce: None,
                chain_id: None,
            },
        ];
        
        let block_data = serde_json::json!({
            "slot": 12345,
            "transactions": transactions,
            "timestamp": 1234567890,
        });
        
        let block_bytes = serde_json::to_vec(&block_data).unwrap();
        let result = engine.execute_block(block_bytes).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.executed_transactions, 2);
        assert!(!result.block_hash.is_empty());
    }

    #[tokio::test]
    async fn test_sync_to_block() {
        let mut engine = SolanaExecutionEngine::new(
            PathBuf::from("/tmp/test-solana-sync"),
            8905,
            "devnet"
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        // In simple mode, sync always succeeds
        let result = engine.sync_to_block(100000).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_simple_state_creation() {
        let state = SimpleSolanaState::new();
        
        assert_eq!(state.slot, 0);
        assert_eq!(state.blockhash, "11111111111111111111111111111111");
        assert!(state.accounts.is_empty());
        assert_eq!(state.transaction_count, 0);
    }

    #[tokio::test]
    async fn test_simple_engine_execution() {
        let engine = SimpleSolanaEngine::new();
        
        let tx = TransactionData {
            from: "sender".to_string(),
            to: Some("receiver".to_string()),
            value: Some("1000000".to_string()),
            data: None,
            gas_limit: None,
            gas_price: None,
            nonce: None,
            chain_id: None,
        };
        
        let result = engine.execute_transaction(tx).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.transaction_hash.len(), 64); // 32 bytes hex encoded
        assert!(result.logs.is_some());
        
        let logs = result.logs.unwrap();
        assert!(logs.contains("Transfer"));
        assert!(logs.contains("sender"));
        assert!(logs.contains("receiver"));
        assert!(logs.contains("1000000"));
    }

    #[tokio::test]
    async fn test_simple_engine_block_execution() {
        let engine = SimpleSolanaEngine::new();
        
        let block_data = serde_json::json!({
            "slot": 100,
            "transactions": [
                {
                    "from": "alice",
                    "to": "bob",
                    "value": "1000"
                },
                {
                    "from": "charlie",
                    "to": "dave",
                    "value": "2000"
                }
            ]
        });
        
        let block_bytes = serde_json::to_vec(&block_data).unwrap();
        let result = engine.execute_block(block_bytes).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.executed_transactions, 2);
        assert!(!result.block_hash.is_empty());
        assert!(result.gas_used > 0);
    }

    #[tokio::test]
    async fn test_simple_engine_state_tracking() {
        let engine = SimpleSolanaEngine::new();
        
        // Execute several transactions
        for i in 0..5 {
            let tx = TransactionData {
                from: format!("sender{}", i),
                to: Some(format!("receiver{}", i)),
                value: Some((1000 * (i + 1)).to_string()),
                data: None,
                gas_limit: None,
                gas_price: None,
                nonce: None,
                chain_id: None,
            };
            
            engine.execute_transaction(tx).await.unwrap();
        }
        
        let state = engine.state.lock().await;
        assert_eq!(state.transaction_count, 5);
        assert_eq!(state.accounts.len(), 10); // 5 senders + 5 receivers
        
        // Check account balances
        assert_eq!(*state.accounts.get("sender0").unwrap(), 999000); // 1M - 1000
        assert_eq!(*state.accounts.get("receiver0").unwrap(), 1001000); // 1M + 1000
    }

    #[test]
    fn test_solana_tx_data_validation() {
        // Test valid Solana address (32 bytes base58)
        let valid_address = "11111111111111111111111111111111";
        assert_eq!(valid_address.len(), 32);
        
        // Test transaction data structure
        let tx = TransactionData {
            from: valid_address.to_string(),
            to: Some(valid_address.to_string()),
            value: Some("1000000000".to_string()), // 1 SOL
            data: Some(vec![1, 2, 3, 4]), // Instruction data
            gas_limit: None, // Not used in Solana
            gas_price: None, // Not used in Solana
            nonce: None, // Handled differently in Solana
            chain_id: None, // Not used in Solana
        };
        
        // Verify structure
        assert!(tx.to.is_some());
        assert!(tx.value.is_some());
        assert!(tx.data.is_some());
        assert!(tx.gas_limit.is_none());
        assert!(tx.gas_price.is_none());
    }

    #[tokio::test] 
    async fn test_engine_error_handling() {
        let engine = SimpleSolanaEngine::new();
        
        // Test transaction with invalid value
        let invalid_tx = TransactionData {
            from: "sender".to_string(),
            to: Some("receiver".to_string()),
            value: Some("not_a_number".to_string()),
            data: None,
            gas_limit: None,
            gas_price: None,
            nonce: None,
            chain_id: None,
        };
        
        // Should handle gracefully by using default value
        let result = engine.execute_transaction(invalid_tx).await.unwrap();
        assert!(result.success); // Simple engine always succeeds
    }
}