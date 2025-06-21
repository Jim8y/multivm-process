//! Performance benchmarks for the MultiVM consensus system
//!
//! This module contains comprehensive benchmarks to measure performance
//! of core consensus operations under various workloads.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use multivm_consensus::{
    BlockSyncConfig, BlockSyncManager, ConsensusEngine, ConsensusMetricsCollector,
    ForkDetectionConfig, ForkDetectionManager, MalachiteConfig, MalachiteConsensus,
    MetricsExporter, MultiVMBlock, NetworkRecoveryConfig, NetworkRecoveryManager,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

/// Create a test block with specified parameters
fn create_test_block(height: u64, _tx_count: usize) -> MultiVMBlock {
    let transactions = vec![];
    MultiVMBlock::new(
        height,
        format!("{:064x}", height.saturating_sub(1)),
        "benchmark-validator".to_string(),
        transactions,
    )
}

/// Benchmark consensus block processing with different block sizes
fn benchmark_consensus_block_processing(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("consensus_block_processing");
    group.sample_size(10); // Reduce sample size for expensive operations

    for tx_count in [10, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("block_size", tx_count),
            tx_count,
            |b, &_tx_count| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = MalachiteConfig::default();
                        let (mut consensus, _block_sender, _commit_receiver) =
                            MalachiteConsensus::new(config).await.unwrap();

                        consensus.start().await.unwrap();

                        let block = create_test_block(1, 10);
                        let _ = consensus.validate_block(black_box(&block)).await;

                        consensus.stop().await.unwrap();
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark fork detection operations
fn benchmark_fork_detection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("fork_detection");

    group.bench_function("process_block", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = ForkDetectionConfig::default();
                let detector = ForkDetectionManager::new(config).await.unwrap();

                let block = create_test_block(1, 10);
                let _ = detector
                    .process_block(black_box(block), "validator1".to_string())
                    .await;
            })
        });
    });

    group.bench_function("detect_fork_concurrent", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = ForkDetectionConfig::default();
                let detector = Arc::new(ForkDetectionManager::new(config).await.unwrap());

                let mut handles = vec![];
                for i in 0..10 {
                    let detector_clone = detector.clone();
                    let handle = tokio::spawn(async move {
                        let block = create_test_block(1, 5); // Same height, different blocks
                        detector_clone
                            .process_block(block, format!("validator{}", i))
                            .await
                    });
                    handles.push(handle);
                }

                for handle in handles {
                    let _ = handle.await.unwrap();
                }
            })
        });
    });

    group.finish();
}

/// Benchmark network recovery operations
fn benchmark_network_recovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("network_recovery");

    group.bench_function("health_check", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = NetworkRecoveryConfig::default();
                let manager = NetworkRecoveryManager::new(config).await.unwrap();

                // Add some validators
                for i in 0..10 {
                    manager
                        .update_validator_status(format!("validator{}", i), i % 3 == 0, 100 + i)
                        .await;
                }

                let _ = manager.perform_health_check().await;
            })
        });
    });

    group.bench_function("partition_detection", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = NetworkRecoveryConfig::default();
                let manager = NetworkRecoveryManager::new(config).await.unwrap();

                // Simulate network partition scenario
                for i in 0..20 {
                    manager
                        .update_validator_status(format!("validator{}", i), i < 5, 100 + i)
                        .await;
                }

                let _ = manager.detect_partition().await;
            })
        });
    });

    group.finish();
}

/// Benchmark block synchronization operations
fn benchmark_block_sync(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("block_sync");

    group.bench_function("needs_sync_check", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = BlockSyncConfig::default();
                let sync_manager = BlockSyncManager::new(config).await.unwrap();

                // Add peer heights
                for i in 0..100 {
                    sync_manager
                        .update_peer_height(format!("peer{}", i), 1000 + i)
                        .await;
                }

                let _ = sync_manager.needs_sync(black_box(950)).await;
            })
        });
    });

    group.bench_function("cache_operations", |b| {
        b.iter(|| {
            rt.block_on(async {
                let config = BlockSyncConfig::default();
                let sync_manager = BlockSyncManager::new(config).await.unwrap();

                // Simulate caching operations
                for i in 1..=100 {
                    let _ = sync_manager.get_cached_blocks(i, 1).await;
                }
            })
        });
    });

    group.finish();
}

