use serde::{Deserialize, Serialize};

/// RPC method call information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcCall {
    pub method: String,
    pub params: serde_json::Value,
    pub id: serde_json::Value,
}

/// RPC response information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub result: Option<serde_json::Value>,
    pub error: Option<RpcError>,
    pub id: serde_json::Value,
}

/// RPC error information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

/// RPC server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: usize,
    pub timeout_seconds: u64,
    pub cors_origins: Vec<String>,
    pub rate_limit_requests_per_minute: Option<u32>,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8545,
            max_connections: 100,
            timeout_seconds: 30,
            cors_origins: vec!["*".to_string()],
            rate_limit_requests_per_minute: Some(1000),
        }
    }
}
