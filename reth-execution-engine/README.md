# Reth Execution Engine

A production-ready Ethereum Virtual Machine (EVM) execution engine that integrates Reth nodes with the MultiVM distributed blockchain system.

## Overview

This crate provides both mock and real implementations of Reth execution engine for MultiVM. It handles:

- Reth node process management
- Engine API integration with JWT authentication
- Transaction execution and block production
- Cross-VM communication via IPC
- Real-time health monitoring and metrics

## Architecture

```
┌─────────────────┐    HTTP/JSON-RPC     ┌──────────────────┐
│   MultiVM       │◄───────────────────► │   Reth Node      │
│   Process       │                      │   Process        │
│                 │    Engine API (JWT)  │                  │
│                 │◄───────────────────► │                  │
└─────────────────┘                      └──────────────────┘
         │                                         │
         │ JWT-Secured IPC                         │ Local Filesystem
         │                                         │ (JWT Secret, Data)
         ▼                                         ▼
┌─────────────────┐                      ┌──────────────────┐
│   Other VM      │◄────JWT Auth────────►│   Database &     │
│   Processes     │                      │   State Storage  │
└─────────────────┘                      └──────────────────┘

JWT Flow:
1. Engine generates shared JWT secret (32-byte hex)
2. Secret stored in ./reth-data/jwt.hex
3. All Engine API calls include JWT Bearer token
4. IPC communication optionally JWT-secured
5. Tokens expire after configurable time (default: 60s)
```

## Features

### Real Engine (`real-node` feature)
- **Process Management**: Automatically starts and manages Reth node processes
- **Engine API**: Full implementation of Ethereum Engine API (newPayload, forkchoiceUpdated)
- **JWT Authentication**: Secure communication with Engine API
- **Transaction Processing**: Individual transaction submission and batch block production
- **Health Monitoring**: Continuous health checks and automatic recovery

### Mock Engine (Default)
- **Development Mode**: Fast testing without real Reth dependencies
- **API Compatibility**: Same interface as real engine for seamless switching
- **Configurable Delays**: Simulate real-world timing and latency

## Setup Guide

### 1. Install Reth

```bash
# Install Reth (choose one method)

# Method 1: Using Cargo
cargo install --git https://github.com/paradigmxyz/reth.git --bin reth

# Method 2: Download precompiled binary
curl -L https://github.com/paradigmxyz/reth/releases/latest/download/reth-x86_64-unknown-linux-gnu.tar.gz | tar -xz
sudo mv reth /usr/local/bin/

# Method 3: Build from source
git clone https://github.com/paradigmxyz/reth.git
cd reth
cargo build --release --bin reth
sudo cp target/release/reth /usr/local/bin/
```

Verify installation:
```bash
reth --version
```

### 2. Configure MultiVM with Reth Engine

Add to your `Cargo.toml`:
```toml
[dependencies]
reth-execution-engine = { path = "./reth-execution-engine", features = ["real-node"] }
```

### 3. Initialize Reth Engine

```rust
use reth_execution_engine::RealRethEngine;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create engine instance
    let mut engine = RealRethEngine::new(
        PathBuf::from("./reth-data"),  // Data directory
        8545,                          // RPC port
        1337,                          // Chain ID (dev mode)
    ).await?;
    
    // Initialize (starts Reth process)
    engine.initialize().await?;
    
    println!("Reth engine ready!");
    Ok(())
}
```

### 4. Setup MultiVM Process Communication

```rust
use reth_execution_engine::{RethExecutionEngine, RethBlock};

// Create execution engine
let engine = RethExecutionEngine::new(false, "./reth-data", 8545, 1337).await?;

// Process a block
let result = engine.process_block(&block).await?;
println!("Block processed: {:?}", result);

// Forward individual transaction
let tx_result = engine.forward_transaction_to_reth(&transaction).await?;
println!("Transaction result: {:?}", tx_result);
```

## Configuration

### Environment Variables
```bash
# Reth node configuration
export RETH_DATA_DIR="./reth-data"
export RETH_HTTP_PORT="8545"
export RETH_ENGINE_PORT="8551"
export RETH_CHAIN_ID="1337"

# MultiVM integration
export MULTIVM_IPC_PATH="/tmp/multivm-reth.sock"
export MULTIVM_LOG_LEVEL="info"

# JWT authentication
export JWT_SECRET_PATH="./reth-data/jwt.hex"
export JWT_EXPIRY_SECONDS="300"
```

### Connection Configuration
```rust
use reth_execution_engine::ConnectionConfig;
use std::time::Duration;

let config = ConnectionConfig {
    max_retries: 5,
    retry_delay: Duration::from_millis(1000),
    request_timeout: Duration::from_secs(30),
    health_check_interval: Duration::from_secs(10),
    connection_pool_size: 10,
};

let engine = RealRethEngine::new_with_config(
    data_dir,
    rpc_port,
    chain_id,
    config
).await?;
```

