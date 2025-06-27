//! RPC Traits
//!
//! This module defines traits for implementing RPC (Remote Procedure Call) handlers
//! and servers within the MultiVM system. These traits provide a common interface
//! for handling JSON-RPC requests across different components.

use crate::MultivmError;
use async_trait::async_trait;

/// Trait for RPC handlers
#[async_trait]
pub trait RpcHandler: Send + Sync {
    /// Handle an RPC method call
    async fn handle_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, MultivmError>;

    /// Get supported RPC methods
    fn supported_methods(&self) -> Vec<String>;

    /// Check if a method is supported
    fn supports_method(&self, method: &str) -> bool {
        self.supported_methods().contains(&method.to_string())
    }
}
