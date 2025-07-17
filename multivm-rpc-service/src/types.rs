//! Common types for the MultiVM RPC service

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::SystemTime;

/// JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

/// JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

/// JSON-RPC error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Request metadata for tracking and routing
#[derive(Debug, Clone)]
pub struct RequestMetadata {
    /// Unique request ID
    pub request_id: String,
    /// Client identifier (IP, API key, etc.)
    pub client_id: String,
    /// Target VM type
    pub vm_type: VmType,
    /// Request timestamp
    pub timestamp: SystemTime,
    /// Authentication info
    pub auth_info: Option<AuthInfo>,
    /// Forwarded headers
    pub headers: HashMap<String, String>,
}

/// VM type for request routing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VmType {
    Ethereum,
    Solana,
    MultiVm,
}

impl std::fmt::Display for VmType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmType::Ethereum => write!(f, "ethereum"),
            VmType::Solana => write!(f, "solana"),
            VmType::MultiVm => write!(f, "multivm"),
        }
    }
}

/// Authentication information
#[derive(Debug, Clone)]
pub struct AuthInfo {
    /// API key used
    pub api_key: Option<String>,
    /// JWT token payload (for future use)
    pub jwt_payload: Option<Value>,
    /// Authenticated user/client name
    pub client_name: Option<String>,
    /// Allowed methods for this auth
    pub allowed_methods: Vec<String>,
}

/// Node health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    /// Node name
    pub name: String,
    /// VM type
    pub vm_type: VmType,
    /// Health status
    pub status: HealthStatus,
    /// Response time in milliseconds
    pub response_time_ms: u64,
    /// Last successful health check
    pub last_success: SystemTime,
    /// Error message if unhealthy
    pub error_message: Option<String>,
    /// Node version/info
    pub version_info: Option<String>,
}

/// Health status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

/// Cache entry wrapper
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// Cached response data
    pub data: Value,
    /// Cache timestamp
    pub cached_at: SystemTime,
    /// Time-to-live
    pub ttl: std::time::Duration,
    /// Request hash for validation
    pub request_hash: String,
}

impl CacheEntry {
    /// Check if cache entry is expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now()
            .duration_since(self.cached_at)
            .unwrap_or_default()
            > self.ttl
    }
}

/// RPC method information
#[derive(Debug, Clone)]
pub struct MethodInfo {
    /// Method name
    pub name: String,
    /// Target VM type
    pub vm_type: VmType,
    /// Whether method is cacheable
    pub cacheable: bool,
    /// Cache TTL override
    pub cache_ttl: Option<std::time::Duration>,
    /// Whether method is read-only
    pub read_only: bool,
    /// Required authentication level
    pub auth_required: bool,
}

/// MultiVM-specific request types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MultiVmRequest {
    /// Cross-chain balance query
    CrossChainBalance {
        address: String,
        vm_types: Vec<VmType>,
    },
    /// Account binding information
    AccountBinding {
        multivm_account: String,
    },
    /// Cross-VM transaction submission
    CrossVmTransaction {
        from_vm: VmType,
        to_vm: VmType,
        transaction_data: Value,
    },
    /// Network statistics
    NetworkStats {
        include_details: bool,
    },
}

/// MultiVM-specific response types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MultiVmResponse {
    /// Cross-chain balance result
    CrossChainBalance {
        balances: HashMap<VmType, String>,
        total_usd: Option<String>,
    },
    /// Account binding result
    AccountBinding {
        ethereum_account: Option<String>,
        solana_account: Option<String>,
        binding_timestamp: SystemTime,
    },
    /// Cross-VM transaction result
    CrossVmTransaction {
        transaction_id: String,
        status: TransactionStatus,
        confirmations: HashMap<VmType, u64>,
    },
    /// Network statistics result
    NetworkStats {
        ethereum: NetworkStatsDetail,
        solana: NetworkStatsDetail,
        cross_vm: CrossVmStats,
    },
}

/// Transaction status for cross-VM operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Rejected,
}

/// Network statistics detail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatsDetail {
    /// Current block height/slot
    pub height: u64,
    /// Transactions per second
    pub tps: f64,
    /// Average block time
    pub avg_block_time: f64,
    /// Number of active nodes
    pub active_nodes: u32,
    /// Network health score (0-100)
    pub health_score: u8,
}

