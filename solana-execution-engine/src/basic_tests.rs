#[cfg(test)]
mod tests {
    use crate::simple_engine::{SimpleSolanaEngine, SimpleSolanaState};
    use multivm_common::{ExecutionResult, TransactionData};

    #[test]
    fn test_simple_state_creation() {
        let state = SimpleSolanaState::new();
        
        assert_eq!(state.slot, 0);
        assert_eq!(state.blockhash, "11111111111111111111111111111111");
        assert!(state.accounts.is_empty());
        assert_eq!(state.transaction_count, 0);
    }

    #[tokio::test]
    async fn test_simple_engine_creation() {
        let engine = SimpleSolanaEngine::new();
        let state = engine.state.lock().await;
        
        assert_eq!(state.slot, 0);
        assert_eq!(state.transaction_count, 0);
    }

    #[tokio::test]
    async fn test_simple_transaction_execution() {
        let engine = SimpleSolanaEngine::new();
        
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
        assert!(result.logs.is_some());
    }

    #[test]
    fn test_solana_address_format() {
        // Solana addresses are typically 32 bytes, represented as base58 strings
        let address = "11111111111111111111111111111111";
        assert_eq!(address.len(), 32);
        
        // Test some common Solana addresses
        let system_program = "11111111111111111111111111111111";
        let token_program = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
        
        assert!(!system_program.is_empty());
        assert!(!token_program.is_empty());
    }

    #[test]
    fn test_lamports_conversion() {
        // 1 SOL = 1_000_000_000 lamports
        let one_sol_in_lamports = 1_000_000_000u64;
        let half_sol_in_lamports = 500_000_000u64;
        
        assert_eq!(one_sol_in_lamports / 2, half_sol_in_lamports);
        assert_eq!(one_sol_in_lamports.to_string(), "1000000000");
    }

    #[test]
    fn test_transaction_data_structure() {
        let tx = TransactionData {
            from: "sender".to_string(),
            to: Some("receiver".to_string()),
            value: Some("100000".to_string()),
            data: Some(vec![1, 2, 3]),
            gas_limit: None, // Not used in Solana
            gas_price: None, // Not used in Solana
            nonce: None,
            chain_id: None,
        };
        
        assert_eq!(tx.from, "sender");
        assert_eq!(tx.to, Some("receiver".to_string()));
        assert_eq!(tx.value, Some("100000".to_string()));
        assert!(tx.data.is_some());
    }

    #[test]
    fn test_execution_result_structure() {
        let result = ExecutionResult {
            success: true,
            transaction_hash: "abc123def456".to_string(),
            block_height: Some(12345),
            gas_used: Some(5000),
            logs: Some(vec!["Transfer successful".to_string()]),
            error: None,
        };
        
        assert!(result.success);
        assert_eq!(result.transaction_hash, "abc123def456");
        assert_eq!(result.block_height, Some(12345));
        assert_eq!(result.gas_used, Some(5000));
        assert!(result.error.is_none());
    }

    #[test]
    fn test_cluster_names() {
        let mainnet = "mainnet-beta";
        let devnet = "devnet";
        let testnet = "testnet";
        let localnet = "localnet";
        
        assert_eq!(mainnet, "mainnet-beta");
        assert_eq!(devnet, "devnet");
        assert_eq!(testnet, "testnet");
        assert_eq!(localnet, "localnet");
    }
}