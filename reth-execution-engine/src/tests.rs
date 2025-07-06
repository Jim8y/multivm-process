#[cfg(test)]
mod tests {
    use crate::engine::*;
    use crate::engine_api::*;
    use multivm_common::{traits::ExecutionEngine, BlockchainType, EngineState, ProcessId};
    use multivm_common::{ExecutionResult, TransactionData};
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_reth_engine_creation() {
        let data_dir = PathBuf::from("/tmp/test-reth");
        let rpc_port = 8545;
        let chain_id = 1337;
        
        let engine = RethExecutionEngine::new(data_dir.clone(), rpc_port, chain_id).await.unwrap();
        
        assert_eq!(engine.chain_id, chain_id);
        assert_eq!(engine.rpc_port, rpc_port);
        assert_eq!(engine.data_dir, data_dir);
    }

    #[tokio::test]
    async fn test_engine_initialization() {
        let engine = RethExecutionEngine::new(
            PathBuf::from("/tmp/test-reth-init"),
            8546,
            1337
        ).await.unwrap();
        
        // In mock mode, initialization should always succeed
        let result = engine.initialize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_engine_ready_state() {
        let mut engine = RethExecutionEngine::new(
            PathBuf::from("/tmp/test-reth-ready"),
            8547,
            1337
        ).await.unwrap();
        
        // Initially not ready
        assert!(!engine.is_ready().await);
        
        // After initialization, should be ready
        engine.initialize().await.unwrap();
        assert!(engine.is_ready().await);
    }

    #[tokio::test]
    async fn test_get_engine_state() {
        let mut engine = RethExecutionEngine::new(
            PathBuf::from("/tmp/test-reth-state"),
            8548,
            1337
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        let state = engine.get_engine_state().await.unwrap();
        assert_eq!(state.process_id, ProcessId::Ethereum);
        assert_eq!(state.blockchain_type, BlockchainType::Ethereum);
        assert_eq!(state.chain_id, 1337);
        assert!(!state.is_syncing);
    }

    #[tokio::test]
    async fn test_execute_transaction_mock() {
        let mut engine = RethExecutionEngine::new(
            PathBuf::from("/tmp/test-reth-execute"),
            8549,
            1337
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        let tx_data = TransactionData {
            from: "0x1234567890123456789012345678901234567890".to_string(),
            to: Some("0x0987654321098765432109876543210987654321".to_string()),
            value: Some("1000000000000000000".to_string()), // 1 ETH
            data: None,
            gas_limit: Some(21000),
            gas_price: Some("20000000000".to_string()), // 20 Gwei
            nonce: Some(0),
            chain_id: Some(1337),
        };
        
        let result = engine.execute_transaction(tx_data).await.unwrap();
        
        // In mock mode, transactions always succeed
        assert!(result.success);
        assert!(result.transaction_hash.starts_with("0x"));
        assert_eq!(result.transaction_hash.len(), 66); // 0x + 64 hex chars
    }

    #[tokio::test]
    async fn test_execute_block_mock() {
        let mut engine = RethExecutionEngine::new(
            PathBuf::from("/tmp/test-reth-block"),
            8550,
            1337
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        // Create a simple block with one transaction
        let tx = TransactionData {
            from: "0xabc".to_string(),
            to: Some("0xdef".to_string()),
            value: Some("1000".to_string()),
            data: None,
            gas_limit: Some(21000),
            gas_price: Some("1000".to_string()),
            nonce: Some(0),
            chain_id: Some(1337),
        };
        
        let block_data = serde_json::json!({
            "number": 1,
            "transactions": [tx],
            "timestamp": 1234567890,
            "gasLimit": 8000000,
        });
        
        let block_bytes = serde_json::to_vec(&block_data).unwrap();
        let result = engine.execute_block(block_bytes).await.unwrap();
        
        assert!(result.success);
        assert_eq!(result.executed_transactions, 1);
        assert!(result.block_hash.starts_with("0x"));
    }

    #[tokio::test]
    async fn test_sync_to_block() {
        let mut engine = RethExecutionEngine::new(
            PathBuf::from("/tmp/test-reth-sync"),
            8551,
            1337
        ).await.unwrap();
        
        engine.initialize().await.unwrap();
        
        // In mock mode, sync always succeeds
        let result = engine.sync_to_block(100).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_engine_state_tracking() {
        cfg_if::cfg_if! {
            if #[cfg(feature = "mock")] {
                let mut engine = RethExecutionEngine::new(
                    PathBuf::from("/tmp/test-reth-tracking"),
                    8552,
                    1337
                ).await.unwrap();
                
                engine.initialize().await.unwrap();
                
                // Execute some transactions
                for i in 0..5 {
                    let tx = TransactionData {
                        from: format!("0x{:040x}", i),
                        to: Some(format!("0x{:040x}", i + 100)),
                        value: Some("1000".to_string()),
                        data: None,
                        gas_limit: Some(21000),
                        gas_price: Some("1000".to_string()),
                        nonce: Some(i),
                        chain_id: Some(1337),
                    };
                    
                    engine.execute_transaction(tx).await.unwrap();
                }
                
                // Check internal state
                let inner = engine.inner.lock().await;
                if let RethEngineInner::Mock(ref mock_state) = *inner {
                    assert_eq!(mock_state.transaction_count, 5);
                    assert_eq!(mock_state.accounts.len(), 10); // 5 from + 5 to addresses
                }
            }
        }
    }

    #[test]
    fn test_engine_api_types() {
        // Test ForkchoiceState
        let forkchoice = ForkchoiceState {
            head_block_hash: "0x1234".to_string(),
            safe_block_hash: "0x1234".to_string(),
            finalized_block_hash: "0x1234".to_string(),
        };
        
        assert_eq!(forkchoice.head_block_hash, "0x1234");
        assert_eq!(forkchoice.safe_block_hash, "0x1234");
        assert_eq!(forkchoice.finalized_block_hash, "0x1234");
        
        // Test PayloadAttributes
        let attributes = PayloadAttributes {
            timestamp: 1234567890,
            prev_randao: "0xabcd".to_string(),
            suggested_fee_recipient: "0xfeee".to_string(),
        };
        
        assert_eq!(attributes.timestamp, 1234567890);
        assert_eq!(attributes.prev_randao, "0xabcd");
        assert_eq!(attributes.suggested_fee_recipient, "0xfeee");
        
        // Test PayloadStatus
        let status = PayloadStatus {
            status: "VALID".to_string(),
            latest_valid_hash: Some("0x5678".to_string()),
            validation_error: None,
        };
        
        assert_eq!(status.status, "VALID");
        assert_eq!(status.latest_valid_hash, Some("0x5678".to_string()));
        assert!(status.validation_error.is_none());
    }

    #[test]
    fn test_execution_payload_serialization() {
        let payload = ExecutionPayload {
            parent_hash: "0x0000".to_string(),
            fee_recipient: "0x1111".to_string(),
            state_root: "0x2222".to_string(),
            receipts_root: "0x3333".to_string(),
            logs_bloom: "0x0000".to_string(),
            prev_randao: "0x4444".to_string(),
            block_number: 100,
            gas_limit: 8000000,
            gas_used: 1000000,
            timestamp: 1234567890,
            extra_data: "0x".to_string(),
            base_fee_per_gas: "1000000000".to_string(),
            block_hash: "0x5555".to_string(),
            transactions: vec!["0xabc".to_string(), "0xdef".to_string()],
        };
        
        // Test serialization
        let json = serde_json::to_string(&payload).unwrap();
        let deserialized: ExecutionPayload = serde_json::from_str(&json).unwrap();
        
        assert_eq!(payload.parent_hash, deserialized.parent_hash);
        assert_eq!(payload.block_number, deserialized.block_number);
        assert_eq!(payload.transactions.len(), deserialized.transactions.len());
    }
}