//! Middleware for the RPC service

use crate::{
    config::{ApiKeyConfig, AuthConfig, RateLimitConfig},
    error::{RpcError, RpcResult},
    types::{AuthInfo, RequestMetadata},
};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
// Note: Using simplified rate limiting for now
use std::{
    collections::HashMap,
    net::IpAddr,
    sync::Arc,
    time::{Duration, SystemTime},
};
use tracing::{debug, warn};

/// Authentication middleware
pub async fn auth_middleware(
    State(auth_config): State<Arc<AuthConfig>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !auth_config.enabled {
        return Ok(next.run(request).await);
    }

    // Extract API key from headers
    let api_key = headers
        .get("x-api-key")
        .or_else(|| headers.get("authorization"))
        .and_then(|h| h.to_str().ok())
        .map(|s: &str| {
            if s.starts_with("Bearer ") {
                &s[7..]
            } else {
                s
            }
        });

    let api_key = match api_key {
        Some(key) => key,
        None => {
            warn!("Missing API key in request");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Validate API key
    let key_config = match auth_config.api_keys.get(api_key) {
        Some(config) if config.enabled => config,
        Some(_) => {
            warn!("Disabled API key used: {}", &api_key[..8.min(api_key.len())]);
            return Err(StatusCode::FORBIDDEN);
        }
        None => {
            warn!("Invalid API key used: {}", &api_key[..8.min(api_key.len())]);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Create auth info
    let auth_info = AuthInfo {
        api_key: Some(api_key.to_string()),
        jwt_payload: None,
        client_name: Some(key_config.name.clone()),
        allowed_methods: key_config.allowed_methods.clone(),
    };

    // Add auth info to request extensions
    request.extensions_mut().insert(auth_info);

    debug!("Authenticated request for client: {}", key_config.name);
    Ok(next.run(request).await)
}

/// Rate limiting middleware (simplified implementation)
pub struct RateLimitMiddleware {
    ip_counters: Arc<parking_lot::RwLock<HashMap<IpAddr, (u32, SystemTime)>>>,
    api_key_counters: Arc<parking_lot::RwLock<HashMap<String, (u32, SystemTime)>>>,
    config: RateLimitConfig,
}

impl RateLimitMiddleware {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            ip_counters: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            api_key_counters: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            config,
        }
    }

    pub async fn middleware(
        &self,
        client_ip: Option<IpAddr>,
        auth_info: Option<&AuthInfo>,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        if !self.config.enabled {
            return Ok(next.run(request).await);
        }

        // Check whitelist
        if let Some(ref auth) = auth_info {
            if let Some(ref api_key) = auth.api_key {
                if self.config.whitelist.contains(api_key) {
                    return Ok(next.run(request).await);
                }
            }
        }

        if let Some(ip) = client_ip {
            let ip_str = ip.to_string();
            if self.config.whitelist.contains(&ip_str) {
                return Ok(next.run(request).await);
            }
        }

        // Rate limit by API key if available
        if self.config.by_api_key {
            if let Some(ref auth) = auth_info {
                if let Some(ref api_key) = auth.api_key {
                    if !self.check_api_key_rate_limit(api_key).await {
                        warn!("Rate limit exceeded for API key: {}", &api_key[..8.min(api_key.len())]);
                        return Err(StatusCode::TOO_MANY_REQUESTS);
                    }
                }
            }
        }

        // Rate limit by IP if enabled and no API key rate limiting applied
        if self.config.by_ip && auth_info.is_none() {
            if let Some(ip) = client_ip {
                if !self.check_ip_rate_limit(ip).await {
                    warn!("Rate limit exceeded for IP: {}", ip);
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
        }

        Ok(next.run(request).await)
    }

    async fn check_ip_rate_limit(&self, ip: IpAddr) -> bool {
        if !self.config.enabled {
            return true;
        }

        let mut counters = self.ip_counters.write();
        let now = SystemTime::now();
        
        let (count, last_reset) = counters.entry(ip).or_insert((0, now));
        
        // Reset counter if more than 1 second has passed
        if now.duration_since(*last_reset).unwrap_or_default().as_secs() >= 1 {
            *count = 1;
            *last_reset = now;
            true
        } else if *count < self.config.requests_per_second {
            *count += 1;
            true
        } else {
            false
        }
    }

    async fn check_api_key_rate_limit(&self, api_key: &str) -> bool {
        if !self.config.enabled {
            return true;
        }

        let mut counters = self.api_key_counters.write();
        let now = SystemTime::now();
        
        let (count, last_reset) = counters.entry(api_key.to_string()).or_insert((0, now));
        
        // Reset counter if more than 1 second has passed
        if now.duration_since(*last_reset).unwrap_or_default().as_secs() >= 1 {
            *count = 1;
            *last_reset = now;
            true
        } else if *count < self.config.requests_per_second {
            *count += 1;
            true
        } else {
            false
        }
    }
}

/// Request metadata middleware
pub async fn request_metadata_middleware(
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Response {
    let request_id = uuid::Uuid::new_v4().to_string();
    
    // Extract client IP (in production, consider X-Forwarded-For)
    let client_ip: std::net::IpAddr = headers
        .get("x-real-ip")
        .or_else(|| headers.get("x-forwarded-for"))
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or_else(|| "127.0.0.1".parse().unwrap());

    // Get auth info from extensions (set by auth middleware)
    let auth_info = request.extensions().get::<AuthInfo>().cloned();

    // Create client ID
    let client_id = if let Some(ref auth) = auth_info {
        auth.client_name.clone().unwrap_or_else(|| {
            auth.api_key
                .as_ref()
                .map(|k| format!("key_{}", &k[..8.min(k.len())]))
                .unwrap_or_else(|| format!("ip_{}", client_ip))
        })
    } else {
        format!("ip_{}", client_ip)
    };

    // Create request metadata
    let metadata = RequestMetadata {
        request_id: request_id.clone(),
        client_id,
        vm_type: crate::types::VmType::MultiVm, // Will be determined by router
        timestamp: SystemTime::now(),
        auth_info,
        headers: headers
            .iter()
            .filter_map(|(name, value)| {
                value.to_str().ok().map(|v| (name.to_string(), v.to_string()))
            })
            .collect(),
    };

    // Add metadata to request extensions
    request.extensions_mut().insert(metadata);

    // Add request ID to response headers
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-request-id",
        request_id.parse().unwrap(),
    );

    response
}

/// CORS middleware
pub async fn cors_middleware(
    _headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    let mut response = next.run(request).await;

    // Add CORS headers
    response.headers_mut().insert(
        "access-control-allow-origin",
        "*".parse().unwrap(),
    );
    response.headers_mut().insert(
        "access-control-allow-methods",
        "GET, POST, OPTIONS".parse().unwrap(),
    );
    response.headers_mut().insert(
        "access-control-allow-headers",
        "content-type, x-api-key, authorization, x-request-id".parse().unwrap(),
    );
    response.headers_mut().insert(
        "access-control-max-age",
        "86400".parse().unwrap(),
    );

    response
}

/// Method validation middleware
pub async fn method_validation_middleware(
    auth_info: Option<&AuthInfo>,
    method: &str,
) -> RpcResult<()> {
    if let Some(auth) = auth_info {
        if !auth.allowed_methods.is_empty() && !auth.allowed_methods.contains(&method.to_string()) {
            return Err(RpcError::AuthenticationFailed {
                reason: format!("Method '{}' not allowed for this API key", method),
            });
        }
    }
    Ok(())
}

/// Metrics middleware
pub struct MetricsMiddleware {
    request_counter: prometheus::IntCounterVec,
    response_time_histogram: prometheus::HistogramVec,
    error_counter: prometheus::IntCounterVec,
}

impl MetricsMiddleware {
    pub fn new() -> Self {
        let request_counter = prometheus::IntCounterVec::new(
            prometheus::Opts::new("rpc_requests_total", "Total number of RPC requests"),
            &["method", "client_type", "status"],
        ).unwrap();

        let response_time_histogram = prometheus::HistogramVec::new(
            prometheus::HistogramOpts::new("rpc_response_time_seconds", "RPC response time in seconds"),
            &["method", "client_type"],
        ).unwrap();

        let error_counter = prometheus::IntCounterVec::new(
            prometheus::Opts::new("rpc_errors_total", "Total number of RPC errors"),
            &["method", "error_type", "client_type"],
        ).unwrap();

        Self {
            request_counter,
            response_time_histogram,
            error_counter,
        }
    }

    pub async fn middleware(
        &self,
        method: &str,
        client_type: &str,
        start_time: SystemTime,
        status: &str,
        error_type: Option<&str>,
    ) {
        // Record request
        self.request_counter
            .with_label_values(&[method, client_type, status])
            .inc();

        // Record response time
        let duration = start_time.elapsed().unwrap_or_default();
        self.response_time_histogram
            .with_label_values(&[method, client_type])
            .observe(duration.as_secs_f64());

        // Record error if applicable
        if let Some(error) = error_type {
            self.error_counter
                .with_label_values(&[method, error, client_type])
                .inc();
        }
    }

    pub fn register_metrics(&self, registry: &prometheus::Registry) -> Result<(), prometheus::Error> {
        registry.register(Box::new(self.request_counter.clone()))?;
        registry.register(Box::new(self.response_time_histogram.clone()))?;
        registry.register(Box::new(self.error_counter.clone()))?;
        Ok(())
    }
}

/// Logging middleware
pub async fn logging_middleware(
    metadata: &RequestMetadata,
    method: &str,
    start_time: SystemTime,
    success: bool,
    error: Option<&str>,
) {
    let duration = start_time.elapsed().unwrap_or_default();
    
    if success {
        debug!(
            request_id = %metadata.request_id,
            client_id = %metadata.client_id,
            method = method,
            duration_ms = duration.as_millis(),
            "RPC request completed successfully"
        );
    } else {
        warn!(
            request_id = %metadata.request_id,
            client_id = %metadata.client_id,
            method = method,
            duration_ms = duration.as_millis(),
            error = error.unwrap_or("unknown"),
            "RPC request failed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_rate_limit_middleware_creation() {
        let config = RateLimitConfig {
            enabled: true,
            requests_per_second: 100,
            burst_size: 200,
            by_ip: true,
            by_api_key: true,
            whitelist: vec!["test_key".to_string()],
        };

        let middleware = RateLimitMiddleware::new(config);
        assert!(middleware.config.enabled);
        assert_eq!(middleware.config.requests_per_second, 100);
    }

    #[test]
    fn test_metrics_middleware_creation() {
        let _middleware = MetricsMiddleware::new();
        // Just verify it can be created without panicking
        assert!(true);
    }

    #[tokio::test]
    async fn test_method_validation() {
        let auth_info = AuthInfo {
            api_key: Some("test_key".to_string()),
            jwt_payload: None,
            client_name: Some("test_client".to_string()),
            allowed_methods: vec!["eth_getBalance".to_string()],
        };

        // Should allow permitted method
        assert!(method_validation_middleware(Some(&auth_info), "eth_getBalance").await.is_ok());

        // Should reject non-permitted method
        assert!(method_validation_middleware(Some(&auth_info), "eth_sendTransaction").await.is_err());

        // Should allow any method if no restrictions
        let unrestricted_auth = AuthInfo {
            api_key: Some("test_key".to_string()),
            jwt_payload: None,
            client_name: Some("test_client".to_string()),
            allowed_methods: vec![],
        };
        assert!(method_validation_middleware(Some(&unrestricted_auth), "any_method").await.is_ok());
    }
}