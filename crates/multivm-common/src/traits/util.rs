use crate::MultivmError;
use async_trait::async_trait;
use std::time::Duration;

/// Helper trait for async initialization
#[async_trait]
pub trait AsyncInit {
    async fn async_init(&mut self) -> Result<(), MultivmError>;
}

/// Helper trait for graceful shutdown
#[async_trait]
pub trait GracefulShutdown {
    async fn graceful_shutdown(&mut self, timeout: Duration) -> Result<(), MultivmError>;
}
