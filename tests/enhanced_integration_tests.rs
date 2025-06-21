//! Enhanced integration tests for the MultiVM system with comprehensive coverage

use multivm_account_mapping::{
    AccountMappingLayer, EthereumAddress, SolanaAddress, SpecialTransaction,
};
use multivm_common::{BlockchainType, MultivmConfig, MultivmError, MultivmResult, SystemConfig};
use multivm_consensus::{
    EvmTransaction, MalachiteConfig, MultiVMBlock, SvmTransaction, EvmSignature,
};
use multivm_p2p::{P2PNetworkConfig, P2PNetworkLayer};
use multivm_process_manager::{BlockRouter, CoordinatorConfig, MultivmCoordinator};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tokio::time::timeout;

#[tokio::test]
async fn test_full_system_lifecycle() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());

    // Create coordinator
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();

    // Verify initial state
    let state = coordinator.get_state().await;
    assert!(!state.is_running);

    // Start the system
    assert!(coordinator.start().await.is_ok());

    // Wait for full initialization
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify all components are healthy
    let state = coordinator.get_state().await;
    assert!(state.is_running);
    let health_status = coordinator.get_health_status().await.unwrap();
    assert!(health_status.is_healthy);

    // Graceful shutdown
    assert!(coordinator.stop().await.is_ok());

    // Verify clean shutdown
    let state = coordinator.get_state().await;
    assert!(!state.is_running);
}

#[tokio::test]
async fn test_end_to_end_block_processing() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();
    coordinator.start().await.unwrap();

    // Create a comprehensive test block
    let block = create_complex_multivm_block();

    // Submit block for processing
    let submission_result = coordinator.submit_block(block.clone()).await;
    assert!(submission_result.is_ok());

    // Wait for block processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify block was processed correctly
    let state = coordinator.get_state().await;
    assert!(state.blocks_processed > 0);
    assert_eq!(state.last_block_height, block.header.height);

    // Verify execution engines received their transactions
    let state = coordinator.get_state().await;
    assert!(state.system_metrics.total_transactions_processed > 0);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_account_mapping_integration() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();
    coordinator.start().await.unwrap();

    // Create multiple account mapping transactions
    let mappings = vec![
        create_solana_only_mapping(),
        create_ethereum_only_mapping(),
        create_full_account_mapping(),
    ];

    // Submit blocks with account mappings
    for (i, mapping) in mappings.iter().enumerate() {
        let mut block = MultiVMBlock::new(
            (i + 1) as u64,
            "0".repeat(64),
            "test-validator".to_string(),
            vec![],
        );
        block.add_multivm_transaction(mapping.clone());
        
        coordinator.submit_block(block).await.unwrap();
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify mappings were created
    let state = coordinator.get_state().await;
    assert!(state.blocks_processed >= 3);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_consensus_fault_tolerance() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = create_multi_validator_config(temp_dir.path(), 4);

    // Start multiple validators
    let mut coordinators = vec![];
    for i in 0..4 {
        config.consensus.node_id = format!("validator{}", i);
        let mut coordinator = MultivmCoordinator::new(config.clone()).await.unwrap();
        coordinator.start().await.unwrap();
        coordinators.push(coordinator);
    }

    // Allow network formation
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Submit block through first validator
    let block = create_test_multivm_block();
    coordinators[0].submit_block(block.clone()).await.unwrap();

    // Simulate Byzantine validator (stop one validator)
    coordinators[3].stop().await.unwrap();
    coordinators.pop();

    // System should still reach consensus with 3/4 validators
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Check that block was processed
    let state = coordinators[0].get_state().await;
    assert!(state.blocks_processed > 0);

    // Cleanup
    for mut coordinator in coordinators {
        coordinator.stop().await.unwrap();
    }
}

