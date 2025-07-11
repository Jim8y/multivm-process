//! Performance and load tests for the MultiVM system

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::{sleep, timeout};
use multivm_core::{NodeId, VmId, ResourceRequirements};
use orchestrator::{Orchestrator, OrchestratorConfig};
use consensus::Config as ConsensusConfig;

/// Performance test results
#[derive(Debug)]
struct PerfResults {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    duration: Duration,
    ops_per_second: f64,
    avg_latency_ms: f64,
    p99_latency_ms: f64,
}

impl PerfResults {
    fn calculate(
        total: u64,
        successful: u64,
        failed: u64,
        duration: Duration,
        latencies: &mut Vec<Duration>,
    ) -> Self {
        latencies.sort_unstable();
        
        let ops_per_second = total as f64 / duration.as_secs_f64();
        let avg_latency_ms = if !latencies.is_empty() {
            latencies.iter().map(|d| d.as_secs_f64() * 1000.0).sum::<f64>() / latencies.len() as f64
        } else {
            0.0
        };
        
        let p99_latency_ms = if !latencies.is_empty() {
            let p99_idx = ((latencies.len() as f64 * 0.99) as usize).min(latencies.len() - 1);
            latencies[p99_idx].as_secs_f64() * 1000.0
        } else {
            0.0
        };
        
        Self {
            total_operations: total,
            successful_operations: successful,
            failed_operations: failed,
            duration,
            ops_per_second,
            avg_latency_ms,
            p99_latency_ms,
        }
    }
}

async fn create_test_orchestrator(node_id: NodeId, data_dir: &std::path::Path) -> Orchestrator {
    let config = OrchestratorConfig {
        node_id: Some(node_id),
        data_dir: data_dir.to_path_buf(),
        consensus: ConsensusConfig {
            node_id,
            cluster_members: vec![(node_id, "127.0.0.1:0".parse().unwrap())],
            election_timeout_min: Duration::from_millis(150),
            election_timeout_max: Duration::from_millis(300),
            heartbeat_interval: Duration::from_millis(50),
            batch_size: 100,
            snapshot_interval: 1000,
        },
        enable_tls: false,
        ..Default::default()
    };
    
    Orchestrator::new(config).await.unwrap()
}

