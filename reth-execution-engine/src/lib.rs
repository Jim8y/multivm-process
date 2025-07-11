//! Reth Execution Engine Library
//!
//! This library provides both mock and real implementations of the Reth execution engine
//! for the MultiVM system. It includes comprehensive Engine API integration, transaction
//! execution, and state management capabilities.

#![allow(deprecated)]
#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

// Re-export multivm-common types
pub use multivm_common::{
    BlockchainType, EngineState, ExecutionEngine, HealthStatus, IpcCommand, IpcMessage,
    IpcResponse, MultivmError, MultivmResult, ProcessingMetrics,
};

// Core engine modules (always available)
pub mod engine;
pub mod engine_api;
pub mod ipc_client;
pub mod rpc_client;
pub mod rpc_server;

// Real reth integration modules (only when real-node feature is enabled)
#[cfg(feature = "real-node")]
pub mod real_engine;
#[cfg(feature = "real-node")]
pub mod real_engine_utils;

// Feature-specific exports
#[cfg(feature = "real-node")]
pub use real_engine::RealRethEngine;

// Re-export Alloy/Reth native types for proper hash calculation
pub use alloy_primitives::{B256, U256, Address, Bytes, Bloom, FixedBytes};

// Re-export commonly used engine types
pub use engine::{RethBlock, RethExecutionEngine, RethExecutionResult};



// Test modules
#[cfg(test)]
mod ipc_client_tests;
