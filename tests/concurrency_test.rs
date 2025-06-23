//! Concurrency tests for MultiVM
//!
//! Tests for race conditions and deadlock prevention.

use multivm_process_manager::{
    MultivmProcessManager, ProcessManagerConfig, ProcessHandle, ProcessId,
};
use multivm_common::error::MultivmResult;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Barrier;
use tokio::time::sleep;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_concurrent_restart_no_race() {
    // Create a process manager with test config
    let config = ProcessManagerConfig::default();
    let manager = Arc::new(MultivmProcessManager::new(config));
    
    // Register a mock process
    let mock_handle = ProcessHandle {
        process_id: ProcessId::Ethereum,
        process_handle: Arc::new(std::sync::Mutex::new(None)),
        ipc_client: None,
        start_time: std::time::Instant::now(),
        restart_count: 0,
        config: Default::default(),
    };
    
    manager.register_process(mock_handle).await.unwrap();
    
    // Create barrier to synchronize threads
    let barrier = Arc::new(Barrier::new(3));
    
    // Spawn multiple tasks trying to restart the same process
    let mut handles = vec![];
    
    for i in 0..3 {
        let manager_clone = Arc::clone(&manager);
        let barrier_clone = Arc::clone(&barrier);
        
        let handle = tokio::spawn(async move {
            // Wait for all tasks to be ready
            barrier_clone.wait().await;
            
            // Try to restart the process
            let result = manager_clone.restart_process(ProcessId::Ethereum).await;
            
            println!("Task {} restart result: {:?}", i, result);
            result
        });
        
        handles.push(handle);
    }
    
    // Collect results
    let mut success_count = 0;
    let mut not_found_count = 0;
    
    for handle in handles {
        match handle.await.unwrap() {
            Ok(_) => success_count += 1,
            Err(e) => {
                if e.to_string().contains("Process not found") {
                    not_found_count += 1;
                }
            }
        }
    }
    
    // Only one should succeed, others should get "not found" error
    assert_eq!(success_count, 1, "Exactly one restart should succeed");
    assert_eq!(not_found_count, 2, "Two restarts should fail with 'not found'");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_shutdown_during_registration_no_race() {
    let config = ProcessManagerConfig::default();
    let manager = Arc::new(MultivmProcessManager::new(config));
    
    // Barrier to synchronize shutdown and registration
    let barrier = Arc::new(Barrier::new(2));
    
    // Task 1: Try to register processes
    let manager_clone = Arc::clone(&manager);
    let barrier_clone = Arc::clone(&barrier);
    let registration_task = tokio::spawn(async move {
        barrier_clone.wait().await;
        
        // Try to register multiple processes
        for i in 0..5 {
            let mock_handle = ProcessHandle {
                process_id: ProcessId::Ethereum,
                process_handle: Arc::new(std::sync::Mutex::new(None)),
                ipc_client: None,
                start_time: std::time::Instant::now(),
                restart_count: i,
                config: Default::default(),
            };
            
            // Small delay between registrations
            sleep(Duration::from_millis(10)).await;
            
            if let Err(e) = manager_clone.register_process(mock_handle).await {
                println!("Registration {} failed (expected during shutdown): {}", i, e);
            }
        }
    });
    
    // Task 2: Shutdown the manager
    let manager_clone = Arc::clone(&manager);
    let barrier_clone = Arc::clone(&barrier);
    let shutdown_task = tokio::spawn(async move {
        barrier_clone.wait().await;
        
        // Small delay to let some registrations happen
        sleep(Duration::from_millis(20)).await;
        
        manager_clone.shutdown(true).await.unwrap();
    });
    
    // Wait for both tasks
    let _ = tokio::join!(registration_task, shutdown_task);
    
    // Verify manager is empty after shutdown
    let processes = manager.inner.processes.read().await;
    assert_eq!(processes.len(), 0, "All processes should be removed after shutdown");
}

#[tokio::test]
async fn test_lock_timeout() {
    use multivm_process_manager::lock_ordering::{acquire_write_lock, LockLevel};
    use tokio::sync::RwLock;
    
    let lock = Arc::new(RwLock::new(42));
    
    // Hold a write lock
    let _guard = lock.write().await;
    
    // Try to acquire with timeout
    let result = acquire_write_lock(
        &lock,
        LockLevel::CoordinatorState,
        Some(Duration::from_millis(100))
    ).await;
    
    assert!(result.is_err());
    match result {
        Err(e) => assert!(e.to_string().contains("Lock timeout")),
        Ok(_) => panic!("Should have timed out"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_no_deadlock_with_ordered_locks() {
    use multivm_process_manager::lock_ordering::{acquire_write_lock, LockLevel};
    use tokio::sync::RwLock;
    
    let lock1 = Arc::new(RwLock::new(1));
    let lock2 = Arc::new(RwLock::new(2));
    
    let barrier = Arc::new(Barrier::new(2));
    
    // Task 1: Acquire lock1 then lock2 (correct order)
    let lock1_clone = Arc::clone(&lock1);
    let lock2_clone = Arc::clone(&lock2);
    let barrier_clone = Arc::clone(&barrier);
    let task1 = tokio::spawn(async move {
        barrier_clone.wait().await;
        
        let _g1 = acquire_write_lock(&lock1_clone, LockLevel::CoordinatorState, None)
            .await
            .unwrap();
        sleep(Duration::from_millis(50)).await;
        let _g2 = acquire_write_lock(&lock2_clone, LockLevel::AccountMappings, None)
            .await
            .unwrap();
        
        "Task 1 completed"
    });
    
    // Task 2: Also acquire lock1 then lock2 (same order - no deadlock)
    let lock1_clone = Arc::clone(&lock1);
    let lock2_clone = Arc::clone(&lock2);
    let barrier_clone = Arc::clone(&barrier);
    let task2 = tokio::spawn(async move {
        barrier_clone.wait().await;
        
        sleep(Duration::from_millis(25)).await; // Start slightly after task1
        let _g1 = acquire_write_lock(&lock1_clone, LockLevel::CoordinatorState, None)
            .await
            .unwrap();
        let _g2 = acquire_write_lock(&lock2_clone, LockLevel::AccountMappings, None)
            .await
            .unwrap();
        
        "Task 2 completed"
    });
    
    // Both tasks should complete without deadlock
    let (r1, r2) = tokio::join!(task1, task2);
    assert_eq!(r1.unwrap(), "Task 1 completed");
    assert_eq!(r2.unwrap(), "Task 2 completed");
}