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

// Complex engine only available in mock mode for testing
pub mod engine;
pub mod ipc_client;
pub mod rpc_server;

pub mod config;
pub mod real_engine;
pub mod real_engine_utils;
pub mod rpc_client;
pub mod validator_api;
