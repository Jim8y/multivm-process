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
pub mod engine_tests;

