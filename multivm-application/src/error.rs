use thiserror::Error;

/// Application layer error types with comprehensive categorization
#[derive(Error, Debug, Clone)]
pub enum ApplicationError {
    /// API-related errors
    #[error("Invalid request: {message}")]
    InvalidRequest { message: String },

    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    #[error("Authorization denied: {resource}")]
    AuthorizationDenied { resource: String },

    #[error("Rate limit exceeded: {limit} requests per {window}")]
    RateLimitExceeded { limit: u64, window: String },

    #[error("Validation error: {field}: {message}")]
    ValidationError { field: String, message: String },

    /// VM-specific errors
    #[error("SVM error: {message}")]
    SvmError { message: String },

    #[error("EVM error: {message}")]
    EvmError { message: String },

    #[error("Cross-VM operation failed: {operation}: {reason}")]
    CrossVmError { operation: String, reason: String },

    /// Service errors
    #[error("Cache error: {operation}: {message}")]
    CacheError { operation: String, message: String },

    #[error("Database error: {operation}: {message}")]
    DatabaseError { operation: String, message: String },

    #[error("Network error: {endpoint}: {message}")]
    NetworkError { endpoint: String, message: String },

    /// Server errors
    #[error("Server configuration error: {component}: {message}")]
    ConfigurationError { component: String, message: String },

    #[error("Server startup error: {service}: {message}")]
    StartupError { service: String, message: String },

    #[error("Service unavailable: {service}: {reason}")]
    ServiceUnavailable { service: String, reason: String },

    /// Resource errors
    #[error("Resource not found: {resource_type}: {identifier}")]
    ResourceNotFound {
        resource_type: String,
        identifier: String,
    },

    #[error("Resource conflict: {resource}: {reason}")]
    ResourceConflict { resource: String, reason: String },

    #[error("Resource exhausted: {resource}: {limit}")]
    ResourceExhausted { resource: String, limit: String },

    /// WebSocket errors
    #[error("WebSocket connection error: {reason}")]
    WebSocketError { reason: String },

    #[error("Subscription error: {subscription_type}: {message}")]
    SubscriptionError {
        subscription_type: String,
        message: String,
    },

    /// GraphQL errors
    #[error("GraphQL parsing error: {query}: {message}")]
    GraphQLParsingError { query: String, message: String },

    #[error("GraphQL execution error: {field}: {message}")]
    GraphQLExecutionError { field: String, message: String },

    /// Timeout and performance errors
    #[error("Operation timeout: {operation} exceeded {timeout_ms}ms")]
    TimeoutError { operation: String, timeout_ms: u64 },

    #[error("Performance degraded: {metric}: {current} > {threshold}")]
    PerformanceDegraded {
        metric: String,
        current: String,
        threshold: String,
    },

