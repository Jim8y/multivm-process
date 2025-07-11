//! RPC implementation for MultiVM system

pub mod client;
pub mod server;
pub mod proto;
pub mod interceptors;

#[cfg(test)]
pub mod test_client;

pub use client::{RpcClient, RpcClientConfig};
pub use server::{RpcServer, RpcServerConfig};

// Re-export generated protobuf types
pub use proto::multivm::*;
