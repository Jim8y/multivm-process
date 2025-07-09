//! Protocol factory implementation for creating communication instances
//!
//! This module provides factory methods to create different protocol instances
//! based on configuration and engine requirements.

use super::{
    ipc_protocol::{IpcProtocol, IpcProtocolConfig},
    jwt_protocol::{JwtProtocol, JwtProtocolConfig},
    rpc_protocol::{RpcProtocol, RpcProtocolConfig},
    CommunicationProtocol, EngineType, ProtocolFactory, ProtocolType,
};
use crate::{MultivmError, MultivmResult};
use async_trait::async_trait;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Default protocol factory implementation
#[derive(Debug, Clone)]
pub struct DefaultProtocolFactory {
    /// Default configurations for each protocol type
    default_configs: HashMap<ProtocolType, serde_json::Value>,
    /// Engine-specific configurations
    engine_configs: HashMap<(ProtocolType, EngineType), serde_json::Value>,
    /// Supported protocols list
    supported_protocols: Vec<ProtocolType>,
}

impl DefaultProtocolFactory {
    /// Create a new protocol factory with default configurations
    pub fn new() -> Self {
        let mut factory = Self {
            default_configs: HashMap::new(),
            engine_configs: HashMap::new(),
            supported_protocols: vec![ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt],
        };

        factory.setup_default_configurations();
        factory
    }

    /// Create a factory with custom supported protocols
    pub fn with_protocols(protocols: Vec<ProtocolType>) -> Self {
        let mut factory = Self {
            default_configs: HashMap::new(),
            engine_configs: HashMap::new(),
            supported_protocols: protocols,
        };

        factory.setup_default_configurations();
        factory
    }

    /// Setup default configurations for each protocol
    fn setup_default_configurations(&mut self) {
        // IPC default configuration
        let ipc_config = IpcProtocolConfig::default();
        self.default_configs
            .insert(ProtocolType::Ipc, serde_json::to_value(ipc_config).unwrap());

        // RPC default configuration
        let rpc_config = RpcProtocolConfig::default();
        self.default_configs
            .insert(ProtocolType::Rpc, serde_json::to_value(rpc_config).unwrap());

        // JWT default configuration
        let jwt_config = JwtProtocolConfig::default();
        self.default_configs
            .insert(ProtocolType::Jwt, serde_json::to_value(jwt_config).unwrap());

        // Engine-specific configurations
        self.setup_engine_specific_configs();
    }

    /// Setup engine-specific default configurations
    fn setup_engine_specific_configs(&mut self) {
        // Ethereum engine configurations
        self.setup_ethereum_configs();

        // Solana engine configurations
        self.setup_solana_configs();
    }

    /// Setup Ethereum-specific configurations
    fn setup_ethereum_configs(&mut self) {
        // Ethereum IPC configuration
        let mut eth_ipc_config = IpcProtocolConfig::default();
        eth_ipc_config.transport_config.tcp_port = 8545;
        eth_ipc_config.transport_config.unix_socket_path =
            Some("/tmp/multivm-ethereum.sock".to_string());
        self.engine_configs.insert(
            (ProtocolType::Ipc, EngineType::Ethereum),
            serde_json::to_value(eth_ipc_config).unwrap(),
        );

        // Ethereum RPC configuration
        let mut eth_rpc_config = RpcProtocolConfig::default();
        eth_rpc_config.endpoint_url = "http://127.0.0.1:8545".to_string();
        eth_rpc_config.health_check_config.health_method = "eth_chainId".to_string();
        self.engine_configs.insert(
            (ProtocolType::Rpc, EngineType::Ethereum),
            serde_json::to_value(eth_rpc_config).unwrap(),
        );

        // Ethereum JWT configuration
        let mut eth_jwt_config = JwtProtocolConfig::default();
        eth_jwt_config.endpoint_url = "http://127.0.0.1:8551".to_string(); // Engine API port
        eth_jwt_config.health_check_config.health_endpoint = "/health".to_string();
        self.engine_configs.insert(
            (ProtocolType::Jwt, EngineType::Ethereum),
            serde_json::to_value(eth_jwt_config).unwrap(),
        );
    }

