use crate::MultivmError;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// RPC server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
    pub request_timeout: Duration,
    pub cors_origins: Vec<String>,
}

impl RpcConfig {
    pub fn validate(&self) -> Result<(), MultivmError> {
        if self.port == 0 {
            return Err(MultivmError::Configuration(
                "RPC port must be greater than 0".to_string(),
            ));
        }

        if self.max_connections == 0 {
            return Err(MultivmError::Configuration(
                "max_connections must be greater than 0".to_string(),
            ));
        }

        if self.request_timeout.as_secs() == 0 {
            return Err(MultivmError::Configuration(
                "request_timeout must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_config_validation() {
        let mut config = RpcConfig {
            host: "127.0.0.1".to_string(),
            port: 0, // Invalid port
            max_connections: 1000,
            request_timeout: Duration::from_secs(30),
            cors_origins: vec!["*".to_string()],
        };
        assert!(config.validate().is_err());

        config.port = 8545;
        assert!(config.validate().is_ok());
    }
}
