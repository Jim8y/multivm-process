//! Integration test for mock processes with MultiVM
//!
//! This test verifies that MultiVM can communicate with mock Reth and Solana processes.

use multivm_common::{
    ipc::{IpcCommand, IpcMessage, IpcResponse},
    BlockchainType, ProcessId,
};
use multivm_process_manager::{ProcessConfig, ProcessHandle, ProcessManager};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_mock_processes_integration() {
    // Initialize process manager
    let mut manager = ProcessManager::new();
    
    // Configure mock Reth process
    let reth_config = ProcessConfig {
        process_id: ProcessId::Ethereum,
        process_type: multivm_common::ProcessType::Ethereum,
        executable_path: PathBuf::from("cargo"),
        args: vec![
            "run".to_string(),
            "--bin".to_string(),
            "mock-reth".to_string(),
            "-p".to_string(),
            "multivm-mock-processes".to_string(),
        ],
        socket_path: PathBuf::from("/tmp/multivm-ethereum.sock"),
        rpc_port: Some(8545),
        expected_startup_time: Duration::from_secs(3),
        health_check_interval: Duration::from_secs(5),
        max_restarts: 3,
        restart_delay: Duration::from_secs(1),
        environment: std::collections::HashMap::new(),
    };
    
    // Configure mock Solana process
    let solana_config = ProcessConfig {
        process_id: ProcessId::Solana,
        process_type: multivm_common::ProcessType::Solana,
        executable_path: PathBuf::from("cargo"),
        args: vec![
            "run".to_string(),
            "--bin".to_string(),
            "mock-solana".to_string(),
            "-p".to_string(),
            "multivm-mock-processes".to_string(),
        ],
        socket_path: PathBuf::from("/tmp/multivm-solana.sock"),
        rpc_port: Some(8899),
        expected_startup_time: Duration::from_secs(3),
        health_check_interval: Duration::from_secs(5),
        max_restarts: 3,
        restart_delay: Duration::from_secs(1),
        environment: std::collections::HashMap::new(),
    };
    
    // Start mock processes
    println!("Starting mock Reth process...");
    let reth_handle = manager.start_process(reth_config).await.unwrap();
    
    println!("Starting mock Solana process...");
    let solana_handle = manager.start_process(solana_config).await.unwrap();
    
    // Wait for processes to initialize
    sleep(Duration::from_secs(5)).await;
    
    // Test health check
    println!("Testing health checks...");
    let reth_health = manager.get_health(&ProcessId::Ethereum).await.unwrap();
    assert!(reth_health.is_healthy);
    assert_eq!(reth_health.process_id, ProcessId::Ethereum);
    
    let solana_health = manager.get_health(&ProcessId::Solana).await.unwrap();
    assert!(solana_health.is_healthy);
    assert_eq!(solana_health.process_id, ProcessId::Solana);
    
    // Test sending a block to Reth
    println!("Testing block processing...");
    let block_data = b"mock_ethereum_block_data";
    let response = manager.send_command(
        &ProcessId::Ethereum,
        IpcCommand::ProcessBlock {
            block_data_bytes: block_data.to_vec(),
            blockchain_type: BlockchainType::Ethereum,
            expect_response: true,
        },
    ).await.unwrap();
    
    match response {
        IpcResponse::BlockProcessed { success, .. } => {
            assert!(success, "Ethereum block processing should succeed");
        }
        _ => panic!("Unexpected response type"),
    }
    
    // Test sending a block to Solana
    let block_data = b"mock_solana_block_data";
    let response = manager.send_command(
        &ProcessId::Solana,
        IpcCommand::ProcessBlock {
            block_data_bytes: block_data.to_vec(),
            blockchain_type: BlockchainType::Solana,
            expect_response: true,
        },
    ).await.unwrap();
    
    match response {
        IpcResponse::BlockProcessed { success, .. } => {
            assert!(success, "Solana block processing should succeed");
        }
        _ => panic!("Unexpected response type"),
    }
    
    // Test getting state
    println!("Testing state retrieval...");
    let reth_state = manager.send_command(
        &ProcessId::Ethereum,
        IpcCommand::GetState,
    ).await.unwrap();
    
    match reth_state {
        IpcResponse::State { state } => {
            assert_eq!(state.process_id, ProcessId::Ethereum);
            assert_eq!(state.blockchain_type, BlockchainType::Ethereum);
            assert!(state.current_block.unwrap_or(0) > 0);
        }
        _ => panic!("Unexpected response type"),
    }
    
    // Shutdown processes
    println!("Shutting down processes...");
    manager.stop_process(&ProcessId::Ethereum).await.unwrap();
    manager.stop_process(&ProcessId::Solana).await.unwrap();
    
    println!("Integration test completed successfully!");
}

#[tokio::test]
async fn test_mock_process_restart() {
    let mut manager = ProcessManager::new();
    
    // Configure mock process with quick restart
    let config = ProcessConfig {
        process_id: ProcessId::Ethereum,
        process_type: multivm_common::ProcessType::Ethereum,
        executable_path: PathBuf::from("cargo"),
        args: vec![
            "run".to_string(),
            "--bin".to_string(),
            "mock-reth".to_string(),
            "-p".to_string(),
            "multivm-mock-processes".to_string(),
        ],
        socket_path: PathBuf::from("/tmp/multivm-ethereum-test.sock"),
        rpc_port: Some(8546),
        expected_startup_time: Duration::from_secs(3),
        health_check_interval: Duration::from_secs(2),
        max_restarts: 2,
        restart_delay: Duration::from_secs(1),
        environment: std::collections::HashMap::new(),
    };
    
    // Start process
    let handle = manager.start_process(config).await.unwrap();
    sleep(Duration::from_secs(3)).await;
    
    // Get initial PID
    let initial_pid = {
        let processes = manager.processes.read().await;
        processes.get(&ProcessId::Ethereum)
            .and_then(|p| p.handle.process_handle.lock().unwrap().as_ref().map(|h| h.id()))
            .unwrap()
    };
    
    // Kill the process to trigger restart
    println!("Killing process to test restart...");
    {
        let processes = manager.processes.read().await;
        if let Some(process) = processes.get(&ProcessId::Ethereum) {
            if let Some(handle) = process.handle.process_handle.lock().unwrap().as_mut() {
                handle.kill().unwrap();
            }
        }
    }
    
    // Wait for restart
    sleep(Duration::from_secs(5)).await;
    
    // Check that process restarted with different PID
    let new_pid = {
        let processes = manager.processes.read().await;
        processes.get(&ProcessId::Ethereum)
            .and_then(|p| p.handle.process_handle.lock().unwrap().as_ref().map(|h| h.id()))
    };
    
    assert!(new_pid.is_some(), "Process should have restarted");
    assert_ne!(Some(initial_pid), new_pid, "Process should have new PID after restart");
    
    // Cleanup
    manager.stop_process(&ProcessId::Ethereum).await.unwrap();
}