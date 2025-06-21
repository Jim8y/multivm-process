//! Performance benchmarks for the MultiVM system

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use multivm_account_mapping::{
    AccountMapping, AccountMappingValidator, EthereumAddress, SolanaAddress,
};
use multivm_consensus::{
    EvmTransaction, MalachiteConfig, MalachiteConsensus, MultivmBlock, SvmTransaction, Transaction,
};
use multivm_process_manager::{BlockRouter, BlockRouterConfig};
use std::time::Duration;
use tokio::runtime::Runtime;

fn create_test_block(height: u64, tx_count: usize) -> MultivmBlock {
    let mut transactions = vec![];

    for i in 0..tx_count {
        if i % 2 == 0 {
            transactions.push(Transaction::Svm(SvmTransaction {
                data: vec![i as u8; 100],
                signatures: vec![vec![i as u8; 64]],
            }));
        } else {
            transactions.push(Transaction::Evm(EvmTransaction {
                data: vec![i as u8; 100],
            }));
        }
    }

    MultivmBlock {
        height,
        timestamp: 1000000 + height,
        parent_hash: vec![0; 32],
        transactions,
        proposer: "benchmark-validator".to_string(),
        hash: vec![],
        signatures: vec![],
    }
}

fn benchmark_consensus_block_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let config = MalachiteConfig {
        validator_id: "bench-validator".to_string(),
        validators: vec!["bench-validator".to_string()],
        propose_timeout: Duration::from_millis(100),
        prevote_timeout: Duration::from_millis(100),
        precommit_timeout: Duration::from_millis(100),
        commit_timeout: Duration::from_millis(100),
        round_step_duration: Duration::from_millis(50),
        max_block_size: 10 * 1024 * 1024,
        max_rounds_per_height: 10,
    };

    let mut group = c.benchmark_group("consensus_block_processing");

    for tx_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("block_size", tx_count),
            tx_count,
            |b, &tx_count| {
                b.to_async(&rt).iter(|| async {
                    let mut consensus = MalachiteConsensus::new(config.clone());
                    consensus.start().await.unwrap();

                    let block = create_test_block(1, tx_count);
                    let _ = consensus.propose_block(black_box(block)).await;

                    consensus.stop().await.unwrap();
                });
            },
        );
    }

    group.finish();
}

fn benchmark_block_routing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let config = BlockRouterConfig {
        max_parallel_blocks: 10,
        routing_timeout: Duration::from_secs(5),
        retry_attempts: 3,
        retry_delay: Duration::from_millis(100),
        enable_transaction_validation: true,
        enable_dependency_tracking: true,
        max_pending_transactions: 10000,
    };

    let mut group = c.benchmark_group("block_routing");

    for tx_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("decompose", tx_count),
            tx_count,
            |b, &tx_count| {
                b.to_async(&rt).iter(|| async {
                    let router = BlockRouter::new(config.clone());
                    let block = create_test_block(1, tx_count);

                    let _ = router.decompose_block(black_box(block)).await;
                });
            },
        );
    }

    group.finish();
}

fn benchmark_account_mapping_validation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("account_mapping");

    group.bench_function("validate_mapping", |b| {
        b.to_async(&rt).iter(|| async {
            let validator = AccountMappingValidator::new();

            let mapping = AccountMapping {
                multivm_account_id: vec![1; 32],
                solana_address: Some(SolanaAddress([2; 32])),
                ethereum_address: Some(EthereumAddress([3; 20])),
                proof: vec![0; 129],
                timestamp: 1000000,
            };

            let _ = validator.validate_mapping(black_box(&mapping));
        });
    });

    group.bench_function("signature_verification", |b| {
        b.to_async(&rt).iter(|| async {
            let validator = AccountMappingValidator::new();
            let address = SolanaAddress([1; 32]);
            let message = b"test message";
            let signature = vec![0; 64];

            let _ = validator.validate_solana_signature(
                black_box(&address),
                black_box(message),
                black_box(&signature),
            );
        });
    });

    group.finish();
}

fn benchmark_transaction_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("transaction_processing");

    group.bench_function("svm_transaction", |b| {
        b.iter(|| {
            let tx = SvmTransaction {
                data: black_box(vec![1; 1000]),
                signatures: vec![vec![0; 64]],
            };

            // Simulate processing
            let _ = tx.data.len();
            let _ = tx.signatures.len();
        });
    });

    group.bench_function("evm_transaction", |b| {
        b.iter(|| {
            let tx = EvmTransaction {
                data: black_box(vec![1; 1000]),
            };

            // Simulate processing
            let _ = tx.data.len();
        });
    });

    group.finish();
}

fn benchmark_concurrent_operations(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_operations");

    for num_tasks in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel_blocks", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.to_async(&rt).iter(|| async {
                    let mut handles = vec![];

                    for i in 0..num_tasks {
                        let handle = tokio::spawn(async move {
                            let block = create_test_block(i as u64, 10);
                            // Simulate block processing
                            tokio::time::sleep(Duration::from_micros(100)).await;
                            block
                        });
                        handles.push(handle);
                    }

                    for handle in handles {
                        let _ = handle.await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

fn benchmark_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    let block = create_test_block(1, 100);

    group.bench_function("block_serialize", |b| {
        b.iter(|| {
            let _ = bincode::serialize(black_box(&block));
        });
    });

    let serialized = bincode::serialize(&block).unwrap();

    group.bench_function("block_deserialize", |b| {
        b.iter(|| {
            let _: MultivmBlock = bincode::deserialize(black_box(&serialized)).unwrap();
        });
    });

    group.finish();
}

fn benchmark_message_processing(c: &mut Criterion) {
    use multivm_consensus::messages::{ConsensusMessage, MessageType, Vote, VoteType};

    let mut group = c.benchmark_group("message_processing");

    group.bench_function("vote_creation", |b| {
        b.iter(|| {
            let vote = Vote {
                vote_type: VoteType::Prevote,
                height: black_box(100),
                round: black_box(0),
                block_hash: black_box(vec![1; 32]),
                validator: black_box("validator1".to_string()),
                signature: black_box(vec![0; 64]),
            };

            ConsensusMessage {
                msg_type: MessageType::Vote(vote),
                sender: "validator1".to_string(),
                timestamp: 1000000,
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_consensus_block_processing,
    benchmark_block_routing,
    benchmark_account_mapping_validation,
    benchmark_transaction_processing,
    benchmark_concurrent_operations,
    benchmark_serialization,
    benchmark_message_processing
);

criterion_main!(benches);
