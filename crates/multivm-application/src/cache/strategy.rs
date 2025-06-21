//! Cache strategy implementations

use crate::error::ApplicationResult;
use serde::{Deserialize, Serialize};

/// Cache strategy trait
#[async_trait::async_trait]
pub trait CacheStrategy: Send + Sync {
    /// Get value from cache
    async fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> ApplicationResult<Option<T>>;

    /// Set value in cache
    async fn set<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Option<std::time::Duration>,
    ) -> ApplicationResult<()>;

    /// Delete value from cache
    async fn delete(&self, key: &str) -> ApplicationResult<()>;

    /// Clear all values from cache
    async fn clear(&self) -> ApplicationResult<()>;

    /// Check if key exists
    async fn exists(&self, key: &str) -> ApplicationResult<bool>;

    /// Get cache size
    async fn size(&self) -> ApplicationResult<usize>;
}