## API Reference

### Core Methods

#### Block Processing
```rust
// Process a complete block
async fn process_block(&self, block: &RethBlock) -> Result<RethExecutionResult, RethEngineError>

// Produce a new block with transactions
async fn produce_block_v3(
    &self,
    transactions: Vec<RethTransaction>,
    parent_hash: B256
) -> Result<RethExecutionResult, RethEngineError>
```

#### Transaction Handling
```rust
// Submit individual transaction
async fn forward_transaction_to_reth(
    &self,
    tx: &RethTransaction
) -> Result<TransactionForwardingResult, RethEngineError>

// Validate transaction before submission
async fn validate_transaction(&self, tx_hex: &str) -> Result<ValidationResult, RethEngineError>
```

#### Engine API
```rust
// Submit execution payload
async fn engine_new_payload_v3(
    &self,
    payload: &ExecutionPayloadV3,
    versioned_hashes: Vec<B256>,
    parent_beacon_block_root: B256
) -> Result<serde_json::Value, RethEngineError>

// Update forkchoice
async fn engine_forkchoice_updated_v2(
    &self,
    forkchoice_state: &ForkchoiceState,
    payload_attributes: Option<&PayloadAttributes>
) -> Result<serde_json::Value, RethEngineError>
```

## MultiVM Integration

### JWT-Based Communication

The engine uses JWT (JSON Web Tokens) for secure communication with Reth's Engine API and MultiVM processes:

#### JWT Secret Generation
```rust
// Automatic JWT secret generation during initialization
let mut engine = RealRethEngine::new(data_dir, rpc_port, chain_id).await?;
engine.initialize().await?; // Generates JWT secret automatically

// Manual JWT operations
let jwt_secret = engine.generate_jwt_secret().await?;
let token = engine.create_jwt_token(&jwt_secret)?;
```

#### Engine API Authentication
```rust
use reth_execution_engine::RealRethEngine;

// Engine API calls are automatically authenticated with JWT
let result = engine.engine_new_payload_v3(
    &execution_payload,
    versioned_hashes,
    parent_beacon_block_root
).await?;

// JWT token is automatically included in Authorization header:
// Authorization: Bearer <jwt_token>
```

#### Custom JWT Configuration
```rust
use reth_execution_engine::JwtConfig;

let jwt_config = JwtConfig {
    secret_path: "./custom-jwt.hex".into(),
    expiry_seconds: 300, // 5 minutes
    algorithm: "HS256",
};

let engine = RealRethEngine::new_with_jwt_config(
    data_dir,
    rpc_port,
    chain_id,
    jwt_config
).await?;
```

### IPC Communication with JWT

The engine also supports JWT-secured IPC communication between MultiVM processes:

```rust
use reth_execution_engine::{IpcClient, JwtCredentials};

// Create JWT credentials for IPC
let jwt_creds = JwtCredentials::from_secret_file("./reth-data/jwt.hex")?;

// Start IPC server with JWT authentication
let ipc_client = IpcClient::new_with_jwt("/tmp/multivm-reth.sock", jwt_creds).await?;
ipc_client.start_server().await?;

// Handle authenticated cross-VM requests
ipc_client.handle_authenticated_request(request).await?;
```

#### JWT Token Validation
```rust
// Validate incoming JWT tokens
let is_valid = engine.verify_jwt_token(&token, &secret)?;
if !is_valid {
    return Err(RethEngineError::Authentication("Invalid JWT token".into()));
}

// Extract claims from JWT
let claims = engine.parse_jwt_claims(&token)?;
println!("Token expires at: {}", claims.exp);
```

### Health Monitoring

```rust
// Check engine health
let health = engine.get_health().await?;
match health {
    HealthStatus::Healthy => println!("Engine is healthy"),
    HealthStatus::Unhealthy => println!("Engine needs attention"),
}

// Get detailed metrics
let metrics = engine.get_processing_metrics().await?;
println!("Blocks processed: {}", metrics.blocks_processed);
println!("Average block time: {:?}", metrics.average_block_time);
```

### Error Handling

```rust
use reth_execution_engine::RethEngineError;

match engine.process_block(&block).await {
    Ok(result) => println!("Success: {:?}", result),
    Err(RethEngineError::Process(msg)) => println!("Process error: {}", msg),
    Err(RethEngineError::Rpc(msg)) => println!("RPC error: {}", msg),
    Err(RethEngineError::Configuration(msg)) => println!("Config error: {}", msg),
    Err(e) => println!("Other error: {:?}", e),
}
```

## Troubleshooting

### Common Issues

1. **Reth not found**
   ```bash
   # Ensure Reth is in PATH
   which reth
   # If not found, install or add to PATH
   export PATH="/usr/local/bin:$PATH"
   ```

2. **Port conflicts**
   ```bash
   # Check if ports are in use
   netstat -tulpn | grep :8545
   netstat -tulpn | grep :8551
   # Use different ports if occupied
   ```

