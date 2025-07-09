# Communication Protocol Abstraction Layer

This module provides a flexible, trait-based abstraction layer for communication between MultiVM and execution engines (Reth for Ethereum, Solana). It supports multiple communication protocols including IPC (Inter-Process Communication), RPC (Remote Procedure Call), and JWT-authenticated HTTP.

## Architecture

The communication layer follows a modular design with these key components:

### Core Traits

- **`CommunicationProtocol`**: The main trait that all protocol implementations must satisfy
- **`ProtocolFactory`**: Factory trait for creating protocol instances
- **`ProtocolManager`**: Manages multiple protocols and handles failover

### Protocol Implementations

1. **IPC Protocol** (`ipc_protocol.rs`)
   - Unix domain sockets (Linux/macOS)
   - TCP sockets (cross-platform fallback)
   - Binary message format with bincode serialization
   - Connection pooling and automatic reconnection

2. **RPC Protocol** (`rpc_protocol.rs`)
   - JSON-RPC 2.0 over HTTP/HTTPS
   - WebSocket support planned
   - Automatic retry with exponential backoff
   - Health checking and connection monitoring

3. **JWT Protocol** (`jwt_protocol.rs`)
   - JWT-authenticated HTTP endpoints
   - Automatic token generation and refresh
   - Support for API key authentication fallback
   - HMAC-SHA256 token signing

## Usage Examples

### Basic Setup

```rust
use multivm_common::communication::{
    CommunicationManager, ProtocolFactory, DefaultProtocolFactory,
    ProtocolType, EngineType, CommunicationRequest
};
use serde_json::json;

// Create a protocol factory
let factory = DefaultProtocolFactory::new();

// Create a communication manager
let mut manager = CommunicationManager::new();

// Add protocols for Ethereum
let eth_ipc = factory.create_protocol(
    ProtocolType::Ipc,
    EngineType::Ethereum,
    json!({})  // Use default configuration
).await?;

manager.add_protocol(eth_ipc).await?;
```

### Sending Requests

```rust
// Create a request
let request = CommunicationRequest {
    id: "req-123".to_string(),
    method: "eth_chainId".to_string(),
    params: json!([]),
    timeout: Some(Duration::from_secs(30)),
    auth_context: None,
};

// Send to specific engine
let response = manager.send_to_engine(EngineType::Ethereum, request).await?;

if let Some(result) = response.result {
    println!("Chain ID: {}", result);
}
```

### Protocol Configuration

Each protocol supports detailed configuration:

```rust
use multivm_common::communication::{
    ipc_protocol::IpcProtocolConfig,
    rpc_protocol::RpcProtocolConfig,
    jwt_protocol::JwtProtocolConfig,
};

// IPC Configuration
let ipc_config = IpcProtocolConfig {
    transport_config: TransportConfig {
        unix_socket_path: Some("/tmp/multivm-ethereum.sock".to_string()),
        tcp_host: "127.0.0.1".to_string(),
        tcp_port: 8545,
        prefer_unix_socket: true,
    },
    connection_config: ConnectionConfig {
        max_connections: 10,
        connect_timeout: Duration::from_secs(10),
        read_timeout: Duration::from_secs(30),
        write_timeout: Duration::from_secs(30),
        reconnect_delay: Duration::from_secs(5),
        max_reconnect_attempts: 3,
    },
    ..Default::default()
};

// RPC Configuration
let rpc_config = RpcProtocolConfig {
    endpoint_url: "http://127.0.0.1:8545".to_string(),
    retry_config: RetryConfig {
        max_retries: 3,
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(5),
        backoff_multiplier: 2.0,
        jitter: true,
    },
    ..Default::default()
};

// JWT Configuration
let jwt_config = JwtProtocolConfig {
    endpoint_url: "http://127.0.0.1:8551".to_string(),
    jwt_secret: "your-secret-key".to_string(),
    token_expiry: Duration::from_secs(3600),
    ..Default::default()
};
```

### Advanced Features

#### Protocol Fallback

The manager supports automatic fallback between protocols:

```rust
// Set protocol preferences
manager.set_protocol_preference(
    EngineType::Ethereum,
    vec![ProtocolType::Ipc, ProtocolType::Rpc, ProtocolType::Jwt]
);

// Manager will try IPC first, then RPC, then JWT
let response = manager.send_to_engine_with_fallback(
    EngineType::Ethereum,
    request
).await?;
```

#### Health Monitoring

All protocols support health checking:

```rust
// Get protocol health status
let health = protocol.health_check().await?;
println!("Protocol healthy: {}", health.is_healthy);
println!("Latency: {:?}", health.metrics.get("latency_ms"));

// Connection status
let status = protocol.get_connection_status().await?;
println!("Connected: {}", status.is_connected);
println!("Success count: {}", status.success_count);
```

#### Custom Authentication

JWT protocol supports custom authentication contexts:

```rust
use multivm_common::communication::AuthContext;

let auth = AuthContext {
    token: Some("custom-jwt-token".to_string()),
    api_key: None,
    headers: HashMap::from([
        ("X-Custom-Header".to_string(), "value".to_string())
    ]),
};

let request = CommunicationRequest {
    id: "req-456".to_string(),
    method: "/api/submit".to_string(),
    params: json!({"data": "example"}),
    timeout: None,
    auth_context: Some(auth),
};
```

## Error Handling

The communication layer uses the unified `MultivmError` type with specific error variants:

```rust
match manager.send_to_engine(engine, request).await {
    Ok(response) => {
        if let Some(error) = response.error {
            eprintln!("Engine error: {} (code: {})", error.message, error.code);
        }
    }
    Err(MultivmError::Network { message, .. }) => {
        eprintln!("Network error: {}", message);
    }
    Err(MultivmError::Timeout { .. }) => {
        eprintln!("Request timed out");
    }
    Err(e) => {
        eprintln!("Other error: {}", e);
    }
}
```

## Testing

The module includes comprehensive unit tests for all protocols:

```bash
# Run all communication tests
cargo test --package multivm-common --lib communication::tests

# Run specific protocol tests
cargo test --package multivm-common --lib communication::tests::ipc_protocol_tests
cargo test --package multivm-common --lib communication::tests::rpc_protocol_tests
cargo test --package multivm-common --lib communication::tests::jwt_protocol_tests
```

## Performance Considerations

1. **Connection Pooling**: IPC and RPC protocols maintain connection pools to reduce latency
2. **Binary Serialization**: IPC uses bincode for efficient binary serialization
3. **Automatic Reconnection**: All protocols handle connection failures gracefully
4. **Request Batching**: Future enhancement for batch request support

## Security

1. **JWT Authentication**: JWT protocol uses HMAC-SHA256 for token signing
2. **TLS Support**: RPC and JWT protocols support TLS encryption
3. **API Key Fallback**: JWT protocol supports API key authentication when JWT is not available
4. **Secure Defaults**: All protocols use secure defaults (e.g., token expiry, timeouts)

## Future Enhancements

- WebSocket support for RPC protocol
- Request/response compression
- Circuit breaker pattern implementation
- Metrics and observability integration
- gRPC protocol support