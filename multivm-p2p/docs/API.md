# MultiVM P2P API Reference

This document provides comprehensive API documentation for the MultiVM P2P networking layer.

## Table of Contents

- [Core Components](#core-components)
- [Configuration](#configuration)
- [Messages](#messages)
- [Security](#security)
- [Error Handling](#error-handling)
- [Examples](#examples)

## Core Components

### P2PManager

The main entry point for P2P networking functionality.

```rust
pub struct P2PManager {
    // Internal fields...
}

impl P2PManager {
    /// Create a new P2P manager
    pub async fn new(config: P2PConfig, keypair: Keypair) -> P2PResult<Self>
    
    /// Start the P2P networking layer
    pub async fn start(&mut self) -> P2PResult<()>
    
    /// Shutdown the P2P networking layer
    pub async fn shutdown(&mut self) -> P2PResult<()>
    
    /// Send a message with specified priority
    pub async fn send_message(
        &self, 
        message: NetworkMessage, 
        priority: Priority
    ) -> P2PResult<()>
    
    /// Get current statistics
    pub async fn get_stats(&self) -> P2PResult<ManagerStats>
}
```

### SecurityCoordinator

Handles all security aspects of P2P communication.

```rust
pub struct SecurityCoordinator {
    // Internal fields...
}

impl SecurityCoordinator {
    /// Create a new security coordinator
    pub fn new(config: Arc<P2PConfig>) -> P2PResult<Self>
    
    /// Authenticate a peer using JWT or API key
    pub async fn authenticate_peer(&self, peer_id: &PeerId, token: &str) -> P2PResult<bool>
    
    /// Validate a message through security filters
    pub async fn validate_message(&self, peer_id: &PeerId, message: &NetworkMessage) -> P2PResult<bool>
    
    /// Encrypt a message for transmission
    pub async fn encrypt_message(&self, peer_public_key: &PublicKey, data: &[u8]) -> P2PResult<Vec<u8>>
    
    /// Decrypt a received message
    pub async fn decrypt_message(&self, peer_public_key: &PublicKey, encrypted_data: &[u8]) -> P2PResult<Vec<u8>>
    
    /// Ban a peer for security violations
    pub async fn ban_peer(&self, peer_id: PeerId, reason: String) -> P2PResult<()>
    
    /// Unban a previously banned peer
    pub async fn unban_peer(&self, peer_id: &PeerId) -> P2PResult<()>
    
    /// Get security statistics
    pub async fn get_security_stats(&self) -> SecurityStats
}
```

### NetworkCoordinator

Manages network connections and transport.

```rust
pub struct NetworkCoordinator {
    // Internal fields...
}

impl NetworkCoordinator {
    /// Create a new network coordinator
    pub fn new(config: Arc<P2PConfig>) -> P2PResult<Self>
    
    /// Connect to a peer
    pub async fn connect_peer(&self, peer_id: PeerId, address: Multiaddr) -> P2PResult<()>
    
    /// Disconnect from a peer
    pub async fn disconnect_peer(&self, peer_id: &PeerId) -> P2PResult<()>
    
    /// Get list of connected peers
    pub async fn get_connected_peers(&self) -> Vec<PeerId>
    
    /// Check network health
    pub async fn health_check(&self) -> NetworkHealthStatus
}
```

## Configuration

### P2PConfig

Main configuration structure for the P2P layer.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct P2PConfig {
    /// Network configuration
    pub network: NetworkConfig,
    /// Transport configuration  
    pub transport: TransportConfig,
    /// Discovery configuration
    pub discovery: DiscoveryConfig,
    /// Protocol configuration
    pub protocol: ProtocolConfig,
    /// Security configuration
    pub security: SecurityConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitConfig,
    /// Authentication configuration
    pub auth: AuthConfig,
}
```

### NetworkConfig

Network layer configuration.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Local peer ID (optional, generated if not provided)
    pub peer_id: Option<String>,
    /// Listen addresses for incoming connections
    pub listen_addresses: Vec<String>,
    /// External addresses to advertise
    pub external_addresses: Vec<String>,
    /// Maximum number of connections
    pub max_connections: usize,
    /// Connection timeout
    pub connection_timeout: Duration,
    /// Keep-alive interval
    pub keep_alive_interval: Duration,
    /// Enable automatic NAT traversal
    pub enable_nat_traversal: bool,
    /// Enable relay support
    pub enable_relay: bool,
    /// Enable AutoNAT
    pub enable_autonat: bool,
    /// Maximum message size in bytes
    pub max_message_size: usize,
}
```

### SecurityConfig

Security configuration options.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Enable noise encryption
    pub enable_noise: bool,
    /// Noise configuration
    pub noise: NoiseConfig,
    /// Rate limiting configuration
    pub rate_limiting: RateLimitConfig,
    /// Authentication configuration
    pub authentication: AuthConfig,
    /// Firewall configuration
    pub firewall: FirewallConfig,
}
```

### AuthConfig

Authentication configuration.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Enable peer authentication
    pub enabled: bool,
    /// Authentication method
    pub method: AuthMethod,
    /// Trusted peer list
    pub trusted_peers: Vec<String>,
    /// Authentication timeout
    pub timeout: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthMethod {
    Ed25519,
    Secp256k1,
    None,
}
```

## Messages

### NetworkMessage

Core message structure for P2P communication.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    /// Unique message identifier
    pub id: String,
    /// Message type and payload
    pub payload: MessagePayload,
    /// Source of the message
    pub source: MessageSource,
    /// Target(s) for the message
    pub target: MessageTarget,
    /// Timestamp when message was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Protocol version
    pub version: u32,
    /// Optional metadata
    pub metadata: HashMap<String, String>,
}

impl NetworkMessage {
    /// Create a new network message
    pub fn new(
        payload: MessagePayload,
        source: MessageSource,
        target: MessageTarget,
    ) -> Self
    
    /// Add metadata to the message
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self
    
    /// Check if message is a broadcast
    pub fn is_broadcast(&self) -> bool
    
    /// Check if message targets a specific peer
    pub fn is_peer_message(&self) -> bool
    
    /// Get target peer ID (if peer message)
    pub fn target_peer(&self) -> Option<&str>
    
    /// Estimate message size in bytes
    pub fn estimated_size(&self) -> usize
    
    /// Infer message type from payload
    pub fn infer_type(&self) -> MessageType
}
```

### MessagePayload

Different types of message payloads.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    /// Solana VM specific messages
    Svm(SvmMessage),
    /// Ethereum VM specific messages
    Evm(EvmMessage),
    /// MultiVM layer specific messages
    MultiVm(MultiVmMessage),
    /// Network control messages
    Control(ControlMessage),
    /// Discovery and peer management
    Discovery(DiscoveryMessage),
    /// Custom/extensible message payload
    Custom(serde_json::Value),
}
```

### MessageSource

Identifies the source of a message.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageSource {
    /// Message from SVM execution layer
    SvmExecution,
    /// Message from EVM execution layer
    EvmExecution,
    /// Message from MultiVM coordination layer
    MultiVmLayer,
    /// Message from consensus layer
    Consensus,
    /// Message from network layer
    NetworkLayer,
    /// Message from external source
    External,
}
```

### MessageTarget

Specifies message routing.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageTarget {
    /// Broadcast to all connected peers
    Broadcast,
    /// Send to specific peer
    Peer(String),
    /// Send to peers with specific capabilities
    Capability(String),
    /// Send to random subset of peers
    Random(usize),
    /// Send to peers in specific group
    Group(String),
}
```

### Priority

Message priority levels.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    Critical = 4,
    High = 3,
    Normal = 2,
    Low = 1,
}
```

## Security

### Authentication

The P2P layer supports multiple authentication methods:

#### JWT Authentication

```rust
// JWT tokens are validated against configured secret
let auth_result = security_coordinator
    .authenticate_peer(&peer_id, jwt_token)
    .await?;
```

#### Ed25519 Signatures

```rust
// Messages can be signed with Ed25519 keys
let signature = EncryptionManager::sign_message(&signing_key, message)?;
let is_valid = EncryptionManager::verify_signature(&verifying_key, message, &signature)?;
```

### Encryption

Messages are encrypted using ChaCha20-Poly1305 with X25519 key exchange:

```rust
// Encrypt message for peer
let encrypted = security_coordinator
    .encrypt_message(&peer_public_key, message_data)
    .await?;

// Decrypt received message
let decrypted = security_coordinator
    .decrypt_message(&peer_public_key, encrypted_data)
    .await?;
```

### Rate Limiting

Configurable rate limiting prevents abuse:

```rust
let rate_config = RateLimitConfig {
    enabled: true,
    max_requests_per_second: 100.0,
    burst_size: 20,
};
```

### DOS Protection

Multi-layered DOS protection:

- Connection rate limiting per IP
- Message size validation
- Reputation-based filtering
- Circuit breaker patterns

## Error Handling

### P2PError

Comprehensive error types for all P2P operations.

```rust
#[derive(Error, Debug, Clone)]
pub enum P2PError {
    #[error("Network connection error: {message}")]
    ConnectionError { message: String },
    
    #[error("Peer not found: {peer_id}")]
    PeerNotFound { peer_id: String },
    
    #[error("Invalid message format: {reason}")]
    InvalidMessageFormat { reason: String },
    
    #[error("Authentication failed for peer: {peer_id}")]
    AuthenticationFailed { peer_id: String },
    
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    
    #[error("Encryption error: {0}")]
    EncryptionError(String),
    
    #[error("Decryption error: {0}")]
    DecryptionError(String),
    
    // ... more error variants
}

pub type P2PResult<T> = Result<T, P2PError>;
```

### Error Categories

Errors are categorized for easier handling:

```rust
impl P2PError {
    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool
    
    /// Check if error is fatal
    pub fn is_fatal(&self) -> bool
    
    /// Get error category for logging
    pub fn category(&self) -> &'static str
}
```

## Examples

### Basic Setup

```rust
use multivm_p2p::{P2PManager, P2PConfig};
use libp2p::identity::Keypair;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = P2PConfig::default();
    let keypair = Keypair::generate_ed25519();
    
    let mut p2p_manager = P2PManager::new(config, keypair).await?;
    p2p_manager.start().await?;
    
    // Use the P2P network...
    
    p2p_manager.shutdown().await?;
    Ok(())
}
```

### Sending Messages

```rust
use multivm_p2p::protocol::messages::*;

// Create and send a cross-VM message
let message = NetworkMessage::new(
    MessagePayload::MultiVm(MultiVmMessage::StateSync {
        state_root: "0x123...".to_string(),
        vm_type: VmType::Svm,
        height: 1000,
    }),
    MessageSource::MultiVmLayer,
    MessageTarget::Broadcast,
);

p2p_manager.send_message(message, Priority::High).await?;
```

### Security Configuration

```rust
use multivm_p2p::config::{SecurityConfig, AuthConfig, RateLimitConfig};

let security_config = SecurityConfig {
    enable_noise: true,
    authentication: AuthConfig {
        enabled: true,
        method: AuthMethod::Ed25519,
        trusted_peers: vec!["trusted_peer_1".to_string()],
        timeout: Duration::from_secs(30),
    },
    rate_limiting: RateLimitConfig {
        enabled: true,
        max_requests_per_second: 100.0,
        burst_size: 20,
    },
    ..Default::default()
};
```

### Monitoring

```rust
// Get current statistics
let stats = p2p_manager.get_stats().await?;
println!("Connected peers: {}", stats.peers_connected);
println!("Messages sent: {}", stats.messages_sent);
println!("Messages received: {}", stats.messages_received);

// Get security statistics
let security_stats = security_coordinator.get_security_stats().await;
println!("Banned peers: {}", security_stats.banned_peers_count);
println!("Rate limit violations: {}", security_stats.rate_limit_violations);
```

For more detailed examples, see the `examples/` directory in the repository.