    /// Internal errors
    #[error("Internal server error: {component}: {message}")]
    InternalError { component: String, message: String },

    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

impl ApplicationError {
    /// Determine if the error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            ApplicationError::RateLimitExceeded { .. } => true,
            ApplicationError::TimeoutError { .. } => true,
            ApplicationError::NetworkError { .. } => true,
            ApplicationError::ServiceUnavailable { .. } => true,
            ApplicationError::CacheError { .. } => true,
            ApplicationError::PerformanceDegraded { .. } => true,
            _ => false,
        }
    }

    /// Determine if the error is critical (requires immediate attention)
    pub fn is_critical(&self) -> bool {
        match self {
            ApplicationError::StartupError { .. } => true,
            ApplicationError::ConfigurationError { .. } => true,
            ApplicationError::ResourceExhausted { .. } => true,
            ApplicationError::DatabaseError { .. } => true,
            ApplicationError::InternalError { .. } => true,
            _ => false,
        }
    }

    /// Get error category for metrics and monitoring
    pub fn category(&self) -> ErrorCategory {
        match self {
            ApplicationError::InvalidRequest { .. } | ApplicationError::ValidationError { .. } => {
                ErrorCategory::Client
            }

            ApplicationError::AuthenticationFailed { .. }
            | ApplicationError::AuthorizationDenied { .. } => ErrorCategory::Authentication,

            ApplicationError::RateLimitExceeded { .. } => ErrorCategory::RateLimit,

            ApplicationError::SvmError { .. }
            | ApplicationError::EvmError { .. }
            | ApplicationError::CrossVmError { .. } => ErrorCategory::Blockchain,

            ApplicationError::ResourceNotFound { .. }
            | ApplicationError::ResourceConflict { .. } => ErrorCategory::Resource,

            ApplicationError::NetworkError { .. } | ApplicationError::TimeoutError { .. } => {
                ErrorCategory::Network
            }

            ApplicationError::CacheError { .. } | ApplicationError::DatabaseError { .. } => {
                ErrorCategory::Storage
            }

            ApplicationError::WebSocketError { .. }
            | ApplicationError::SubscriptionError { .. } => ErrorCategory::WebSocket,

            ApplicationError::GraphQLParsingError { .. }
            | ApplicationError::GraphQLExecutionError { .. } => ErrorCategory::GraphQL,

            ApplicationError::ConfigurationError { .. }
            | ApplicationError::StartupError { .. }
            | ApplicationError::ServiceUnavailable { .. }
            | ApplicationError::ResourceExhausted { .. }
            | ApplicationError::PerformanceDegraded { .. }
            | ApplicationError::InternalError { .. }
            | ApplicationError::Unknown { .. } => ErrorCategory::Server,
        }
    }

    /// Get HTTP status code for REST API responses
    pub fn http_status(&self) -> u16 {
        match self {
            ApplicationError::InvalidRequest { .. } | ApplicationError::ValidationError { .. } => {
                400
            } // Bad Request

            ApplicationError::AuthenticationFailed { .. } => 401, // Unauthorized

            ApplicationError::AuthorizationDenied { .. } => 403, // Forbidden

            ApplicationError::ResourceNotFound { .. } => 404, // Not Found

            ApplicationError::ResourceConflict { .. } => 409, // Conflict

            ApplicationError::RateLimitExceeded { .. } => 429, // Too Many Requests

            ApplicationError::InternalError { .. } | ApplicationError::Unknown { .. } => 500, // Internal Server Error

            ApplicationError::ServiceUnavailable { .. } => 503, // Service Unavailable

            ApplicationError::TimeoutError { .. } => 504, // Gateway Timeout

            _ => 500, // Default to Internal Server Error
        }
    }

    /// Create error for metrics tagging
    pub fn metric_tags(&self) -> Vec<(&str, &str)> {
        let mut tags = vec![
            ("category", self.category().as_str()),
            (
                "recoverable",
                if self.is_recoverable() {
                    "true"
                } else {
                    "false"
                },
            ),
            (
                "critical",
                if self.is_critical() { "true" } else { "false" },
            ),
        ];

        match self {
            ApplicationError::SvmError { .. } => tags.push(("vm_type", "svm")),
            ApplicationError::EvmError { .. } => tags.push(("vm_type", "evm")),
            ApplicationError::CrossVmError { .. } => tags.push(("vm_type", "cross")),
            _ => {}
        }

        tags
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    Client,
    Authentication,
    RateLimit,
    Blockchain,
    Resource,
    Network,
    Storage,
    WebSocket,
    GraphQL,
    Server,
}

impl ErrorCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCategory::Client => "client",
            ErrorCategory::Authentication => "auth",
            ErrorCategory::RateLimit => "rate_limit",
            ErrorCategory::Blockchain => "blockchain",
            ErrorCategory::Resource => "resource",
            ErrorCategory::Network => "network",
            ErrorCategory::Storage => "storage",
            ErrorCategory::WebSocket => "websocket",
            ErrorCategory::GraphQL => "graphql",
            ErrorCategory::Server => "server",
        }
    }
}

/// Result type for application operations
pub type ApplicationResult<T> = Result<T, ApplicationError>;

