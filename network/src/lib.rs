//! Production-ready network transport implementation for MultiVM

pub mod tcp;
pub mod tls;
pub mod connection;
pub mod error;
pub mod security;

pub use error::{NetworkError, Result};
pub use tcp::{TcpTransport, TcpTransportConfig};
pub use tls::TlsConfig;
pub use security::{AuthToken, AuthManager, MessageCrypto, SecurityRateLimiter};