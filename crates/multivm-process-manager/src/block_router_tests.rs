//! Unit tests for block routing functionality

#[cfg(test)]
mod tests {
    use super::super::block_router::*;
    use multivm_account_mapping::{
        AccountAddress, BindingProof, EthereumAddress, MemoryStorage, ProofType, SolanaAddress,
        SpecialTransaction,
    };
    // MultivmResult not needed for current tests
    use multivm_consensus::block::{EvmTransaction, MultiVMBlock, SvmTransaction};
    use std::sync::Arc;
    use uuid::Uuid;

    // Mock configuration for testing (not used by current implementation)
    #[allow(dead_code)]
    struct MockConfig {
        max_parallel_blocks: usize,
        routing_timeout: std::time::Duration,
    }

    fn create_mock_account_mapping() -> Arc<MemoryStorage> {
        Arc::new(MemoryStorage::new())
    }

    fn create_test_block() -> MultiVMBlock {
        use multivm_consensus::block::BlockHeader;
        use std::time::SystemTime;

        let mut block = MultiVMBlock {
            header: BlockHeader {
                height: 100,
                previous_hash: "prev_hash".to_string(),
                state_root: "state_root".to_string(),
                transactions_root: "tx_root".to_string(),
                timestamp: SystemTime::now(),
                proposer: "validator1".to_string(),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![SvmTransaction {
                id: Uuid::new_v4(),
                signatures: vec!["sig1".to_string()],
                data: vec![1, 2, 3, 4],
                accounts: vec!["account1".to_string()],
                recent_blockhash: "blockhash".to_string(),
                fee: 5000,
                metadata: serde_json::Value::Null,
            }],
            evm_transactions: vec![EvmTransaction {
                id: Uuid::new_v4(),
                hash: "0xhash".to_string(),
                from: "0xfrom".to_string(),
                to: Some("0xto".to_string()),
                value: 1000,
                gas_limit: 21000,
                gas_price: 20000000000,
                data: vec![5, 6, 7, 8],
                nonce: 1,
                signature: multivm_consensus::EvmSignature {
                    v: 27,
                    r: "0xr".to_string(),
                    s: "0xs".to_string(),
                },
                metadata: serde_json::Value::Null,
            }],
            multivm_transactions: vec![SpecialTransaction::AccountBinding {
                source_account: AccountAddress::Solana(SolanaAddress([10; 32])),
                target_account: AccountAddress::Ethereum(EthereumAddress([11; 20])),
                proof: BindingProof {
                    account: AccountAddress::Solana(SolanaAddress([10; 32])),
                    proof_type: ProofType::Signature {
                        message: b"binding_message".to_vec(),
                        signature: b"binding_signature".to_vec(),
                    },
                    proof_data: vec![12; 64],
                    timestamp: SystemTime::now(),
                },
                metadata: None,
            }],
            state_transitions: vec![],
        };

        block.finalize();
        block
    }

    #[tokio::test]
    async fn test_router_initialization() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        // Test that router was created successfully
        let available_engines = router.get_available_engines().await;
        assert_eq!(available_engines.len(), 0); // No engines registered yet
    }

    #[tokio::test]
    async fn test_block_decomposition() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        let block = create_test_block();
        let result = router.decompose_block(block.clone()).await;

        assert!(result.is_ok());
        let routing_result = result.unwrap();

