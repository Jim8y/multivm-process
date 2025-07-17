//! # MultiVM RPC Service
//!
//! A unified RPC gateway for the MultiVM platform that provides:
//! - Ethereum JSON-RPC compatibility via Reth relay
//! - Solana JSON-RPC compatibility via Solana node relay
//! - MultiVM-specific APIs for cross-chain operations
//! - Request routing, caching, and rate limiting
//! - Account mapping integration
//!
//! ## Architecture
//!
//! ```text
//! Client → MultiVM RPC Gateway → {Reth Node, Solana Node}
//!                ↓
//!         Account Mapping Layer
//!                ↓
//!            Cache & Metrics
//! ```

pub mod config;
pub mod error;
pub mod proxy;
pub mod relay;
pub mod server;
pub mod types;
pub mod cache;
pub mod middleware;
pub mod multivm_api;
pub mod rpc_router;
pub mod logging;
pub mod health;
pub mod performance;
pub mod observability;

pub use config::RpcServiceConfig;
pub use error::{RpcError, RpcResult};
pub use server::RpcServer;
pub use types::*;
pub use logging::{LoggingConfig, LogTarget};
pub use health::{HealthChecker, HealthConfig, SystemHealth};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Re-export common types for convenience
pub use multivm_common::{VmType, MultivmError, MultivmResult};
pub use multivm_account_mapping::{AccountAddress, MultivmAccountId};

/// RPC service initialization
pub async fn init_service(config: RpcServiceConfig) -> RpcResult<RpcServer> {
    RpcServer::new(config).await
}
