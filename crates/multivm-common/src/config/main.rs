use crate::{EthereumConfig, IpcConfig, LoggingConfig, MultivmError, SolanaConfig, SystemConfig};
use serde::{Deserialize, Serialize};

/// Main configuration for the multi-VM system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultivmConfig {
    pub system: SystemConfig,
    pub solana: SolanaConfig,
    pub ethereum: EthereumConfig,
    pub ipc: IpcConfig,
    pub logging: LoggingConfig,
}

impl MultivmConfig {
    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, MultivmError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            MultivmError::Configuration(format!("Failed to read config file: {}", e))
        })?;

        let config: Self = toml::from_str(&content)
            .map_err(|e| MultivmError::Configuration(format!("Failed to parse config: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save_to_file(&self, path: &str) -> Result<(), MultivmError> {
        let content = toml::to_string_pretty(self).map_err(|e| {
            MultivmError::Configuration(format!("Failed to serialize config: {}", e))
        })?;

        std::fs::write(path, content).map_err(|e| {
            MultivmError::Configuration(format!("Failed to write config file: {}", e))
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

impl Default for MultivmConfig {
    fn default() -> Self {
        Self {
            system: SystemConfig::default(),
            solana: SolanaConfig::default(),
            ethereum: EthereumConfig::default(),
            ipc: IpcConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multivm_config_default() {
        let config = MultivmConfig::default();
        assert!(config.validate().is_ok());
    }
}
