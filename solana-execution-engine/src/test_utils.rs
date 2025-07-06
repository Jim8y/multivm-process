//! Test utilities for Solana execution engine tests
//!
//! This module contains common test utilities and helper functions
//! that are shared across different test modules.

use crate::engine::{SolanaEngine, SolanaEngineError};
use tracing::{error, info};

/// Initializes logging for tests.
pub fn setup_logging() {
    // Initialize logging with info level, no timestamp
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(false)
        .with_line_number(false)
        .without_time()
        .try_init()
        .ok(); // Ignore error if already initialized
}

/// Creates and initializes a new SolanaEngine for testing.
pub async fn create_and_initialize_engine() -> Result<SolanaEngine, SolanaEngineError> {
    info!("Creating and initializing SolanaEngine for test...");
    let mut engine = SolanaEngine::new_default().await?;

    // Try to initialize, but cleanup on failure
    if let Err(e) = engine.initialize().await {
        error!("✗ Failed to initialize SolanaEngine: {}", e);
        // Attempt a cleanup shutdown on initialization failure
        let _ = engine
            .shutdown(Some(tokio::time::Duration::from_secs(5)))
            .await;
        return Err(e);
    }

    info!("✓ SolanaEngine initialized successfully");
    Ok(engine)
}

/// Safely shutdown an engine with error handling
pub async fn shutdown_engine(mut engine: SolanaEngine) -> Result<(), SolanaEngineError> {
    info!("Shutting down Solana engine...");
    engine
        .shutdown(Some(tokio::time::Duration::from_secs(10)))
        .await?;
    info!("✓ Engine shutdown successfully");
    Ok(())
}
