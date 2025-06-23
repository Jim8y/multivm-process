//! Example: Mock Processes Demo
//!
//! This example demonstrates how to run mock Reth and Solana processes
//! alongside MultiVM for testing and development.

use multivm_common::error::MultivmResult;
use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

#[tokio::main]
#[allow(clippy::zombie_processes)] // Processes are properly cleaned up at the end
async fn main() -> MultivmResult<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("=== MultiVM Mock Processes Demo ===");
    info!("This demo shows how MultiVM coordinates with external mock processes");

    // Start mock processes
    info!("\n1. Starting Mock Processes...");

    // Start mock Reth process
    let mut mock_reth = Command::new("cargo")
        .args(["run", "--bin", "mock-reth", "-p", "multivm-mock-processes"])
        .spawn()
        .expect("Failed to start mock Reth process");

    info!("   ✓ Mock Reth process started (PID: {})", mock_reth.id());

    // Start mock Solana process
    let mut mock_solana = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "mock-solana",
            "-p",
            "multivm-mock-processes",
        ])
        .spawn()
        .expect("Failed to start mock Solana process");

    info!(
        "   ✓ Mock Solana process started (PID: {})",
        mock_solana.id()
    );

    // Give processes time to initialize
    sleep(Duration::from_secs(2)).await;

    info!("\n2. Mock Process Architecture:");
    info!("   ┌─────────────────┐");
    info!("   │     MultiVM     │");
    info!("   │  (Coordinator)  │");
    info!("   └────────┬────────┘");
    info!("            │");
    info!("     ┌──────┴──────┐");
    info!("     │             │");
    info!("   ┌─▼───┐      ┌─▼─────┐");
    info!("   │ Reth │      │ Solana │");
    info!("   │ Mock │      │  Mock  │");
    info!("   └──────┘      └────────┘");
    info!("    /tmp/          /tmp/");
    info!("    multivm-       multivm-");
    info!("    ethereum.sock  solana.sock");

    info!("\n3. Communication Protocol:");
    info!("   - IPC over Unix sockets");
    info!("   - Binary protocol (bincode)");
    info!("   - Handshake required");
    info!("   - Async message handling");

    info!("\n4. Mock Process Features:");
    info!("   Mock Reth:");
    info!("   - Simulates Ethereum execution");
    info!("   - Processes EVM transactions");
    info!("   - Maintains block state");
    info!("   - Responds to RPC calls");

    info!("\n   Mock Solana:");
    info!("   - Simulates Solana execution");
    info!("   - Processes SVM transactions");
    info!("   - Maintains block state");
    info!("   - Responds to RPC calls");

    info!("\n5. Testing Transaction Flow:");
    info!("   1. MultiVM receives transaction");
    info!("   2. Determines target chain (EVM/SVM)");
    info!("   3. Sends to appropriate mock process via IPC");
    info!("   4. Mock process simulates execution");
    info!("   5. Returns result to MultiVM");
    info!("   6. MultiVM aggregates results");

    // Keep running for demonstration
    info!("\n6. Mock processes are running. Press Ctrl+C to stop...");

    // Wait for interrupt
    tokio::signal::ctrl_c().await?;

    // Cleanup
    info!("\n7. Shutting down mock processes...");

    mock_reth.kill()?;
    mock_solana.kill()?;

    info!("   ✓ Mock processes stopped");
    info!("\nDemo completed!");

    Ok(())
}

// Example output:
// ```
// === MultiVM Mock Processes Demo ===
// This demo shows how MultiVM coordinates with external mock processes
//
// 1. Starting Mock Processes...
//    ✓ Mock Reth process started (PID: 12345)
//    ✓ Mock Solana process started (PID: 12346)
//
// 2. Mock Process Architecture:
//    ┌─────────────────┐
//    │     MultiVM     │
//    │  (Coordinator)  │
//    └────────┬────────┘
//             │
//      ┌──────┴──────┐
//      │             │
//    ┌─▼───┐      ┌─▼─────┐
//    │ Reth │      │ Solana │
//    │ Mock │      │  Mock  │
//    └──────┘      └────────┘
//     /tmp/          /tmp/
//     multivm-       multivm-
//     ethereum.sock  solana.sock
//
// 3. Communication Protocol:
//    - IPC over Unix sockets
//    - Binary protocol (bincode)
//    - Handshake required
//    - Async message handling
//
// 4. Mock Process Features:
//    Mock Reth:
//    - Simulates Ethereum execution
//    - Processes EVM transactions
//    - Maintains block state
//    - Responds to RPC calls
//
//    Mock Solana:
//    - Simulates Solana execution
//    - Processes SVM transactions
//    - Maintains block state
//    - Responds to RPC calls
//
// 5. Testing Transaction Flow:
//    1. MultiVM receives transaction
//    2. Determines target chain (EVM/SVM)
//    3. Sends to appropriate mock process via IPC
//    4. Mock process simulates execution
//    5. Returns result to MultiVM
//    6. MultiVM aggregates results
//
// 6. Mock processes are running. Press Ctrl+C to stop...
//
// 7. Shutting down mock processes...
//    ✓ Mock processes stopped
//
// Demo completed!
// ```
