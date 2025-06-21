//! # REST API Middleware
//!
//! Middleware components for request processing including authentication,
//! rate limiting, metrics collection, and request tracking.

use crate::ApplicationState;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;
use uuid::Uuid;

/// Request ID middleware - adds unique request ID to each request
pub async fn request_id_middleware(
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Check if request already has an ID
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Add the request ID to request headers
    request.headers_mut().insert(
        "x-request-id",
        request_id
            .parse()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );

    let mut response = next.run(request).await;

    // Add request ID to response headers
    response.headers_mut().insert(
        "x-request-id",
        request_id
            .parse()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    );

    Ok(response)
}

/// Authentication middleware
pub async fn auth_middleware(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Skip authentication for health check and public endpoints
    let path = request.uri().path();
    if should_skip_auth(path) {
        return Ok(next.run(request).await);
    }

    // Check for API key
    if let Some(api_key) = headers.get("x-api-key") {
        let api_key_str = api_key.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;

        let auth_manager = state.auth_manager.read().await;
        match auth_manager.validate_api_key(api_key_str).await {
            Ok(_) => return Ok(next.run(request).await),
            Err(_) => return Err(StatusCode::UNAUTHORIZED),
        }
    }

    // Check for Bearer token
    if let Some(auth_header) = headers.get("authorization") {
        let auth_str = auth_header.to_str().map_err(|_| StatusCode::BAD_REQUEST)?;

        if let Some(token) = auth_str.strip_prefix("Bearer ") {
            let auth_manager = state.auth_manager.read().await;
            match auth_manager.validate_jwt_token(token).await {
                Ok(_) => return Ok(next.run(request).await),
                Err(_) => return Err(StatusCode::UNAUTHORIZED),
            }
        }
    }

    Err(StatusCode::UNAUTHORIZED)
}

/// Rate limiting middleware
pub async fn rate_limit_middleware(
    State(state): State<Arc<ApplicationState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract client identifier (IP, API key, or user ID)
    let client_id = extract_client_identifier(&headers, &request);

    // Check rate limit
    match check_rate_limit(&state, &client_id).await {
        Ok(true) => Ok(next.run(request).await),
        Ok(false) => Err(StatusCode::TOO_MANY_REQUESTS),
        Err(_) => {
            // If rate limiting fails, allow the request to proceed
            // This ensures availability over strict rate limiting
            Ok(next.run(request).await)
        }
    }
}

/// Metrics collection middleware
pub async fn metrics_middleware(
    State(state): State<Arc<ApplicationState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let start_time = std::time::Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();

    let response = next.run(request).await;

    let duration = start_time.elapsed();
    let status = response.status();

    // Record metrics
    tokio::spawn({
        let state = state.clone();
        let method_str = method.to_string();
        async move {
            let _ = state
                .monitoring
                .record_request_metrics(
                    &method_str,
                    &path,
                    status.as_u16(),
                    duration.as_millis() as u64,
                )
                .await;
        }
    });

    Ok(response)
}

// Helper functions

/// Check if authentication should be skipped for this path
fn should_skip_auth(path: &str) -> bool {
    matches!(
        path,
        "/health" | "/api/v1/system/health" | "/api/v1/system/info"
    )
}

/// Extract client identifier for rate limiting
fn extract_client_identifier(headers: &HeaderMap, request: &Request) -> String {
    // Try API key first
    if let Some(api_key) = headers.get("x-api-key") {
        if let Ok(key_str) = api_key.to_str() {
            return format!("api_key:{}", key_str);
        }
    }

    // Try to get IP from X-Forwarded-For or X-Real-IP
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(ip_str) = forwarded.to_str() {
            // Take the first IP in the chain
            if let Some(first_ip) = ip_str.split(',').next() {
                return format!("ip:{}", first_ip.trim());
            }
        }
    }

    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(ip_str) = real_ip.to_str() {
            return format!("ip:{}", ip_str);
        }
    }

    // Fall back to connection info using axum extensions
    if let Some(connect_info) = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
    {
        return format!("ip:{}", connect_info.0.ip());
    }

    // Ultimate fallback for unknown clients
    format!("ip:127.0.0.1")
}

/// Check rate limit for a client
async fn check_rate_limit(
    state: &Arc<ApplicationState>,
    client_id: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    // Get rate limiting configuration
    let rate_limit_config = &state.config.rate_limiting;

    if !rate_limit_config.enabled {
        return Ok(true);
    }

    // Create cache key for this client
    let cache_key = format!("rate_limit:{}", client_id);

    // Check current request count
    match state.cache.get::<u32>(&cache_key).await {
        Ok(Some(current_count)) => {
            if current_count >= rate_limit_config.default_rpm {
                tracing::warn!("Rate limit exceeded for client: {}", client_id);
                return Ok(false);
            }

            // Increment counter
            let new_count = current_count + 1;
            let _ = state
                .cache
                .set(&cache_key, &new_count, std::time::Duration::from_secs(60))
                .await;
            Ok(true)
        }
        Ok(None) => {
            // First request from this client in the current window
            let _ = state
                .cache
                .set(&cache_key, &1u32, std::time::Duration::from_secs(60))
                .await;
            Ok(true)
        }
        Err(e) => {
            tracing::error!("Rate limit check failed for {}: {}", client_id, e);
            // Fail open - allow request if rate limiting system is down
            Ok(true)
        }
    }
}

/// CORS middleware configuration
pub fn cors_headers() -> [(String, String); 4] {
    [
        ("Access-Control-Allow-Origin".to_string(), "*".to_string()),
        (
            "Access-Control-Allow-Methods".to_string(),
            "GET, POST, PUT, DELETE, OPTIONS".to_string(),
        ),
        (
            "Access-Control-Allow-Headers".to_string(),
            "Content-Type, Authorization, X-API-Key, X-Request-ID".to_string(),
        ),
        ("Access-Control-Max-Age".to_string(), "86400".to_string()),
    ]
}
