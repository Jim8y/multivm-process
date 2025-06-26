//! # API Module
//!
//! This module provides all external API interfaces for the MultiVM Application Layer.
//! It includes REST API, GraphQL, and WebSocket servers that enable client applications
//! to interact with both SVM and EVM execution environments.

pub mod graphql;
pub mod rest;
pub mod websocket;

// Re-export main types
pub use graphql::{GraphQLConfig, GraphQLServer};
pub use rest::{RestApiConfig, RestApiServer};
pub use websocket::{WebSocketConfig, WebSocketServer};

use crate::error::ApplicationResult;
use crate::ApplicationState;
use std::sync::Arc;

/// Common API response format
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiResponse<T> {
    /// Response data
    pub data: Option<T>,

    /// Error information
    pub error: Option<ApiError>,

    /// Request metadata
    pub metadata: ApiMetadata,
}

/// API error information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiError {
    /// Error code
    pub code: String,

    /// Error message
    pub message: String,

    /// Additional error details
    pub details: Option<serde_json::Value>,
}

/// API request metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ApiMetadata {
    /// Request ID for tracing
    pub request_id: String,

    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Response time in milliseconds
    pub response_time_ms: u64,

    /// API version
    pub api_version: String,
}

impl<T> ApiResponse<T> {
    /// Create a successful response
    pub fn success(data: T, request_id: String, response_time_ms: u64) -> Self {
        Self {
            data: Some(data),
            error: None,
            metadata: ApiMetadata {
                request_id,
                timestamp: chrono::Utc::now(),
                response_time_ms,
                api_version: "v1".to_string(),
            },
        }
    }

    /// Create an error response
    pub fn error(code: String, message: String, request_id: String, response_time_ms: u64) -> Self {
        Self {
            data: None,
            error: Some(ApiError {
                code,
                message,
                details: None,
            }),
            metadata: ApiMetadata {
                request_id,
                timestamp: chrono::Utc::now(),
                response_time_ms,
                api_version: "v1".to_string(),
            },
        }
    }
}

/// Unified API server that manages all API interfaces
pub struct UnifiedApiServer {
    #[allow(dead_code)]
    state: Arc<ApplicationState>,
}

impl UnifiedApiServer {
    /// Create a new unified API server
    pub fn new(state: Arc<ApplicationState>) -> Self {
        Self { state }
    }

    /// Start all API servers
    pub async fn start_all(&self) -> ApplicationResult<()> {
        // This would coordinate the startup of all API servers
        // For now, individual servers are started by ApplicationServer
        Ok(())
    }
}

/// Common utilities for API modules
pub mod utils {
    use uuid::Uuid;

    /// Generate a new request ID
    pub fn generate_request_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Extract request ID from headers
    pub fn extract_request_id(headers: &axum::http::HeaderMap) -> String {
        headers
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(generate_request_id)
    }

    /// Create standard error response
    pub fn create_error_response(
        code: &str,
        message: &str,
        request_id: String,
        response_time_ms: u64,
    ) -> super::ApiResponse<()> {
        super::ApiResponse::error(
            code.to_string(),
            message.to_string(),
            request_id,
            response_time_ms,
        )
    }
}