/// Cross-VM specific statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossVmStats {
    /// Total cross-VM transactions
    pub total_cross_vm_txs: u64,
    /// Cross-VM transactions in last 24h
    pub cross_vm_txs_24h: u64,
    /// Number of bound accounts
    pub bound_accounts: u64,
    /// Cross-VM success rate
    pub success_rate: f64,
}

/// Request routing information
#[derive(Debug, Clone)]
pub struct RouteInfo {
    /// Target VM type
    pub vm_type: VmType,
    /// Preferred node name
    pub preferred_node: Option<String>,
    /// Load balancing strategy
    pub load_balance_strategy: LoadBalanceStrategy,
    /// Retry policy
    pub retry_policy: RetryPolicy,
}

/// Load balancing strategies
#[derive(Debug, Clone, Copy)]
pub enum LoadBalanceStrategy {
    RoundRobin,
    Priority,
    LeastConnections,
    ResponseTime,
}

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum retry attempts
    pub max_attempts: u32,
    /// Base delay between retries
    pub base_delay: std::time::Duration,
    /// Maximum delay between retries
    pub max_delay: std::time::Duration,
    /// Retry on specific error types
    pub retry_conditions: Vec<RetryCondition>,
}

/// Conditions that trigger retries
#[derive(Debug, Clone)]
pub enum RetryCondition {
    NetworkError,
    Timeout,
    ServerError,
    RateLimited,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: std::time::Duration::from_millis(100),
            max_delay: std::time::Duration::from_secs(5),
            retry_conditions: vec![
                RetryCondition::NetworkError,
                RetryCondition::Timeout,
                RetryCondition::ServerError,
            ],
        }
    }
}

/// Utility functions
impl JsonRpcRequest {
    /// Determine VM type from method name
    pub fn vm_type(&self) -> VmType {
        if self.method.starts_with("multivm_") {
            VmType::MultiVm
        } else if self.method.starts_with("eth_") || 
                  self.method.starts_with("debug_") || 
                  self.method.starts_with("trace_") ||
                  self.method.starts_with("web3_") {
            VmType::Ethereum
        } else {
            // Solana methods don't have a common prefix, so default to Solana
            // for non-Ethereum, non-MultiVM methods
            VmType::Solana
        }
    }

    /// Check if method is read-only
    pub fn is_read_only(&self) -> bool {
        // Most methods are read-only except for transaction submission
        !matches!(self.method.as_str(),
            "eth_sendTransaction" | 
            "eth_sendRawTransaction" |
            "sendTransaction" |
            "multivm_sendCrossVmTransaction"
        )
    }

    /// Generate cache key
    pub fn cache_key(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.method.hash(&mut hasher);
        if let Some(ref params) = self.params {
            params.to_string().hash(&mut hasher);
        }
        format!("rpc:{}:{:x}", self.method, hasher.finish())
    }
}

impl JsonRpcResponse {
    /// Create success response
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create error response
    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_type_detection() {
        let eth_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_blockNumber".to_string(),
            params: None,
            id: Some(Value::Number(1.into())),
        };
        assert_eq!(eth_request.vm_type(), VmType::Ethereum);

        let multivm_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "multivm_getBalance".to_string(),
            params: None,
            id: Some(Value::Number(2.into())),
        };
        assert_eq!(multivm_request.vm_type(), VmType::MultiVm);

        let solana_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "getBalance".to_string(),
            params: None,
            id: Some(Value::Number(3.into())),
        };
        assert_eq!(solana_request.vm_type(), VmType::Solana);
    }

    #[test]
    fn test_cache_entry_expiration() {
        let entry = CacheEntry {
            data: Value::String("test".to_string()),
            cached_at: SystemTime::now() - std::time::Duration::from_secs(10),
            ttl: std::time::Duration::from_secs(5),
            request_hash: "test".to_string(),
        };
        assert!(entry.is_expired());

        let entry = CacheEntry {
            data: Value::String("test".to_string()),
            cached_at: SystemTime::now(),
            ttl: std::time::Duration::from_secs(60),
            request_hash: "test".to_string(),
        };
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_read_only_detection() {
        let read_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_getBalance".to_string(),
            params: None,
            id: None,
        };
        assert!(read_request.is_read_only());

        let write_request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: "eth_sendTransaction".to_string(),
            params: None,
            id: None,
        };
        assert!(!write_request.is_read_only());
    }
}