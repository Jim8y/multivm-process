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
