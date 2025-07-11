//! Example: Using ConsensusBlockGenerator with Reth Integration
//!
//! This example demonstrates how to use the ConsensusBlockGenerator with real Reth
//! block data instead of mock EVM transactions.

use multivm_consensus::MalachiteConfig;
use multivm_process_manager::consensus_block_generator::{
    ConsensusBlockGenerator, ConsensusBlockGeneratorConfig,
};
use multivm_process_manager::coordinator::{CoordinatorConfig, MultivmCoordinator};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Reth-integrated ConsensusBlockGenerator example");

    // Create coordinator configuration
    let coordinator_config = CoordinatorConfig {
        consensus: MalachiteConfig::default(),
        health_check_interval: Duration::from_secs(30),
        block_timeout: Duration::from_secs(60),
        max_concurrent_blocks: 10,
        enable_recovery: true,
        db_path: Some("/tmp/multivm_example_db".to_string()),
    };

    // Create coordinator
    let coordinator = Arc::new(RwLock::new(
        MultivmCoordinator::new(coordinator_config)
            .await
            .expect("Failed to create coordinator"),
    ));

    // Create block generator configuration with Reth settings
    let config = ConsensusBlockGeneratorConfig {
        block_interval_ms: 12000, // 12 seconds to match Ethereum mainnet
        svm_tx_per_block: 10,     // Reduced for demonstration
        evm_tx_per_block: 15,     // Will be sourced from Reth
        enabled: true,
        reth_rpc_url: "http://127.0.0.1:8545".to_string(),
        reth_rpc_timeout_ms: 30000,
        reth_max_retries: 3,
        reth_retry_delay_ms: 1000,
        enable_state_root_verification: true,
        block_sync_timeout_ms: 5000,
    };

    // Create block generator
    let mut block_generator = ConsensusBlockGenerator::new(config, coordinator.clone())
        .expect("Failed to create block generator");

    info!("Block generator created successfully");

    // Start the coordinator
    {
        let mut coordinator_guard = coordinator.write().await;
        coordinator_guard
            .start()
            .await
            .expect("Failed to start coordinator");
        info!("Coordinator started");
    }

    // Start block generation
    tokio::spawn(async move {
        if let Err(e) = block_generator.start().await {
            warn!("Block generator stopped with error: {}", e);
        }
    });

    // Wait for some blocks to be generated
    tokio::time::sleep(Duration::from_secs(60)).await;

    // Check system health
    {
        let coordinator_guard = coordinator.read().await;
        match coordinator_guard.get_health_status().await {
            Ok(health) => {
                info!("System health: {:?}", health);
            }
            Err(e) => {
                warn!("Failed to get system health: {}", e);
            }
        }
    }

    // Stop the coordinator
    {
        let mut coordinator_guard = coordinator.write().await;
        coordinator_guard
            .stop()
            .await
            .expect("Failed to stop coordinator");
        info!("Coordinator stopped");
    }

    info!("Example completed successfully");
    Ok(())
}
