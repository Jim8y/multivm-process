//! Configuration integration for communication protocols
//!
//! This module integrates the communication protocol abstraction with
//! the MultiVM configuration system, making it easy to configure
//! protocol preferences through standard configuration files.

use super::{
    factory::ProtocolConfigBuilder, ipc_protocol::IpcProtocolConfig,
    jwt_protocol::JwtProtocolConfig, rpc_protocol::RpcProtocolConfig, CommunicationManager,
    EngineType, ProtocolType,
};
use crate::{MultivmError, MultivmResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Global communication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationConfig {
    /// Protocol preferences for each engine
    pub protocol_preferences: HashMap<EngineType, Vec<ProtocolType>>,
    /// Protocol-specific configurations
    pub protocol_configs: ProtocolConfigs,
    /// Global settings
    pub global_settings: GlobalSettings,
}

/// Protocol-specific configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfigs {
    /// IPC configurations by engine
    pub ipc: HashMap<EngineType, IpcProtocolConfig>,
    /// RPC configurations by engine
    pub rpc: HashMap<EngineType, RpcProtocolConfig>,
    /// JWT configurations by engine
    pub jwt: HashMap<EngineType, JwtProtocolConfig>,
}

/// Global communication settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    /// Enable automatic health checks
    pub enable_health_checks: bool,
    /// Default request timeout for all protocols
    pub default_timeout_seconds: u64,
    /// Maximum number of concurrent connections per protocol
    pub max_concurrent_connections: usize,
    /// Enable protocol fallback on failure
    pub enable_fallback: bool,
    /// Protocol connection retry settings
    pub retry_settings: RetrySettings,
}

/// Global retry settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrySettings {
    /// Maximum retry attempts for connection failures
    pub max_connection_retries: u32,
    /// Base delay between connection retry attempts
    pub connection_retry_delay_ms: u64,
    /// Maximum delay between retry attempts
    pub max_retry_delay_ms: u64,
    /// Retry backoff multiplier
    pub retry_backoff_multiplier: f64,
}

impl Default for CommunicationConfig {
    fn default() -> Self {
        let mut protocol_preferences = HashMap::new();

        // Ethereum: prefer JWT for security, RPC for compatibility, IPC for performance
        protocol_preferences.insert(
            EngineType::Ethereum,
            vec![ProtocolType::Jwt, ProtocolType::Rpc, ProtocolType::Ipc],
        );

        // Solana: prefer RPC (most common), JWT for auth, IPC for local
        protocol_preferences.insert(
            EngineType::Solana,
            vec![ProtocolType::Rpc, ProtocolType::Jwt, ProtocolType::Ipc],
        );

        Self {
            protocol_preferences,
            protocol_configs: ProtocolConfigs::default(),
            global_settings: GlobalSettings::default(),
        }
    }
}

impl Default for ProtocolConfigs {
    fn default() -> Self {
        let mut ipc = HashMap::new();
        let mut rpc = HashMap::new();
        let mut jwt = HashMap::new();

        // Ethereum configurations
        let mut eth_ipc = IpcProtocolConfig::default();
        eth_ipc.transport_config.tcp_port = 8545;
        eth_ipc.transport_config.unix_socket_path = Some("/tmp/multivm-ethereum.sock".to_string());
        ipc.insert(EngineType::Ethereum, eth_ipc);

        let mut eth_rpc = RpcProtocolConfig::default();
        eth_rpc.endpoint_url = "http://127.0.0.1:8545".to_string();
        eth_rpc.health_check_config.health_method = "eth_chainId".to_string();
        rpc.insert(EngineType::Ethereum, eth_rpc);

        let mut eth_jwt = JwtProtocolConfig::default();
        eth_jwt.endpoint_url = "http://127.0.0.1:8551".to_string(); // Engine API port
        jwt.insert(EngineType::Ethereum, eth_jwt);

        // Solana configurations
        let mut sol_ipc = IpcProtocolConfig::default();
        sol_ipc.transport_config.tcp_port = 8899;
        sol_ipc.transport_config.unix_socket_path = Some("/tmp/multivm-solana.sock".to_string());
        ipc.insert(EngineType::Solana, sol_ipc);

        let mut sol_rpc = RpcProtocolConfig::default();
        sol_rpc.endpoint_url = "http://127.0.0.1:8899".to_string();
        sol_rpc.health_check_config.health_method = "getVersion".to_string();
        rpc.insert(EngineType::Solana, sol_rpc);

        let mut sol_jwt = JwtProtocolConfig::default();
        sol_jwt.endpoint_url = "http://127.0.0.1:8900".to_string();
        jwt.insert(EngineType::Solana, sol_jwt);

        Self { ipc, rpc, jwt }
    }
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            enable_health_checks: true,
            default_timeout_seconds: 30,
            max_concurrent_connections: 10,
            enable_fallback: true,
            retry_settings: RetrySettings::default(),
        }
    }
}

