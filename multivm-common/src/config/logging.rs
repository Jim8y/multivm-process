use crate::MultivmError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: LogLevel,
    pub file_path: Option<PathBuf>,
    pub enable_json: bool,
    pub enable_colors: bool,
}

impl LoggingConfig {
    pub fn validate(&self) -> Result<(), MultivmError> {
        if self.file_path.is_none() {
            return Err(MultivmError::Configuration(
                "file_path must be specified".to_string(),
            ));
        }

        Ok(())
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: LogLevel::Info,
            file_path: Some(PathBuf::from("./logs/multivm.log")),
            enable_json: false,
            enable_colors: true,
        }
    }
}

/// Log level enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}
