# MultiVM P2P Networking Layer

A comprehensive peer-to-peer networking solution for the MultiVM blockchain system, enabling distributed consensus and communication across different virtual machines (Ethereum VM and Solana VM).

## Overview

The MultiVM P2P networking layer provides a unified interface for distributed communication, supporting:

- **Cross-VM Communication**: Seamless message routing between Ethereum VM and Solana VM nodes
- **Distributed Consensus**: Integration with Malachite consensus for Byzantine fault tolerance
- **Peer Discovery**: Automatic discovery and connection management using mDNS and Kademlia DHT
- **Message Propagation**: Efficient gossip protocol for broadcasting consensus messages
- **Security**: End-to-end encryption, authentication, and rate limiting
- **Fault Tolerance**: Circuit breaker patterns and connection resilience
- **Performance**: Load balancing, connection pooling, and adaptive routing

## Architecture

```text
┌─────────────────────────────────────────────────────────────────┐
│                    MultiVM P2P Manager                          │
├─────────────────┬─────────────────┬─────────────────────────────┤
│   Discovery     │   Gossip        │   Security & Rate Limiting  │
│   Service       │   Protocol      │                             │
├─────────────────┼─────────────────┼─────────────────────────────┤
│   Message       │   Load          │   Circuit Breaker &         │
│   Router        │   Balancer      │   Connection Manager        │
├─────────────────┴─────────────────┴─────────────────────────────┤
│                    libp2p Network Layer                         │
│              (Gossipsub, Kademlia, mDNS, TCP)                  │
└─────────────────────────────────────────────────────────────────┘
```

## Key Components

### P2P Manager (`manager.rs`)
Central coordinator that orchestrates all P2P networking components:
- Unified interface for consensus integration
- Command/event architecture for async operations
- Health monitoring and statistics collection
- Component lifecycle management

### Gossip Protocol (`gossip.rs`)
Epidemic-style message propagation system:
- Priority-based message routing
- Duplicate detection and loop prevention
- Adaptive fanout based on peer reliability
- Message compression and TTL management

### Discovery Service (`discovery.rs`)
Peer discovery and connection management:
- mDNS for local network discovery
- Kademlia DHT for global peer routing
- Bootstrap peer management
- Peer lifecycle tracking

### Message Router (`routing.rs`)
Intelligent message routing with multiple strategies:
- Direct peer routing
- Broadcast routing
- DHT-based routing
- Gossip propagation
- Random routing for load distribution

### Load Balancer (`load_balancer.rs`)
Advanced load balancing for optimal performance:
- Round-robin distribution
- Least connections strategy
- Adaptive routing based on peer performance
- Health-based peer selection

### Security Layer (`security.rs`)
Comprehensive security features:
- Ed25519 authentication
- ChaCha20-Poly1305 encryption
- Message validation and integrity
- Rate limiting and DoS protection

### Circuit Breaker (`circuit_breaker.rs`)
Fault tolerance and resilience:
- Automatic failure detection
- Circuit state management (Closed/Open/HalfOpen)
- Configurable failure thresholds
- Self-healing capabilities

## Usage

### Basic Setup

```rust
use multivm_p2p::{P2PManager, P2PConfig};
use libp2p::identity::Keypair;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();
    
    // Generate keypair for this node
    let keypair = Keypair::generate_ed25519();
    
    // Create P2P configuration (using defaults with some customization)
    let mut config = P2PConfig::default();
    config.network.listen_addresses = vec!["/ip4/0.0.0.0/tcp/0".to_string()];
    config.network.max_connections = 50;
    config.security.rate_limiting.enabled = true;
    
    // Create and start P2P manager
    let mut p2p_manager = P2PManager::new(config, keypair).await?;
    p2p_manager.start().await?;
    
    println!("P2P node started successfully!");
    
    // Use the P2P network...
    // (See examples/ directory for more detailed usage)
    
    // Graceful shutdown
    p2p_manager.shutdown().await?;
    
    Ok(())
}
```

