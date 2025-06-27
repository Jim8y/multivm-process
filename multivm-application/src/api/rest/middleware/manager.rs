//! Middleware Manager
//!
//! Coordinates and manages all middleware components including security headers,
//! authentication, rate limiting, and monitoring.

use super::security::{Environment, SecurityConfig};
use crate::error::ApplicationResult;
use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Comprehensive middleware configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MiddlewareConfig {
    /// Security headers configuration
    pub security: SecurityHeadersConfig,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitingConfig,
    /// Authentication configuration
    pub authentication: AuthenticationConfig,
    /// Monitoring configuration
    pub monitoring: MonitoringConfig,
    /// Request tracking configuration
    pub request_tracking: RequestTrackingConfig,
}

/// Security headers configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityHeadersConfig {
    /// Enable security headers middleware
    pub enabled: bool,
    /// Environment for security configuration
    pub environment: String,
    /// Custom CSP policy override
    pub custom_csp_policy: Option<String>,
    /// Enable HSTS
    pub enable_hsts: bool,
    /// HSTS max age in seconds
    pub hsts_max_age: u32,
    /// Additional custom headers
    pub custom_headers: HashMap<String, String>,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitingConfig {
    /// Enable rate limiting
    pub enabled: bool,
    /// Default requests per minute
    pub default_rpm: u32,
    /// Burst size
    pub burst_size: u32,
    /// Enable per-IP rate limiting
    pub per_ip_limiting: bool,
    /// Enable per-API-key rate limiting
    pub per_api_key_limiting: bool,
}

/// Authentication configuration for middleware
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticationConfig {
    /// Enable authentication middleware
    pub enabled: bool,
    /// Paths to skip authentication
    pub skip_paths: Vec<String>,
    /// Enable JWT validation
    pub enable_jwt: bool,
    /// Enable API key validation
    pub enable_api_keys: bool,
    /// Strict mode (reject invalid tokens vs skip)
    pub strict_mode: bool,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable metrics collection
    pub enabled: bool,
    /// Enable request logging
    pub enable_request_logging: bool,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
    /// Sample rate for monitoring (0.0 to 1.0)
    pub sample_rate: f64,
}

/// Request tracking configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestTrackingConfig {
    /// Enable request ID generation
    pub enabled: bool,
    /// Header name for request ID
    pub header_name: String,
    /// Enable request tracing
    pub enable_tracing: bool,
    /// Include request details in logs
    pub include_request_details: bool,
}


impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            environment: "production".to_string(),
            custom_csp_policy: None,
            enable_hsts: true,
            hsts_max_age: 31536000, // 1 year
            custom_headers: HashMap::new(),
        }
    }
}

impl Default for RateLimitingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_rpm: 1000,
            burst_size: 100,
            per_ip_limiting: true,
            per_api_key_limiting: true,
        }
    }
}

impl Default for AuthenticationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            skip_paths: vec![
                "/health".to_string(),
                "/api/v1/system/health".to_string(),
                "/api/v1/system/info".to_string(),
                "/metrics".to_string(),
            ],
            enable_jwt: true,
            enable_api_keys: true,
            strict_mode: true,
        }
    }
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            enable_request_logging: true,
            enable_performance_monitoring: true,
            sample_rate: 1.0, // 100% sampling by default
        }
    }
}

impl Default for RequestTrackingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            header_name: "x-request-id".to_string(),
            enable_tracing: true,
            include_request_details: false, // Privacy consideration
        }
    }
}

/// Middleware manager for coordinating all middleware
pub struct MiddlewareManager {
    config: MiddlewareConfig,
    security_config: SecurityConfig,
    stats: Arc<RwLock<MiddlewareStats>>,
}

