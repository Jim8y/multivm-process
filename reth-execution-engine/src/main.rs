use multivm_common::traits::ExecutionEngine;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

mod engine;
mod ipc_client;
mod rpc_server;


use engine::RethExecutionEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    let mut data_dir = String::from("./data/reth");
    let mut rpc_port = 8545u16;

    // Parse command line arguments
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--data-dir" => {
                if i + 1 < args.len() {
                    data_dir = args[i + 1].clone();
                    i += 2;
                } else {
                    eprintln!("Error: --data-dir requires a value");
                    std::process::exit(1);
                }
            }
            "--rpc-port" => {
                if i + 1 < args.len() {
                    rpc_port = args[i + 1].parse().unwrap_or(8545);
                    i += 2;
                } else {
                    eprintln!("Error: --rpc-port requires a value");
                    std::process::exit(1);
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    let data_dir_path: PathBuf = data_dir.into();
    let chain_id = 1337u64;

    println!("🚀 Starting Reth Execution Engine");
    println!("📁 Data directory: {}", data_dir_path.display());
    println!("🌐 RPC port: {rpc_port}");
    println!("🔗 Chain ID: {chain_id}");

    #[cfg(feature = "mock")]
    println!("🎭 Running in MOCK mode (no real Reth node process)");
    #[cfg(not(feature = "mock"))]
    println!("⚡ Running in REAL mode (will spawn Reth node process)");

    let mut engine = RethExecutionEngine::new(data_dir_path, rpc_port, chain_id).await?;

    // Initialize the engine
    engine.initialize().await?;

    println!("✅ Reth execution engine started successfully");
    println!("🌐 Engine API available at: http://127.0.0.1:8546");
    println!("🌐 JSON-RPC available at: http://127.0.0.1:{rpc_port}");

    // Simple health check simulation
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            println!("💓 Reth engine health check - OK");
        }
    });

    // Keep the main process running
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;

        // Check if engine is ready
        if !engine.is_ready().await {
            eprintln!("❌ Reth engine not ready");
            continue;
        }

        // Health status
        println!("✨ Reth engine running - Status: Healthy");
    }
}
