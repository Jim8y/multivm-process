use crate::MultivmError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

/// IPC configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcConfig {
    pub transport: IpcTransportConfig,
    pub message_timeout: Duration,
    pub max_message_size: usize,
    pub buffer_size: usize,
}

impl IpcConfig {
    pub fn validate(&self) -> Result<(), MultivmError> {
        self.transport.validate()?;

        if self.message_timeout.as_secs() == 0 {
            return Err(MultivmError::Configuration {
                component: "ipc".to_string(),
                message: "message_timeout must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        if self.max_message_size == 0 || self.buffer_size == 0 {
            return Err(MultivmError::Configuration {
                component: "ipc".to_string(),
                message: "message and buffer sizes must be greater than 0".to_string(),
                validation_errors: None,
            });
        }

        Ok(())
    }
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            transport: IpcTransportConfig::default(),
            message_timeout: Duration::from_secs(30),
            max_message_size: 16 * 1024 * 1024,
            buffer_size: 1024 * 1024,
        }
    }
}

/// IPC transport configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IpcTransportConfig {
    UnixSocket { path: PathBuf },
    TcpSocket { host: String, port: u16 },
}

impl IpcTransportConfig {
    pub fn validate(&self) -> Result<(), MultivmError> {
        match self {
            IpcTransportConfig::TcpSocket { port, .. } => {
                if *port == 0 {
                    return Err(MultivmError::Configuration {
                        component: "ipc".to_string(),
                        message: "TCP port must be greater than 0".to_string(),
                        validation_errors: None,
                    });
                }
            }
            IpcTransportConfig::UnixSocket { path } => {
                if let Some(parent) = path.parent() {
                    if !parent.exists() {
                        return Err(MultivmError::Configuration {
                            component: "ipc".to_string(),
                            message: format!(
                                "Unix socket parent directory does not exist: {:?}",
                                parent
                            ),
                            validation_errors: None,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

impl Default for IpcTransportConfig {
    fn default() -> Self {
        #[cfg(unix)]
        {
            Self::UnixSocket {
                path: PathBuf::from("/tmp/multivm.sock"),
            }
        }
        #[cfg(not(unix))]
        {
            Self::TcpSocket {
                host: "127.0.0.1".to_string(),
                port: 9999,
            }
        }
    }
}
