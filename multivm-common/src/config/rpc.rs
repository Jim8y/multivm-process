//! RPC Configuration Module
//!
//! This module defines the configuration for RPC servers used by various MultiVM
//! components. It includes settings for connection management, timeouts, and
//! protocol-specific parameters.

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
            return Err(MultivmError::Configuration {
                component: "rpc".to_string(),
                message: "RPC port must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.max_connections == 0 {
            return Err(MultivmError::Configuration {
                component: "rpc".to_string(),
                message: "max_connections must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.request_timeout.as_secs() == 0 {
            return Err(MultivmError::Configuration {
                component: "rpc".to_string(),
                message: "request_timeout must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        Ok(())
    }
}