/// Specialized result types for different components
pub type ApiResult<T> = ApplicationResult<T>;
pub type AuthResult<T> = ApplicationResult<T>;
pub type CacheResult<T> = ApplicationResult<T>;
pub type WebSocketResult<T> = ApplicationResult<T>;
pub type GraphQLResult<T> = ApplicationResult<T>;

// Conversion implementations for external error types

impl From<redis::RedisError> for ApplicationError {
    fn from(err: redis::RedisError) -> Self {
        ApplicationError::CacheError {
            operation: "redis_operation".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<reqwest::Error> for ApplicationError {
    fn from(err: reqwest::Error) -> Self {
        ApplicationError::NetworkError {
            endpoint: err.url().map_or("unknown".to_string(), |u| u.to_string()),
            message: err.to_string(),
        }
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for ApplicationError {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        ApplicationError::WebSocketError {
            reason: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for ApplicationError {
    fn from(err: serde_json::Error) -> Self {
        ApplicationError::ValidationError {
            field: "json".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<config::ConfigError> for ApplicationError {
    fn from(err: config::ConfigError) -> Self {
        ApplicationError::ConfigurationError {
            component: "config".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<jsonwebtoken::errors::Error> for ApplicationError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        ApplicationError::AuthenticationFailed {
            reason: err.to_string(),
        }
    }
}

impl From<multivm_consensus::error::ConsensusError> for ApplicationError {
    fn from(err: multivm_consensus::error::ConsensusError) -> Self {
        ApplicationError::InternalError {
            component: "consensus".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<multivm_common::error::MultivmError> for ApplicationError {
    fn from(err: multivm_common::error::MultivmError) -> Self {
        ApplicationError::InternalError {
            component: "multivm".to_string(),
            message: err.to_string(),
        }
    }
}

// Additional helper functions for common error patterns
impl ApplicationError {
    pub fn invalid_request(message: impl Into<String>) -> Self {
        ApplicationError::InvalidRequest {
            message: message.into(),
        }
    }

    pub fn auth_failed(reason: impl Into<String>) -> Self {
        ApplicationError::AuthenticationFailed {
            reason: reason.into(),
        }
    }

    pub fn not_found(resource_type: impl Into<String>, identifier: impl Into<String>) -> Self {
        ApplicationError::ResourceNotFound {
            resource_type: resource_type.into(),
            identifier: identifier.into(),
        }
    }

    pub fn internal_error(component: impl Into<String>, message: impl Into<String>) -> Self {
        ApplicationError::InternalError {
            component: component.into(),
            message: message.into(),
        }
    }

    pub fn service_unavailable(service: impl Into<String>, reason: impl Into<String>) -> Self {
        ApplicationError::ServiceUnavailable {
            service: service.into(),
            reason: reason.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        let auth_error = ApplicationError::AuthenticationFailed {
            reason: "Invalid token".to_string(),
        };
        assert_eq!(auth_error.category(), ErrorCategory::Authentication);
        assert!(!auth_error.is_recoverable());
        assert!(!auth_error.is_critical());
        assert_eq!(auth_error.http_status(), 401);
    }

    #[test]
    fn test_recoverable_errors() {
        let timeout_error = ApplicationError::TimeoutError {
            operation: "api_call".to_string(),
            timeout_ms: 5000,
        };
        assert!(timeout_error.is_recoverable());
        assert!(!timeout_error.is_critical());
    }

    #[test]
    fn test_critical_errors() {
        let startup_error = ApplicationError::StartupError {
            service: "rest_api".to_string(),
            message: "Port already in use".to_string(),
        };
        assert!(!startup_error.is_recoverable());
        assert!(startup_error.is_critical());
    }

    #[test]
    fn test_metric_tags() {
        let svm_error = ApplicationError::SvmError {
            message: "Transaction failed".to_string(),
        };
        let tags = svm_error.metric_tags();
        assert!(tags.contains(&("vm_type", "svm")));
        assert!(tags.contains(&("category", "blockchain")));
    }
}
