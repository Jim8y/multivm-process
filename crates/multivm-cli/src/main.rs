use clap::{Arg, Command};
use multivm_common::config::MultivmConfig;
use multivm_consensus::{MalachiteConfig, ValidatorInfo};
use multivm_process_manager::{
    ConsensusBlockGenerator, ConsensusBlockGeneratorConfig, CoordinatorConfig, MultivmCoordinator,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("multivm-node")
        .version("0.1.0")
        .about("MultiVM Process Node - Unified SVM+EVM Blockchain Execution")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path")
                .default_value("/opt/multivm/config/multivm.toml"),
        )
        .arg(
            Arg::new("data-dir")
                .short('d')
                .long("data-dir")
                .value_name("DIR")
                .help("Data directory path")
                .default_value("/opt/multivm/data"),
        )
        .arg(
            Arg::new("log-level")
                .short('l')
                .long("log-level")
                .value_name("LEVEL")
                .help("Log level (trace, debug, info, warn, error)")
                .default_value("info"),
        )
        .get_matches();

    // Initialize logging
    let log_level = matches.get_one::<String>("log-level").unwrap();
    setup_logging(log_level)?;

    info!("Starting MultiVM Node...");

    // Parse configuration
    let config_path = PathBuf::from(matches.get_one::<String>("config").unwrap());
    let data_dir = PathBuf::from(matches.get_one::<String>("data-dir").unwrap());

    info!("Configuration file: {:?}", config_path);
    info!("Data directory: {:?}", data_dir);

    // Load configuration from file
    let _config = load_config(&config_path).await?;
    info!("Configuration loaded successfully");

    // Get node configuration from environment
    let node_id = std::env::var("NODE_ID").unwrap_or_else(|_| "single-node".to_string());
    let validator_key =
        std::env::var("VALIDATOR_KEY").unwrap_or_else(|_| "single-validator-key".to_string());
    let is_bootstrap =
        std::env::var("NODE_TYPE").unwrap_or_else(|_| "bootstrap".to_string()) == "bootstrap";

    // Configure validators for single node consensus
    let validators = if is_bootstrap {
        // For single node, we need at least one validator
        vec![ValidatorInfo {
            public_key: validator_key.clone(),
            voting_power: 1000, // Single node has all voting power
        }]
    } else {
        vec![]
    };

    let validator_count = validators.len();

    // Create coordinator configuration with proper consensus setup
    let coordinator_config = CoordinatorConfig {
        consensus: MalachiteConfig {
            node_id: node_id.clone(),
            validators,
            ..MalachiteConfig::default()
        },
        health_check_interval: Duration::from_secs(30),
        block_timeout: Duration::from_secs(60),
        max_concurrent_blocks: 10,
        enable_recovery: true,
    };

    // Initialize coordinator
    let mut coordinator = MultivmCoordinator::new(coordinator_config).await?;
    info!("MultiVM Coordinator initialized");

    // Start the coordinator
    coordinator.start().await?;
    info!("MultiVM Coordinator started");

    // Create consensus-aware block generator for continuous block production
    let coordinator_arc = Arc::new(RwLock::new(coordinator));
    let block_gen_config = ConsensusBlockGeneratorConfig {
        block_interval_ms: std::env::var("BLOCK_INTERVAL_MS")
            .unwrap_or_else(|_| "2000".to_string())
            .parse()
            .unwrap_or(2000),
        svm_tx_per_block: std::env::var("SVM_TX_PER_BLOCK")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3),
        evm_tx_per_block: std::env::var("EVM_TX_PER_BLOCK")
            .unwrap_or_else(|_| "3".to_string())
            .parse()
            .unwrap_or(3),
        enabled: std::env::var("BLOCK_GENERATION_ENABLED")
            .unwrap_or_else(|_| "true".to_string())
            .parse()
            .unwrap_or(true),
    };

    let block_interval = block_gen_config.block_interval_ms;

    let block_generator =
        ConsensusBlockGenerator::new(block_gen_config, Arc::clone(&coordinator_arc));

    // Start block generator in background
    let generator_handle = {
        let mut gen = block_generator;
        tokio::spawn(async move {
            if let Err(e) = gen.start().await {
                tracing::error!("Block generator failed: {}", e);
            }
        })
    };

    info!("MultiVM Node is running with consensus block generation...");
    info!("Node ID: {}", node_id);
    info!("Validator count: {}", validator_count);
    info!("Using data directory: {:?}", data_dir);
    info!("Generating signed blocks every {} ms", block_interval);

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Received shutdown signal");

    // Graceful shutdown
    info!("Shutting down MultiVM Node...");

    // Stop block generator
    generator_handle.abort();

    // Stop coordinator
    {
        let mut coordinator = coordinator_arc.write().await;
        coordinator.stop().await?;
    }
    info!("MultiVM Node stopped successfully");

    Ok(())
}

fn setup_logging(level: &str) -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new(level))
        .unwrap();

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();

    Ok(())
}

async fn load_config(config_path: &PathBuf) -> Result<MultivmConfig, Box<dyn std::error::Error>> {
    if config_path.exists() {
        let config_str = tokio::fs::read_to_string(config_path).await?;
        let config: MultivmConfig = toml::from_str(&config_str)?;
        Ok(config)
    } else {
        info!("Configuration file not found, using defaults");
        Ok(MultivmConfig::default())
    }
}
