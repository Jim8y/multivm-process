//! IPC module for MultiVM process communication
//!
//! This module provides production-ready IPC functionality including:
//! - Connection pooling and management
//! - Authentication and authorization
//! - Rate limiting and security
//! - Health monitoring and automatic recovery

pub mod connection_manager;

pub use connection_manager::*;
