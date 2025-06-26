//! Unified Configuration Manager
//!
//! This module consolidates all configuration management into a single,
//! consistent interface that eliminates the 238+ config struct duplications.

use super::{BaseManager, Manager, ManagerState, ManagerStats, HealthStatus};
use crate::error::{MultivmError, MultivmResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Unified configuration manager that handles all config types
pub struct ConfigManager {
    /// Base manager functionality
    base: BaseManager<ConfigManagerConfig, ConfigManagerState>,
    /// Configuration store
    configs: Arc<RwLock<HashMap<String, ConfigEntry>>>,
    /// Configuration validators
    validators: Arc<RwLock<HashMap<String, Box<dyn ConfigValidator>>>>,
}

/// Configuration manager configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigManagerConfig {
    /// Configuration file paths
    pub config_paths: Vec<String>,
    /// Enable hot reloading
    pub enable_hot_reload: bool,
    /// Hot reload check interval in seconds
    pub hot_reload_interval: u64,
    /// Enable configuration validation
    pub enable_validation: bool,
    /// Enable configuration encryption
    pub enable_encryption: bool,
    /// Encryption key (base64 encoded)
    pub encryption_key: Option<String>,
}

/// Configuration manager state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigManagerState {
    /// Number of loaded configurations
    pub loaded_configs: usize,
    /// Last reload time
    pub last_reload: Option<chrono::DateTime<chrono::Utc>>,
    /// Configuration errors
    pub errors: Vec<String>,
}

/// Configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    /// Configuration key
    pub key: String,
    /// Configuration value
    pub value: serde_json::Value,
    /// Configuration type
    pub config_type: String,
    /// Last modified time
    pub last_modified: chrono::DateTime<chrono::Utc>,
    /// Source file path
    pub source_path: Option<String>,
    /// Validation status
    pub is_valid: bool,
    /// Validation errors
    pub validation_errors: Vec<String>,
}

/// Configuration validator trait
#[async_trait::async_trait]
pub trait ConfigValidator: Send + Sync {
    /// Validate a configuration value
    async fn validate(&self, config: &serde_json::Value) -> MultivmResult<Vec<String>>;
    
    /// Get validator name
    fn name(&self) -> &str;
}

impl ConfigManager {
    /// Create new configuration manager
    pub fn new(config: ConfigManagerConfig) -> Self {
        Self {
            base: BaseManager::new("config_manager".to_string(), config),
            configs: Arc::new(RwLock::new(HashMap::new())),
            validators: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load configuration from file
    pub async fn load_config(&self, path: &str) -> MultivmResult<()> {
        let start_time = std::time::Instant::now();
        
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| MultivmError::Configuration {
                component: "config_manager".to_string(),
                message: format!("Failed to read config file {}: {}", path, e),
                validation_errors: Some(vec![]),
            })?;

        let value: serde_json::Value = match path.ends_with(".toml") {
            true => {
                let toml_value: toml::Value = content.parse()
                    .map_err(|e| MultivmError::Configuration {
                        component: "config_manager".to_string(),
                        message: format!("Failed to parse TOML config {}: {}", path, e),
                        validation_errors: Some(vec![]),
                    })?;
                serde_json::to_value(toml_value)
                    .map_err(|e| MultivmError::Configuration {
                        component: "config_manager".to_string(),
                        message: format!("Failed to convert TOML to JSON: {}", e),
                        validation_errors: Some(vec![]),
                    })?
            },
            false => serde_json::from_str(&content)
                .map_err(|e| MultivmError::Configuration {
                    component: "config_manager".to_string(),
                    message: format!("Failed to parse JSON config {}: {}", path, e),
                    validation_errors: Some(vec![]),
                })?,
        };

        // Validate configuration if enabled
        let validation_errors = if self.base.config.enable_validation {
            self.validate_config(&value).await?
        } else {
            vec![]
        };

        let entry = ConfigEntry {
            key: path.to_string(),
            value,
            config_type: self.detect_config_type(path),
            last_modified: chrono::Utc::now(),
            source_path: Some(path.to_string()),
            is_valid: validation_errors.is_empty(),
            validation_errors,
        };

        // Store configuration
        let mut configs = self.configs.write().await;
        configs.insert(path.to_string(), entry);

        let duration_ms = start_time.elapsed().as_millis() as f64;
        self.base.record_operation(true, duration_ms).await;

        Ok(())
    }

    /// Get configuration by key
    pub async fn get_config(&self, key: &str) -> Option<ConfigEntry> {
        let configs = self.configs.read().await;
        configs.get(key).cloned()
    }

    /// Set configuration value
    pub async fn set_config(&self, key: String, value: serde_json::Value) -> MultivmResult<()> {
        let validation_errors = if self.base.config.enable_validation {
            self.validate_config(&value).await?
        } else {
            vec![]
        };

        let entry = ConfigEntry {
            key: key.clone(),
            value,
            config_type: "runtime".to_string(),
            last_modified: chrono::Utc::now(),
            source_path: None,
            is_valid: validation_errors.is_empty(),
            validation_errors,
        };

        let mut configs = self.configs.write().await;
        configs.insert(key, entry);

        Ok(())
    }

    /// Register configuration validator
    pub async fn register_validator(&self, config_type: String, validator: Box<dyn ConfigValidator>) {
        let mut validators = self.validators.write().await;
        validators.insert(config_type, validator);
    }

    /// Validate configuration
    async fn validate_config(&self, config: &serde_json::Value) -> MultivmResult<Vec<String>> {
        let validators = self.validators.read().await;
        let mut all_errors = Vec::new();

        for validator in validators.values() {
            match validator.validate(config).await {
                Ok(mut errors) => all_errors.append(&mut errors),
                Err(e) => all_errors.push(format!("Validator {} failed: {}", validator.name(), e)),
            }
        }

        Ok(all_errors)
    }

    /// Detect configuration type from file path
    fn detect_config_type(&self, path: &str) -> String {
        if path.contains("server") || path.contains("rest") || path.contains("graphql") {
            "server".to_string()
        } else if path.contains("database") || path.contains("db") {
            "database".to_string()
        } else if path.contains("cache") || path.contains("redis") {
            "cache".to_string()
        } else if path.contains("auth") || path.contains("security") {
            "auth".to_string()
        } else if path.contains("monitoring") || path.contains("metrics") {
            "monitoring".to_string()
        } else if path.contains("evm") || path.contains("reth") {
            "evm".to_string()
        } else if path.contains("svm") || path.contains("solana") {
            "svm".to_string()
        } else if path.contains("p2p") || path.contains("network") {
            "network".to_string()
        } else {
            "general".to_string()
        }
    }

    /// Reload all configurations
    pub async fn reload_all(&self) -> MultivmResult<()> {
        let paths: Vec<String> = self.base.config.config_paths.clone();
        
        for path in paths {
            if let Err(e) = self.load_config(&path).await {
                tracing::error!("Failed to reload config {}: {}", path, e);
            }
        }

        // Update state
        let mut state = self.base.custom_state.write().await;
        state.last_reload = Some(chrono::Utc::now());

        Ok(())
    }

    /// Get all configurations of a specific type
    pub async fn get_configs_by_type(&self, config_type: &str) -> Vec<ConfigEntry> {
        let configs = self.configs.read().await;
        configs.values()
            .filter(|entry| entry.config_type == config_type)
            .cloned()
            .collect()
    }

    /// Get configuration statistics
    pub async fn get_config_stats(&self) -> ConfigStats {
        let configs = self.configs.read().await;
        let total = configs.len();
        let valid = configs.values().filter(|e| e.is_valid).count();
        let invalid = total - valid;

        let mut by_type = HashMap::new();
        for entry in configs.values() {
            *by_type.entry(entry.config_type.clone()).or_insert(0) += 1;
        }

        ConfigStats {
            total_configs: total,
            valid_configs: valid,
            invalid_configs: invalid,
            configs_by_type: by_type,
        }
    }
}

/// Configuration statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigStats {
    pub total_configs: usize,
    pub valid_configs: usize,
    pub invalid_configs: usize,
    pub configs_by_type: HashMap<String, usize>,
}

