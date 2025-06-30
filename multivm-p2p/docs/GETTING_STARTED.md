# Getting Started with MultiVM P2P

This guide will help you get up and running with the MultiVM P2P networking layer.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Basic Examples](#basic-examples)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)

## Prerequisites

### System Requirements

- **Rust**: 1.70.0 or later
- **Operating System**: Linux, macOS, or Windows
- **Memory**: Minimum 2GB RAM (4GB recommended)
- **Network**: Stable internet connection for peer discovery

### Dependencies

The P2P layer requires the following major dependencies:

- `libp2p`: Core P2P networking functionality
- `tokio`: Async runtime
- `serde`: Serialization/deserialization
- `tracing`: Logging and instrumentation

## Installation

### Adding to Your Project

Add the following to your `Cargo.toml`:

```toml
[dependencies]
multivm-p2p = { path = "../multivm-p2p" }
tokio = { version = "1.0", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

### Development Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/your-org/multivm-process
   cd multivm-process/multivm-p2p
   ```

2. **Install Rust (if not already installed):**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

3. **Build the project:**
   ```bash
   cargo build
   ```

4. **Run tests:**
   ```bash
   cargo test
   ```

## Quick Start

### Minimal Example

Here's the absolute minimum code needed to start a P2P node:

```rust
use multivm_p2p::{P2PManager, P2PConfig};
use libp2p::identity::Keypair;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::init();
    
    // Create default configuration
    let config = P2PConfig::default();
    
    // Generate a keypair for this node
    let keypair = Keypair::generate_ed25519();
    
    // Create and start P2P manager
    let mut p2p_manager = P2PManager::new(config, keypair).await?;
    p2p_manager.start().await?;
    
    println!("P2P node started successfully!");
    
    // Keep the node running
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    
    // Graceful shutdown
    p2p_manager.shutdown().await?;
    
    Ok(())
}
```

### Running the Example

Save the code above as `main.rs` and run:

```bash
cargo run
```

You should see output similar to:

```
2024-01-01T12:00:00.000Z INFO multivm_p2p::core::manager: P2P Manager created successfully
2024-01-01T12:00:00.001Z INFO multivm_p2p::core::manager: P2P Manager started
P2P node started successfully!
```

## Configuration

### Basic Configuration

Start with a basic configuration and customize as needed:

```rust
use multivm_p2p::config::*;
use std::time::Duration;

fn create_config() -> P2PConfig {
    P2PConfig {
        network: NetworkConfig {
            listen_addresses: vec![
                "/ip4/0.0.0.0/tcp/4001".to_string(),
                "/ip4/0.0.0.0/tcp/4002/ws".to_string(),
            ],
            max_connections: 50,
            connection_timeout: Duration::from_secs(10),
            ..Default::default()
        },
        security: SecurityConfig {
            enable_noise: true,
            rate_limiting: RateLimitConfig {
                enabled: true,
                max_requests_per_second: 100.0,
                burst_size: 20,
            },
            authentication: AuthConfig {
                enabled: true,
                method: AuthMethod::Ed25519,
                timeout: Duration::from_secs(30),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}
```

### Environment-Based Configuration

For different environments, create configuration functions:

```rust
// Development configuration
pub fn dev_config() -> P2PConfig {
    P2PConfig {
        network: NetworkConfig {
            listen_addresses: vec!["/ip4/127.0.0.1/tcp/0".to_string()],
            max_connections: 10,
            ..Default::default()
        },
        security: SecurityConfig {
            rate_limiting: RateLimitConfig {
                enabled: false, // Disable for local testing
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}

// Production configuration
pub fn prod_config() -> P2PConfig {
    P2PConfig {
        network: NetworkConfig {
            listen_addresses: vec![
                "/ip4/0.0.0.0/tcp/4001".to_string(),
                "/ip6/::/tcp/4001".to_string(),
            ],
            max_connections: 1000,
            connection_timeout: Duration::from_secs(30),
            enable_nat_traversal: true,
            ..Default::default()
        },
        security: SecurityConfig {
            enable_noise: true,
            rate_limiting: RateLimitConfig {
                enabled: true,
                max_requests_per_second: 1000.0,
                burst_size: 100,
            },
            authentication: AuthConfig {
                enabled: true,
                method: AuthMethod::Ed25519,
                timeout: Duration::from_secs(60),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }
}
```

## Basic Examples

### Example 1: Two-Node Network

Create two nodes that can communicate with each other:

**Node 1:**
```rust
use multivm_p2p::{P2PManager, P2PConfig, protocol::messages::*};
use libp2p::identity::Keypair;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::init();
    
    let mut config = P2PConfig::default();
    config.network.listen_addresses = vec!["/ip4/127.0.0.1/tcp/4001".to_string()];
    
    let keypair = Keypair::generate_ed25519();
    let mut p2p_manager = P2PManager::new(config, keypair).await?;
    p2p_manager.start().await?;
    
    println!("Node 1 started on 127.0.0.1:4001");
    
    // Send a message every 5 seconds
    loop {
        let message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::Heartbeat {
                status: NodeStatus::Active,
                uptime: Duration::from_secs(60),
            }),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );
        
        p2p_manager.send_message(message, Priority::Low).await?;
        println!("Sent heartbeat message");
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
```

**Node 2:**
```rust
use multivm_p2p::{P2PManager, P2PConfig};
use libp2p::{identity::Keypair, multiaddr::multiaddr};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::init();
    
    let mut config = P2PConfig::default();
    config.network.listen_addresses = vec!["/ip4/127.0.0.1/tcp/4002".to_string()];
    
    let keypair = Keypair::generate_ed25519();
    let mut p2p_manager = P2PManager::new(config, keypair).await?;
    p2p_manager.start().await?;
    
    println!("Node 2 started on 127.0.0.1:4002");
    
    // Connect to Node 1
    let node1_addr = multiaddr!(Ip4([127, 0, 0, 1]), Tcp(4001u16));
    // Note: You would need to get Node 1's PeerId to connect
    // This is simplified for the example
    
    // Keep node running
    tokio::time::sleep(Duration::from_secs(300)).await;
    
    p2p_manager.shutdown().await?;
    Ok(())
}
```

### Example 2: Message Handling

Handle incoming messages from other peers:

```rust
use multivm_p2p::{P2PManager, P2PConfig, protocol::messages::*};
use libp2p::identity::Keypair;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::init();
    
    let config = P2PConfig::default();
    let keypair = Keypair::generate_ed25519();
    let mut p2p_manager = P2PManager::new(config, keypair).await?;
    p2p_manager.start().await?;
    
    println!("Node started, waiting for messages...");
    
    // In a real application, you would set up an event receiver
    // to handle incoming messages. The current implementation
    // handles messages internally through the coordinators.
    
    // Send some test messages
    for i in 0..5 {
        let message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        )
        .with_metadata("sequence", &i.to_string());
        
        p2p_manager.send_message(message, Priority::Normal).await?;
        println!("Sent test message {}", i);
        
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    
    // Get statistics
    let stats = p2p_manager.get_stats().await?;
    println!("Stats: {:?}", stats);
    
    p2p_manager.shutdown().await?;
    Ok(())
}
```

### Example 3: Cross-VM Communication

Demonstrate cross-VM message types:

```rust
use multivm_p2p::{P2PManager, P2PConfig, protocol::messages::*};
use libp2p::identity::Keypair;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::init();
    
    let config = P2PConfig::default();
    let keypair = Keypair::generate_ed25519();
    let mut p2p_manager = P2PManager::new(config, keypair).await?;
    p2p_manager.start().await?;
    
    // Send SVM transaction
    let svm_tx = NetworkMessage::new(
        MessagePayload::Svm(SvmMessage::Transaction {
            transaction_data: Box::new(vec![1, 2, 3, 4]),
            signature: "svm_signature".to_string(),
        }),
        MessageSource::SvmExecution,
        MessageTarget::Broadcast,
    );
    p2p_manager.send_message(svm_tx, Priority::High).await?;
    
    // Send EVM transaction
    let evm_tx = NetworkMessage::new(
        MessagePayload::Evm(EvmMessage::Transaction {
            transaction_data: Box::new(vec![5, 6, 7, 8]),
            tx_hash: "0xevm_hash".to_string(),
        }),
        MessageSource::EvmExecution,
        MessageTarget::Broadcast,
    );
    p2p_manager.send_message(evm_tx, Priority::High).await?;
    
    // Send cross-VM state sync
    let state_sync = NetworkMessage::new(
        MessagePayload::MultiVm(MultiVmMessage::StateSync {
            state_root: "0x123456789abcdef".to_string(),
            vm_type: VmType::Svm,
            height: 1000,
        }),
        MessageSource::MultiVmLayer,
        MessageTarget::Broadcast,
    );
    p2p_manager.send_message(state_sync, Priority::Critical).await?;
    
    println!("Sent cross-VM messages");
    
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    p2p_manager.shutdown().await?;
    
    Ok(())
}
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test security

# Run integration tests
cargo test --test integration_tests

# Run with debug logging
RUST_LOG=debug cargo test
```

### Writing Tests

Here's how to write tests for your P2P code:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use multivm_p2p::config::*;
    
    #[tokio::test]
    async fn test_p2p_manager_creation() {
        let config = P2PConfig::default();
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        
        let result = P2PManager::new(config, keypair).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_message_sending() {
        let config = P2PConfig::default();
        let keypair = libp2p::identity::Keypair::generate_ed25519();
        let mut manager = P2PManager::new(config, keypair).await.unwrap();
        
        manager.start().await.unwrap();
        
        let message = NetworkMessage::new(
            MessagePayload::Control(ControlMessage::StatusRequest),
            MessageSource::NetworkLayer,
            MessageTarget::Broadcast,
        );
        
        let result = manager.send_message(message, Priority::Normal).await;
        assert!(result.is_ok());
        
        manager.shutdown().await.unwrap();
    }
}
```

## Troubleshooting

### Common Issues

#### 1. Port Already in Use

**Error:**
```
Error: Os { code: 48, kind: AddrInUse, message: "Address already in use" }
```

**Solution:**
```rust
// Use port 0 to let the system choose an available port
config.network.listen_addresses = vec!["/ip4/127.0.0.1/tcp/0".to_string()];
```

#### 2. Permission Denied

**Error:**
```
Error: Os { code: 13, kind: PermissionDenied, message: "Permission denied" }
```

**Solution:**
```bash
# On Linux/macOS, use ports > 1024 or run with sudo
# Better: use unprivileged ports
config.network.listen_addresses = vec!["/ip4/127.0.0.1/tcp/8001".to_string()];
```

#### 3. Connection Refused

**Error:**
```
Connection to peer failed: connection refused
```

**Solutions:**
- Check if the target peer is actually running
- Verify firewall settings
- Ensure correct peer ID and address

#### 4. Rate Limiting Issues

**Error:**
```
Rate limit exceeded for peer
```

**Solution:**
```rust
// Adjust rate limiting configuration
config.security.rate_limiting = RateLimitConfig {
    enabled: true,
    max_requests_per_second: 1000.0, // Increase limit
    burst_size: 100,                 // Increase burst
};
```

### Debug Logging

Enable detailed logging to troubleshoot issues:

```bash
# Enable all P2P logging
RUST_LOG=multivm_p2p=debug cargo run

# Enable specific component logging
RUST_LOG=multivm_p2p::security=trace cargo run

# Enable libp2p logging
RUST_LOG=libp2p=debug cargo run

# Combine multiple loggers
RUST_LOG=multivm_p2p=debug,libp2p=info cargo run
```

### Performance Issues

If you experience performance problems:

1. **Check resource usage:**
   ```bash
   # Monitor CPU and memory usage
   top -p $(pgrep your-app)
   ```

2. **Tune configuration:**
   ```rust
   // Increase connection limits
   config.network.max_connections = 1000;
   
   // Adjust message batching
   config.transport.message_batch_size = 50;
   
   // Enable compression
   config.transport.enable_compression = true;
   ```

3. **Monitor statistics:**
   ```rust
   let stats = p2p_manager.get_stats().await?;
   println!("Messages/sec: {}", stats.messages_sent / stats.uptime.as_secs());
   ```

### Getting Help

If you encounter issues not covered here:

1. Check the [API documentation](API.md)
2. Review the [architecture documentation](ARCHITECTURE.md)
3. Look at the [examples directory](../examples/)
4. File an issue on the project repository

## Next Steps

Once you have basic P2P functionality working:

1. **Explore Advanced Features:**
   - Security configuration
   - Custom message types
   - Monitoring and metrics

2. **Integration:**
   - Integrate with consensus layer
   - Add application-specific message handlers
   - Implement custom routing strategies

3. **Production Deployment:**
   - Configure for your network topology
   - Set up monitoring and alerting
   - Test under load conditions

4. **Contributing:**
   - Read the contribution guidelines
   - Submit bug reports and feature requests
   - Contribute improvements and optimizations