//! Configuration migration utilities
//!
//! This module provides utilities to migrate existing configuration files
//! to the new unified configuration schema.

use multivm_common::config::{MultivmConfig, MultivmUnifiedConfig};
use std::path::Path;
use tracing::{info, warn};

/// Migration result containing the converted config and any warnings
pub struct MigrationResult {
    pub config: MultivmUnifiedConfig,
    pub warnings: Vec<String>,
}

/// Migrate legacy configuration to unified schema
pub fn migrate_legacy_config(legacy_config: MultivmConfig) -> MigrationResult {
    let mut warnings = Vec::new();
    let mut unified_config = MultivmUnifiedConfig::default();

    info!("Migrating legacy configuration to unified schema...");

    // Migrate system settings from legacy config structure
    // Convert relative paths to absolute paths for production deployment
    unified_config.system.data_dir = if legacy_config.system.data_dir.is_relative() {
        std::path::PathBuf::from("/opt/multivm/data")
    } else {
        legacy_config.system.data_dir
    };
    unified_config.system.max_processes = legacy_config.system.max_processes;
    unified_config.system.process_restart_delay_seconds =
        legacy_config.system.process_restart_delay.as_secs();
    unified_config.system.health_check_interval_seconds =
        legacy_config.system.health_check_interval.as_secs();
    unified_config.system.shutdown_timeout_seconds =
        legacy_config.system.shutdown_timeout.as_secs();
    unified_config.system.enable_metrics = legacy_config.system.enable_metrics;

    if let Some(port) = legacy_config.system.metrics_port {
        unified_config.monitoring.prometheus_port = port;
    }

    // Network configuration - use defaults since current config doesn't have network field
    unified_config.network.enable_p2p = true;
    unified_config.network.listen_port = 26656;
    warnings.push("Network configuration not found in legacy config, using defaults".to_string());

    // Server configuration - use defaults since current config doesn't have rpc field
    unified_config.server.rest.host = "127.0.0.1".to_string();
    unified_config.server.rest.port = 8080;
    warnings.push("RPC configuration not found in legacy config, using defaults".to_string());

    // Consensus configuration - use defaults since current config doesn't have consensus field
    unified_config.consensus.algorithm = "malachite".to_string();
    unified_config.consensus.block_time_milliseconds = 2000;
    unified_config.consensus.validator_count = 1;
    unified_config.consensus.enable_single_node = true;
    warnings.push("Consensus configuration not found in legacy config, using defaults".to_string());

    // Ethereum configuration
    let eth_config = &legacy_config.ethereum;
    if let Some(rpc_config) = &eth_config.rpc_config {
        unified_config.blockchain_clients.ethereum.rpc_url =
            format!("http://{}:{}", rpc_config.host, rpc_config.port);
        unified_config.blockchain_clients.ethereum.timeout_seconds =
            rpc_config.request_timeout.as_secs();
        unified_config.blockchain_clients.ethereum.max_retries = 3; // Default value
    } else {
        unified_config.blockchain_clients.ethereum.rpc_url = "http://localhost:8545".to_string();
        warnings
            .push("Ethereum RPC configuration not found, using default localhost:8545".to_string());
    }

    // Solana configuration
    let sol_config = &legacy_config.solana;
    if let Some(rpc_config) = &sol_config.rpc_config {
        unified_config.blockchain_clients.solana.rpc_url =
            format!("http://{}:{}", rpc_config.host, rpc_config.port);
        unified_config.blockchain_clients.solana.timeout_seconds =
            rpc_config.request_timeout.as_secs();
        unified_config.blockchain_clients.solana.max_retries = 3; // Default value
    } else {
        unified_config.blockchain_clients.solana.rpc_url = "http://localhost:8899".to_string();
        warnings
            .push("Solana RPC configuration not found, using default localhost:8899".to_string());
    }

    // Execution engines - use defaults since current config doesn't have this field
    unified_config.execution_engines.ethereum.binary_path = std::path::PathBuf::from("reth");
    unified_config.execution_engines.ethereum.data_dir =
        std::path::PathBuf::from("/opt/multivm/data/reth");
    unified_config.execution_engines.solana.binary_path =
        std::path::PathBuf::from("solana-validator");
    unified_config.execution_engines.solana.data_dir =
        std::path::PathBuf::from("/opt/multivm/data/solana");
    warnings.push(
        "Execution engines configuration not found in legacy config, using defaults".to_string(),
    );

    // IPC configuration from legacy
    unified_config.ipc.socket_path = std::path::PathBuf::from("/tmp/multivm.sock");
    unified_config.ipc.tcp_host = "127.0.0.1".to_string();
    unified_config.ipc.tcp_port = 9999;
    unified_config.ipc.max_message_size_bytes = legacy_config.ipc.max_message_size as u64;
    unified_config.ipc.timeout_milliseconds = legacy_config.ipc.message_timeout.as_millis() as u64;
    unified_config.ipc.enable_encryption = false; // Default to false

    // Logging configuration - use simple defaults since the complex types don't exist
    warnings.push(
        "Logging configuration simplified - complex logging options not supported".to_string(),
    );

    // Monitoring - enable by default
    unified_config.monitoring.enable_prometheus = true;
    unified_config.monitoring.enable_jaeger = false;

    // Security - enable basic security by default
    unified_config.security.enable_authentication = true;
    unified_config.security.enable_encryption = false; // Can be enabled for production
    unified_config.security.enable_rate_limiting = true;
    warnings
        .push("Security settings using defaults - update JWT secret for production!".to_string());

    info!(
        "Configuration migration completed with {} warnings",
        warnings.len()
    );

    MigrationResult {
        config: unified_config,
        warnings,
    }
}

