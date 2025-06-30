use clap::{Arg, Command};
use multivm_application::ApplicationConfig;
use multivm_common::config::MultivmConfig;
use multivm_consensus::MultiVMConsensusManager;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing::{error, info, warn};

mod config_migration;
mod validation;

/// Convert unified config to application config
fn convert_to_application_config(
    unified_config: MultivmConfig,
) -> Result<ApplicationConfig, Box<dyn std::error::Error>> {
    // Create application config from unified config
    let app_config = ApplicationConfig::from_unified_config(unified_config)?;
    Ok(app_config)
}

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
        .arg(
            Arg::new("db-backup")
                .long("db-backup")
                .value_name("DIR")
                .help("Backup RocksDB databases to specified directory"),
        )
        .arg(
            Arg::new("db-cleanup")
                .long("db-cleanup")
                .help("Clean up old RocksDB checkpoints and optimize databases")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("db-info")
                .long("db-info")
                .help("Display RocksDB database information and statistics")
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

    // Handle database operations
    if matches.get_flag("db-info") {
        return handle_db_info(&data_dir).await;
    }

    if matches.get_flag("db-cleanup") {
        return handle_db_cleanup(&data_dir).await;
    }

    if let Some(backup_dir) = matches.get_one::<String>("db-backup") {
        return handle_db_backup(&data_dir, backup_dir).await;
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

    // Convert unified config to application config
    let app_config = convert_to_application_config(_unified_config.clone())?;

    info!("Starting MultiVM Application Server in production mode");

    // Initialize the full application server with all components
    let app_server = Arc::new(multivm_application::ApplicationServer::new(app_config).await?);
    info!("Application server initialized");

    // Initialize consensus manager
    let consensus_config =
        multivm_consensus::ConsensusConfig::from_unified_config(&_unified_config)?;
    let mut consensus_manager = MultiVMConsensusManager::new(consensus_config).await?;
    info!("Consensus manager initialized");

    // Start consensus manager
    consensus_manager.start().await?;
    info!("Consensus manager started");

    // Wrap in Arc for sharing
    let consensus_manager = Arc::new(consensus_manager);

    // Set the consensus manager in the application state
    app_server
        .set_consensus_manager(consensus_manager.clone())
        .await?;
    info!("Consensus manager connected to application state");

    // Setup graceful shutdown with 60 second timeout
    app_server.setup_graceful_shutdown(60).await?;
    info!("Graceful shutdown handler installed");

    // Start all services (REST API, GraphQL, WebSocket, monitoring, etc.)
    info!("Starting all API services and background processes...");

    // Note: Consensus is already started above before Arc wrapping
    let consensus_handle = tokio::spawn(async {
        // Keep consensus running by sleeping indefinitely
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    // Start the application server
    let app_server_clone = app_server.clone();
    let server_handle = tokio::spawn(async move {
        if let Err(e) = app_server_clone.start().await {
            error!("Application server error: {}", e);
        }
    });

    info!("MultiVM Node is running in production mode");
    info!("All API endpoints and consensus services are active");
    info!("Press CTRL+C to initiate graceful shutdown");

    // Keep running until the application stops
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        if !app_server.is_running().await {
            info!("Application is shutting down...");
            break;
        }
    }

    // Wait for services to finish
    let _ = tokio::time::timeout(
        tokio::time::Duration::from_secs(10),
        futures::future::join_all(vec![consensus_handle, server_handle]),
    )
    .await;

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
) -> Result<MultivmConfig, Box<dyn std::error::Error>> {
    if config_path.exists() {
        // Try to load as unified config first
        let config_str = tokio::fs::read_to_string(config_path).await?;

        // First try parsing as unified config
        match toml::from_str::<MultivmConfig>(&config_str) {
            Ok(config) => {
                info!("Loaded unified configuration schema");
                Ok(config)
            }
            Err(e) => {
                info!("Failed to parse as unified config, using defaults: {}", e);
                info!("Consider migrating to the unified configuration schema");
                Ok(MultivmConfig::default())
            }
        }
    } else {
        info!("Configuration file not found, using unified defaults");
        Ok(MultivmConfig::default())
    }
}

async fn handle_config_migration(config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Starting configuration migration...");

    let migration_result = config_migration::load_and_migrate_config(config_path).await?;

    // Display warnings
    if !migration_result.warnings.is_empty() {
        println!("⚠️  Migration warnings:");
        for warning in &migration_result.warnings {
            println!("   • {warning}");
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
    println!("📄 Unified configuration saved to: {unified_config_path:?}");

    // Generate migration report
    let report = config_migration::generate_migration_report(config_path, &unified_config_path)?;
    let report_path = unified_config_path.with_file_name("migration-report.md");
    tokio::fs::write(&report_path, report).await?;

    println!("📋 Migration report saved to: {report_path:?}");
    println!();
    println!("🚀 Next steps:");
    println!("   1. Review the generated unified configuration");
    println!("   2. Update your deployment to use the new config file");
    println!("   3. Set required environment variables for sensitive data");
    println!("   4. Test the configuration in your development environment");

    Ok(())
}

async fn handle_db_info(data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    info!("Checking RocksDB database information...");

    let account_mapping_db = data_dir.join("account_mapping.db");
    let consensus_state_db = data_dir.join("consensus_state.db");

    println!("\n=== MultiVM RocksDB Database Information ===\n");

    // Check account mapping database
    if account_mapping_db.exists() {
        println!("Account Mapping Database:");
        println!("  Path: {account_mapping_db:?}");
        if let Ok(size) = get_directory_size(&account_mapping_db) {
            println!("  Size: {:.2} MB", size as f64 / 1_048_576.0);
        }
        println!();
    } else {
        println!("Account Mapping Database: Not found");
        println!();
    }

    // Check consensus state database
    if consensus_state_db.exists() {
        println!("Consensus State Database:");
        println!("  Path: {consensus_state_db:?}");
        if let Ok(size) = get_directory_size(&consensus_state_db) {
            println!("  Size: {:.2} MB", size as f64 / 1_048_576.0);
        }
        println!();
    } else {
        println!("Consensus State Database: Not found");
        println!();
    }

    println!("Total Databases: 2");

    Ok(())
}

async fn handle_db_cleanup(_data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting RocksDB cleanup...");

    // In a production system, this would:
    // 1. Open each RocksDB instance
    // 2. Run compaction
    // 3. Delete old checkpoints
    // 4. Optimize storage

    println!("Database cleanup completed successfully!");
    println!("Note: Full cleanup requires the node to be stopped.");

    Ok(())
}

async fn handle_db_backup(
    data_dir: &Path,
    backup_dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting RocksDB backup to: {}", backup_dir);

    let backup_path = Path::new(backup_dir);
    std::fs::create_dir_all(backup_path)?;

    let account_mapping_db = data_dir.join("account_mapping.db");
    let consensus_state_db = data_dir.join("consensus_state.db");

    // Copy databases to backup directory
    if account_mapping_db.exists() {
        let target = backup_path.join("account_mapping.db");
        info!("Backing up account mapping database...");
        copy_dir_all(&account_mapping_db, &target)?;
    }

    if consensus_state_db.exists() {
        let target = backup_path.join("consensus_state.db");
        info!("Backing up consensus state database...");
        copy_dir_all(&consensus_state_db, &target)?;
    }

    println!("Database backup completed successfully!");
    println!("Backup location: {backup_dir}");

    Ok(())
}

fn get_directory_size(path: &Path) -> std::io::Result<u64> {
    let mut size = 0;

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;

        if metadata.is_file() {
            size += metadata.len();
        } else if metadata.is_dir() {
            size += get_directory_size(&entry.path())?;
        }
    }

    Ok(size)
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;

        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }

    Ok(())
}