/// Middleware statistics
#[derive(Debug, Clone, Default)]
pub struct MiddlewareStats {
    /// Total requests processed
    pub total_requests: u64,
    /// Security headers applied count
    pub security_headers_applied: u64,
    /// Authentication attempts
    pub auth_attempts: u64,
    /// Authentication successes
    pub auth_successes: u64,
    /// Rate limit violations
    pub rate_limit_violations: u64,
    /// Average response time
    pub avg_response_time_ms: f64,
    /// Last updated timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl MiddlewareManager {
    /// Create a new middleware manager
    pub fn new(config: MiddlewareConfig) -> ApplicationResult<Self> {
        let security_config = Self::create_security_config(&config.security)?;

        Ok(Self {
            config,
            security_config,
            stats: Arc::new(RwLock::new(MiddlewareStats::default())),
        })
    }

    /// Create security configuration from middleware config
    fn create_security_config(config: &SecurityHeadersConfig) -> ApplicationResult<SecurityConfig> {
        let environment = match config.environment.as_str() {
            "development" | "dev" => Environment::Development,
            "staging" | "test" => Environment::Staging,
            "production" | "prod" => Environment::Production,
            _ => Environment::Production, // Default to production for security
        };

        let mut security_config = match environment {
            Environment::Development => SecurityConfig::development(),
            Environment::Staging | Environment::Production => SecurityConfig::production(),
        };

        // Apply custom configuration
        if let Some(ref csp_policy) = config.custom_csp_policy {
            security_config.set_csp_policy(csp_policy.clone());
        }

        security_config.set_hsts(config.enable_hsts, config.hsts_max_age);

        // Add custom headers
        for (name, value) in &config.custom_headers {
            security_config.add_custom_header(name.clone(), value.clone());
        }

        Ok(security_config)
    }

    /// Get the security configuration
    pub fn security_config(&self) -> &SecurityConfig {
        &self.security_config
    }

    /// Update middleware configuration
    pub async fn update_config(&mut self, new_config: MiddlewareConfig) -> ApplicationResult<()> {
        self.security_config = Self::create_security_config(&new_config.security)?;
        self.config = new_config;
        info!("Middleware configuration updated");
        Ok(())
    }

    /// Get middleware statistics
    pub async fn get_stats(&self) -> MiddlewareStats {
        self.stats.read().await.clone()
    }

    /// Record request processing
    pub async fn record_request(&self, response_time_ms: f64) {
        let mut stats = self.stats.write().await;
        stats.total_requests += 1;

        // Update rolling average response time
        let weight = 0.1; // Weight for exponential moving average
        stats.avg_response_time_ms =
            stats.avg_response_time_ms * (1.0 - weight) + response_time_ms * weight;

        stats.last_updated = chrono::Utc::now();
    }

    /// Record security headers application
    pub async fn record_security_headers_applied(&self) {
        let mut stats = self.stats.write().await;
        stats.security_headers_applied += 1;
    }

    /// Record authentication attempt
    pub async fn record_auth_attempt(&self, success: bool) {
        let mut stats = self.stats.write().await;
        stats.auth_attempts += 1;
        if success {
            stats.auth_successes += 1;
        }
    }

    /// Record rate limit violation
    pub async fn record_rate_limit_violation(&self) {
        let mut stats = self.stats.write().await;
        stats.rate_limit_violations += 1;
    }

    /// Combined middleware function
    pub async fn process_request(
        &self,
        request: Request,
        next: Next,
    ) -> Result<Response, StatusCode> {
        let start_time = std::time::Instant::now();

        // Process the request through the middleware chain
        let response = self.apply_middleware(request, next).await?;

        // Record processing time
        let processing_time = start_time.elapsed().as_millis() as f64;
        self.record_request(processing_time).await;

        Ok(response)
    }

    /// Apply all configured middleware
    async fn apply_middleware(&self, request: Request, next: Next) -> Result<Response, StatusCode> {
        // Apply security headers if enabled
        if self.config.security.enabled {
            let mut response = next.run(request).await;
            super::security::apply_security_headers(response.headers_mut(), &self.security_config);
            self.record_security_headers_applied().await;
            Ok(response)
        } else {
            Ok(next.run(request).await)
        }
    }

