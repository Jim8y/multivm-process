use crate::MultivmError;
use async_trait::async_trait;

/// Trait for state storage backends
#[async_trait]
pub trait StateStorage: Send + Sync {
    /// Store state data
    async fn store_state(&self, key: &[u8], value: &[u8]) -> Result<(), MultivmError>;

    /// Retrieve state data
    async fn get_state(&self, key: &[u8]) -> Result<Option<Vec<u8>>, MultivmError>;

    /// Delete state data
    async fn delete_state(&self, key: &[u8]) -> Result<(), MultivmError>;

    /// Get the current state root hash
    async fn get_state_root(&self) -> Result<Vec<u8>, MultivmError>;

    /// Create a checkpoint for rollback purposes
    async fn create_checkpoint(&self) -> Result<String, MultivmError>;

    /// Rollback to a previous checkpoint
    async fn rollback_to_checkpoint(&self, checkpoint_id: &str) -> Result<(), MultivmError>;

    /// Commit all pending changes
    async fn commit(&self) -> Result<(), MultivmError>;
}