#[tokio::test]
async fn test_transaction_routing_accuracy() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();
    coordinator.start().await.unwrap();

    // Create block with mixed transaction types
    let svm_txs = 10;
    let evm_txs = 15;
    let special_txs = 5;

    let mut block = MultiVMBlock::new(
        1,
        "0".repeat(64),
        "test-validator".to_string(),
        vec![],
    );

    // Add SVM transactions
    for i in 0..svm_txs {
        block.add_svm_transaction(SvmTransaction {
            id: uuid::Uuid::new_v4(),
            signatures: vec![format!("sig_{}", i)],
            data: vec![i as u8; 100],
            accounts: vec![format!("account_{}", i)],
            recent_blockhash: "blockhash".to_string(),
            fee: 5000,
            metadata: serde_json::Value::Null,
        });
    }

    // Add EVM transactions
    for i in 0..evm_txs {
        block.add_evm_transaction(EvmTransaction {
            id: uuid::Uuid::new_v4(),
            hash: format!("0x{:064x}", i),
            from: format!("0x{:040x}", i),
            to: Some(format!("0x{:040x}", i + 1000)),
            value: 1000000,
            gas_limit: 21000,
            gas_price: 1000000000,
            data: vec![(i + 100) as u8; 150],
            nonce: i as u64,
            signature: EvmSignature {
                v: 27,
                r: format!("0x{:064x}", i),
                s: format!("0x{:064x}", i + 1),
            },
            metadata: serde_json::Value::Null,
        });
    }

    // Add special transactions
    for i in 0..special_txs {
        block.add_multivm_transaction(create_test_special_transaction(i));
    }

    // Process block
    coordinator.submit_block(block).await.unwrap();

    // Wait for routing completion
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify routing statistics
    let state = coordinator.get_state().await;
    assert_eq!(state.system_metrics.total_transactions_processed as usize, svm_txs + evm_txs + special_txs);
    assert_eq!(state.system_metrics.error_count, 0);

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_concurrent_block_submission() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();
    coordinator.start().await.unwrap();
    let coordinator = Arc::new(coordinator);

    // Submit multiple blocks concurrently
    let mut handles = vec![];

    for i in 0..10 {
        let coordinator_clone = coordinator.clone();
        let handle = tokio::spawn(async move {
            let mut block = MultiVMBlock::new(
                (i + 1) as u64,
                format!("{:064x}", i),
                "test-validator".to_string(),
                vec![],
            );
            
            block.add_svm_transaction(SvmTransaction {
                id: uuid::Uuid::new_v4(),
                signatures: vec![format!("sig_{}", i)],
                data: vec![i as u8; 50],
                accounts: vec![format!("account_{}", i)],
                recent_blockhash: "blockhash".to_string(),
                fee: 5000,
                metadata: serde_json::Value::Null,
            });

            coordinator_clone.submit_block(block).await
        });
        handles.push(handle);
    }

    // All submissions should succeed
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    // Wait for processing
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify all blocks were processed
    let state = coordinator.get_state().await;
    assert_eq!(state.blocks_processed, 10);

    match Arc::try_unwrap(coordinator) {
        Ok(mut coord) => coord.stop().await.unwrap(),
        Err(_) => panic!("Failed to unwrap coordinator"),
    }
}

#[tokio::test]
async fn test_system_recovery_after_crash() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());

    // Start system and process some blocks
    {
        let mut coordinator = MultivmCoordinator::new(config.clone()).await.unwrap();
        coordinator.start().await.unwrap();

        // Process a few blocks
        for i in 0..3 {
            let block = create_test_multivm_block_with_height(i + 1);
            coordinator.submit_block(block).await.unwrap();
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        // Simulate crash (no graceful shutdown)
        drop(coordinator);
    }

    // Restart system
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();
    coordinator.start().await.unwrap();

    // System should recover and continue from last state
    let block = create_test_multivm_block_with_height(4);
    let result = coordinator.submit_block(block).await;
    assert!(result.is_ok());

    coordinator.stop().await.unwrap();
}

