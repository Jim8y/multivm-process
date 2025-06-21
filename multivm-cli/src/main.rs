use clap::{Arg, Command};
use multivm_common::config::MultivmConfig;
use multivm_consensus::MalachiteConfig;
use multivm_process_manager::{MultivmCoordinator, CoordinatorConfig};
use std::path::PathBuf;
use std::time::Duration;
use tokio;
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

    // Load configuration (for future use)
    let _config = load_config(&config_path).await?;
    info!("Configuration loaded successfully");

    // Create coordinator configuration with proper fields
    let coordinator_config = CoordinatorConfig {
        consensus: MalachiteConfig::default(),
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

    info!("MultiVM Node is running...");
    info!("Using data directory: {:?}", data_dir);

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Received shutdown signal");

    // Graceful shutdown
    info!("Shutting down MultiVM Node...");
    coordinator.stop().await?;
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