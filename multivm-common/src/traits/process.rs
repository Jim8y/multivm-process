use crate::{HealthStatus, IpcCommand, IpcResponse, MultivmConfig, MultivmError};
use async_trait::async_trait;

/// Trait for process lifecycle management
#[async_trait]
pub trait ProcessManager: Send + Sync {
    /// Start a new process
    async fn start_process(
        &self,
        process_id: crate::ProcessId,
        config: Vec<u8>,
    ) -> Result<(), MultivmError>;

    /// Stop a running process
    async fn stop_process(
        &self,
        process_id: crate::ProcessId,
        graceful: bool,
    ) -> Result<(), MultivmError>;

    /// Restart a process
    async fn restart_process(&self, process_id: crate::ProcessId) -> Result<(), MultivmError>;

    /// Check if a process is running
    async fn is_process_running(&self, process_id: crate::ProcessId) -> Result<bool, MultivmError>;

    /// Get process health status
    async fn get_process_health(
        &self,
        process_id: crate::ProcessId,
    ) -> Result<HealthStatus, MultivmError>;

    /// Send a command to a process
    async fn send_command(
        &self,
        process_id: crate::ProcessId,
        command: IpcCommand,
    ) -> Result<IpcResponse, MultivmError>;
}

/// Trait for configuration management
pub trait ConfigManager: Send + Sync {
    /// Load configuration from source
    fn load_config(&self) -> Result<MultivmConfig, MultivmError>;

    /// Save configuration to source
    fn save_config(&self, config: &MultivmConfig) -> Result<(), MultivmError>;

    /// Validate configuration
    fn validate_config(&self, config: &MultivmConfig) -> Result<(), MultivmError>;

    /// Get configuration for a specific component
    fn get_component_config(&self, component: &str) -> Result<serde_json::Value, MultivmError>;

    /// Update configuration for a specific component
    fn update_component_config(
        &self,
        component: &str,
        config: serde_json::Value,
    ) -> Result<(), MultivmError>;
}
