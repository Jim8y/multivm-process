# MultiVM P2P Network

Peer-to-peer networking layer for the MultiVM blockchain execution platform.

## Overview

The MultiVM P2P module provides the networking foundation for distributed communication between MultiVM nodes. It implements a custom P2P protocol optimized for cross-VM blockchain operations with built-in security and reliability features.

## Features

### 🌐 Network Architecture
- **Custom Protocol**: Optimized for MultiVM operations
- **Peer Discovery**: Automatic peer finding and connection
- **Message Routing**: Efficient message propagation
- **Network Topology**: Flexible mesh networking

### 🔒 Security
- **Authenticated Connections**: Peer identity verification
- **Encrypted Transport**: Optional TLS encryption
- **DOS Protection**: Rate limiting and connection limits
- **Message Validation**: Cryptographic message verification

### 📡 Communication
- **Reliable Delivery**: Message acknowledgment system
- **Prioritization**: QoS for critical messages
- **Compression**: Optional message compression
- **Multiplexing**: Multiple streams per connection

## Architecture

```
┌─────────────────────────────────────────┐
│            P2P Network Layer            │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │  Transport  │    │  Discovery  │   │
│   │    Layer    │    │   Service   │   │
│   └─────────────┘    └─────────────┘   │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │   Message   │    │   Routing   │   │
│   │  Protocol   │    │   Engine    │   │
│   └─────────────┘    └─────────────┘   │
└─────────────────────────────────────────┘
```

## Usage

### Basic Network Setup

```rust
use multivm_p2p::*;

// Create P2P configuration
let config = P2PConfig {
    listen_addr: "/ip4/0.0.0.0/tcp/9000".parse()?,
    external_addr: Some("/ip4/1.2.3.4/tcp/9000".parse()?),
    bootstrap_peers: vec![
        "/ip4/5.6.7.8/tcp/9000/p2p/QmBootstrap1".parse()?,
        "/ip4/9.10.11.12/tcp/9000/p2p/QmBootstrap2".parse()?,
    ],
    max_peers: 50,
    enable_mdns: true,
};

// Initialize P2P network
let network = P2PNetwork::new(config).await?;

// Start network
network.start().await?;
```

### Message Handling

```rust
// Define message handler
network.on_message(|peer_id, message| async move {
    match message {
        NetworkMessage::Block(block) => {
            println!("Received block from {}: {:?}", peer_id, block);
        }
        NetworkMessage::Transaction(tx) => {
            println!("Received transaction from {}: {:?}", peer_id, tx);
        }
        _ => {}
    }
});

// Send message to peer
let message = NetworkMessage::new_block_announcement(block_hash);
network.send_to_peer(peer_id, message).await?;

// Broadcast to all peers
network.broadcast(message).await?;
```

### Peer Management

```rust
// Get connected peers
let peers = network.connected_peers().await;
println!("Connected to {} peers", peers.len());

// Get peer info
if let Some(info) = network.peer_info(&peer_id).await {
    println!("Peer {}: latency={}ms", peer_id, info.latency_ms);
}

// Disconnect peer
network.disconnect_peer(&peer_id).await?;
```

## Configuration

### Network Configuration

```toml
[p2p]
# Network identity
peer_id = "auto" # or specific peer ID

# Listening address
listen_addr = "/ip4/0.0.0.0/tcp/9000"

# External address (for NAT traversal)
external_addr = "/ip4/YOUR_PUBLIC_IP/tcp/9000"

# Bootstrap nodes
bootstrap_peers = [
    "/ip4/boot1.multivm.network/tcp/9000/p2p/QmBoot1...",
    "/ip4/boot2.multivm.network/tcp/9000/p2p/QmBoot2...",
]

# Peer limits
max_peers = 50
max_inbound = 25
max_outbound = 25

# Discovery
enable_mdns = true
enable_kad = true

# Security
enable_noise = true
enable_tls = false
```

## Message Types

### Core Messages

```rust
pub enum NetworkMessage {
    // Block propagation
    Block(MultiVMBlock),
    BlockAnnouncement(BlockHash),
    BlockRequest(BlockHash),
    
    // Transaction propagation
    Transaction(MultivmTransaction),
    TransactionBatch(Vec<MultivmTransaction>),
    
    // State synchronization
    StateRequest(StateQuery),
    StateResponse(StateData),
    
    // Consensus messages
    ConsensusMessage(ConsensusPayload),
    
    // Peer management
    Ping(u64),
    Pong(u64),
    PeerInfo(PeerMetadata),
}
```

## Implementation Status

✅ **Functional Stub Implementation**
- Basic P2P network structure
- Message type definitions
- Peer management interface
- Network configuration
- Ready for production implementation

⚠️ **Note**: Current implementation is a functional stub. Production implementation will require:
- Actual libp2p integration
- Real peer discovery
- Message routing implementation
- Network security features

## Testing

```bash
# Run unit tests
cargo test -p multivm-p2p

# Run network simulation
cargo test -p multivm-p2p --test network_simulation

# Run with debug logging
RUST_LOG=debug cargo test -p multivm-p2p
```

## Future Enhancements

- **libp2p Integration**: Full libp2p implementation
- **DHT Support**: Distributed hash table for peer discovery
- **NAT Traversal**: STUN/TURN support
- **Gossip Protocol**: Efficient message propagation
- **Sharding Support**: Network sharding for scalability

## License

Licensed under either Apache 2.0 or MIT license at your option.