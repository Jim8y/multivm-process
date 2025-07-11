pub mod client;
pub mod messages;
pub mod secure_transport;
pub mod server;
pub mod transport;

// Re-export commonly used types
pub use messages::{IpcCommand, IpcMessage, IpcResponse};
pub use transport::IpcTransport;
