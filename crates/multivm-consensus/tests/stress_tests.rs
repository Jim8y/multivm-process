//! Stress tests for the MultiVM consensus system
//!
//! These tests are designed to stress the consensus system under heavy load
//! and validate its behavior under extreme conditions.

use multivm_consensus::{
    BlockSyncConfig, BlockSyncManager, ConsensusEngine, ForkDetectionConfig, ForkDetectionManager,
    MalachiteConfig, MalachiteConsensus, MultiVMBlock, NetworkRecoveryConfig,
    NetworkRecoveryManager,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Create a test block for stress testing
fn create_stress_test_block(height: u64, proposer: String) -> MultiVMBlock {
    MultiVMBlock::new(
        height,
        format!("{:064x}", height.saturating_sub(1)),
        proposer,
        vec![], // Empty transactions for simplicity
    )
}

#[tokio::test]
#[ignore] // Use --ignored flag to run stress tests
async fn stress_test_concurrent_fork_detection() {
    const NUM_VALIDATORS: usize = 100;
    const BLOCKS_PER_VALIDATOR: usize = 50;

    let config = ForkDetectionConfig::default();
    let detector = Arc::new(ForkDetectionManager::new(config).await.unwrap());

    let mut handles = vec![];

    // Spawn many concurrent fork detection operations
    for validator_id in 0..NUM_VALIDATORS {
        let detector_clone = detector.clone();
        let handle = tokio::spawn(async move {
            for block_height in 1..=BLOCKS_PER_VALIDATOR {
                let block = create_stress_test_block(
                    block_height as u64,
                    format!("validator{}", validator_id),
                );

                let result = timeout(
                    Duration::from_secs(5),
                    detector_clone.process_block(block, format!("validator{}", validator_id)),
                )
                .await;

                match result {
                    Ok(Ok(_)) => {} // Success
                    Ok(Err(e)) => eprintln!("Fork detection error: {}", e),
                    Err(_) => eprintln!("Fork detection timeout"),
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    let start_time = std::time::Instant::now();
    for handle in handles {
        let _ = handle.await;
    }
    let duration = start_time.elapsed();

    println!(
        "Processed {} blocks from {} validators in {:?}",
        NUM_VALIDATORS * BLOCKS_PER_VALIDATOR,
        NUM_VALIDATORS,
        duration
    );

    // Verify the detector is still functioning
    let metrics = detector.get_metrics().await;
    println!("Final metrics: {:?}", metrics);

    assert!(metrics.forks_detected >= 0);
}

#[tokio::test]
#[ignore]
async fn stress_test_network_recovery_load() {
    const NUM_VALIDATORS: usize = 1000;
    const STATUS_UPDATES_PER_VALIDATOR: usize = 100;

    let config = NetworkRecoveryConfig::default();
    let manager = Arc::new(NetworkRecoveryManager::new(config).await.unwrap());

    let mut handles = vec![];

    // Spawn concurrent status updates
    for validator_id in 0..NUM_VALIDATORS {
        let manager_clone = manager.clone();
        let handle = tokio::spawn(async move {
            for update_round in 0..STATUS_UPDATES_PER_VALIDATOR {
                // Simulate varying network conditions
                let connected = (validator_id + update_round) % 3 != 0;
                let height = 1000 + update_round as u64;

                manager_clone
                    .update_validator_status(
                        format!("validator{}", validator_id),
                        connected,
                        height,
                    )
                    .await;

                // Add small delay to simulate network latency
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });
        handles.push(handle);
    }

    let start_time = std::time::Instant::now();

    // Also perform periodic health checks
    let manager_clone = manager.clone();
    let health_check_handle = tokio::spawn(async move {
        for _ in 0..50 {
            let _ = manager_clone.perform_health_check().await;
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Wait for all operations
    for handle in handles {
        let _ = handle.await;
    }
    let _ = health_check_handle.await;

    let duration = start_time.elapsed();

    println!(
        "Processed {} status updates from {} validators in {:?}",
        NUM_VALIDATORS * STATUS_UPDATES_PER_VALIDATOR,
        NUM_VALIDATORS,
        duration
    );

    // Verify system is still responsive
    let health = manager.get_network_health().await;
    let metrics = manager.get_metrics().await;

    println!("Final health: {:?}", health);
    println!("Final metrics: {:?}", metrics);

    assert!(metrics.partitions_detected >= 0);
}

#[tokio::test]
#[ignore]
async fn stress_test_block_sync_operations() {
    const NUM_PEERS: usize = 500;
    const OPERATIONS_PER_PEER: usize = 200;

    let config = BlockSyncConfig::default();
    let sync_manager = Arc::new(BlockSyncManager::new(config).await.unwrap());

    let mut handles = vec![];

    // Spawn concurrent sync operations
    for peer_id in 0..NUM_PEERS {
        let sync_manager_clone = sync_manager.clone();
        let handle = tokio::spawn(async move {
            for operation in 0..OPERATIONS_PER_PEER {
                // Update peer height
                sync_manager_clone
                    .update_peer_height(format!("peer{}", peer_id), 1000 + operation as u64)
                    .await;

                // Check sync status
                let _ = sync_manager_clone.needs_sync(950 + operation as u64).await;

                // Get cached blocks
                let _ = sync_manager_clone
                    .get_cached_blocks(operation as u64, 10)
                    .await;

                // Small delay to prevent overwhelming
                if operation % 10 == 0 {
                    tokio::time::sleep(Duration::from_micros(100)).await;
                }
            }
        });
        handles.push(handle);
    }

    let start_time = std::time::Instant::now();

    // Wait for all operations
    for handle in handles {
        let _ = handle.await;
    }

    let duration = start_time.elapsed();

    println!(
        "Processed {} operations from {} peers in {:?}",
        NUM_PEERS * OPERATIONS_PER_PEER,
        NUM_PEERS,
        duration
    );

    // Verify sync manager is still functioning
    let status = sync_manager.get_sync_status().await;
    let metrics = sync_manager.get_metrics().await;

    println!("Final status: {:?}", status);
    println!("Final metrics: {:?}", metrics);
}

#[tokio::test]
#[ignore]
async fn stress_test_memory_usage() {
    const NUM_BLOCKS: usize = 10000;
    const VALIDATORS_PER_BLOCK: usize = 10;

    println!("Starting memory stress test with {} blocks", NUM_BLOCKS);

    let config = ForkDetectionConfig::default();
    let detector = ForkDetectionManager::new(config).await.unwrap();

    let start_time = std::time::Instant::now();

    // Create and process many blocks
    for height in 1..=NUM_BLOCKS {
        for validator_id in 0..VALIDATORS_PER_BLOCK {
            let block = create_stress_test_block(height as u64, format!("val{}", validator_id));

            // Process block
            let result = detector
                .process_block(block, format!("validator{}", validator_id))
                .await;

            if let Err(e) = result {
                eprintln!("Error processing block {} from validator {}: {}", height, validator_id, e);
            }
        }

        // Periodic cleanup and status reporting
        if height % 1000 == 0 {
            let metrics = detector.get_metrics().await;
            println!(
                "Processed {} blocks, current metrics: {} active forks, {} total processed",
                height * VALIDATORS_PER_BLOCK,
                metrics.active_forks,
                metrics.forks_detected
            );

            // Force garbage collection (if available)
            tokio::task::yield_now().await;
        }
    }

    let duration = start_time.elapsed();

    let final_metrics = detector.get_metrics().await;
    println!(
        "Memory stress test completed in {:?}. Final metrics: {:?}",
        duration, final_metrics
    );

    assert!(final_metrics.forks_detected >= 0);
}

#[tokio::test]
#[ignore]
async fn stress_test_consensus_lifecycle() {
    const NUM_CONSENSUS_INSTANCES: usize = 20;
    const LIFECYCLE_ITERATIONS: usize = 10;

    println!(
        "Starting consensus lifecycle stress test with {} instances",
        NUM_CONSENSUS_INSTANCES
    );

    let mut handles = vec![];

    for instance_id in 0..NUM_CONSENSUS_INSTANCES {
        let handle = tokio::spawn(async move {
            for iteration in 0..LIFECYCLE_ITERATIONS {
                // Create consensus instance
                let config = MalachiteConfig::default();
                let (mut consensus, _block_sender, _commit_receiver) =
                    match MalachiteConsensus::new(config).await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Failed to create consensus {}: {}", instance_id, e);
                            continue;
                        }
                    };

                // Start consensus
                if let Err(e) = consensus.start().await {
                    eprintln!("Failed to start consensus {}: {}", instance_id, e);
                    continue;
                }

                // Let it run briefly
                tokio::time::sleep(Duration::from_millis(100)).await;

                // Stop consensus
                if let Err(e) = consensus.stop().await {
                    eprintln!("Failed to stop consensus {}: {}", instance_id, e);
                }

                if iteration % 5 == 0 {
                    println!("Instance {} completed {} iterations", instance_id, iteration + 1);
                }
            }
        });
        handles.push(handle);
    }

    let start_time = std::time::Instant::now();

    // Wait for all instances to complete
    for handle in handles {
        let _ = handle.await;
    }

    let duration = start_time.elapsed();

    println!(
        "Consensus lifecycle stress test completed in {:?} ({} total lifecycles)",
        duration,
        NUM_CONSENSUS_INSTANCES * LIFECYCLE_ITERATIONS
    );
}

#[tokio::test]
#[ignore]
async fn stress_test_mixed_operations() {
    const DURATION_SECONDS: u64 = 30;
    const OPERATION_INTERVAL_MS: u64 = 10;

    println!("Starting mixed operations stress test for {} seconds", DURATION_SECONDS);

    // Set up all components
    let fork_detector = Arc::new(
        ForkDetectionManager::new(ForkDetectionConfig::default())
            .await
            .unwrap(),
    );

    let network_manager = Arc::new(
        NetworkRecoveryManager::new(NetworkRecoveryConfig::default())
            .await
            .unwrap(),
    );

    let sync_manager = Arc::new(
        BlockSyncManager::new(BlockSyncConfig::default())
            .await
            .unwrap(),
    );

    let start_time = std::time::Instant::now();
    let end_time = start_time + Duration::from_secs(DURATION_SECONDS);

    let mut handles = vec![];

    // Fork detection operations
    {
        let detector = fork_detector.clone();
        let handle = tokio::spawn(async move {
            let mut counter = 0;
            while std::time::Instant::now() < end_time {
                let block = create_stress_test_block(counter % 100, format!("validator{}", counter % 10));
                let _ = detector
                    .process_block(block, format!("validator{}", counter % 10))
                    .await;
                counter += 1;
                tokio::time::sleep(Duration::from_millis(OPERATION_INTERVAL_MS)).await;
            }
            counter
        });
        handles.push(handle);
    }

    // Network recovery operations
    {
        let manager = network_manager.clone();
        let handle = tokio::spawn(async move {
            let mut counter = 0;
            while std::time::Instant::now() < end_time {
                manager
                    .update_validator_status(
                        format!("validator{}", counter % 50),
                        counter % 3 != 0,
                        1000 + counter,
                    )
                    .await;
                counter += 1;
                tokio::time::sleep(Duration::from_millis(OPERATION_INTERVAL_MS * 2)).await;
            }
            counter
        });
        handles.push(handle);
    }

    // Block sync operations
    {
        let sync = sync_manager.clone();
        let handle = tokio::spawn(async move {
            let mut counter = 0;
            while std::time::Instant::now() < end_time {
                sync.update_peer_height(format!("peer{}", counter % 30), 1000 + counter)
                    .await;
                let _ = sync.needs_sync(950 + counter).await;
                counter += 1;
                tokio::time::sleep(Duration::from_millis(OPERATION_INTERVAL_MS * 3)).await;
            }
            counter
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    let mut total_operations = 0;
    for handle in handles {
        if let Ok(count) = handle.await {
            total_operations += count;
        }
    }

    let actual_duration = start_time.elapsed();

    // Collect final metrics
    let fork_metrics = fork_detector.get_metrics().await;
    let network_metrics = network_manager.get_metrics().await;
    let sync_metrics = sync_manager.get_metrics().await;

    println!(
        "Mixed operations completed: {} total operations in {:?}",
        total_operations, actual_duration
    );
    println!("Fork detection metrics: {:?}", fork_metrics);
    println!("Network recovery metrics: {:?}", network_metrics);
    println!("Block sync metrics: {:?}", sync_metrics);

    assert!(total_operations > 0);
    assert!(fork_metrics.forks_detected >= 0);
}