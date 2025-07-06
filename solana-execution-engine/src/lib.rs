//! Solana Execution Engine Library
//!
//! This library provides both mock and real implementations of the Solana execution engine
//! for the MultiVM system. It includes comprehensive Solana RPC integration, transaction
//! execution, and state management capabilities.

// Re-export multivm-common types for convenience
pub use multivm_common::{
    BlockchainType, EngineState, ExecutionEngine, HealthStatus, IpcCommand, IpcMessage,
    IpcResponse, MultivmError, MultivmResult, ProcessingMetrics,
};

pub mod config;
pub mod engine;
pub mod engine_rpc_client;
pub mod engine_rpc_server;
pub mod engine_rpc_tests;
pub mod engine_tests;

#[cfg(test)]
pub mod test_utils;
