use solana_execution_engine::engine::SolanaEngine;
use solana_execution_engine::{SolanaEngineError, config::SolanaConfigBuilder};
use solana_sdk::signature::Keypair;
use std::sync::atomic::{AtomicU16, Ordering};
use tracing::info;

/// Create a Solana keypair for testing
pub fn create_test_keypair() -> Keypair {
    Keypair::new()
}

// Atomic counter for generating unique ports
static PORT_COUNTER: AtomicU16 = AtomicU16::new(0);

/// Get unique ports for testing
pub fn get_unique_ports() -> (u16, u16, u16) {
    // Use process ID and time to ensure uniqueness across test runs
    let pid = std::process::id() as u16;
    let time_component = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() % 1000) as u16;
    
    // Use a larger increment to avoid collisions
    let increment = PORT_COUNTER.fetch_add(1, Ordering::SeqCst);
    let base = 30000 + (pid % 1000) + time_component + (increment * 10);
    let gossip_port = base;
    let rpc_port = base + 1;
    let rpc_server_port = base + 2;
    
    // Ensure ports are in valid range (1024-65535)
    let gossip_port = gossip_port.min(65530);
    let rpc_port = rpc_port.min(65531);
    let rpc_server_port = rpc_server_port.min(65532);
    
    info!("Allocated ports - gossip: {}, rpc: {}, rpc_server: {}", gossip_port, rpc_port, rpc_server_port);
    
    (gossip_port, rpc_port, rpc_server_port)
}

pub fn setup_logging() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .without_time()
        .try_init()
        .ok();
}

/// Create a new SolanaEngine with unique ports for testing
pub async fn create_engine() -> Result<SolanaEngine, SolanaEngineError> {
    info!("Creating SolanaEngine for test...");
    let (gossip_port, rpc_port, rpc_server_port) = get_unique_ports();
    
    let config = SolanaConfigBuilder::new()
        .gossip_port(gossip_port)
        .rpc_port(rpc_port)
        .build();
    
    let engine = SolanaEngine::new(config, rpc_server_port).await?;
    Ok(engine)
}

/// Initialize a SolanaEngine with proper error handling and cleanup
pub async fn initialize_engine(
    mut engine: SolanaEngine,
) -> Result<SolanaEngine, SolanaEngineError> {
    engine.initialize().await?;
    Ok(engine)
}

/// Create and initialize a SolanaEngine (convenience function)
pub async fn create_and_initialize_engine() -> Result<SolanaEngine, SolanaEngineError> {
    let engine = create_engine().await?;
    initialize_engine(engine).await
}

/// Safely shutdown an engine with error handling
pub async fn shutdown_engine(mut engine: SolanaEngine) -> Result<(), SolanaEngineError> {
    engine
        .shutdown(Some(tokio::time::Duration::from_secs(10)))
        .await?;
    Ok(())
}
