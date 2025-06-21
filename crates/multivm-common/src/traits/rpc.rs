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
