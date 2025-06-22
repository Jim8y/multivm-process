pub mod config;
pub mod error;
pub mod ipc;
pub mod monitoring;
pub mod traits;
pub mod types;

// Re-export core types
pub use types::{
    BlockRequest, BlockResponse, BlockchainType, EngineState, HealthStatus, MessageId, ProcessId,
    ProcessingMetrics, ResourceLimits, RpcCall, RpcError, RpcResponse,
};

// Re-export traits
pub use traits::*;

// Re-export IPC
pub use ipc::*;

// Re-export config
pub use config::{
    EthereumConfig, IpcConfig, IpcTransportConfig, LogLevel, LoggingConfig, MultivmConfig,
    RpcConfig, SolanaConfig, SystemConfig,
};

// Re-export errors
pub use error::*;

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::len_zero)]
    fn test_version_info() {
        assert!(VERSION.len() > 0);
    }
}
