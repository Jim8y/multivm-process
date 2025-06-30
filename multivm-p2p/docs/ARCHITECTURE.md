# MultiVM P2P Architecture

This document describes the architecture and design principles of the MultiVM P2P networking layer.

## Table of Contents

- [Overview](#overview)
- [Design Principles](#design-principles)
- [Component Architecture](#component-architecture)
- [Data Flow](#data-flow)
- [Security Architecture](#security-architecture)
- [Performance Considerations](#performance-considerations)
- [Extension Points](#extension-points)

## Overview

The MultiVM P2P networking layer is designed as a modular, scalable, and secure networking solution that enables communication between different virtual machine environments (Ethereum VM and Solana VM) in a distributed blockchain system.

### Key Goals

1. **Cross-VM Communication**: Enable seamless message routing between different VM types
2. **Security**: Provide comprehensive security features including encryption, authentication, and DOS protection
3. **Scalability**: Support large numbers of peers with efficient resource usage
4. **Modularity**: Clean separation of concerns with well-defined interfaces
5. **Reliability**: Fault tolerance with circuit breakers and self-healing capabilities
6. **Performance**: High throughput and low latency message delivery

## Design Principles

### Single Responsibility Principle (SRP)

Each component has a single, well-defined responsibility:

- **P2PManager**: Orchestration and coordination
- **SecurityCoordinator**: All security concerns
- **NetworkCoordinator**: Connection management
- **MessageCoordinator**: Message routing and delivery
- **DiscoveryCoordinator**: Peer discovery
- **MonitoringCoordinator**: Metrics and health monitoring

### Dependency Injection

Components receive their dependencies through constructor injection, enabling:
- Easy testing with mock dependencies
- Runtime configuration flexibility
- Clear dependency relationships

### Command/Query Segregation

The architecture separates commands (state-changing operations) from queries (read-only operations):

```rust
// Commands
pub enum ManagerCommand {
    SendMessage { message: NetworkMessage, priority: Priority },
    ConnectPeer { peer_id: PeerId, address: Multiaddr },
    DisconnectPeer { peer_id: PeerId },
}

// Queries
impl P2PManager {
    pub async fn get_stats(&self) -> P2PResult<ManagerStats>
    pub async fn get_connected_peers(&self) -> Vec<PeerId>
}
```

### Event-Driven Architecture

Components communicate through events, reducing coupling:

```rust
pub enum NetworkEvent {
    PeerConnected { peer_id: PeerId },
    PeerDisconnected { peer_id: PeerId },
    MessageReceived { message: NetworkMessage, from: PeerId },
}
```

## Component Architecture

### High-Level Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│                         P2PManager                             │
│                    (Orchestration Layer)                       │
├─────────────┬─────────────┬─────────────┬─────────────────────┤
│  Security   │  Network    │  Message    │     Discovery       │
│ Coordinator │ Coordinator │ Coordinator │   Coordinator       │
├─────────────┼─────────────┼─────────────┼─────────────────────┤
│                    Monitoring Coordinator                      │
├─────────────────────────────────────────────────────────────────┤
│                      Transport Layer                           │
│                 (UnifiedTransport)                             │
├─────────────────────────────────────────────────────────────────┤
│                       libp2p Stack                             │
│            (Gossipsub, Kademlia, mDNS, Noise, etc.)           │
└─────────────────────────────────────────────────────────────────┘
```

### Component Details

#### P2PManager

The central orchestrator that coordinates all P2P operations.

```rust
pub struct P2PManager {
    config: Arc<P2PConfig>,
    local_peer_id: PeerId,
    network_coordinator: Arc<NetworkCoordinator>,
    message_coordinator: Arc<MessageCoordinator>,
    security_coordinator: Arc<SecurityCoordinator>,
    discovery_coordinator: Arc<DiscoveryCoordinator>,
    monitoring_coordinator: Arc<MonitoringCoordinator>,
    state: Arc<RwLock<ManagerState>>,
    command_tx: mpsc::Sender<ManagerCommand>,
}
```

**Responsibilities:**
- Coordinate between all components
- Handle external API requests
- Manage component lifecycle
- Provide unified interface to consumers

#### SecurityCoordinator

Handles all security aspects of P2P communication.

```rust
pub struct SecurityCoordinator {
    encryption: Arc<EncryptionManager>,
    auth: Arc<AuthManager>,
    rate_limiter: Arc<RateLimiter>,
    dos_protection: Arc<DosProtectionManager>,
    security_policy: Arc<RwLock<SecurityPolicy>>,
}
```

**Security Pipeline:**
1. **Authentication**: Verify peer identity using JWT/API keys
2. **Authorization**: Check peer permissions and banned status
3. **Rate Limiting**: Enforce message rate limits per peer
4. **Message Validation**: Validate message size and content
5. **DOS Protection**: Apply circuit breaker and reputation filtering
6. **Encryption/Decryption**: Secure message content

#### NetworkCoordinator

Manages network connections and transport layer.

```rust
pub struct NetworkCoordinator {
    config: Arc<P2PConfig>,
    transport: Arc<RwLock<Option<UnifiedTransport>>>,
    connections: Arc<RwLock<ConnectionState>>,
    event_tx: mpsc::UnboundedSender<NetworkEvent>,
}
```

**Responsibilities:**
- Connection establishment and management
- Transport protocol handling (TCP, WebSocket, QUIC)
- Connection pooling and limits
- Network health monitoring

#### MessageCoordinator

Handles message routing and delivery.

```rust
pub struct MessageCoordinator {
    router: Arc<RwLock<MessageRouter>>,
    handlers: Arc<RwLock<MessageHandlers>>,
    outbound_queue: Arc<RwLock<MessageQueue>>,
    stats: Arc<RwLock<MessageStats>>,
}
```

**Message Flow:**
1. **Routing Decision**: Determine target peers based on message target
2. **Priority Queuing**: Queue messages by priority level
3. **Delivery**: Send messages through appropriate transport
4. **Acknowledgment**: Handle delivery confirmations
5. **Retry Logic**: Retry failed deliveries with exponential backoff

## Data Flow

### Message Sending Flow

```text
Application
    │
    ▼
P2PManager::send_message()
    │
    ▼
SecurityCoordinator::validate_message()
    │
    ▼
MessageCoordinator::route_message()
    │
    ▼
NetworkCoordinator::send_to_peers()
    │
    ▼
UnifiedTransport::send()
    │
    ▼
libp2p Network
```

### Message Receiving Flow

```text
libp2p Network
    │
    ▼
UnifiedTransport::receive()
    │
    ▼
SecurityCoordinator::decrypt_message()
    │
    ▼
SecurityCoordinator::validate_message()
    │
    ▼
MessageCoordinator::handle_message()
    │
    ▼
Application (via events)
```

### Security Validation Pipeline

```text
Incoming Message
    │
    ▼
Check Banned Peers
    │
    ▼
Rate Limiting Check
    │
    ▼
Message Size Validation
    │
    ▼
DOS Protection Check
    │
    ▼
Message Decryption
    │
    ▼
Content Validation
    │
    ▼
Deliver to Application
```

## Security Architecture

### Multi-Layered Security

The security architecture implements defense in depth:

1. **Network Layer Security**
   - Noise protocol for transport encryption
   - Connection-level authentication

2. **Application Layer Security**
   - Message-level encryption (ChaCha20-Poly1305)
   - Digital signatures (Ed25519)
   - Message integrity verification

3. **Access Control**
   - Peer authentication (JWT/API keys)
   - Capability-based authorization
   - Peer banning/unbanning

4. **DOS Protection**
   - Rate limiting per peer and globally
   - Connection limits per IP
   - Circuit breaker patterns
   - Reputation system

### Encryption Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                    Encryption Stack                        │
├─────────────────────────────────────────────────────────────┤
│  Application Messages                                       │
│      │                                                     │
│      ▼                                                     │
│  ChaCha20-Poly1305 Encryption                             │
│      │                                                     │
│      ▼                                                     │
│  libp2p Noise Protocol                                     │
│      │                                                     │
│      ▼                                                     │
│  Transport Layer (TCP/WebSocket/QUIC)                      │
└─────────────────────────────────────────────────────────────┘
```

### Key Management

- **X25519 Key Exchange**: For establishing shared secrets
- **Ed25519 Signatures**: For message authentication
- **Key Rotation**: Configurable key rotation intervals
- **Key Caching**: LRU cache with TTL for performance

## Performance Considerations

### Scalability Features

1. **Connection Pooling**: Reuse connections to reduce overhead
2. **Message Batching**: Batch multiple messages for efficiency
3. **Priority Queuing**: Ensure critical messages are delivered first
4. **Compression**: Optional message compression for bandwidth optimization
5. **Circuit Breakers**: Prevent cascade failures

### Memory Management

- **LRU Caches**: Bounded caches with automatic eviction
- **Connection Limits**: Prevent memory exhaustion
- **Message Size Limits**: Protect against large message attacks
- **Cleanup Tasks**: Periodic cleanup of expired resources

### Performance Metrics

```rust
pub struct ManagerStats {
    pub uptime: Duration,
    pub peers_connected: usize,
    pub messages_sent: u64,
    pub messages_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

pub struct SecurityStats {
    pub banned_peers_count: usize,
    pub rate_limit_violations: u64,
    pub dos_protection_blocks: u64,
    pub encryption_cache_stats: CacheStats,
}
```

## Extension Points

The architecture provides several extension points:

### Custom Message Types

```rust
// Add custom message payload types
#[derive(Serialize, Deserialize)]
pub enum CustomMessagePayload {
    MyCustomMessage { data: String },
}

// Extend MessagePayload enum
MessagePayload::Custom(serde_json::to_value(custom_payload)?)
```

### Custom Authentication Methods

```rust
// Implement custom authentication
impl AuthManager {
    pub async fn authenticate_custom(&self, token: &str) -> AuthResult {
        // Custom authentication logic
    }
}
```

### Custom Transport Protocols

```rust
// Add new transport protocols
impl UnifiedTransport {
    pub fn add_transport_protocol(&mut self, protocol: Box<dyn TransportProtocol>) {
        // Add custom transport
    }
}
```

### Custom Message Routing

```rust
// Implement custom routing strategies
#[derive(Debug, Clone)]
pub enum CustomRoutingStrategy {
    GeographicRouting { region: String },
    CapabilityBasedRouting { required_capability: String },
}
```

### Monitoring Extensions

```rust
// Custom metrics collection
pub trait MetricsCollector {
    fn collect_custom_metrics(&self) -> HashMap<String, f64>;
}
```

## Future Enhancements

### Planned Features

1. **Dynamic Protocol Negotiation**: Automatically negotiate optimal protocols
2. **Adaptive Rate Limiting**: ML-based rate limit adjustment
3. **Geographic Routing**: Location-aware message routing
4. **Mesh Networking**: Automatic mesh topology formation
5. **Cross-Chain Bridges**: Direct integration with other blockchain networks

### Scalability Improvements

1. **Sharding Support**: Horizontal scaling through network sharding
2. **Layer 2 Integration**: Support for Layer 2 scaling solutions
3. **Edge Computing**: Edge node support for reduced latency
4. **CDN Integration**: Content delivery network integration

This architecture provides a solid foundation for secure, scalable, and maintainable P2P networking while remaining flexible enough to accommodate future requirements and extensions.