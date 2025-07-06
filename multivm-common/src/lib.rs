//! MultiVM Common Library
//!
//! This library provides unified functionality, types, and utilities shared
//! across all MultiVM components. It eliminates duplication and provides
//! consistent patterns throughout the system.
//!
//! # Examples
//!
//! ```rust
//! use multivm_common::{MultivmError, MultivmResult, VmType};
//!
//! // Create a VM type
//! let vm_type = VmType::Svm;
//! assert_eq!(format!("{:?}", vm_type), "Svm");
//!
//! // Work with results
//! let result: MultivmResult<i32> = Ok(42);
//! assert!(result.is_ok());
//! ```
//!
//! # Configuration
//!
//! ```rust
//! use multivm_common::config::VmType;
//!
//! let solana_vm = VmType::Svm;
//! let ethereum_vm = VmType::Evm;
//!
//! assert_ne!(solana_vm, ethereum_vm);
//! ```

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

#[cfg(test)]
mod tests;
