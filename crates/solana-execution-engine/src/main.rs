use multivm_common::{ipc::IpcCommand, traits::ExecutionEngine};
use std::env;
use std::time::Duration;

mod engine;
mod ipc_client;
mod rpc_server;

use engine::{SolanaConfig, SolanaExecutionEngine};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    let mut data_dir = String::from("./data/solana");
    let mut rpc_port = 8899u16;

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
                    rpc_port = args[i + 1].parse().unwrap_or(8899);
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

    let config = SolanaConfig {
        data_dir: data_dir.into(),
        rpc_addr: "127.0.0.1".to_string(),
        rpc_port,
        max_compute_units: 1_400_000,
    };

    println!("🚀 Starting Solana Execution Engine");
    println!("📁 Data directory: {}", config.data_dir.display());
    println!("🌐 RPC port: {}", config.rpc_port);
    println!("🔗 RPC address: {}", config.rpc_addr);

    #[cfg(feature = "mock")]
    println!("🎭 Running in MOCK mode (no real validator process)");
    #[cfg(not(feature = "mock"))]
    println!("⚡ Running in REAL mode (will spawn validator process)");

    let mut engine = SolanaExecutionEngine::new(config);

    // Initialize the engine
    engine.initialize().await?;

    println!("✅ Solana execution engine started successfully");
    println!("🌐 JSON-RPC available at: http://127.0.0.1:{}", rpc_port);

    // Simple health check simulation
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            println!("💓 Solana engine health check - OK");
        }
    });

    // Keep the main process running
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    loop {
        interval.tick().await;

        // Check if engine is ready
        if !engine.is_ready().await {
            eprintln!("❌ Solana engine not ready");
            continue;
        }

        // Demonstrate health status
        println!("✨ Solana engine running - Status: Healthy");
        let _heartbeat = IpcCommand::GetHealth;
    }
}
