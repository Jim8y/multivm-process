pub mod evm;
pub mod multivm;
pub mod svm;

pub use evm::EvmApiGateway;
pub use multivm::MultivmApiGateway;
pub use svm::{SvmApiGateway, SvmTransactionInfo};

use crate::cache::CacheLayer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

/// Common gateway configuration
#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub timeout: Duration,
    pub max_retries: u32,
    pub retry_delay: Duration,
}

/// Gateway response with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayResponse<T> {
    pub data: T,
    pub metadata: ResponseMetadata,
}

/// Response metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub cached: bool,
    pub response_time_ms: u64,
    pub vm_type: VmType,
    pub request_id: String,
}

/// Virtual machine type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VmType {
    Svm,
    Evm,
    MultiVm,
}

/// Common gateway traits
#[async_trait::async_trait]
pub trait ApiGateway: Send + Sync {
    type Config;
    type Error;

    /// Initialize the gateway
    async fn new(config: &Self::Config, cache: Arc<CacheLayer>) -> Result<Self, Self::Error>
    where
        Self: Sized;

    /// Check if the backend is healthy
    async fn health_check(&self) -> Result<bool, Self::Error>;

    /// Get gateway statistics
    async fn get_stats(&self) -> Result<GatewayStats, Self::Error>;
}

/// Gateway statistics
#[derive(Debug, Clone, Serialize)]
pub struct GatewayStats {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub average_response_time_ms: f64,
    pub uptime_seconds: u64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
        }
    }
}

impl<T> GatewayResponse<T> {
    /// Create a new gateway response
    pub fn new(data: T, vm_type: VmType, cached: bool, response_time_ms: u64) -> Self {
        Self {
            data,
            metadata: ResponseMetadata {
                cached,
                response_time_ms,
                vm_type,
                request_id: uuid::Uuid::new_v4().to_string(),
            },
        }
    }
}

impl std::fmt::Display for VmType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmType::Svm => write!(f, "SVM"),
            VmType::Evm => write!(f, "EVM"),
            VmType::MultiVm => write!(f, "MultiVM"),
        }
    }
}

impl Default for GatewayStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_requests: 0,
            failed_requests: 0,
            cache_hits: 0,
            cache_misses: 0,
            average_response_time_ms: 0.0,
            uptime_seconds: 0,
        }
    }
}