/// Check if a configuration file needs migration
pub fn needs_migration(config_path: &Path) -> Result<bool, std::io::Error> {
    let content = std::fs::read_to_string(config_path)?;

    // Check for new unified config structure markers
    let has_unified_fields = content.contains("[system]")
        && content.contains("[consensus]")
        && content.contains("[network]")
        && content.contains("[blockchain_clients]");

    // If it doesn't have all unified fields, it needs migration
    Ok(!has_unified_fields)
}

/// Save unified config to file
pub async fn save_unified_config(
    config: &MultivmUnifiedConfig,
    config_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let content = toml::to_string_pretty(config)?;
    tokio::fs::write(config_path, content).await?;
    Ok(())
}

/// Generate migration report
pub fn generate_migration_report(
    original_path: &Path,
    migrated_path: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let report = format!(
        "# Configuration Migration Report\n\n\
        **Original Configuration**: {:?}\n\
        **Migrated Configuration**: {:?}\n\
        **Migration Date**: {}\n\n\
        ## Summary\n\n\
        The legacy configuration has been migrated to the new unified schema.\n\
        Please review the migrated configuration and update any default values as needed.\n\n\
        ## Important Notes\n\n\
        - Network configuration was set to defaults\n\
        - RPC configuration was set to defaults\n\
        - Consensus configuration was set to defaults\n\
        - Security settings use default values - update JWT secret for production!\n\n\
        ## Next Steps\n\n\
        1. Review the migrated configuration file\n\
        2. Update any placeholder values\n\
        3. Test the configuration with your environment\n\
        4. Update your deployment scripts to use the new configuration\n",
        original_path,
        migrated_path,
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    );
    Ok(report)
}

/// Load and migrate configuration file if needed
pub async fn load_and_migrate_config(
    config_path: &Path,
) -> Result<MigrationResult, Box<dyn std::error::Error>> {
    if needs_migration(config_path)? {
        info!("Legacy configuration detected, performing migration...");

        // Load legacy config
        let legacy_config = MultivmConfig::from_file(config_path.to_str().unwrap())?;

        // Migrate to unified schema
        let migration_result = migrate_legacy_config(legacy_config);

        // Log warnings
        for warning in &migration_result.warnings {
            warn!("{}", warning);
        }

        // Save migrated config to new file
        let backup_path = config_path.with_extension("toml.backup");
        tokio::fs::copy(config_path, &backup_path).await?;
        info!("Original config backed up to: {:?}", backup_path);

        let migrated_path = config_path.with_extension("migrated.toml");
        save_unified_config(&migration_result.config, &migrated_path).await?;
        info!("Migrated configuration saved to: {:?}", migrated_path);

        Ok(migration_result)
    } else {
        // Already a unified config, load directly
        let config = MultivmUnifiedConfig::from_file(config_path)?;
        Ok(MigrationResult {
            config,
            warnings: vec!["Configuration already in unified format".to_string()],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_legacy_config_migration() {
        let mut legacy = MultivmConfig::default();
        legacy.system.max_processes = 10;
        legacy.system.process_restart_delay = Duration::from_secs(5);
        legacy.ethereum.rpc_config = Some(multivm_common::config::RpcConfig {
            host: "localhost".to_string(),
            port: 8545,
            max_connections: 100,
            request_timeout: Duration::from_secs(30),
            cors_origins: vec!["*".to_string()],
        });
        legacy.solana.rpc_config = Some(multivm_common::config::RpcConfig {
            host: "localhost".to_string(),
            port: 8899,
            max_connections: 100,
            request_timeout: Duration::from_secs(30),
            cors_origins: vec!["*".to_string()],
        });

        let result = migrate_legacy_config(legacy);

        assert_eq!(result.config.system.max_processes, 10);
        assert_eq!(result.config.system.process_restart_delay_seconds, 5);
        assert_eq!(
            result.config.blockchain_clients.ethereum.rpc_url,
            "http://localhost:8545"
        );
        assert_eq!(
            result.config.blockchain_clients.solana.rpc_url,
            "http://localhost:8899"
        );
        assert!(!result.warnings.is_empty());
    }
}
