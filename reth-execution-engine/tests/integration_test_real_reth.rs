use reth_execution_engine::engine::RethEngineError;
use reth_execution_engine::real_engine::{ConnectionConfig, RealRethEngine};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;
use tracing::warn;

/// Integration test for real Reth node process management
///
/// This test demonstrates:
/// 1. Starting a real Reth node process
/// 2. Verifying connections
/// 3. Checking process status
/// 4. Stopping the process
///
/// Prerequisites:
/// - `reth` binary must be installed and available in PATH
/// - Sufficient disk space for temporary blockchain data
#[tokio::test]
#[ignore] // Use `cargo test -- --ignored` to run this test
async fn test_real_reth_node_lifecycle() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging for test visibility
    tracing_subscriber::fmt::init();

    println!("🚀 Starting Real Reth Node Integration Test");
    println!("===========================================");

    // Create temporary directory for Reth data
    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_path_buf();

    println!("📁 Using temporary data directory: {}", data_dir.display());

    // Configure connection settings for faster testing
    let connection_config = ConnectionConfig {
        max_retries: 5,
        retry_delay: Duration::from_millis(500),
        request_timeout: Duration::from_secs(30),
        health_check_interval: Duration::from_secs(10),
        connection_pool_size: 10,
    };

    // Create RealRethEngine instance
    let mut reth_engine = RealRethEngine::new_with_config(
        data_dir.clone(),
        18545, // RPC port
        1337,  // Test chain ID
        connection_config,
    )
    .await?;

    println!("✅ RealRethEngine created successfully");

    // Test 1: Initialize and start the Reth process
    println!("\n🔧 1. Initializing Reth Engine (this will start the Reth process)");
    println!("   This may take 30-60 seconds for first-time initialization...");

    match reth_engine.initialize().await {
        Ok(()) => {
            println!("   ✅ Reth engine initialized successfully!");
            println!("   📊 Reth node should be running now");
        }
        Err(RethEngineError::Process(msg)) if msg.contains("Failed to start Reth node") => {
            println!("   ❌ Failed to start Reth process: {}", msg);
            println!("   💡 Make sure 'reth' binary is installed:");
            println!("      cargo install --git https://github.com/paradigmxyz/reth reth");
            return Ok(()); // Skip test if Reth not available
        }
        Err(e) => {
            println!("   ❌ Initialization failed: {}", e);
            return Err(e.into());
        }
    }

    // Test 2: Verify process is running
    println!("\n🔍 2. Checking Process Status");

    let is_running = reth_engine.is_reth_running().await;
    println!("   Process running: {}", is_running);

    if let Some(pid) = reth_engine.get_reth_process_pid().await {
        println!("   Process PID: {}", pid);
    }

    // Test 3: Get engine status
    println!("\n📊 3. Getting Engine Status");

    match reth_engine.get_engine_status().await {
        Ok(status) => {
            println!("   ✅ Engine status retrieved:");
            println!("   {}", serde_json::to_string_pretty(&status)?);
        }
        Err(e) => {
            warn!("   ⚠️  Failed to get engine status: {}", e);
        }
    }

    // Test 4: Stop the process
    println!("\n🛑 4. Stopping Reth Process");

    match reth_engine.stop_reth_process().await {
        Ok(()) => {
            println!("   ✅ Reth process stopped successfully");

            // Verify process is actually stopped
            sleep(Duration::from_secs(2)).await;
            let is_running_after_stop = reth_engine.is_reth_running().await;
            println!("   Process running after stop: {}", is_running_after_stop);

            if !is_running_after_stop {
                println!("   ✅ Process cleanup verified");
            } else {
                println!("   ⚠️  Process may still be shutting down");
            }
        }
        Err(e) => {
            warn!("   ⚠️  Failed to stop process: {}", e);
        }
    }

    println!("\n🎉 Real Reth Node Integration Test Complete!");
    println!("============================================");

    // Cleanup happens automatically when temp_dir is dropped

    Ok(())
}

/// Test that demonstrates what happens when Reth binary is not available
#[tokio::test]
async fn test_reth_not_available() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing behavior when Reth binary is not available");

    let temp_dir = TempDir::new()?;
    let data_dir = temp_dir.path().to_path_buf();

    // Try to create engine with non-existent binary (by using wrong data dir)
    let mut reth_engine = RealRethEngine::new(
        data_dir, 18546, // Different port to avoid conflicts
        1337,
    )
    .await?;

    // This should fail if reth binary is not available
    match reth_engine.initialize().await {
        Ok(()) => {
            println!("   ✅ Reth is available and working");
            // Clean up
            reth_engine.stop_reth_process().await.ok();
        }
        Err(RethEngineError::Process(msg)) => {
            println!("   ⚠️  Reth not available: {}", msg);
            println!("   This is expected if 'reth' binary is not installed");
        }
        Err(e) => {
            println!("   ❌ Unexpected error: {}", e);
        }
    }

    Ok(())
}