    /// Setup Solana-specific configurations
    fn setup_solana_configs(&mut self) {
        // Solana IPC configuration
        let mut sol_ipc_config = IpcProtocolConfig::default();
        sol_ipc_config.transport_config.tcp_port = 8899;
        sol_ipc_config.transport_config.unix_socket_path =
            Some("/tmp/multivm-solana.sock".to_string());
        self.engine_configs.insert(
            (ProtocolType::Ipc, EngineType::Solana),
            serde_json::to_value(sol_ipc_config).unwrap(),
        );

        // Solana RPC configuration
        let mut sol_rpc_config = RpcProtocolConfig::default();
        sol_rpc_config.endpoint_url = "http://127.0.0.1:8899".to_string();
        sol_rpc_config.health_check_config.health_method = "getVersion".to_string();
        self.engine_configs.insert(
            (ProtocolType::Rpc, EngineType::Solana),
            serde_json::to_value(sol_rpc_config).unwrap(),
        );

        // Solana JWT configuration
        let mut sol_jwt_config = JwtProtocolConfig::default();
        sol_jwt_config.endpoint_url = "http://127.0.0.1:8900".to_string();
        sol_jwt_config.health_check_config.health_endpoint = "/health".to_string();
        self.engine_configs.insert(
            (ProtocolType::Jwt, EngineType::Solana),
            serde_json::to_value(sol_jwt_config).unwrap(),
        );
    }

    /// Set default configuration for a protocol type
    pub fn set_default_config(&mut self, protocol_type: ProtocolType, config: serde_json::Value) {
        self.default_configs.insert(protocol_type, config);
    }

    /// Set engine-specific configuration
    pub fn set_engine_config(
        &mut self,
        protocol_type: ProtocolType,
        engine_type: EngineType,
        config: serde_json::Value,
    ) {
        self.engine_configs
            .insert((protocol_type, engine_type), config);
    }

    /// Get effective configuration (engine-specific overrides default)
    fn get_effective_config(
        &self,
        protocol_type: &ProtocolType,
        engine_type: &EngineType,
        user_config: Option<serde_json::Value>,
    ) -> MultivmResult<serde_json::Value> {
        // Start with default config
        let mut config = self
            .default_configs
            .get(protocol_type)
            .cloned()
            .unwrap_or(serde_json::Value::Null);

        // Apply engine-specific config if available
        if let Some(engine_config) = self
            .engine_configs
            .get(&(protocol_type.clone(), engine_type.clone()))
        {
            if let (Some(base), Some(override_config)) =
                (config.as_object_mut(), engine_config.as_object())
            {
                for (key, value) in override_config {
                    base.insert(key.clone(), value.clone());
                }
            } else {
                config = engine_config.clone();
            }
        }

        // Apply user-provided config if available
        if let Some(user_config) = user_config {
            if let (Some(base), Some(user_override)) =
                (config.as_object_mut(), user_config.as_object())
            {
                for (key, value) in user_override {
                    base.insert(key.clone(), value.clone());
                }
            } else if user_config != serde_json::Value::Null {
                config = user_config;
            }
        }

        Ok(config)
    }

