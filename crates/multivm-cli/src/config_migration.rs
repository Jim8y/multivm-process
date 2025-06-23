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

    // TODO: Add more field mappings as we identify the legacy structure
    // This is a placeholder for the migration logic

    warnings.push(
        "Migration from legacy config is incomplete. Please review the generated configuration."
            .to_string(),
    );
    warnings.push(
        "Consider manually updating to the unified schema for full feature support.".to_string(),
    );

    MigrationResult {
        config: unified_config,
        warnings,
    }
}

/// Load and auto-migrate configuration file
pub async fn load_and_migrate_config(
    config_path: &Path,
) -> Result<MigrationResult, Box<dyn std::error::Error>> {
    if !config_path.exists() {
        info!("No configuration file found, using unified defaults");
        return Ok(MigrationResult {
            config: MultivmUnifiedConfig::default(),
            warnings: vec![
                "Using default configuration. Consider creating a configuration file.".to_string(),
            ],
        });
    }

    let config_str = tokio::fs::read_to_string(config_path).await?;

    // Try parsing as unified config first
    match toml::from_str::<MultivmUnifiedConfig>(&config_str) {
        Ok(config) => {
            info!("Successfully loaded unified configuration");
            Ok(MigrationResult {
                config,
                warnings: Vec::new(),
            })
        }
        Err(_) => {
            warn!("Failed to parse as unified config, attempting legacy migration");

            // Try parsing as legacy config
            match toml::from_str::<MultivmConfig>(&config_str) {
                Ok(legacy_config) => {
                    let mut result = migrate_legacy_config(legacy_config);
                    result
                        .warnings
                        .insert(0, "Loaded legacy configuration format.".to_string());
                    Ok(result)
                }
                Err(e) => {
                    warn!("Failed to parse configuration file: {}", e);
                    warn!("Using default configuration");
                    Ok(MigrationResult {
                        config: MultivmUnifiedConfig::default(),
                        warnings: vec![
                            format!("Failed to parse configuration file: {}", e),
                            "Using default configuration. Please check your config file syntax."
                                .to_string(),
                        ],
                    })
                }
            }
        }
    }
}

/// Save unified configuration to file
pub async fn save_unified_config(
    config: &MultivmUnifiedConfig,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_str = toml::to_string_pretty(config)?;
    tokio::fs::write(path, config_str).await?;
    info!("Saved unified configuration to: {:?}", path);
    Ok(())
}

/// Generate migration report comparing old and new schemas
pub fn generate_migration_report(
    _legacy_path: &Path,
    _unified_path: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    // TODO: Implement detailed migration report
    let report = r#"
# Configuration Migration Report

## Summary
Your configuration has been migrated to the new unified schema.

## Key Changes
1. **Consistent Naming**: All field names now use snake_case
2. **Structured Sections**: Related settings are grouped logically
3. **Security Defaults**: Security features are enabled by default
4. **Environment Variables**: Sensitive values use environment variables

## Next Steps
1. Review the generated unified configuration
2. Update your deployment scripts to use the new format
3. Set required environment variables for sensitive data
4. Test the configuration in your development environment

## Documentation
For detailed information about the unified schema, see:
- config/unified-schema.toml (reference template)
- docs/configuration-guide.md (full documentation)
"#;

    Ok(report.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_migration_with_default_config() {
        let result = migrate_legacy_config(MultivmConfig::default());

        assert!(!result.warnings.is_empty());
        assert_eq!(
            result.config.system.data_dir,
            std::path::PathBuf::from("/opt/multivm/data")
        );
    }

    #[tokio::test]
    async fn test_load_nonexistent_config() {
        let temp_path = std::path::PathBuf::from("/tmp/nonexistent-config.toml");
        let result = load_and_migrate_config(&temp_path).await.unwrap();

        assert!(!result.warnings.is_empty());
        assert!(result.warnings[0].contains("default configuration"));
    }

    #[tokio::test]
    async fn test_save_and_load_unified_config() {
        let temp_file = NamedTempFile::new().unwrap();
        let temp_path = temp_file.path();

        let original_config = MultivmUnifiedConfig::default();
        save_unified_config(&original_config, temp_path)
            .await
            .unwrap();

        let result = load_and_migrate_config(temp_path).await.unwrap();
        assert!(result.warnings.is_empty());

        // Basic validation that round-trip works
        assert_eq!(
            result.config.system.data_dir,
            original_config.system.data_dir
        );
    }
}
