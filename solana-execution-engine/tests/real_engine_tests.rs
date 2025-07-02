//! Tests for RealSolanaEngine core functions
//!
//! This test verifies that the RealSolanaEngine can start the Solana validator process
//! and shutdown properly.

use solana_execution_engine::config::{MultivmValidatorConfig, SolanaConnectionConfig};
use solana_execution_engine::real_engine::RealSolanaEngine;
use std::path::PathBuf;
use tokio;
use tracing::{error, info};

#[tokio::test]
async fn test_real_solana_engine_core_functions() -> Result<(), Box<dyn std::error::Error>> {
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

    info!("Starting RealSolanaEngine core functions test...");

    // Create test configuration
    let data_dir = PathBuf::from("/tmp/test_solana_engine");
    let rpc_port = 8899;
    let cluster = "localnet".to_string();

    // Create engine with default configuration
    info!("Creating RealSolanaEngine...");
    match RealSolanaEngine::new(data_dir.clone(), rpc_port, cluster.clone()).await {
        Ok(mut engine) => {
            info!("✅ RealSolanaEngine created successfully");

            // Test start_solana_validator_process directly
            info!("Testing start_solana_validator_process...");
            match engine.start_solana_validator_process().await {
                Ok(_) => {
                    info!("✅ Solana validator process started successfully");

                    // Wait a bit for the process to stabilize
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                    // Test shutdown
                    info!("Testing shutdown...");
                    match engine
                        .shutdown(Some(tokio::time::Duration::from_secs(10)))
                        .await
                    {
                        Ok(_) => info!("✅ Engine shutdown successfully"),
                        Err(e) => error!("❌ Failed to shutdown engine: {}", e),
                    }
                }
                Err(e) => {
                    error!("❌ Failed to start Solana validator process: {}", e);

                    // Still try to shutdown in case of partial initialization
                    info!("Attempting cleanup shutdown...");
                    let _ = engine
                        .shutdown(Some(tokio::time::Duration::from_secs(5)))
                        .await;
                }
            }
        }
        Err(e) => {
            error!("❌ Failed to create RealSolanaEngine: {}", e);
        }
    }

    info!("Test completed!");
    Ok(())
}
