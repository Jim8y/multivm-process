use solana_execution_engine::engine::SolanaEngine;
use solana_execution_engine::SolanaEngineError;
use solana_sdk::signature::Keypair;
use tracing::info;

/// Create a Solana keypair for testing
pub fn create_test_keypair() -> Keypair {
    Keypair::new()
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

/// Create a new SolanaEngine with default configuration
pub async fn create_engine() -> Result<SolanaEngine, SolanaEngineError> {
    info!("Creating SolanaEngine for test...");
    let engine = SolanaEngine::new_default().await?;
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