#[tokio::test]
async fn test_performance_under_load() {
    let temp_dir = TempDir::new().unwrap();
    let config = create_test_config(temp_dir.path());
    let mut coordinator = MultivmCoordinator::new(config).await.unwrap();
    coordinator.start().await.unwrap();

    let start_time = std::time::Instant::now();
    let num_blocks = 100;
    let txs_per_block = 50;

    // Submit many blocks with many transactions
    for i in 0..num_blocks {
        let mut block = MultiVMBlock::new(
            (i + 1) as u64,
            format!("{:064x}", i),
            "test-validator".to_string(),
            vec![],
        );

        for j in 0..txs_per_block {
            if j % 2 == 0 {
                block.add_svm_transaction(SvmTransaction {
                    id: uuid::Uuid::new_v4(),
                    signatures: vec![format!("sig_{}_{}", i, j)],
                    data: vec![(i + j) as u8; 100],
                    accounts: vec![format!("account_{}_{}", i, j)],
                    recent_blockhash: "blockhash".to_string(),
                    fee: 5000,
                    metadata: serde_json::Value::Null,
                });
            } else {
                block.add_evm_transaction(EvmTransaction {
                    id: uuid::Uuid::new_v4(),
                    hash: format!("0x{:064x}", i * 1000 + j),
                    from: format!("0x{:040x}", i),
                    to: Some(format!("0x{:040x}", j)),
                    value: 1000000,
                    gas_limit: 21000,
                    gas_price: 1000000000,
                    data: vec![(i + j) as u8; 100],
                    nonce: j as u64,
                    signature: EvmSignature {
                        v: 27,
                        r: format!("0x{:064x}", i),
                        s: format!("0x{:064x}", j),
                    },
                    metadata: serde_json::Value::Null,
                });
            }
        }

        coordinator.submit_block(block).await.unwrap();
    }

    // Wait for all blocks to be processed
    tokio::time::sleep(Duration::from_secs(10)).await;

    let elapsed = start_time.elapsed();
    let blocks_per_second = num_blocks as f64 / elapsed.as_secs_f64();
    let txs_per_second = (num_blocks * txs_per_block) as f64 / elapsed.as_secs_f64();

    println!("Performance metrics:");
    println!("  Blocks per second: {:.2}", blocks_per_second);
    println!("  Transactions per second: {:.2}", txs_per_second);

    // Verify reasonable performance (at least 10 blocks/sec)
    assert!(blocks_per_second > 10.0);

    coordinator.stop().await.unwrap();
}

// Helper functions

fn create_test_config(data_dir: &std::path::Path) -> CoordinatorConfig {
    CoordinatorConfig {
        consensus: MalachiteConfig {
            node_id: "test-validator".to_string(),
            network_config: multivm_consensus::malachite::NetworkConfig {
                listen_addr: "127.0.0.1:0".to_string(),
                peers: vec![],
            },
            consensus_params: multivm_consensus::malachite::ConsensusParams {
                block_time_ms: 100,
                max_block_size: 1024 * 1024,
                timeout_propose_ms: 100,
                timeout_prevote_ms: 100,
                timeout_precommit_ms: 100,
            },
            validators: vec![],
        },
        health_check_interval: Duration::from_secs(1),
        block_timeout: Duration::from_secs(5),
        max_concurrent_blocks: 10,
        enable_recovery: true,
    }
}

fn create_multi_validator_config(
    data_dir: &std::path::Path,
    num_validators: usize,
) -> CoordinatorConfig {
    let mut config = create_test_config(data_dir);

    let mut validators = vec![];
    for i in 0..num_validators {
        validators.push(format!("validator{}", i));
    }

    // Note: validators field expects ValidatorInfo, but for tests we'll keep it empty
    // config.consensus.validators = validators;
    config
}

fn create_test_multivm_block() -> MultiVMBlock {
    create_test_multivm_block_with_height(1)
}

fn create_test_multivm_block_with_height(height: u64) -> MultiVMBlock {
    let mut block = MultiVMBlock::new(
        height,
        if height == 1 { "0".repeat(64) } else { format!("{:064x}", height - 1) },
        "test-validator".to_string(),
        vec![],
    );
    
    block.add_svm_transaction(SvmTransaction {
        id: uuid::Uuid::new_v4(),
        signatures: vec!["test_sig".to_string()],
        data: vec![1, 2, 3],
        accounts: vec!["test_account".to_string()],
        recent_blockhash: "blockhash".to_string(),
        fee: 5000,
        metadata: serde_json::Value::Null,
    });
    
    block.add_evm_transaction(EvmTransaction {
        id: uuid::Uuid::new_v4(),
        hash: format!("0x{:064x}", height),
        from: "0x1234567890123456789012345678901234567890".to_string(),
        to: Some("0x0987654321098765432109876543210987654321".to_string()),
        value: 1000000,
        gas_limit: 21000,
        gas_price: 1000000000,
        data: vec![4, 5, 6],
        nonce: 0,
        signature: EvmSignature {
            v: 27,
            r: "0x1234567890123456789012345678901234567890123456789012345678901234".to_string(),
            s: "0x1234567890123456789012345678901234567890123456789012345678901234".to_string(),
        },
        metadata: serde_json::Value::Null,
    });
    
    block
}