3. **JWT authentication failures**
   ```bash
   # Check JWT secret file
   ls -la ./reth-data/jwt.hex
   cat ./reth-data/jwt.hex  # Should be 64 hex characters
   
   # Verify JWT token generation
   export RUST_LOG="reth_execution_engine::real_engine_utils=debug"
   
   # Regenerate if corrupted
   rm ./reth-data/jwt.hex
   # Engine will regenerate on next initialization
   ```

4. **JWT token validation errors**
   ```bash
   # Check token expiry
   # Tokens expire after 60 seconds by default
   # Increase expiry for debugging:
   export JWT_EXPIRY_SECONDS="3600"  # 1 hour
   
   # Verify HMAC signature
   # Ensure both client and server use same secret
   ```

5. **Database initialization errors**
   ```bash
   # Reset Reth database
   rm -rf ./reth-data/db
   # Engine will reinitialize on next start
   ```

### Debug Mode

Enable detailed logging:
```bash
export RUST_LOG="reth_execution_engine=debug,reth=debug"
```

### Performance Tuning

For production environments:
```rust
let config = ConnectionConfig {
    max_retries: 3,
    retry_delay: Duration::from_millis(500),
    request_timeout: Duration::from_secs(60),
    health_check_interval: Duration::from_secs(5),
    connection_pool_size: 20,
};
```

## JWT Communication Deep Dive

### How JWT Authentication Works

1. **Secret Generation**
   ```rust
   // Engine automatically generates a 256-bit (32-byte) random secret
   let secret: [u8; 32] = rand::Rng::gen(&mut rng);
   let hex_secret = hex::encode(secret); // Converts to 64-char hex string
   ```

2. **Token Creation**
   ```rust
   // Creates JWT with HS256 algorithm
   let header = json!({"alg": "HS256", "typ": "JWT"});
   let payload = json!({
       "iat": current_timestamp,
       "exp": current_timestamp + 60  // 60 second expiry
   });
   
   // Sign with HMAC-SHA256
   let token = format!("{}.{}.{}", 
       base64_encode(header),
       base64_encode(payload), 
       hmac_sha256_signature
   );
   ```

3. **HTTP Request Authentication**
   ```rust
   // All Engine API calls include Authorization header
   let client = reqwest::Client::new();
   let response = client
       .post("http://127.0.0.1:8551")
       .header("Authorization", format!("Bearer {}", jwt_token))
       .json(&engine_request)
       .send()
       .await?;
   ```

### Multi-Process JWT Sharing

```rust
use reth_execution_engine::{RealRethEngine, JwtManager};
use std::sync::Arc;

// Main process creates and shares JWT manager
let jwt_manager = Arc::new(JwtManager::new("./shared-jwt.hex")?);

// Process 1: Reth Engine
let engine = RealRethEngine::new_with_jwt_manager(
    data_dir, 
    rpc_port, 
    chain_id,
    jwt_manager.clone()
).await?;

// Process 2: Another VM Engine  
let other_vm = OtherVMEngine::new_with_jwt_manager(jwt_manager.clone()).await?;

// Both processes can now authenticate with same secret
let token1 = jwt_manager.create_token()?;
let token2 = jwt_manager.create_token()?; // Same secret, different tokens
```

### IPC with JWT Authentication

```rust
use reth_execution_engine::{IpcServer, IpcClient, JwtMessage};

// Server side (Reth engine process)
let ipc_server = IpcServer::new_with_jwt("/tmp/multivm-reth.sock", jwt_secret).await?;

ipc_server.handle_requests(|request: JwtMessage| async {
    // Verify JWT token
    let claims = request.verify_and_decode(&jwt_secret)?;
    
    // Process authenticated request
    match request.method {
        "submit_transaction" => handle_transaction(request.params).await,
        "get_block" => handle_get_block(request.params).await,
        _ => Err(IpcError::UnknownMethod),
    }
}).await?;

// Client side (other VM processes)
let ipc_client = IpcClient::new_with_jwt("/tmp/multivm-reth.sock", jwt_secret).await?;

let response = ipc_client.call_authenticated(
    "submit_transaction",
    json!({"tx_data": "0x..."}),
    &jwt_secret
).await?;
```

## Examples

See the `examples/` directory for complete working examples:
- `basic_setup.rs`: Simple engine initialization
- `block_production.rs`: Block creation and submission  
- `transaction_processing.rs`: Individual transaction handling
- `jwt_authentication.rs`: JWT setup and token management
- `ipc_integration.rs`: Multi-process JWT communication
- `cross_vm_messaging.rs`: Secure VM-to-VM communication

## Contributing

When contributing to this crate:
1. Ensure both mock and real implementations are updated
2. Add appropriate tests for new functionality
3. Update this README for any new features
4. Follow the existing error handling patterns

## License

This project is licensed under the MIT License - see the LICENSE file for details.