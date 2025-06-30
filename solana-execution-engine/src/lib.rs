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

pub mod simple_engine;

// Complex engine only available in mock mode for testing
#[cfg(feature = "mock")]
pub mod engine;
#[cfg(feature = "mock")]
pub mod ipc_client;
#[cfg(feature = "mock")]
pub mod rpc_server;

#[cfg(feature = "real-validator")]
pub mod real_engine;
#[cfg(feature = "real-validator")]
pub mod real_engine_utils;
#[cfg(feature = "real-validator")]
pub mod rpc_client;
#[cfg(feature = "real-validator")]
pub mod validator_api;
