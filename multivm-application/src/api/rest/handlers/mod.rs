//! # REST API Handlers
//!
//! Contains all HTTP request handlers for the REST API endpoints.

pub mod accounts;
pub mod blocks;
pub mod dashboard;
pub mod evm;
pub mod execution_engines;
pub mod explorer;
pub mod health_detail;
pub mod multivm;
pub mod security;
pub mod svm;
pub mod system;
pub mod transaction_submission;
pub mod transactions;

use crate::api::ApiResponse;
use axum::{http::StatusCode, response::Json};

/// Start request timer for measuring response time
pub fn start_request_timer() -> std::time::Instant {
    std::time::Instant::now()
}

/// Calculate response time in milliseconds
pub fn calculate_response_time(start_time: std::time::Instant) -> u64 {
    start_time.elapsed().as_millis() as u64
}

/// Create a success response
pub fn success_response<T: serde::Serialize>(
    data: T,
    request_id: String,
    response_time: u64,
) -> Json<ApiResponse<T>> {
    Json(ApiResponse::success(data, request_id, response_time))
}

/// Health check endpoint
pub async fn health_check() -> Result<Json<ApiResponse<HealthStatus>>, StatusCode> {
    let request_id = crate::api::utils::generate_request_id();
    let start_time = std::time::Instant::now();

    let health_status = HealthStatus {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now(),
        version: crate::VERSION.to_string(),
    };

    let response_time = start_time.elapsed().as_millis() as u64;
    let response = ApiResponse::success(health_status, request_id, response_time);

    Ok(Json(response))
}

/// Health status response
#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthStatus {
    /// Service status
    pub status: String,

    /// Current timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Service version
    pub version: String,
}

/// Common error handling for handlers
pub async fn handle_error(
    error: crate::error::ApplicationError,
    request_id: String,
    response_time: u64,
) -> Json<ApiResponse<()>> {
    let api_response = match error {
        crate::error::ApplicationError::AuthenticationFailed { .. } => ApiResponse::error(
            "AUTHENTICATION_ERROR".to_string(),
            error.to_string(),
            request_id,
            response_time,
        ),
        crate::error::ApplicationError::AuthorizationDenied { .. } => ApiResponse::error(
            "AUTHORIZATION_ERROR".to_string(),
            error.to_string(),
            request_id,
            response_time,
        ),
        crate::error::ApplicationError::ValidationError { .. } => ApiResponse::error(
            "VALIDATION_ERROR".to_string(),
            error.to_string(),
            request_id,
            response_time,
        ),
        crate::error::ApplicationError::RateLimitExceeded { .. } => ApiResponse::error(
            "RATE_LIMIT_EXCEEDED".to_string(),
            error.to_string(),
            request_id,
            response_time,
        ),
        crate::error::ApplicationError::ResourceNotFound { .. } => ApiResponse::error(
            "NOT_FOUND".to_string(),
            error.to_string(),
            request_id,
            response_time,
        ),
        _ => ApiResponse::error(
            "INTERNAL_ERROR".to_string(),
            "An internal error occurred".to_string(),
            request_id,
            response_time,
        ),
    };

    Json(api_response)
}

/// Common response wrapper for error operations
pub fn error_response(
    code: &str,
    message: &str,
    request_id: String,
    response_time: u64,
) -> Json<ApiResponse<()>> {
    Json(ApiResponse::error(
        code.to_string(),
        message.to_string(),
        request_id,
        response_time,
    ))
}

/// Generic error response that can be converted to any type
pub fn error_response_typed<T>(
    code: &str,
    _message: &str,
    _request_id: String,
    _response_time: u64,
) -> Result<Json<ApiResponse<T>>, StatusCode> {
    Err(match code {
        "NOT_FOUND" => StatusCode::NOT_FOUND,
        "VALIDATION_ERROR" => StatusCode::BAD_REQUEST,
        "AUTHENTICATION_ERROR" => StatusCode::UNAUTHORIZED,
        "AUTHORIZATION_ERROR" => StatusCode::FORBIDDEN,
        "RATE_LIMIT_EXCEEDED" => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })
}
