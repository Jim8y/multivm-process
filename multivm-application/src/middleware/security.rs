//! Security middleware for adding security headers and protections

use std::time::Duration;
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer};

/// Create a timeout layer with default 30 second timeout
pub fn default_timeout_layer() -> TimeoutLayer {
    TimeoutLayer::new(Duration::from_secs(30))
}

/// Create a timeout layer with custom duration
pub fn timeout_layer(duration: Duration) -> TimeoutLayer {
    TimeoutLayer::new(duration)
}

/// Create a request body size limit layer with default 10MB limit
pub fn default_body_limit_layer() -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(10 * 1024 * 1024) // 10MB
}

/// Create a request body size limit layer with custom limit
pub fn body_limit_layer(max_bytes: u64) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(max_bytes as usize)
}