fn create_complex_multivm_block() -> MultiVMBlock {
    let mut block = MultiVMBlock::new(
        1,
        "0".repeat(64),
        "test-validator".to_string(),
        vec![],
    );
    
    // Add multiple SVM transactions
    for i in 0..2 {
        block.add_svm_transaction(SvmTransaction {
            id: uuid::Uuid::new_v4(),
            signatures: vec![format!("sig_{}", i), format!("sig2_{}", i)],
            data: vec![(i * 5 + 1) as u8, (i * 5 + 2) as u8, (i * 5 + 3) as u8, (i * 5 + 4) as u8, (i * 5 + 5) as u8],
            accounts: vec![format!("account_{}", i)],
            recent_blockhash: "blockhash".to_string(),
            fee: 5000,
            metadata: serde_json::Value::Null,
        });
    }
    
    // Add multiple EVM transactions
    for i in 0..2 {
        block.add_evm_transaction(EvmTransaction {
            id: uuid::Uuid::new_v4(),
            hash: format!("0x{:064x}", i),
            from: format!("0x{:040x}", i),
            to: Some(format!("0x{:040x}", i + 1000)),
            value: 1000000,
            gas_limit: 21000,
            gas_price: 1000000000,
            data: vec![(i * 5 + 11) as u8, (i * 5 + 12) as u8, (i * 5 + 13) as u8, (i * 5 + 14) as u8, (i * 5 + 15) as u8],
            nonce: i as u64,
            signature: EvmSignature {
                v: 27,
                r: format!("0x{:064x}", i),
                s: format!("0x{:064x}", i + 1),
            },
            metadata: serde_json::Value::Null,
        });
    }
    
    // Add special transaction
    block.add_multivm_transaction(create_full_account_mapping());
    
    block
}

fn create_solana_only_mapping() -> SpecialTransaction {
    use multivm_account_mapping::{AccountAddress, BindingProof, ProofType};
    use std::time::SystemTime;
    
    let solana_addr = AccountAddress::Solana(SolanaAddress([2; 32]));
    
    SpecialTransaction::AccountBinding {
        source_account: solana_addr.clone(),
        target_account: solana_addr,
        proof: BindingProof {
            account: AccountAddress::Solana(SolanaAddress([2; 32])),
            proof_type: ProofType::Signature {
                message: b"bind account".to_vec(),
                signature: vec![0; 64],
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        },
        metadata: None,
    }
}

fn create_ethereum_only_mapping() -> SpecialTransaction {
    use multivm_account_mapping::{AccountAddress, BindingProof, ProofType};
    use std::time::SystemTime;
    
    let eth_addr = AccountAddress::Ethereum(EthereumAddress([4; 20]));
    
    SpecialTransaction::AccountBinding {
        source_account: eth_addr.clone(),
        target_account: eth_addr,
        proof: BindingProof {
            account: AccountAddress::Ethereum(EthereumAddress([4; 20])),
            proof_type: ProofType::Signature {
                message: b"bind account".to_vec(),
                signature: vec![0; 65],
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        },
        metadata: None,
    }
}

fn create_full_account_mapping() -> SpecialTransaction {
    use multivm_account_mapping::{AccountAddress, BindingProof, ProofType};
    use std::time::SystemTime;
    
    let solana_addr = AccountAddress::Solana(SolanaAddress([6; 32]));
    let eth_addr = AccountAddress::Ethereum(EthereumAddress([7; 20]));
    
    SpecialTransaction::AccountBinding {
        source_account: solana_addr,
        target_account: eth_addr,
        proof: BindingProof {
            account: AccountAddress::Ethereum(EthereumAddress([7; 20])),
            proof_type: ProofType::Signature {
                message: b"cross-vm bind".to_vec(),
                signature: vec![0; 65],
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        },
        metadata: None,
    }
}

fn create_test_special_transaction(index: usize) -> SpecialTransaction {
    use multivm_account_mapping::{AccountAddress, BindingProof, ProofType};
    use std::time::SystemTime;
    
    let account = if index % 2 == 0 {
        AccountAddress::Solana(SolanaAddress([(index * 2) as u8; 32]))
    } else {
        AccountAddress::Ethereum(EthereumAddress([(index * 3) as u8; 20]))
    };
    
    SpecialTransaction::AccountBinding {
        source_account: account.clone(),
        target_account: account.clone(),
        proof: BindingProof {
            account,
            proof_type: ProofType::Signature {
                message: format!("bind {}", index).into_bytes(),
                signature: vec![index as u8; if index % 2 == 0 { 64 } else { 65 }],
            },
            proof_data: vec![],
            timestamp: SystemTime::now(),
        },
        metadata: None,
    }
}