# Communication Protocol Abstraction

This module provides a unified interface for communicating with execution engines (Reth for Ethereum, Solana) through multiple protocols.

## Architecture

```
┌─────────────────────────────────────────────────┐
│          Communication Manager                   │
├─────────────────────────────────────────────────┤
│  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│  │   IPC    │  │   RPC    │  │   JWT    │     │
│  │ Protocol │  │ Protocol │  │ Protocol │     │
│  └──────────┘  └──────────┘  └──────────┘     │
└─────────────────────────────────────────────────┘
         ↓              ↓              ↓
    Unix/TCP      HTTP/JSON-RPC   Authenticated
    Sockets                         HTTP API
```

## Supported Protocols

### IPC (Inter-Process Communication)
- **Use Case**: High-performance local communication
- **Transports**: Unix domain sockets (preferred), TCP sockets (fallback)
- **Features**: 
  - Binary message serialization with bincode
  - Automatic reconnection
  - Health monitoring
  - Message framing with length prefixes

### RPC (Remote Procedure Call)
- **Use Case**: Standard JSON-RPC communication
- **Transport**: HTTP/1.1 and HTTP/2
- **Features**:
  - Connection pooling
  - Rate limit handling
  - Automatic retries with exponential backoff
  - Support for both Ethereum and Solana RPC methods

### JWT (JSON Web Token)
- **Use Case**: Secure authenticated communication
- **Transport**: HTTPS with JWT bearer tokens
- **Features**:
  - Automatic token generation and refresh
  - HMAC-SHA256 signature validation
  - API key fallback authentication
  - Custom authentication headers support

## Configuration

Each protocol can be configured per engine type:

```rust
let config = CommunicationConfig {
    protocol_preferences: {
        // Ethereum prefers IPC for local, RPC as fallback
        EngineType::Ethereum => vec![ProtocolType::Ipc, ProtocolType::Rpc],
        // Solana prefers JWT for auth, RPC as fallback  
        EngineType::Solana => vec![ProtocolType::Jwt, ProtocolType::Rpc],
    },
    protocol_configs: ProtocolConfigs {
        ipc: /* IPC configurations by engine */,
        rpc: /* RPC configurations by engine */,
        jwt: /* JWT configurations by engine */,
    },
    global_settings: GlobalSettings {
        enable_health_checks: true,
        enable_fallback: true,
        // ... other settings
    },
};
```

## Error Handling

The protocols handle various error scenarios:

- **Network Errors**: Connection failures, timeouts
- **Rate Limiting**: Automatic retry with backoff
- **Authentication Failures**: Token refresh, fallback to API key
- **Serialization Errors**: Invalid message formats
- **Protocol Errors**: Unsupported operations

## Security Considerations

1. **JWT Secrets**: Must be at least 32 characters, never use defaults in production
2. **TLS/SSL**: Always use HTTPS for RPC and JWT protocols in production
3. **Unix Sockets**: Ensure proper file permissions for socket files
4. **API Keys**: Store securely, never commit to version control

## Usage Example

```rust
use multivm_common::communication::{
    CommunicationManager, CommunicationRequest,
    EngineType, ProtocolType,
};

// Create manager with factory
let factory = Box::new(DefaultProtocolFactory::new());
let mut manager = CommunicationManager::new(factory);

// Add protocols
manager.add_protocol(
    ProtocolType::Ipc,
    EngineType::Ethereum,
    config,
).await?;

// Send request with automatic fallback
let request = CommunicationRequest {
    id: Uuid::new_v4().to_string(),
    method: "eth_blockNumber".to_string(),
    params: json!([]),
    timeout: Some(Duration::from_secs(10)),
    auth_context: None,
};

let response = manager.send_with_fallback(
    &EngineType::Ethereum,
    request
).await?;
```

## Monitoring and Metrics

Each protocol tracks:
- Connection status
- Request latency
- Success/error counts
- Health check results

Access metrics via:
```rust
let status = protocol.get_connection_status().await?;
let health = protocol.health_check().await?;
```