#[tokio::test]
async fn test_vm_creation_performance() {
    let temp_dir = TempDir::new().unwrap();
    let orchestrator = Arc::new(create_test_orchestrator(NodeId::new(), temp_dir.path()).await);
    
    // Force leadership
    orchestrator.consensus.force_leader().await;
    
    let total_vms = 1000;
    let concurrent_workers = 10;
    let vms_per_worker = total_vms / concurrent_workers;
    
    let successful = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    
    let start = Instant::now();
    let mut handles = vec![];
    
    for worker_id in 0..concurrent_workers {
        let orchestrator_clone = orchestrator.clone();
        let successful_clone = successful.clone();
        let failed_clone = failed.clone();
        let latencies_clone = latencies.clone();
        
        let handle = tokio::spawn(async move {
            for i in 0..vms_per_worker {
                let vm_id = VmId::new();
                let resources = ResourceRequirements::new(0.5, 256, 5, Some(10)).unwrap();
                let vm_name = format!("perf-vm-{}-{}", worker_id, i);
                
                let op_start = Instant::now();
                match orchestrator_clone.create_vm(vm_id, vm_name, resources).await {
                    Ok(_) => {
                        successful_clone.fetch_add(1, Ordering::Relaxed);
                        let latency = op_start.elapsed();
                        latencies_clone.lock().await.push(latency);
                    }
                    Err(_) => {
                        failed_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
        
        handles.push(handle);
    }
    
    futures::future::join_all(handles).await;
    let duration = start.elapsed();
    
    let mut latencies_vec = latencies.lock().await.clone();
    let results = PerfResults::calculate(
        total_vms as u64,
        successful.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed),
        duration,
        &mut latencies_vec,
    );
    
    println!("VM Creation Performance Test Results:");
    println!("  Total VMs: {}", results.total_operations);
    println!("  Successful: {}", results.successful_operations);
    println!("  Failed: {}", results.failed_operations);
    println!("  Duration: {:?}", results.duration);
    println!("  Throughput: {:.2} VMs/second", results.ops_per_second);
    println!("  Avg Latency: {:.2}ms", results.avg_latency_ms);
    println!("  P99 Latency: {:.2}ms", results.p99_latency_ms);
    
    // Assert minimum performance requirements
    assert!(results.ops_per_second > 50.0, "Should create at least 50 VMs/second");
    assert!(results.avg_latency_ms < 200.0, "Average latency should be under 200ms");
    assert_eq!(results.failed_operations, 0, "All operations should succeed");
}

#[tokio::test]
async fn test_concurrent_operations_stress() {
    let temp_dir = TempDir::new().unwrap();
    let orchestrator = Arc::new(create_test_orchestrator(NodeId::new(), temp_dir.path()).await);
    orchestrator.consensus.force_leader().await;
    
    // Create a mix of operations
    let num_operations = 500;
    let concurrent_workers = 20;
    
    let successful = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    
    let start = Instant::now();
    let mut handles = vec![];
    
    // Pre-create some VMs for operations
    let mut vm_ids = vec![];
    for _ in 0..50 {
        let vm_id = VmId::new();
        let resources = ResourceRequirements::new(1.0, 512, 10, None).unwrap();
        orchestrator.create_vm(vm_id, "stress-vm".to_string(), resources).await.unwrap();
        vm_ids.push(vm_id);
    }
    
    let vm_ids = Arc::new(vm_ids);
    
    for worker_id in 0..concurrent_workers {
        let orchestrator_clone = orchestrator.clone();
        let successful_clone = successful.clone();
        let failed_clone = failed.clone();
        let vm_ids_clone = vm_ids.clone();
        
        let handle = tokio::spawn(async move {
            for i in 0..num_operations/concurrent_workers {
                // Mix of operations
                let op = i % 5;
                let result = match op {
                    0 => {
                        // Create VM
                        let vm_id = VmId::new();
                        let resources = ResourceRequirements::new(0.5, 256, 5, None).unwrap();
                        orchestrator_clone.create_vm(vm_id, format!("stress-{}", i), resources).await
                    }
                    1 => {
                        // List VMs
                        orchestrator_clone.list_vms().await.map(|_| ())
                    }
                    2 => {
                        // Get VM state
                        let vm_id = vm_ids_clone[i % vm_ids_clone.len()];
                        orchestrator_clone.get_vm_state(vm_id).await.map(|_| ())
                    }
                    3 => {
                        // Start VM
                        let vm_id = vm_ids_clone[i % vm_ids_clone.len()];
                        orchestrator_clone.start_vm(vm_id).await
                    }
                    4 => {
                        // Stop VM
                        let vm_id = vm_ids_clone[i % vm_ids_clone.len()];
                        orchestrator_clone.stop_vm(vm_id).await
                    }
                    _ => unreachable!(),
                };
                
                match result {
                    Ok(_) => successful_clone.fetch_add(1, Ordering::Relaxed),
                    Err(_) => failed_clone.fetch_add(1, Ordering::Relaxed),
                };
            }
        });
        
        handles.push(handle);
    }
    
    futures::future::join_all(handles).await;
    let duration = start.elapsed();
    
    let total = successful.load(Ordering::Relaxed) + failed.load(Ordering::Relaxed);
    let ops_per_second = total as f64 / duration.as_secs_f64();
    
    println!("Concurrent Operations Stress Test Results:");
    println!("  Total Operations: {}", total);
    println!("  Successful: {}", successful.load(Ordering::Relaxed));
    println!("  Failed: {}", failed.load(Ordering::Relaxed));
    println!("  Duration: {:?}", duration);
    println!("  Throughput: {:.2} ops/second", ops_per_second);
    
    // Assert system remains stable under load
    let failure_rate = failed.load(Ordering::Relaxed) as f64 / total as f64;
    assert!(failure_rate < 0.05, "Failure rate should be less than 5%");
    assert!(ops_per_second > 100.0, "Should handle at least 100 ops/second");
}

#[tokio::test]
async fn test_memory_usage_under_load() {
    use sysinfo::System;
    
    let temp_dir = TempDir::new().unwrap();
    let orchestrator = Arc::new(create_test_orchestrator(NodeId::new(), temp_dir.path()).await);
    orchestrator.consensus.force_leader().await;
    
    // Get initial memory usage
    let mut system = System::new_all();
    system.refresh_memory();
    let initial_memory = system.used_memory();
    
    // Create many VMs to test memory usage
    let total_vms = 5000;
    let batch_size = 100;
    
    for batch in 0..total_vms/batch_size {
        let mut handles = vec![];
        
        for i in 0..batch_size {
            let orchestrator_clone = orchestrator.clone();
            let vm_id = VmId::new();
            let resources = ResourceRequirements::new(0.1, 64, 1, None).unwrap();
            
            let handle = tokio::spawn(async move {
                orchestrator_clone.create_vm(
                    vm_id,
                    format!("mem-test-vm-{}-{}", batch, i),
                    resources,
                ).await
            });
            
            handles.push(handle);
        }
        
        futures::future::join_all(handles).await;
        
        // Check memory periodically
        if batch % 10 == 0 {
            system.refresh_memory();
            let current_memory = system.used_memory();
            let memory_increase_mb = (current_memory - initial_memory) / 1024 / 1024;
            println!("Memory after {} VMs: +{}MB", batch * batch_size, memory_increase_mb);
        }
    }
    
    // Final memory check
    system.refresh_memory();
    let final_memory = system.used_memory();
    let total_memory_increase_mb = (final_memory - initial_memory) / 1024 / 1024;
    
    println!("Memory Usage Test Results:");
    println!("  Total VMs created: {}", total_vms);
    println!("  Memory increase: {}MB", total_memory_increase_mb);
    println!("  Memory per VM: {:.2}KB", (total_memory_increase_mb * 1024) as f64 / total_vms as f64);
    
    // Assert reasonable memory usage
    let memory_per_vm_kb = (total_memory_increase_mb * 1024) as f64 / total_vms as f64;
    assert!(memory_per_vm_kb < 100.0, "Memory usage per VM should be under 100KB");
}

#[tokio::test]
async fn test_cluster_scalability() {
    // Test with different cluster sizes
    let cluster_sizes = vec![1, 3, 5];
    
    for size in cluster_sizes {
        println!("\nTesting cluster size: {}", size);
        
        let temp_dirs: Vec<TempDir> = (0..size).map(|_| TempDir::new().unwrap()).collect();
        let node_ids: Vec<NodeId> = (0..size).map(|_| NodeId::new()).collect();
        let addresses: Vec<_> = (0..size)
            .map(|i| format!("127.0.0.1:{}", 9000 + i).parse().unwrap())
            .collect();
        
        let cluster_members: Vec<_> = node_ids.iter().zip(addresses.iter())
            .map(|(id, addr)| (*id, *addr))
            .collect();
        
        // Create cluster nodes
        let mut orchestrators = vec![];
        for i in 0..size {
            let config = OrchestratorConfig {
                node_id: Some(node_ids[i]),
                data_dir: temp_dirs[i].path().to_path_buf(),
                consensus: ConsensusConfig {
                    node_id: node_ids[i],
                    cluster_members: cluster_members.clone(),
                    election_timeout_min: Duration::from_millis(150),
                    election_timeout_max: Duration::from_millis(300),
                    heartbeat_interval: Duration::from_millis(50),
                    batch_size: 50,
                    snapshot_interval: 500,
                },
                enable_tls: false,
                ..Default::default()
            };
            
            let orchestrator = Orchestrator::new(config).await.unwrap();
            orchestrators.push(Arc::new(orchestrator));
        }
        
        // Start all nodes
        let mut handles = vec![];
        for orchestrator in &orchestrators {
            let orchestrator_clone = orchestrator.clone();
            let handle = tokio::spawn(async move {
                orchestrator_clone.run().await.unwrap();
            });
            handles.push(handle);
        }
        
        // Wait for leader election
        sleep(Duration::from_secs(2)).await;
        
        // Find leader
        let leader = orchestrators.iter()
            .find(|o| futures::executor::block_on(o.is_leader()))
            .expect("Should have a leader");
        
        // Benchmark operations
        let start = Instant::now();
        let operations = 100;
        
        for i in 0..operations {
            let vm_id = VmId::new();
            let resources = ResourceRequirements::new(0.5, 256, 5, None).unwrap();
            leader.create_vm(vm_id, format!("scale-vm-{}", i), resources).await.unwrap();
        }
        
        let duration = start.elapsed();
        let ops_per_second = operations as f64 / duration.as_secs_f64();
        
        println!("  Operations: {}", operations);
        println!("  Duration: {:?}", duration);
        println!("  Throughput: {:.2} ops/second", ops_per_second);
        
        // Stop all nodes
        for handle in handles {
            handle.abort();
        }
    }
}

#[tokio::test]
async fn test_sustained_load() {
    let temp_dir = TempDir::new().unwrap();
    let orchestrator = Arc::new(create_test_orchestrator(NodeId::new(), temp_dir.path()).await);
    orchestrator.consensus.force_leader().await;
    
    // Run sustained load for 30 seconds
    let duration = Duration::from_secs(30);
    let target_ops_per_second = 50;
    let operation_interval = Duration::from_millis(1000 / target_ops_per_second);
    
    let successful = Arc::new(AtomicU64::new(0));
    let failed = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    
    let start = Instant::now();
    let orchestrator_clone = orchestrator.clone();
    let successful_clone = successful.clone();
    let failed_clone = failed.clone();
    let latencies_clone = latencies.clone();
    
    let load_generator = tokio::spawn(async move {
        let mut interval = tokio::time::interval(operation_interval);
        let mut vm_count = 0;
        
        while start.elapsed() < duration {
            interval.tick().await;
            
            let vm_id = VmId::new();
            let resources = ResourceRequirements::new(0.5, 256, 5, None).unwrap();
            let vm_name = format!("sustained-vm-{}", vm_count);
            vm_count += 1;
            
            let op_start = Instant::now();
            match orchestrator_clone.create_vm(vm_id, vm_name, resources).await {
                Ok(_) => {
                    successful_clone.fetch_add(1, Ordering::Relaxed);
                    let latency = op_start.elapsed();
                    latencies_clone.lock().await.push(latency);
                }
                Err(_) => {
                    failed_clone.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    });
    
    load_generator.await.unwrap();
    let actual_duration = start.elapsed();
    
    let mut latencies_vec = latencies.lock().await.clone();
    let results = PerfResults::calculate(
        successful.load(Ordering::Relaxed) + failed.load(Ordering::Relaxed),
        successful.load(Ordering::Relaxed),
        failed.load(Ordering::Relaxed),
        actual_duration,
        &mut latencies_vec,
    );
    
    println!("Sustained Load Test Results:");
    println!("  Target Duration: {:?}", duration);
    println!("  Actual Duration: {:?}", actual_duration);
    println!("  Total Operations: {}", results.total_operations);
    println!("  Successful: {}", results.successful_operations);
    println!("  Failed: {}", results.failed_operations);
    println!("  Throughput: {:.2} ops/second", results.ops_per_second);
    println!("  Avg Latency: {:.2}ms", results.avg_latency_ms);
    println!("  P99 Latency: {:.2}ms", results.p99_latency_ms);
    
    // Assert sustained performance
    assert!(results.failed_operations == 0, "No operations should fail under sustained load");
    assert!(results.avg_latency_ms < 100.0, "Average latency should remain low");
    assert!(results.p99_latency_ms < 500.0, "P99 latency should be reasonable");
}