### Sending Messages

```rust
use multivm_p2p::protocol::messages::*;

// Create a cross-VM state sync message
let message = NetworkMessage::new(
    MessagePayload::MultiVm(MultiVmMessage::StateSync {
        state_root: "0x123456789abcdef".to_string(),
        vm_type: VmType::Svm,
        height: 1000,
    }),
    MessageSource::MultiVmLayer,
    MessageTarget::Broadcast,
);

// Send message with priority
p2p_manager.send_message(message, Priority::High).await?;

// Create a heartbeat message
let heartbeat = NetworkMessage::new(
    MessagePayload::Control(ControlMessage::Heartbeat {
        status: NodeStatus::Active,
        uptime: std::time::Duration::from_secs(3600),
    }),
    MessageSource::NetworkLayer,
    MessageTarget::Broadcast,
);

p2p_manager.send_message(heartbeat, Priority::Low).await?;
```

### Handling Events

```rust
use multivm_p2p::{P2PEvent, P2PCommand};

// Get event receiver
let mut event_receiver = p2p_manager.get_event_receiver().await;

// Handle events
while let Some(event) = event_receiver.recv().await {
    match event {
        P2PEvent::MessageReceived { message, from_peer } => {
            println!("Received message from {}: {:?}", from_peer, message);
        }
        P2PEvent::PeerConnected { peer_id } => {
            println!("Peer connected: {}", peer_id);
        }
        P2PEvent::PeerDisconnected { peer_id } => {
            println!("Peer disconnected: {}", peer_id);
        }
        _ => {}
    }
}
```

### Integration with Consensus

```rust
use multivm_p2p::P2PNetworkLayer;

// Implement consensus integration
struct ConsensusNode {
    p2p: P2PManager,
}

impl ConsensusNode {
    async fn handle_consensus_message(&mut self, message: ConsensusMessage) -> Result<(), Error> {
        // Convert consensus message to network message
        let network_message = NetworkMessage::new(
            MessagePayload::Consensus(message),
            MessageSource::Consensus,
            MessageTarget::Broadcast,
        );
        
        // Broadcast through P2P network
        self.p2p.broadcast(network_message).await?;
        Ok(())
    }
}
```

## Configuration

### P2P Configuration

```rust
use multivm_p2p::P2PConfig;
use std::time::Duration;

let config = P2PConfig {
    // Network addresses to listen on
    listen_addresses: vec![
        "/ip4/0.0.0.0/tcp/4001".to_string(),
        "/ip6/::/tcp/4001".to_string(),
    ],
    
    // Bootstrap peers for initial connection
    bootstrap_peers: vec![
        "/ip4/127.0.0.1/tcp/4001/p2p/12D3KooW...".parse().unwrap(),
    ],
    
    // Connection limits
    max_peers: 100,
    connection_timeout: Duration::from_secs(30),
    
    // Health monitoring
    heartbeat_interval: Duration::from_secs(5),
    
    // Protocol features
    enable_mdns: true,        // Local discovery
    enable_kademlia: true,    // DHT routing
    enable_gossipsub: true,   // Message broadcasting
    enable_metrics: true,     // Performance monitoring
};
```

### Gossip Configuration

```rust
use multivm_p2p::GossipConfig;
use std::time::Duration;

let gossip_config = GossipConfig {
    fanout: 6,                                    // Peers to gossip to per round
    gossip_interval: Duration::from_millis(100),  // Gossip frequency
    message_ttl: Duration::from_secs(300),        // Message lifetime
    max_cache_size: 10000,                        // Duplicate detection cache
    duplicate_window: Duration::from_secs(60),    // Duplicate detection window
    enable_compression: true,                     // Message compression
    enable_priority_propagation: true,            // Priority-based routing
    heartbeat_interval: Duration::from_secs(1),   // Peer health checks
    max_retransmissions: 3,                       // Retry attempts
};
```

### Security Configuration

