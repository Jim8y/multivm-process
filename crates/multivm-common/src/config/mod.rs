pub mod blockchain;
pub mod ipc;
pub mod logging;
pub mod main;
pub mod rpc;
pub mod system;

use serde::{Deserialize, Serialize};

// Re-export all config types for convenience
pub use blockchain::*;
pub use ipc::*;
pub use logging::*;
pub use main::*;
pub use rpc::RpcConfig;
pub use system::*;

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
