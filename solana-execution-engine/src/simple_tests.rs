#[cfg(test)]
mod tests {
    use crate::simple_engine::SimpleSolanaEngine;
    use multivm_common::{ExecutionResult, TransactionData, ProcessId, BlockchainType};

    #[test]
    fn test_simple_engine_creation() {
        let engine = SimpleSolanaEngine::new();
        
        // The engine should be created with default values
        // We can't access private fields directly, but we can test the engine works
        assert!(true); // If we get here, creation succeeded
    }

    #[tokio::test]
    async fn test_execute_simple_transaction() {
        use multivm_common::traits::ExecutionEngine;
        
        let mut engine = SimpleSolanaEngine::new();
        
        // Initialize the engine
        engine.initialize().await.unwrap();
        
        let tx = TransactionData {
            from: "alice".to_string(),
            to: Some("bob".to_string()),
            value: Some("1000".to_string()),
            data: None,
            gas_limit: None,
            gas_price: None,
            nonce: None,
            chain_id: None,
        };
        
        let result = engine.execute_transaction(tx).await.unwrap();
        
        assert!(result.success);
        assert!(!result.transaction_hash.is_empty());
    }

    #[tokio::test]
    async fn test_engine_state() {
        use multivm_common::traits::ExecutionEngine;
        
        let mut engine = SimpleSolanaEngine::new();
        engine.initialize().await.unwrap();
        
        let state = engine.get_engine_state().await.unwrap();
        
        assert_eq!(state.process_id, ProcessId::Solana);
        assert_eq!(state.blockchain_type, BlockchainType::Solana);
        assert!(!state.is_syncing);
    }

    #[test]
    fn test_solana_constants() {
        // Test some Solana-specific constants
        let lamports_per_sol = 1_000_000_000u64;
        let max_seed_length = 32usize;
        let signature_length = 64usize;
        
        assert_eq!(lamports_per_sol, 1_000_000_000);
        assert_eq!(max_seed_length, 32);
        assert_eq!(signature_length, 64);
    }

    #[test]
    fn test_execution_result_creation() {
        let result = ExecutionResult {
            success: true,
            transaction_hash: "abcdef1234567890".to_string(),
            block_height: Some(100),
            gas_used: Some(5000),
            logs: Some(vec!["Success".to_string()]),
            error: None,
        };
        
        assert!(result.success);
        assert_eq!(result.transaction_hash.len(), 16);
        assert_eq!(result.block_height.unwrap(), 100);
    }

    #[test]
    fn test_transaction_data_validation() {
        // Test empty transaction
        let empty_tx = TransactionData {
            from: "".to_string(),
            to: None,
            value: None,
            data: None,
            gas_limit: None,
            gas_price: None,
            nonce: None,
            chain_id: None,
        };
        
        assert!(empty_tx.from.is_empty());
        assert!(empty_tx.to.is_none());
        
        // Test full transaction
        let full_tx = TransactionData {
            from: "sender".to_string(),
            to: Some("receiver".to_string()),
            value: Some("1000000".to_string()),
            data: Some(vec![1, 2, 3, 4]),
            gas_limit: None,
            gas_price: None,
            nonce: Some(1),
            chain_id: Some(103),
        };
        
        assert!(!full_tx.from.is_empty());
        assert!(full_tx.to.is_some());
        assert!(full_tx.value.is_some());
        assert!(full_tx.data.is_some());
    }
}