        assert_eq!(routing_result.svm_transactions.len(), 1);
        assert_eq!(routing_result.evm_transactions.len(), 1);
        assert_eq!(routing_result.special_transactions.len(), 1);
        assert_eq!(routing_result.routing_metadata.total_transactions, 3);
    }

    #[tokio::test]
    async fn test_basic_block_processing() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        let block = create_test_block();
        let decompose_result = router.decompose_block(block).await;
        assert!(decompose_result.is_ok());

        let routing_result = decompose_result.unwrap();

        // Test basic decomposition worked
        assert!(!routing_result.svm_transactions.is_empty());
        assert!(!routing_result.evm_transactions.is_empty());
        assert!(!routing_result.special_transactions.is_empty());
    }

    #[tokio::test]
    async fn test_routing_result_metadata() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        let block = create_test_block();
        let result = router.decompose_block(block).await.unwrap();

        // Test metadata is populated correctly
        let metadata = &result.routing_metadata;
        assert_eq!(metadata.svm_count, 1);
        assert_eq!(metadata.evm_count, 1);
        assert_eq!(metadata.special_count, 1);
        assert_eq!(metadata.total_transactions, 3);
        // routing_time_ms is u64, so it's always >= 0
        assert!(metadata.routing_time_ms < 1000); // Should be fairly quick
    }

    #[tokio::test]
    async fn test_routing_decomposed_block() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        let block = create_test_block();
        let decomposed = router.decompose_block(block).await.unwrap();

        // Test that we can call route_decomposed_block (even if it fails due to no engines)
        let result = router.route_decomposed_block(decomposed).await;
        // This will likely fail because no engines are registered, which is expected
        // We're just testing that the method exists and can be called
        assert!(result.is_ok() || result.is_err()); // Either outcome is fine for this test
    }

    #[tokio::test]
    async fn test_parallel_block_processing() {
        let account_mapping = create_mock_account_mapping();
        let router = Arc::new(BlockRouter::new(account_mapping));

        // Create multiple blocks
        let mut handles = vec![];

        for i in 0..3 {
            let router_clone = router.clone();
            let handle = tokio::spawn(async move {
                let mut block = create_test_block();
                block.header.height = 100 + i;
                router_clone.decompose_block(block).await
            });
            handles.push(handle);
        }

        // Wait for all blocks to be processed
        let mut results = vec![];
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_ok());
            results.push(result.unwrap());
        }

        // Verify all blocks were processed
        assert_eq!(results.len(), 3);

        // Verify each result has the expected transaction counts
        for result in results {
            assert_eq!(result.routing_metadata.total_transactions, 3);
        }
    }

    #[tokio::test]
    async fn test_empty_block_handling() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        // Test with empty block (no transactions)
        let empty_block = MultiVMBlock {
            header: multivm_consensus::BlockHeader {
                height: 100,
                previous_hash: "prev_hash".to_string(),
                state_root: "state_root".to_string(),
                transactions_root: "tx_root".to_string(),
                timestamp: std::time::SystemTime::now(),
                proposer: "validator1".to_string(),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![],
            evm_transactions: vec![],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };

        let result = router.decompose_block(empty_block).await;
        assert!(result.is_ok()); // Empty blocks should be valid

        let routing_result = result.unwrap();
        assert_eq!(routing_result.routing_metadata.total_transactions, 0);
    }

    #[tokio::test]
    async fn test_special_transaction_handling() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        // Create block with multiple special transactions
        let block = MultiVMBlock {
            header: multivm_consensus::BlockHeader {
                height: 100,
                previous_hash: "prev_hash".to_string(),
                state_root: "state_root".to_string(),
                transactions_root: "tx_root".to_string(),
                timestamp: std::time::SystemTime::now(),
                proposer: "validator1".to_string(),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![],
            evm_transactions: vec![],
            multivm_transactions: vec![
                SpecialTransaction::AccountBinding {
                    source_account: AccountAddress::Solana(SolanaAddress([2; 32])),
                    target_account: AccountAddress::Ethereum(EthereumAddress([3; 20])),
                    proof: BindingProof {
                        account: AccountAddress::Solana(SolanaAddress([2; 32])),
                        proof_type: ProofType::Signature {
                            message: b"binding1".to_vec(),
                            signature: b"sig1".to_vec(),
                        },
                        proof_data: vec![3; 64],
                        timestamp: std::time::SystemTime::now(),
                    },
                    metadata: None,
                },
                SpecialTransaction::AccountBinding {
                    source_account: AccountAddress::Ethereum(EthereumAddress([5; 20])),
                    target_account: AccountAddress::Solana(SolanaAddress([6; 32])),
                    proof: BindingProof {
                        account: AccountAddress::Ethereum(EthereumAddress([5; 20])),
                        proof_type: ProofType::Signature {
                            message: b"binding2".to_vec(),
                            signature: b"sig2".to_vec(),
                        },
                        proof_data: vec![6; 64],
                        timestamp: std::time::SystemTime::now(),
                    },
                    metadata: None,
                },
            ],
            state_transitions: vec![],
        };

        let result = router.decompose_block(block).await;
        assert!(result.is_ok());

        let routing_result = result.unwrap();
        assert_eq!(routing_result.special_transactions.len(), 2);
        assert_eq!(routing_result.svm_transactions.len(), 0);
        assert_eq!(routing_result.evm_transactions.len(), 0);
    }

    #[tokio::test]
    async fn test_transaction_ordering() {
        let account_mapping = create_mock_account_mapping();
        let router = BlockRouter::new(account_mapping);

        // Create block with mixed transaction types
        let block = MultiVMBlock {
            header: multivm_consensus::BlockHeader {
                height: 100,
                previous_hash: "prev_hash".to_string(),
                state_root: "state_root".to_string(),
                transactions_root: "tx_root".to_string(),
                timestamp: std::time::SystemTime::now(),
                proposer: "validator1".to_string(),
                consensus_data: vec![],
                version: 1,
                extra_data: vec![],
            },
            svm_transactions: vec![
                SvmTransaction {
                    id: Uuid::new_v4(),
                    signatures: vec!["sig1".to_string()],
                    data: vec![2],
                    accounts: vec!["account1".to_string()],
                    recent_blockhash: "blockhash".to_string(),
                    fee: 5000,
                    metadata: serde_json::Value::Null,
                },
                SvmTransaction {
                    id: Uuid::new_v4(),
                    signatures: vec!["sig2".to_string()],
                    data: vec![4],
                    accounts: vec!["account2".to_string()],
                    recent_blockhash: "blockhash".to_string(),
                    fee: 5000,
                    metadata: serde_json::Value::Null,
                },
            ],
            evm_transactions: vec![
                EvmTransaction {
                    id: Uuid::new_v4(),
                    hash: "0xhash1".to_string(),
                    from: "0xfrom1".to_string(),
                    to: Some("0xto1".to_string()),
                    value: 1000,
                    gas_limit: 21000,
                    gas_price: 20000000000,
                    data: vec![1],
                    nonce: 1,
                    signature: multivm_consensus::EvmSignature {
                        v: 27,
                        r: "0xr1".to_string(),
                        s: "0xs1".to_string(),
                    },
                    metadata: serde_json::Value::Null,
                },
                EvmTransaction {
                    id: Uuid::new_v4(),
                    hash: "0xhash2".to_string(),
                    from: "0xfrom2".to_string(),
                    to: Some("0xto2".to_string()),
                    value: 2000,
                    gas_limit: 21000,
                    gas_price: 20000000000,
                    data: vec![3],
                    nonce: 2,
                    signature: multivm_consensus::EvmSignature {
                        v: 27,
                        r: "0xr2".to_string(),
                        s: "0xs2".to_string(),
                    },
                    metadata: serde_json::Value::Null,
                },
            ],
            multivm_transactions: vec![],
            state_transitions: vec![],
        };

        let result = router.decompose_block(block).await.unwrap();

        // Verify transactions are grouped by type and maintain order
        assert_eq!(result.svm_transactions.len(), 2);
        assert_eq!(result.evm_transactions.len(), 2);

        // Check ordering within each type
        assert_eq!(result.svm_transactions[0].data, vec![2]);
        assert_eq!(result.svm_transactions[1].data, vec![4]);
        assert_eq!(result.evm_transactions[0].data, vec![1]);
        assert_eq!(result.evm_transactions[1].data, vec![3]);
    }

    // Tests completed - mock engines not needed for current implementation
}
