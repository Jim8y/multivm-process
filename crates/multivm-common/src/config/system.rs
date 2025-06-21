use crate::{MultivmError, ResourceLimits};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// System-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub data_dir: PathBuf,
    pub resource_limits: ResourceLimits,
    pub max_processes: u32,
    pub process_restart_delay: Duration,
    pub health_check_interval: Duration,
    pub shutdown_timeout: Duration,
    pub enable_metrics: bool,
    pub metrics_port: Option<u16>,
}

impl SystemConfig {
    pub fn validate(&self) -> Result<(), MultivmError> {
        // Only check if data directory exists if its parent exists
        // This allows for directories to be created later
        if let Some(parent) = self.data_dir.parent() {
            if !parent.exists() && parent != std::path::Path::new(".") {
                return Err(MultivmError::Configuration(format!(
                    "Data directory parent does not exist: {:?}",
                    parent
                )));
            }
        }

        if self.max_processes == 0 {
            return Err(MultivmError::Configuration(
                "max_processes must be greater than 0".to_string(),
            ));
        }

        if self.health_check_interval.as_secs() == 0 {
            return Err(MultivmError::Configuration(
                "health_check_interval must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("./data"),
            resource_limits: ResourceLimits::default(),
            max_processes: 10,
            process_restart_delay: Duration::from_secs(5),
            health_check_interval: Duration::from_secs(30),
            shutdown_timeout: Duration::from_secs(30),
            enable_metrics: true,
            metrics_port: Some(9090),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_config_validation() {
        let config = SystemConfig {
            max_processes: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
