//! Common Traits Module
//!
//! This module defines the core traits used throughout the MultiVM system,
//! providing standard interfaces for execution engines, monitoring, storage,
//! and inter-process communication.

pub mod events;
pub mod execution;
pub mod monitoring;
pub mod process;
pub mod rpc;
pub mod storage;
pub mod util;

// Re-export all traits for convenience
pub use events::*;
pub use execution::*;
pub use monitoring::*;
pub use process::*;
pub use rpc::*;
pub use storage::*;
pub use util::*;
