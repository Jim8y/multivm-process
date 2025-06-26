use crate::config::{EthereumConfig, LegacyIpcConfig, LegacyLoggingConfig, LegacySystemConfig};
use crate::{MultivmError};
use crate::config::SolanaExecutionConfig;
use serde::{Deserialize, Serialize};

/// Main configuration for the multi-VM system
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MultivmConfig {
    pub system: LegacySystemConfig,
    pub solana: SolanaExecutionConfig,
    pub ethereum: EthereumConfig,
    pub ipc: LegacyIpcConfig,
    pub logging: LegacyLoggingConfig,
}

impl MultivmConfig {
    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, MultivmError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            MultivmError::Configuration {
                component: "main".to_string(),
                message: format!("Failed to read config file: {}", e),
                validation_errors: None,
            }
        })?;

        let config: Self = toml::from_str(&content)
            .map_err(|e| MultivmError::Configuration {
                component: "main".to_string(),
                message: format!("Failed to parse config: {}", e),
                validation_errors: None,
            })?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: &str) -> Result<(), MultivmError> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            MultivmError::Configuration {
                component: "main".to_string(),
                message: format!("Failed to serialize config: {}", e),
                validation_errors: None,
            }
        })?;

        std::fs::write(path, content).map_err(|e| {
            MultivmError::Configuration {
                component: "main".to_string(),
                message: format!("Failed to write config file: {}", e),
                validation_errors: None,
            }
        })?;

        Ok(())
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), MultivmError> {
        self.system.validate()?;
        self.solana.validate()?;
        self.ethereum.validate()?;
        self.ipc.validate()?;
        self.logging.validate()?;
        Ok(())
    }
}