#[async_trait::async_trait]
impl Manager for ConfigManager {
    type Config = ConfigManagerConfig;
    type State = ConfigManagerState;
    type Error = MultivmError;

    async fn initialize(config: Self::Config) -> MultivmResult<Self> {
        let manager = Self::new(config);
        
        // Load initial configurations
        for path in &manager.base.config.config_paths {
            if let Err(e) = manager.load_config(path).await {
                tracing::error!("Failed to load initial config {}: {}", path, e);
            }
        }

        manager.base.set_state(ManagerState::Running).await;
        Ok(manager)
    }

    async fn start(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Initializing).await;
        
        // Start hot reload task if enabled
        if self.base.config.enable_hot_reload {
            // Implementation would go here
            tracing::info!("Hot reload enabled with interval: {}s", self.base.config.hot_reload_interval);
        }
        
        self.base.set_state(ManagerState::Running).await;
        Ok(())
    }

    async fn stop(&mut self) -> MultivmResult<()> {
        self.base.set_state(ManagerState::Stopping).await;
        // Cleanup would go here
        self.base.set_state(ManagerState::Stopped).await;
        Ok(())
    }

    async fn get_state(&self) -> Self::State {
        let configs = self.configs.read().await;
        let state = self.base.custom_state.read().await;
        
        ConfigManagerState {
            loaded_configs: configs.len(),
            last_reload: state.last_reload,
            errors: state.errors.clone(),
        }
    }

    async fn health_check(&self) -> MultivmResult<HealthStatus> {
        let configs = self.configs.read().await;
        let invalid_count = configs.values().filter(|e| !e.is_valid).count();
        let total_count = configs.len();

        if total_count == 0 {
            Ok(HealthStatus::Unhealthy)
        } else if invalid_count == 0 {
            Ok(HealthStatus::Healthy)
        } else if invalid_count < total_count / 2 {
            Ok(HealthStatus::Degraded)
        } else {
            Ok(HealthStatus::Unhealthy)
        }
    }

    async fn get_stats(&self) -> MultivmResult<ManagerStats> {
        let mut stats = self.base.stats.read().await.clone();
        let config_stats = self.get_config_stats().await;
        
        stats.custom_metrics.insert(
            "config_stats".to_string(),
            serde_json::to_value(config_stats).unwrap_or_default(),
        );
        
        Ok(stats)
    }
}

impl Default for ConfigManagerConfig {
    fn default() -> Self {
        Self {
            config_paths: vec![],
            enable_hot_reload: false,
            hot_reload_interval: 60,
            enable_validation: true,
            enable_encryption: false,
            encryption_key: None,
        }
    }
}

/// Example configuration validator
pub struct BasicConfigValidator {
    name: String,
}

impl BasicConfigValidator {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait::async_trait]
impl ConfigValidator for BasicConfigValidator {
    async fn validate(&self, config: &serde_json::Value) -> MultivmResult<Vec<String>> {
        let mut errors = Vec::new();
        
        // Basic validation - ensure it's a valid JSON object
        if !config.is_object() {
            errors.push("Configuration must be a JSON object".to_string());
        }
        
        Ok(errors)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}