```rust
use multivm_p2p::SecurityConfig;
use std::time::Duration;

let security_config = SecurityConfig {
    enable_encryption: true,
    enable_authentication: true,
    key_rotation_interval: Duration::from_hours(24),
    max_message_size: 1024 * 1024, // 1MB
    rate_limit_per_peer: 100,       // Messages per second
    rate_limit_global: 1000,        // Total messages per second
    ban_duration: Duration::from_minutes(10),
};
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run integration tests
cargo test --test integration_tests

# Run performance benchmarks
cargo test --release --ignored

# Run with logging
RUST_LOG=debug cargo test
```

### Test Coverage

The test suite includes:

- **Unit Tests**: Individual component testing
- **Integration Tests**: Multi-node scenarios
- **Performance Tests**: Throughput and latency benchmarks
- **Resilience Tests**: Network partitioning and recovery
- **Security Tests**: Authentication and encryption validation

### Example Test Scenarios

```rust
#[tokio::test]
async fn test_multi_node_consensus() {
    // Create 4-node network
    let nodes = create_test_network(4).await;
    
    // Simulate consensus round
    let proposal = create_test_proposal();
    nodes[0].broadcast_proposal(proposal).await?;
    
    // Verify all nodes received proposal
    for node in &nodes[1..] {
        assert!(node.has_received_proposal().await);
    }
}
```

## Performance

### Benchmarks

Typical performance characteristics:

- **Message Throughput**: 10,000+ messages/second
- **Peer Discovery**: <5 seconds for 100 peers
- **Message Latency**: <10ms in local network
- **Memory Usage**: <100MB for 1000 peers
- **CPU Usage**: <5% during normal operation

### Optimization Tips

1. **Tune Gossip Parameters**: Adjust fanout and intervals based on network size
2. **Enable Compression**: Reduces bandwidth usage for large messages
3. **Configure Rate Limits**: Prevent DoS attacks and resource exhaustion
4. **Use Priority Routing**: Ensure critical messages are delivered first
5. **Monitor Metrics**: Track performance and adjust configuration

## Monitoring

### Available Metrics

```rust
let stats = p2p_manager.get_stats().await;
println!("Connected peers: {}", stats.connected_peers);
println!("Messages sent: {}", stats.messages_sent);
println!("Messages received: {}", stats.messages_received);
println!("Average latency: {:.2}ms", stats.avg_latency_ms);
```

### Health Checks

```rust
// Check if P2P layer is healthy
if p2p_manager.is_healthy().await {
    println!("P2P network is operating normally");
} else {
    println!("P2P network issues detected");
}
```

## Troubleshooting

### Common Issues

1. **Peer Discovery Failures**
   - Check firewall settings
   - Verify bootstrap peers are reachable
   - Enable mDNS for local discovery

2. **Message Delivery Issues**
   - Check rate limiting configuration
   - Verify message size limits
   - Monitor circuit breaker status

3. **Performance Problems**
   - Tune gossip parameters
   - Check network bandwidth
   - Monitor CPU and memory usage

### Debug Logging

```bash
# Enable detailed logging
RUST_LOG=multivm_p2p=debug cargo run

# Focus on specific components
RUST_LOG=multivm_p2p::gossip=trace cargo run
```

## Contributing

### Development Setup

```bash
# Clone repository
git clone https://github.com/your-org/multivm-process
cd multivm-process/crates/multivm-p2p

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy
```

### Adding New Features

1. Implement the feature with comprehensive tests
2. Update documentation and examples
3. Add performance benchmarks if applicable
4. Ensure backward compatibility

## Documentation

For detailed documentation, see:

- **[Getting Started Guide](docs/GETTING_STARTED.md)** - Quick setup and basic usage
- **[API Reference](docs/API.md)** - Complete API documentation  
- **[Architecture Guide](docs/ARCHITECTURE.md)** - Detailed architecture and design principles
- **[Examples](examples/)** - Working code examples for various use cases

## License

This project is licensed under the MIT License - see the LICENSE file for details.