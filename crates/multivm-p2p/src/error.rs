//! Error types for the P2P networking layer

use multivm_common::MultivmError;
use thiserror::Error;

/// Errors that can occur in the P2P networking layer
#[derive(Error, Debug, Clone)]
pub enum P2PError {
    #[error("Network connection error: {message}")]
    ConnectionError { message: String },

    #[error("Peer not found: {peer_id}")]
    PeerNotFound { peer_id: String },

    #[error("Invalid message format: {reason}")]
    InvalidMessage { reason: String },

    #[error("Protocol error: {protocol} - {message}")]
    ProtocolError { protocol: String, message: String },

    #[error("Transport error: {transport} - {message}")]
    TransportError { transport: String, message: String },

    #[error("Discovery error: {message}")]
    DiscoveryError { message: String },

    #[error("Routing error: {message}")]
    RoutingError { message: String },

    #[error("Protocol translation error: {0}")]
    ProtocolTranslation(String),

    #[error("Configuration error: {message}")]
    ConfigurationError { message: String },

    #[error("Timeout error: operation timed out after {duration:?}")]
    TimeoutError { duration: std::time::Duration },

    #[error("Network not started")]
    NetworkNotStarted,

    #[error("Network already started")]
    NetworkAlreadyStarted,

    #[error("Insufficient peers: required {required}, available {available}")]
    InsufficientPeers { required: usize, available: usize },

    #[error("Message too large: {size} bytes exceeds limit {limit}")]
    MessageTooLarge { size: usize, limit: usize },

    #[error("Unsupported protocol version: {version}")]
    UnsupportedProtocol { version: String },

    #[error("Authentication failed for peer: {peer_id}")]
    AuthenticationFailed { peer_id: String },

    #[error("Rate limit exceeded for peer: {peer_id}")]
    RateLimitExceeded { peer_id: String },

    #[error("Serialization error: {message}")]
    Serialization { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("libp2p error: {message}")]
    Libp2p { message: String },

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Transport error: {0}")]
    Transport(String),
}

/// Result type alias for P2P operations
pub type P2PResult<T> = std::result::Result<T, P2PError>;

// Conversion to MultivmError
impl From<P2PError> for MultivmError {
    fn from(err: P2PError) -> Self {
        MultivmError::Network(err.to_string())
    }
}

// Conversion from common serialization errors
impl From<serde_json::Error> for P2PError {
    fn from(err: serde_json::Error) -> Self {
        P2PError::Serialization {
            message: err.to_string(),
        }
    }
}

impl From<bincode::Error> for P2PError {
    fn from(err: bincode::Error) -> Self {
        P2PError::Serialization {
            message: err.to_string(),
        }
    }
}

// Error conversions for standard library errors

impl From<std::io::Error> for P2PError {
    fn from(err: std::io::Error) -> Self {
        P2PError::Io {
            message: err.to_string(),
        }
    }
}

impl P2PError {
    /// Create a connection error
    pub fn connection_error(message: impl Into<String>) -> Self {
        Self::ConnectionError {
            message: message.into(),
        }
    }

    /// Create a protocol error
    pub fn protocol_error(protocol: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ProtocolError {
            protocol: protocol.into(),
            message: message.into(),
        }
    }

    /// Create a peer not found error
    pub fn peer_not_found(peer_id: impl Into<String>) -> Self {
        Self::PeerNotFound {
            peer_id: peer_id.into(),
        }
    }

    /// Create an invalid message error
    pub fn invalid_message(reason: impl Into<String>) -> Self {
        Self::InvalidMessage {
            reason: reason.into(),
        }
    }

    /// Create a timeout error
    pub fn timeout(duration: std::time::Duration) -> Self {
        Self::TimeoutError { duration }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            P2PError::ConnectionError { .. }
                | P2PError::TimeoutError { .. }
                | P2PError::InsufficientPeers { .. }
                | P2PError::RateLimitExceeded { .. }
                | P2PError::TransportError { .. }
                | P2PError::DiscoveryError { .. }
        )
    }

    /// Check if this error is fatal
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            P2PError::ConfigurationError { .. }
                | P2PError::UnsupportedProtocol { .. }
                | P2PError::InvalidMessage { .. }
                | P2PError::Serialization { .. }
        )
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            P2PError::ConnectionError { .. } => "connection",
            P2PError::PeerNotFound { .. } => "peer",
            P2PError::InvalidMessage { .. } => "message",
            P2PError::ProtocolError { .. } => "protocol",
            P2PError::TransportError { .. } => "transport",
            P2PError::DiscoveryError { .. } => "discovery",
            P2PError::RoutingError { .. } => "routing",
            P2PError::ConfigurationError { .. } => "configuration",
            P2PError::TimeoutError { .. } => "timeout",
            P2PError::NetworkNotStarted => "lifecycle",
            P2PError::NetworkAlreadyStarted => "lifecycle",
            P2PError::InsufficientPeers { .. } => "peers",
            P2PError::MessageTooLarge { .. } => "message",
            P2PError::UnsupportedProtocol { .. } => "protocol",
            P2PError::AuthenticationFailed { .. } => "auth",
            P2PError::RateLimitExceeded { .. } => "rate_limit",
            P2PError::Serialization { .. } => "serialization",
            P2PError::Io { .. } => "io",
            P2PError::Libp2p { .. } => "libp2p",
            P2PError::ProtocolTranslation(_) => "protocol_translation",
            P2PError::Internal(_) => "internal",
            P2PError::Transport(_) => "transport",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = P2PError::connection_error("Test connection error");
        assert!(matches!(err, P2PError::ConnectionError { .. }));
        assert!(err.is_recoverable());
        assert!(!err.is_fatal());
        assert_eq!(err.category(), "connection");
    }

    #[test]
    fn test_error_conversion() {
        let p2p_err = P2PError::peer_not_found("peer123");
        let multivm_err: MultivmError = p2p_err.into();

        match multivm_err {
            MultivmError::Network(msg) => assert!(msg.contains("peer123")),
            _ => panic!("Expected Network error"),
        }
    }

    #[test]
    fn test_error_categories() {
        assert_eq!(P2PError::NetworkNotStarted.category(), "lifecycle");
        assert_eq!(
            P2PError::timeout(std::time::Duration::from_secs(5)).category(),
            "timeout"
        );
        assert_eq!(
            P2PError::protocol_error("multivm", "test").category(),
            "protocol"
        );
    }

    #[test]
    fn test_recoverable_errors() {
        assert!(P2PError::connection_error("test").is_recoverable());
        assert!(P2PError::timeout(std::time::Duration::from_secs(1)).is_recoverable());
        assert!(!P2PError::invalid_message("test").is_recoverable());
    }

    #[test]
    fn test_fatal_errors() {
        assert!(P2PError::ConfigurationError {
            message: "test".to_string()
        }
        .is_fatal());
        assert!(P2PError::UnsupportedProtocol {
            version: "1.0".to_string()
        }
        .is_fatal());
        assert!(!P2PError::ConnectionError {
            message: "test".to_string()
        }
        .is_fatal());
    }
}
