pub mod blockchain;
pub mod ipc;
pub mod logging;
pub mod main;
pub mod rpc;
pub mod system;
pub mod unified;

use serde::{Deserialize, Serialize};

// Re-export all config types for convenience
pub use blockchain::*;
pub use main::*;
pub use rpc::RpcConfig;

// Legacy config types (to maintain backward compatibility)
pub use ipc::IpcConfig as LegacyIpcConfig;
pub use ipc::IpcTransportConfig;
pub use logging::LogLevel;
pub use logging::LoggingConfig as LegacyLoggingConfig;
pub use system::SystemConfig as LegacySystemConfig;

// Unified config types (new schema)
pub use unified::*;

/// Blockchain configuration for a specific VM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainConfig {
    pub chain_id: String,
    pub vm_type: VmType,
    pub rpc_endpoints: Vec<String>,
    pub ws_endpoints: Vec<String>,
    pub network_id: u64,
    pub enable_p2p: bool,
    pub data_directory: Option<String>,
}

/// VM type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VmType {
    Evm,
    Svm,
}
