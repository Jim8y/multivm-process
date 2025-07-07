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
    InvalidMessageFormat { reason: String },

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

    #[error("Connection failed: {reason}")]
    ConnectionFailed { reason: String },

    #[error("Timeout error: operation timed out after {duration:?}")]
    TimeoutError { duration: std::time::Duration },

    #[error("Network not started")]
    NetworkNotStarted,

    #[error("Network already started")]
    NetworkAlreadyStarted,

    #[error("Insufficient peers: required {required}, available {available}")]
    InsufficientPeers { required: usize, available: usize },

    #[error("Message too large: {size} bytes exceeds limit {limit}")]
    MessageTooLargeWithLimit { size: usize, limit: usize },

    #[error("Unsupported protocol version: {version}")]
    UnsupportedProtocol { version: String },

    #[error("Authentication failed for peer: {peer_id}")]
    AuthenticationFailed { peer_id: String },

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Message too large: {0} bytes")]
    MessageTooLarge(usize),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),

    #[error("Message expired: age {0:?}")]
    MessageExpired(std::time::Duration),

    #[error("Replay attack detected from peer {0} with nonce {1}")]
    ReplayAttack(libp2p::PeerId, u64),

    #[error("Unknown peer: {0}")]
    UnknownPeer(libp2p::PeerId),

    #[error("Invalid signature from peer: {0}")]
    InvalidSignature(libp2p::PeerId),

    #[error("Unauthorized peer: {0}")]
    UnauthorizedPeer(libp2p::PeerId),

    #[error("Connection blocked: {0}")]
    ConnectionBlocked(String),

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

    #[error("Already started")]
    AlreadyStarted,

    #[error("Manager shutdown")]
    ManagerShutdown,

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption error: {0}")]
    DecryptionError(String),

    #[error("Signature verification error: {0}")]
    SignatureVerificationError(String),

    #[error("Key generation error: {0}")]
    KeyGenerationError(String),
}

/// Result type alias for P2P operations
pub type P2PResult<T> = std::result::Result<T, P2PError>;

// Conversion to MultivmError
impl From<P2PError> for MultivmError {
    fn from(err: P2PError) -> Self {
        MultivmError::Network {
            message: err.to_string(),
            endpoint: None,
            retry_after: None,
        }
    }
}

// Conversion from MultivmError
impl From<MultivmError> for P2PError {
    fn from(err: MultivmError) -> Self {
        P2PError::Internal(err.to_string())
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

impl From<anyhow::Error> for P2PError {
    fn from(err: anyhow::Error) -> Self {
        P2PError::Internal(err.to_string())
    }
}

#[cfg(feature = "metrics")]
impl From<prometheus::Error> for P2PError {
    fn from(err: prometheus::Error) -> Self {
        P2PError::Internal(format!("Prometheus error: {}", err))
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
        Self::InvalidMessage(reason.into())
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
                | P2PError::RateLimitExceeded(_)
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
                | P2PError::InvalidMessageFormat { .. }
                | P2PError::InvalidMessage(_)
                | P2PError::Serialization { .. }
        )
    }

    /// Get error category for logging and metrics
    pub fn category(&self) -> &'static str {
        match self {
            P2PError::ConnectionError { .. } => "connection",
            P2PError::PeerNotFound { .. } => "peer",
            P2PError::InvalidMessageFormat { .. } => "message",
            P2PError::InvalidMessage(_) => "message",
            P2PError::ProtocolError { .. } => "protocol",
            P2PError::TransportError { .. } => "transport",
            P2PError::DiscoveryError { .. } => "discovery",
            P2PError::RoutingError { .. } => "routing",
            P2PError::ConfigurationError { .. } => "configuration",
            P2PError::TimeoutError { .. } => "timeout",
            P2PError::NetworkNotStarted => "lifecycle",
            P2PError::NetworkAlreadyStarted => "lifecycle",
            P2PError::InsufficientPeers { .. } => "peers",
            P2PError::MessageTooLargeWithLimit { .. } => "message",
            P2PError::MessageTooLarge(_) => "message",
            P2PError::UnsupportedProtocol { .. } => "protocol",
            P2PError::AuthenticationFailed { .. } => "auth",
            P2PError::RateLimitExceeded(_) => "rate_limit",
            P2PError::Serialization { .. } => "serialization",
            P2PError::Io { .. } => "io",
            P2PError::Libp2p { .. } => "libp2p",
            P2PError::ProtocolTranslation(_) => "protocol_translation",
            P2PError::Internal(_) => "internal",
            P2PError::Transport(_) => "transport",
            P2PError::MessageExpired(_) => "message",
            P2PError::ReplayAttack(_, _) => "security",
            P2PError::UnknownPeer(_) => "peer",
            P2PError::InvalidSignature(_) => "security",
            P2PError::UnauthorizedPeer(_) => "security",
            P2PError::ConnectionBlocked(_) => "security",
            P2PError::ConnectionFailed { .. } => "connection",
            P2PError::AlreadyStarted => "lifecycle",
            P2PError::ManagerShutdown => "lifecycle",
            P2PError::EncryptionError(_) => "security",
            P2PError::DecryptionError(_) => "security",
            P2PError::SignatureVerificationError(_) => "security",
            P2PError::KeyGenerationError(_) => "security",
        }
    }

    /// Create a security error
    pub fn security_error<S: Into<String>>(msg: S) -> Self {
        Self::InvalidMessage(format!("Security error: {}", msg.into()))
    }

    /// Create an authentication error
    pub fn auth_error<S: Into<String>>(msg: S) -> Self {
        Self::AuthenticationFailed {
            peer_id: msg.into(),
        }
    }
}