impl Default for RetrySettings {
    fn default() -> Self {
        Self {
            max_connection_retries: 3,
            connection_retry_delay_ms: 1000,
            max_retry_delay_ms: 10000,
            retry_backoff_multiplier: 2.0,
        }
    }
}

impl CommunicationConfig {
    /// Load configuration from file
    pub async fn load_from_file(path: &str) -> MultivmResult<Self> {
        let content =
            tokio::fs::read_to_string(path)
                .await
                .map_err(|e| MultivmError::Configuration {
                    component: "communication_config".to_string(),
                    message: format!("Failed to read configuration file {path}: {e}"),
                    validation_errors: None,
                })?;

        let config: Self = if path.ends_with(".toml") {
            toml::from_str(&content).map_err(|e| MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: format!("Failed to parse TOML configuration: {e}"),
                validation_errors: None,
            })?
        } else if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content).map_err(|e| MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: format!("Failed to parse YAML configuration: {e}"),
                validation_errors: None,
            })?
        } else {
            serde_json::from_str(&content).map_err(|e| MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: format!("Failed to parse JSON configuration: {e}"),
                validation_errors: None,
            })?
        };

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to file
    pub async fn save_to_file(&self, path: &str) -> MultivmResult<()> {
        let content = if path.ends_with(".toml") {
            toml::to_string_pretty(self).map_err(|e| MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: format!("Failed to serialize to TOML: {e}"),
                validation_errors: None,
            })?
        } else if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::to_string(self).map_err(|e| MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: format!("Failed to serialize to YAML: {e}"),
                validation_errors: None,
            })?
        } else {
            serde_json::to_string_pretty(self).map_err(|e| MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: format!("Failed to serialize to JSON: {e}"),
                validation_errors: None,
            })?
        };

        tokio::fs::write(path, content)
            .await
            .map_err(|e| MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: format!("Failed to write configuration file {path}: {e}"),
                validation_errors: None,
            })
    }

    /// Validate the configuration
    pub fn validate(&self) -> MultivmResult<()> {
        let mut errors = Vec::new();

        // Validate protocol preferences
        for (engine, protocols) in &self.protocol_preferences {
            if protocols.is_empty() {
                errors.push(format!("No protocols configured for engine: {engine}"));
            }

            // Check that all referenced protocols have configurations
            for protocol in protocols {
                match protocol {
                    ProtocolType::Ipc => {
                        if !self.protocol_configs.ipc.contains_key(engine) {
                            errors
                                .push(format!("Missing IPC configuration for engine: {engine}"));
                        }
                    }
                    ProtocolType::Rpc => {
                        if !self.protocol_configs.rpc.contains_key(engine) {
                            errors
                                .push(format!("Missing RPC configuration for engine: {engine}"));
                        }
                    }
                    ProtocolType::Jwt => {
                        if !self.protocol_configs.jwt.contains_key(engine) {
                            errors
                                .push(format!("Missing JWT configuration for engine: {engine}"));
                        }
                    }
                }
            }
        }

        // Validate global settings
        if self.global_settings.default_timeout_seconds == 0 {
            errors.push("Default timeout cannot be zero".to_string());
        }

        if self.global_settings.max_concurrent_connections == 0 {
            errors.push("Max concurrent connections cannot be zero".to_string());
        }

        if self.global_settings.retry_settings.max_connection_retries > 10 {
            warn!(
                "Very high retry count configured: {}",
                self.global_settings.retry_settings.max_connection_retries
            );
        }

        if !errors.is_empty() {
            return Err(MultivmError::Configuration {
                component: "communication_config".to_string(),
                message: "Configuration validation failed".to_string(),
                validation_errors: Some(errors),
            });
        }

        Ok(())
    }

    /// Create a communication manager from this configuration
    pub async fn create_manager(&self) -> MultivmResult<CommunicationManager> {
        info!("Creating communication manager from configuration");

        // Create protocol factory with configurations
        let mut config_builder = ProtocolConfigBuilder::new();

        // Add IPC configurations
        for (engine, config) in &self.protocol_configs.ipc {
            config_builder = config_builder.ipc(engine.clone(), config.clone());
        }

        // Add RPC configurations
        for (engine, config) in &self.protocol_configs.rpc {
            config_builder = config_builder.rpc(engine.clone(), config.clone());
        }

        // Add JWT configurations
        for (engine, config) in &self.protocol_configs.jwt {
            config_builder = config_builder.jwt(engine.clone(), config.clone());
        }

        let factory = Box::new(config_builder.build_factory());
        let mut manager = CommunicationManager::new(factory);

        // Set protocol preferences
        for (engine, preferences) in &self.protocol_preferences {
            manager.set_protocol_preferences(engine.clone(), preferences.clone());
            debug!("Set protocol preferences for {}: {:?}", engine, preferences);
        }

        // Initialize protocols based on preferences
        for (engine, preferences) in &self.protocol_preferences {
            for protocol in preferences {
                let config_value = match protocol {
                    ProtocolType::Ipc => self
                        .protocol_configs
                        .ipc
                        .get(engine)
                        .map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null),
                    ProtocolType::Rpc => self
                        .protocol_configs
                        .rpc
                        .get(engine)
                        .map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null),
                    ProtocolType::Jwt => self
                        .protocol_configs
                        .jwt
                        .get(engine)
                        .map(|c| serde_json::to_value(c).unwrap_or(serde_json::Value::Null))
                        .unwrap_or(serde_json::Value::Null),
                };

                if config_value != serde_json::Value::Null {
                    match manager
                        .add_protocol(protocol.clone(), engine.clone(), config_value)
                        .await
                    {
                        Ok(()) => {
                            info!("Added {} protocol for {} engine", protocol, engine);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to add {} protocol for {} engine: {}",
                                protocol, engine, e
                            );
                        }
                    }
                }
            }
        }

        info!("Communication manager created successfully");
        Ok(manager)
    }

    /// Get configuration for a specific protocol and engine
    pub fn get_protocol_config(
        &self,
        protocol_type: &ProtocolType,
        engine_type: &EngineType,
    ) -> Option<serde_json::Value> {
        match protocol_type {
            ProtocolType::Ipc => self
                .protocol_configs
                .ipc
                .get(engine_type)
                .and_then(|c| serde_json::to_value(c).ok()),
            ProtocolType::Rpc => self
                .protocol_configs
                .rpc
                .get(engine_type)
                .and_then(|c| serde_json::to_value(c).ok()),
            ProtocolType::Jwt => self
                .protocol_configs
                .jwt
                .get(engine_type)
                .and_then(|c| serde_json::to_value(c).ok()),
        }
    }

    /// Update configuration for a specific protocol and engine
    pub fn set_protocol_config(
        &mut self,
        protocol_type: ProtocolType,
        engine_type: EngineType,
        config: serde_json::Value,
    ) -> MultivmResult<()> {
        match protocol_type {
            ProtocolType::Ipc => {
                let ipc_config: IpcProtocolConfig =
                    serde_json::from_value(config).map_err(|e| MultivmError::Configuration {
                        component: "communication_config".to_string(),
                        message: format!("Invalid IPC configuration: {e}"),
                        validation_errors: None,
                    })?;
                self.protocol_configs.ipc.insert(engine_type, ipc_config);
            }
            ProtocolType::Rpc => {
                let rpc_config: RpcProtocolConfig =
                    serde_json::from_value(config).map_err(|e| MultivmError::Configuration {
                        component: "communication_config".to_string(),
                        message: format!("Invalid RPC configuration: {e}"),
                        validation_errors: None,
                    })?;
                self.protocol_configs.rpc.insert(engine_type, rpc_config);
            }
            ProtocolType::Jwt => {
                let jwt_config: JwtProtocolConfig =
                    serde_json::from_value(config).map_err(|e| MultivmError::Configuration {
                        component: "communication_config".to_string(),
                        message: format!("Invalid JWT configuration: {e}"),
                        validation_errors: None,
                    })?;
                self.protocol_configs.jwt.insert(engine_type, jwt_config);
            }
        }

        Ok(())
    }

    /// Merge with another configuration (other takes precedence)
    pub fn merge(&mut self, other: CommunicationConfig) {
        // Merge protocol preferences
        for (engine, preferences) in other.protocol_preferences {
            self.protocol_preferences.insert(engine, preferences);
        }

        // Merge protocol configurations
        for (engine, config) in other.protocol_configs.ipc {
            self.protocol_configs.ipc.insert(engine, config);
        }
        for (engine, config) in other.protocol_configs.rpc {
            self.protocol_configs.rpc.insert(engine, config);
        }
        for (engine, config) in other.protocol_configs.jwt {
            self.protocol_configs.jwt.insert(engine, config);
        }

        // Update global settings
        self.global_settings = other.global_settings;
    }

    /// Create a minimal configuration for testing
    pub fn minimal() -> Self {
        let mut config = Self::default();

        // Only configure essential protocols
        config.protocol_preferences.clear();
        config
            .protocol_preferences
            .insert(EngineType::Ethereum, vec![ProtocolType::Rpc]);
        config
            .protocol_preferences
            .insert(EngineType::Solana, vec![ProtocolType::Rpc]);

        // Disable health checks for minimal setup
        config.global_settings.enable_health_checks = false;
        config.global_settings.enable_fallback = false;

        config
    }
}