    /// Validate configuration for a specific protocol
    fn validate_config(
        &self,
        protocol_type: &ProtocolType,
        engine_type: &EngineType,
        config: &serde_json::Value,
    ) -> MultivmResult<()> {
        match protocol_type {
            ProtocolType::Ipc => {
                let _: IpcProtocolConfig = serde_json::from_value(config.clone()).map_err(|e| {
                    MultivmError::Configuration {
                        component: "ipc_protocol_factory".to_string(),
                        message: format!("Invalid IPC configuration: {e}"),
                        validation_errors: None,
                    }
                })?;
            }
            ProtocolType::Rpc => {
                let _: RpcProtocolConfig = serde_json::from_value(config.clone()).map_err(|e| {
                    MultivmError::Configuration {
                        component: "rpc_protocol_factory".to_string(),
                        message: format!("Invalid RPC configuration: {e}"),
                        validation_errors: None,
                    }
                })?;
            }
            ProtocolType::Jwt => {
                let _: JwtProtocolConfig = serde_json::from_value(config.clone()).map_err(|e| {
                    MultivmError::Configuration {
                        component: "jwt_protocol_factory".to_string(),
                        message: format!("Invalid JWT configuration: {e}"),
                        validation_errors: None,
                    }
                })?;
            }
        }

        // Additional engine-specific validation
        match (protocol_type, engine_type) {
            (ProtocolType::Rpc, EngineType::Ethereum) => {
                if let Some(url) = config.get("endpoint_url").and_then(|v| v.as_str()) {
                    if !url.starts_with("http://") && !url.starts_with("https://") {
                        return Err(MultivmError::Configuration {
                            component: "ethereum_rpc_factory".to_string(),
                            message: "Ethereum RPC endpoint must be a valid HTTP/HTTPS URL"
                                .to_string(),
                            validation_errors: Some(vec![url.to_string()]),
                        });
                    }
                }
            }
            (ProtocolType::Jwt, _) => {
                if config.get("jwt_secret").is_none()
                    || config
                        .get("jwt_secret")
                        .and_then(|v| v.as_str())
                        .is_none_or(|s| s.is_empty())
                {
                    warn!("JWT secret not configured - using default (insecure for production)");
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl Default for DefaultProtocolFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProtocolFactory for DefaultProtocolFactory {
    async fn create_protocol(
        &self,
        protocol_type: ProtocolType,
        engine_type: EngineType,
        config: serde_json::Value,
    ) -> MultivmResult<Box<dyn CommunicationProtocol>> {
        info!(
            "Creating {} protocol for {} engine",
            protocol_type, engine_type
        );

        if !self.supports_protocol(&protocol_type) {
            return Err(MultivmError::UnsupportedOperation {
                operation: format!("Protocol: {protocol_type}"),
                alternatives: Some(
                    self.supported_protocols
                        .iter()
                        .map(|p| p.to_string())
                        .collect(),
                ),
            });
        }

        // Get effective configuration
        let effective_config = self.get_effective_config(
            &protocol_type,
            &engine_type,
            if config == serde_json::Value::Null {
                None
            } else {
                Some(config)
            },
        )?;

        // Validate configuration
        self.validate_config(&protocol_type, &engine_type, &effective_config)?;

        debug!(
            "Using configuration for {} {} protocol: {}",
            engine_type,
            protocol_type,
            serde_json::to_string(&effective_config).unwrap_or_else(|_| "invalid".to_string())
        );

        // Create the appropriate protocol instance
        let protocol: Box<dyn CommunicationProtocol> = match protocol_type {
            ProtocolType::Ipc => {
                let ipc_config: IpcProtocolConfig = serde_json::from_value(effective_config)
                    .map_err(|e| MultivmError::Configuration {
                        component: "ipc_protocol_factory".to_string(),
                        message: format!("Failed to parse IPC configuration: {e}"),
                        validation_errors: None,
                    })?;

                let protocol = IpcProtocol::new(engine_type.clone(), ipc_config);
                Box::new(protocol)
            }
            ProtocolType::Rpc => {
                let rpc_config: RpcProtocolConfig = serde_json::from_value(effective_config)
                    .map_err(|e| MultivmError::Configuration {
                        component: "rpc_protocol_factory".to_string(),
                        message: format!("Failed to parse RPC configuration: {e}"),
                        validation_errors: None,
                    })?;

                let protocol = RpcProtocol::new(engine_type.clone(), rpc_config)
                    .await
                    .map_err(|e| MultivmError::Configuration {
                        component: "rpc_protocol_factory".to_string(),
                        message: format!("Failed to create RPC protocol: {e}"),
                        validation_errors: None,
                    })?;

                Box::new(protocol)
            }
            ProtocolType::Jwt => {
                let jwt_config: JwtProtocolConfig = serde_json::from_value(effective_config)
                    .map_err(|e| MultivmError::Configuration {
                        component: "jwt_protocol_factory".to_string(),
                        message: format!("Failed to parse JWT configuration: {e}"),
                        validation_errors: None,
                    })?;

                let protocol = JwtProtocol::new(engine_type.clone(), jwt_config)
                    .await
                    .map_err(|e| MultivmError::Configuration {
                        component: "jwt_protocol_factory".to_string(),
                        message: format!("Failed to create JWT protocol: {e}"),
                        validation_errors: None,
                    })?;

                Box::new(protocol)
            }
        };

        info!(
            "Successfully created {} protocol for {} engine",
            protocol_type, engine_type
        );

        Ok(protocol)
    }

    fn supported_protocols(&self) -> Vec<ProtocolType> {
        self.supported_protocols.clone()
    }
}

/// Configuration builder for protocols
#[derive(Debug, Clone, Default)]
pub struct ProtocolConfigBuilder {
    configs: HashMap<(ProtocolType, EngineType), serde_json::Value>,
}

impl ProtocolConfigBuilder {
    /// Create a new configuration builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add IPC configuration for an engine
    pub fn ipc(mut self, engine_type: EngineType, config: IpcProtocolConfig) -> Self {
        let config_value = serde_json::to_value(config).unwrap_or(serde_json::Value::Null);
        self.configs
            .insert((ProtocolType::Ipc, engine_type), config_value);
        self
    }

    /// Add RPC configuration for an engine
    pub fn rpc(mut self, engine_type: EngineType, config: RpcProtocolConfig) -> Self {
        let config_value = serde_json::to_value(config).unwrap_or(serde_json::Value::Null);
        self.configs
            .insert((ProtocolType::Rpc, engine_type), config_value);
        self
    }

    /// Add JWT configuration for an engine
    pub fn jwt(mut self, engine_type: EngineType, config: JwtProtocolConfig) -> Self {
        let config_value = serde_json::to_value(config).unwrap_or(serde_json::Value::Null);
        self.configs
            .insert((ProtocolType::Jwt, engine_type), config_value);
        self
    }

    /// Add raw JSON configuration
    pub fn raw_config(
        mut self,
        protocol_type: ProtocolType,
        engine_type: EngineType,
        config: serde_json::Value,
    ) -> Self {
        self.configs.insert((protocol_type, engine_type), config);
        self
    }

    /// Build a protocol factory with the configured settings
    pub fn build_factory(self) -> DefaultProtocolFactory {
        let mut factory = DefaultProtocolFactory::new();

        for ((protocol_type, engine_type), config) in self.configs {
            factory.set_engine_config(protocol_type, engine_type, config);
        }

        factory
    }

    /// Get configuration for a specific protocol and engine
    pub fn get_config(
        &self,
        protocol_type: &ProtocolType,
        engine_type: &EngineType,
    ) -> Option<&serde_json::Value> {
        self.configs
            .get(&(protocol_type.clone(), engine_type.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_factory_creation() {
        let factory = DefaultProtocolFactory::new();
        assert!(factory.supports_protocol(&ProtocolType::Ipc));
        assert!(factory.supports_protocol(&ProtocolType::Rpc));
        assert!(factory.supports_protocol(&ProtocolType::Jwt));
    }

    #[tokio::test]
    async fn test_config_builder() {
        let mut ipc_config = IpcProtocolConfig::default();
        ipc_config.transport_config.tcp_port = 9999;

        let factory = ProtocolConfigBuilder::new()
            .ipc(EngineType::Ethereum, ipc_config)
            .build_factory();

        let config = factory
            .engine_configs
            .get(&(ProtocolType::Ipc, EngineType::Ethereum));
        assert!(config.is_some());

        let port = config
            .unwrap()
            .get("transport_config")
            .and_then(|tc| tc.get("tcp_port"))
            .and_then(|p| p.as_u64());
        assert_eq!(port, Some(9999));
    }

    #[tokio::test]
    async fn test_protocol_creation() {
        let factory = DefaultProtocolFactory::new();

        // Test IPC protocol creation
        let ipc_protocol = factory
            .create_protocol(
                ProtocolType::Ipc,
                EngineType::Ethereum,
                serde_json::Value::Null,
            )
            .await;

        assert!(ipc_protocol.is_ok());
        assert_eq!(ipc_protocol.unwrap().protocol_type(), ProtocolType::Ipc);
    }

    #[test]
    fn test_unsupported_protocol() {
        let factory = DefaultProtocolFactory::with_protocols(vec![ProtocolType::Ipc]);
        assert!(!factory.supports_protocol(&ProtocolType::Rpc));
        assert!(!factory.supports_protocol(&ProtocolType::Jwt));
    }
}