    /// Health check for middleware components
    pub async fn health_check(&self) -> MiddlewareHealthReport {
        let stats = self.get_stats().await;

        let mut issues = Vec::new();
        let mut status = MiddlewareHealthStatus::Healthy;

        // Check authentication success rate
        if stats.auth_attempts > 0 {
            let auth_success_rate = stats.auth_successes as f64 / stats.auth_attempts as f64;
            if auth_success_rate < 0.5 {
                issues.push("Low authentication success rate".to_string());
                status = MiddlewareHealthStatus::Warning;
            }
        }

        // Check rate limiting
        if stats.rate_limit_violations > 1000 {
            issues.push("High number of rate limit violations".to_string());
            if status == MiddlewareHealthStatus::Healthy {
                status = MiddlewareHealthStatus::Warning;
            }
        }

        // Check response times
        if stats.avg_response_time_ms > 1000.0 {
            issues.push("High average response time".to_string());
            if status == MiddlewareHealthStatus::Healthy {
                status = MiddlewareHealthStatus::Warning;
            }
        }

        MiddlewareHealthReport {
            status,
            total_requests: stats.total_requests,
            auth_success_rate: if stats.auth_attempts > 0 {
                Some(stats.auth_successes as f64 / stats.auth_attempts as f64)
            } else {
                None
            },
            avg_response_time_ms: stats.avg_response_time_ms,
            rate_limit_violations: stats.rate_limit_violations,
            issues,
            last_updated: stats.last_updated,
        }
    }
}

/// Middleware health status
#[derive(Debug, Clone, PartialEq)]
pub enum MiddlewareHealthStatus {
    Healthy,
    Warning,
    Critical,
}

/// Middleware health report
#[derive(Debug, Clone)]
pub struct MiddlewareHealthReport {
    pub status: MiddlewareHealthStatus,
    pub total_requests: u64,
    pub auth_success_rate: Option<f64>,
    pub avg_response_time_ms: f64,
    pub rate_limit_violations: u64,
    pub issues: Vec<String>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Builder for middleware configuration
pub struct MiddlewareConfigBuilder {
    config: MiddlewareConfig,
}

impl MiddlewareConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: MiddlewareConfig::default(),
        }
    }

    pub fn security_headers(mut self, enabled: bool) -> Self {
        self.config.security.enabled = enabled;
        self
    }

    pub fn environment(mut self, env: &str) -> Self {
        self.config.security.environment = env.to_string();
        self
    }

    pub fn custom_csp(mut self, policy: String) -> Self {
        self.config.security.custom_csp_policy = Some(policy);
        self
    }

    pub fn rate_limiting(mut self, enabled: bool, rpm: u32) -> Self {
        self.config.rate_limiting.enabled = enabled;
        self.config.rate_limiting.default_rpm = rpm;
        self
    }

    pub fn authentication(mut self, enabled: bool) -> Self {
        self.config.authentication.enabled = enabled;
        self
    }

    pub fn monitoring(mut self, enabled: bool) -> Self {
        self.config.monitoring.enabled = enabled;
        self
    }

    pub fn build(self) -> MiddlewareConfig {
        self.config
    }
}

impl Default for MiddlewareConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_middleware_config_builder() {
        let config = MiddlewareConfigBuilder::new()
            .security_headers(true)
            .environment("production")
            .rate_limiting(true, 500)
            .authentication(true)
            .monitoring(true)
            .build();

        assert!(config.security.enabled);
        assert_eq!(config.security.environment, "production");
        assert_eq!(config.rate_limiting.default_rpm, 500);
        assert!(config.authentication.enabled);
        assert!(config.monitoring.enabled);
    }

    #[tokio::test]
    async fn test_middleware_manager_creation() {
        let config = MiddlewareConfig::default();
        let manager = MiddlewareManager::new(config);
        assert!(manager.is_ok());
    }

    #[tokio::test]
    async fn test_middleware_stats() {
        let config = MiddlewareConfig::default();
        let manager = MiddlewareManager::new(config).unwrap();

        manager.record_request(100.0).await;
        manager.record_auth_attempt(true).await;
        manager.record_rate_limit_violation().await;

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.auth_attempts, 1);
        assert_eq!(stats.auth_successes, 1);
        assert_eq!(stats.rate_limit_violations, 1);
    }
}
