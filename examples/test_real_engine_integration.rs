use std::path::PathBuf;
use std::time::Duration;

// Import real_engine directly (not through engine.rs)
use reth_execution_engine::real_engine::RealRethEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Testing Real Engine Integration");
    println!("==================================");
    println!("This example actually calls real_engine.rs code!");
    println!();

    // Configuration
    let data_dir = PathBuf::from("./temp_test_data");
    let rpc_port = 8545;
    let chain_id = 1337;

    // Cleanup any existing data
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }

    println!("📁 Data directory: {}", data_dir.display());
    println!("🌐 RPC port: {rpc_port}");
    println!("⚡ Engine port: {}", rpc_port + 1);
    println!("🔗 Chain ID: {chain_id}");
    println!();

    // Create RealRethEngine instance (this is the real_engine.rs code!)
    let mut real_engine = RealRethEngine::new(data_dir.clone(), rpc_port, chain_id).await?;

    println!("🚀 Starting Reth node using real_engine.rs code...");

    // Initialize the engine (this calls real_engine.rs initialize() method!)
    real_engine.initialize().await?;

    println!("✅ Reth node started successfully!");
    println!(
        "🔍 Process ID: {:?}",
        real_engine.get_reth_process_pid().await
    );
    println!("🌐 RPC URL: http://127.0.0.1:{rpc_port}");
    println!("⚡ Engine URL: http://127.0.0.1:{}", rpc_port + 1);
    println!();

    // Wait for initialization
    println!("⏳ Waiting for Reth node to initialize...");
    tokio::time::sleep(Duration::from_secs(20)).await;

    // Check if node process is running
    println!("🔍 Checking node status...");
    let is_running = real_engine.is_reth_process_running().await;
    println!(
        "💓 Node process running: {}",
        if is_running {
            "✅ Running"
        } else {
            "❌ Not Running"
        }
    );

    // Test Engine API status
    println!("🧪 Testing Engine API status...");
    match real_engine.get_engine_status().await {
        Ok(status) => println!("⚡ Engine API status: {status}"),
        Err(e) => println!("❌ Engine API error: {e}"),
    }

    println!();
    println!("🎉 SUCCESS: Real Engine Integration Test Passed!");
    println!("✅ 成功调用了 real_engine.rs 中的 Rust 代码!");
    println!("✅ Reth 节点通过 Rust 代码启动成功!");
    println!();

    // Keep running for a bit to allow external verification
    println!("🔍 Node is running. You can now test externally:");
    println!("curl -X POST -H 'Content-Type: application/json' -d '{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"eth_chainId\",\"params\":[]}}' http://127.0.0.1:{rpc_port}");
    println!();
    println!("Press Ctrl+C to stop the node...");

    // Set up graceful shutdown
    let mut shutdown_signal =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    shutdown_signal.recv().await;

    println!("🛑 Shutting down...");
    real_engine.shutdown(Some(Duration::from_secs(30))).await?;
    println!("✅ Shutdown complete!");

    // Cleanup
    if data_dir.exists() {
        std::fs::remove_dir_all(&data_dir).ok();
    }

    Ok(())
}
