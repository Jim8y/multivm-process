//! Reth Execution Engine Library
//!
//! This library provides both mock and real implementations of the Reth execution engine
//! for the MultiVM system. It includes comprehensive Engine API integration, transaction
//! execution, and state management capabilities.

pub mod engine;
pub mod engine_api;
pub mod ipc_client;
pub mod real_engine;
pub mod real_engine_utils;
pub mod rpc_client;
pub mod rpc_server;

