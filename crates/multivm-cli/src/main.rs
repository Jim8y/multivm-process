use clap::{Arg, Command};
use multivm_common::config::{MultivmConfig, MultivmUnifiedConfig};
use multivm_consensus::{MalachiteConfig, ValidatorInfo};
use multivm_process_manager::{
    ConsensusBlockGenerator, ConsensusBlockGeneratorConfig, CoordinatorConfig, MultivmCoordinator,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

mod config_migration;
mod validation;

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
        .arg(
            Arg::new("migrate-config")
                .long("migrate-config")
                .help("Migrate existing configuration to unified schema and exit")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    // Initialize logging
    let log_level = matches.get_one::<String>("log-level").unwrap();
    setup_logging(log_level)?;

    info!("Starting MultiVM Node...");

    // Parse and validate configuration paths
    let config_path_str = matches.get_one::<String>("config").unwrap();
    let data_dir_str = matches.get_one::<String>("data-dir").unwrap();

    let config_path = validation::validate_file_path(config_path_str, "configuration file")?;
    let data_dir = validation::validate_file_path(data_dir_str, "data directory")?;

    info!("Configuration file: {:?}", config_path);
    info!("Data directory: {:?}", data_dir);

    // Validate config file safety if it exists
    if config_path.exists() {
        validation::validate_config_file_safety(&config_path)?;
    }

    // Check if user wants to migrate configuration and exit
    if matches.get_flag("migrate-config") {
        return handle_config_migration(&config_path).await;
    }

    // Validate validator count for security
    validation::validate_validator_count(1)?; // Single node for now

    // Load and migrate configuration
    let migration_result = config_migration::load_and_migrate_config(&config_path).await?;
    let _unified_config = migration_result.config;

    // Display any migration warnings
    for warning in &migration_result.warnings {
        tracing::warn!("Config migration: {}", warning);
    }

    info!("Configuration loaded successfully");

    // Get and validate node configuration from environment
    let node_id_raw = std::env::var("NODE_ID").unwrap_or_else(|_| "single-node".to_string());
    let node_id = validation::validate_node_id(&node_id_raw)?;

    let validator_key_raw =
        std::env::var("VALIDATOR_KEY").unwrap_or_else(|_| "single-validator-key".to_string());
    let validator_key = validation::validate_env_var("VALIDATOR_KEY", &validator_key_raw)?;

    let node_type_raw = std::env::var("NODE_TYPE").unwrap_or_else(|_| "bootstrap".to_string());
    let node_type = validation::validate_env_var("NODE_TYPE", &node_type_raw)?;
    let is_bootstrap = node_type == "bootstrap";

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

    // Validate block interval for security
    let validated_interval =
        validation::validate_block_interval(block_gen_config.block_interval_ms)?;
    let block_interval = validated_interval;

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

// Legacy configuration loading functions (kept for reference)
#[allow(dead_code)]
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

#[allow(dead_code)]
async fn load_unified_config(
    config_path: &PathBuf,
) -> Result<MultivmUnifiedConfig, Box<dyn std::error::Error>> {
    if config_path.exists() {
        // Try to load as unified config first
        let config_str = tokio::fs::read_to_string(config_path).await?;

        // First try parsing as unified config
        match toml::from_str::<MultivmUnifiedConfig>(&config_str) {
            Ok(config) => {
                info!("Loaded unified configuration schema");
                Ok(config)
            }
            Err(e) => {
                info!("Failed to parse as unified config, using defaults: {}", e);
                info!("Consider migrating to the unified configuration schema");
                Ok(MultivmUnifiedConfig::default())
            }
        }
    } else {
        info!("Configuration file not found, using unified defaults");
        Ok(MultivmUnifiedConfig::default())
    }
}

async fn handle_config_migration(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Starting configuration migration...");

    let migration_result = config_migration::load_and_migrate_config(config_path).await?;

    // Display warnings
    if !migration_result.warnings.is_empty() {
        println!("⚠️  Migration warnings:");
        for warning in &migration_result.warnings {
            println!("   • {}", warning);
        }
        println!();
    }

    // Generate output filename
    let unified_config_path = if config_path.exists() {
        config_path.with_file_name("multivm-unified.toml")
    } else {
        PathBuf::from("multivm-unified.toml")
    };

    // Save migrated configuration
    config_migration::save_unified_config(&migration_result.config, &unified_config_path).await?;

    println!("✅ Configuration migration completed!");
    println!(
        "📄 Unified configuration saved to: {:?}",
        unified_config_path
    );

    // Generate migration report
    let report = config_migration::generate_migration_report(config_path, &unified_config_path)?;
    let report_path = unified_config_path.with_file_name("migration-report.md");
    tokio::fs::write(&report_path, report).await?;

    println!("📋 Migration report saved to: {:?}", report_path);
    println!();
    println!("🚀 Next steps:");
    println!("   1. Review the generated unified configuration");
    println!("   2. Update your deployment to use the new config file");
    println!("   3. Set required environment variables for sensitive data");
    println!("   4. Test the configuration in your development environment");

    Ok(())
}