/// Configuration builder for fluent configuration creation
#[derive(Debug, Default)]
pub struct CommunicationConfigBuilder {
    config: CommunicationConfig,
}

impl CommunicationConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set protocol preferences for an engine
    pub fn protocol_preferences(
        mut self,
        engine: EngineType,
        preferences: Vec<ProtocolType>,
    ) -> Self {
        self.config.protocol_preferences.insert(engine, preferences);
        self
    }

    /// Add IPC configuration for an engine
    pub fn ipc_config(mut self, engine: EngineType, config: IpcProtocolConfig) -> Self {
        self.config.protocol_configs.ipc.insert(engine, config);
        self
    }

    /// Add RPC configuration for an engine
    pub fn rpc_config(mut self, engine: EngineType, config: RpcProtocolConfig) -> Self {
        self.config.protocol_configs.rpc.insert(engine, config);
        self
    }

    /// Add JWT configuration for an engine
    pub fn jwt_config(mut self, engine: EngineType, config: JwtProtocolConfig) -> Self {
        self.config.protocol_configs.jwt.insert(engine, config);
        self
    }

    /// Set global settings
    pub fn global_settings(mut self, settings: GlobalSettings) -> Self {
        self.config.global_settings = settings;
        self
    }

    /// Enable/disable health checks
    pub fn enable_health_checks(mut self, enable: bool) -> Self {
        self.config.global_settings.enable_health_checks = enable;
        self
    }

    /// Enable/disable protocol fallback
    pub fn enable_fallback(mut self, enable: bool) -> Self {
        self.config.global_settings.enable_fallback = enable;
        self
    }

    /// Set default timeout
    pub fn default_timeout_seconds(mut self, timeout: u64) -> Self {
        self.config.global_settings.default_timeout_seconds = timeout;
        self
    }

    /// Build the configuration
    pub fn build(self) -> CommunicationConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CommunicationConfig::default();

        // Should have preferences for both engines
        assert!(config
            .protocol_preferences
            .contains_key(&EngineType::Ethereum));
        assert!(config
            .protocol_preferences
            .contains_key(&EngineType::Solana));

        // Should have configurations for both engines
        assert!(config
            .protocol_configs
            .ipc
            .contains_key(&EngineType::Ethereum));
        assert!(config
            .protocol_configs
            .rpc
            .contains_key(&EngineType::Ethereum));
        assert!(config
            .protocol_configs
            .jwt
            .contains_key(&EngineType::Ethereum));
    }

    #[test]
    fn test_config_validation() {
        let mut config = CommunicationConfig::default();

        // Valid config should pass
        assert!(config.validate().is_ok());

        // Remove protocol configuration to make it invalid
        config.protocol_configs.rpc.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_builder() {
        let config = CommunicationConfigBuilder::new()
            .protocol_preferences(EngineType::Ethereum, vec![ProtocolType::Rpc])
            .enable_health_checks(false)
            .default_timeout_seconds(60)
            .build();

        assert_eq!(
            config.protocol_preferences.get(&EngineType::Ethereum),
            Some(&vec![ProtocolType::Rpc])
        );
        assert!(!config.global_settings.enable_health_checks);
        assert_eq!(config.global_settings.default_timeout_seconds, 60);
    }

    #[test]
    fn test_minimal_config() {
        let config = CommunicationConfig::minimal();

        // Should only have RPC protocols
        for preferences in config.protocol_preferences.values() {
            assert_eq!(preferences, &vec![ProtocolType::Rpc]);
        }

        // Health checks should be disabled
        assert!(!config.global_settings.enable_health_checks);
        assert!(!config.global_settings.enable_fallback);
    }

    #[tokio::test]
    async fn test_create_manager() {
        let config = CommunicationConfig::minimal();
        let manager = config.create_manager().await;

        // Should create successfully
        assert!(manager.is_ok());
    }
}
