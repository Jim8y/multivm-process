//! MultiVM Common Library
//!
//! This library provides unified functionality, types, and utilities shared
//! across all MultiVM components. It eliminates duplication and provides
//! consistent patterns throughout the system.

pub mod config;
pub mod error;
pub mod ipc;
pub mod managers;
// pub mod monitoring;  // Temporarily commented out - missing file
pub mod traits;
pub mod types;

// Re-export core types for convenience
pub use config::{MultivmConfig, VmType};
pub use error::{MultivmError, MultivmResult};

// Re-export types (excluding rpc to avoid conflict)
pub use types::{
    account::AccountBindingInfo, core::*, health::*, metrics::*, requests::*, resources::*,
};
// Re-export rpc types explicitly to avoid conflicts
pub use traits::rpc as traits_rpc;
pub use types::rpc as types_rpc;

// Re-export traits (excluding rpc to avoid conflict)
pub use traits::{events::*, execution::*, monitoring::*, process::*, storage::*, util::*};

// Re-export IPC types
pub use ipc::messages::{IpcCommand, IpcMessage, IpcResponse};

// Re-export manager types
pub use managers::{BaseManager, Manager, ManagerState, ManagerStats};

// Re-export monitoring types
// pub use monitoring::MonitoringService;  // Temporarily commented out

// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