/// Benchmark metrics collection and aggregation
fn benchmark_metrics_collection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("metrics_collection");

    group.bench_function("collect_all_metrics", |b| {
        b.iter(|| {
            rt.block_on(async {
                let collector = ConsensusMetricsCollector::new(Duration::from_secs(60));
                let _ = collector.collect_metrics().await;
            })
        });
    });

    group.bench_function("export_prometheus", |b| {
        b.iter(|| {
            rt.block_on(async {
                let collector = ConsensusMetricsCollector::new(Duration::from_secs(60));
                let metrics = collector.collect_metrics().await;

                let exporter = multivm_consensus::PrometheusExporter;
                let _ = exporter.export(black_box(&metrics)).await;
            })
        });
    });

    group.bench_function("export_json", |b| {
        b.iter(|| {
            rt.block_on(async {
                let collector = ConsensusMetricsCollector::new(Duration::from_secs(60));
                let metrics = collector.collect_metrics().await;

                let exporter = multivm_consensus::JsonExporter;
                let _ = exporter.export(black_box(&metrics)).await;
            })
        });
    });

    group.finish();
}

/// Benchmark block creation and validation
fn benchmark_block_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("block_operations");

    group.bench_function("block_creation", |b| {
        b.iter(|| {
            let _ = create_test_block(black_box(1), black_box(100));
        });
    });

    group.bench_function("block_serialization", |b| {
        let block = create_test_block(1, 100);
        b.iter(|| {
            let _ = serde_json::to_string(black_box(&block)).unwrap();
        });
    });

    group.bench_function("block_deserialization", |b| {
        let block = create_test_block(1, 100);
        let serialized = serde_json::to_string(&block).unwrap();
        b.iter(|| {
            let _: MultiVMBlock = serde_json::from_str(black_box(&serialized)).unwrap();
        });
    });

    group.finish();
}

/// Benchmark concurrent operations under stress
fn benchmark_concurrent_stress_test(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_stress");
    group.sample_size(10); // Reduce sample size for stress tests

    for num_tasks in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("parallel_fork_detection", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut handles = vec![];

                        // Spawn multiple fork detection operations
                        for i in 0..num_tasks {
                            let handle = tokio::spawn(async move {
                                let config = ForkDetectionConfig::default();
                                let detector = ForkDetectionManager::new(config).await.unwrap();

                                let block = create_test_block(i as u64, 5);
                                detector
                                    .process_block(block, format!("validator{}", i))
                                    .await
                                    .unwrap()
                            });
                            handles.push(handle);
                        }

                        // Wait for all to complete
                        for handle in handles {
                            let _ = handle.await.unwrap();
                        }
                    })
                });
            },
        );
    }

    for memory_load in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("memory_stress", memory_load),
            memory_load,
            |b, &memory_load| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut blocks = Vec::new();

                        // Create many blocks to stress memory
                        for i in 0..memory_load {
                            blocks.push(create_test_block(i as u64, 10));
                        }

                        // Process them through fork detection
                        let config = ForkDetectionConfig::default();
                        let detector = ForkDetectionManager::new(config).await.unwrap();

                        for (i, block) in blocks.into_iter().enumerate() {
                            let _ = detector
                                .process_block(block, format!("validator{}", i))
                                .await;
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark state persistence operations
fn benchmark_state_persistence(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut group = c.benchmark_group("state_persistence");

    group.bench_function("state_manager_operations", |b| {
        b.iter(|| {
            rt.block_on(async {
                use multivm_consensus::state::{CrossVMStateManager, StateManagerConfig};

                let config = StateManagerConfig::default();
                let state_manager = CrossVMStateManager::new(config);

                // Simulate state operations
                for i in 1..=100 {
                    let _ = state_manager.update_height(i);
                }

                let _ = state_manager.get_current_height();
            })
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_consensus_block_processing,
    benchmark_fork_detection,
    benchmark_network_recovery,
    benchmark_block_sync,
    benchmark_metrics_collection,
    benchmark_block_operations,
    benchmark_concurrent_stress_test,
    benchmark_state_persistence
);

criterion_main!(benches);
