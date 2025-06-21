# Reth Execution Engine

Ethereum Virtual Machine (EVM) integration using Reth for the MultiVM blockchain execution platform.

## Overview

The Reth Execution Engine provides integration with Paradigm's Reth Ethereum client for executing EVM transactions within the MultiVM system. It manages Reth node processes in execution-only mode, with consensus and P2P networking disabled.

## Features

### ⚡ EVM Execution
- **Full EVM Compatibility**: Execute Ethereum smart contracts
- **Engine API Support**: Latest execution layer interface
- **Transaction Processing**: Standard Ethereum transaction execution
- **State Management**: Ethereum state and storage handling

### 🔧 Process Management
- **Node Lifecycle**: Automated Reth node startup/shutdown
- **Health Monitoring**: Continuous health checks
- **Resource Management**: Memory and CPU monitoring
- **Crash Recovery**: Automatic restart on failures

### 🔌 Integration
- **Engine API Client**: Secure Engine API communication
- **JWT Authentication**: Secure node authentication
- **Mock Implementation**: Development-ready mock for testing (default mode)
- **Real Node Mode**: Production-ready with actual Reth node process
- **Feature Flags**: Configurable mock/real execution modes
- **Production Ready**: Architecture supports real Reth integration

## Architecture

```
┌─────────────────────────────────────────┐
│        Reth Execution Engine            │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │   Engine    │    │   Engine    │   │
│   │    Core     │    │     API     │   │
│   └─────────────┘    └─────────────┘   │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │   Process   │    │    IPC      │   │
│   │  Manager    │    │   Client    │   │
│   └─────────────┘    └─────────────┘   │
└─────────────────────────────────────────┘
```

## Usage

### Basic Setup

```rust
use reth_execution_engine::*;

// Create engine configuration
let config = RethConfig {
    engine_api_url: "http://127.0.0.1:8551".to_string(),
    jwt_secret_path: "/etc/multivm/jwt.hex",
    chain_id: 31337,
    data_dir: "/var/lib/multivm/reth",
    ipc_endpoint: "/tmp/reth.ipc",
};

// Initialize engine
let engine = RethExecutionEngine::new(config).await?;

// Start engine
engine.start().await?;
```

### Block Execution

```rust
// Create execution payload
let payload = ExecutionPayload {
    parent_hash,
    fee_recipient,
    state_root,
    receipts_root,
    logs_bloom,
    prev_randao,
    block_number,
    gas_limit,
    gas_used,
    timestamp,
    extra_data,
    base_fee_per_gas,
    block_hash,
    transactions,
    withdrawals,
};

// Execute block
let result = engine.new_payload(payload).await?;
match result.status {
    PayloadStatus::Valid => println!("Block executed successfully"),
    PayloadStatus::Invalid { reason } => println!("Invalid block: {}", reason),
    PayloadStatus::Syncing => println!("Node is syncing"),
}
```

### Transaction Execution

```rust
// Submit transaction
let tx_hash = engine.send_transaction(transaction).await?;

// Get transaction receipt
let receipt = engine.get_transaction_receipt(&tx_hash).await?;
if let Some(receipt) = receipt {
    println!("Gas used: {}", receipt.gas_used);
    println!("Status: {}", receipt.status);
}
```

### State Queries

```rust
// Get account balance
let balance = engine.get_balance(&address).await?;

// Get storage value
let value = engine.get_storage_at(&address, &position).await?;

// Call contract
let result = engine.call(CallRequest {
    to: Some(contract_address),
    data: Some(call_data),
    ..Default::default()
}).await?;
```

## Configuration

### Reth Node Configuration

```toml
[reth]
# Engine API
engine_api_url = "http://127.0.0.1:8551"
jwt_secret_path = "/etc/multivm/jwt.hex"

# Chain settings
chain_id = 31337
network_id = 31337

# Data directories
data_dir = "/var/lib/multivm/reth"
db_path = "/var/lib/multivm/reth/db"

# Performance settings
cache_size_mb = 2048
max_concurrent_requests = 100

# Network (disabled for MultiVM)
disable_discovery = true
max_peers = 0
```

## Engine API Methods

### Supported Methods

- `engine_newPayloadV3` - Execute new block
- `engine_forkchoiceUpdatedV3` - Update fork choice
- `engine_getPayloadV3` - Build new block
- `engine_getPayloadBodiesByRangeV1` - Get block bodies
- `engine_exchangeCapabilities` - Exchange capabilities

### JWT Authentication

```bash
# Generate JWT secret
openssl rand -hex 32 > jwt.hex

# Configure both Reth and MultiVM with same secret
```

## Implementation Status

✅ **Architecture Complete**
- Engine interface defined
- Engine API client structure
- Process management framework
- Mock implementation for development

🚧 **Production Integration Ready**
- Real Reth integration documented in [RETH_NODE_INTEGRATION_TASKS.md](../docs/RETH_NODE_INTEGRATION_TASKS.md)
- Engine API client implementation needed
- Reth process management needed
- Estimated effort: 3-4 weeks

## Mock and Real Modes

### Mock Mode (Default)
When built with the `mock` feature (default), the engine provides:
- **Simulated Execution**: Fast block processing without Reth node
- **Consistent Responses**: Predictable behavior for testing
- **No External Dependencies**: Runs without Reth binary
- **Identical API**: Same interface as real mode

### Real Node Mode
When built with the `real-node` feature:
- **Full EVM Execution**: Uses actual Reth node process
- **Production Performance**: Real-world block processing
- **Node Management**: Automatic process lifecycle management
- **P2P Disabled**: Execution-only mode without consensus

### Feature Configuration
```toml
# Cargo.toml
[features]
default = ["mock"]           # Default to mock mode
mock = []                   # Mock implementation
real-node = []              # Real Reth node process

# Build commands
cargo build                      # Uses mock mode
cargo build --features real-node    # Uses real Reth node
cargo build --no-default-features --features real-node  # Only real mode
```

## Testing

```bash
# Run unit tests (mock mode)
cargo test -p reth-execution-engine

# Run with mock engine (default)
cargo run -p reth-execution-engine

# Run with real Reth node
cargo run -p reth-execution-engine --features real-node

# Integration test
cargo test -p reth-execution-engine --test integration

# Test both modes
cargo test -p reth-execution-engine --all-features
```

## Performance

- **Target TPS**: 5,000+ transactions per second
- **Block Time**: 1-2 seconds
- **Memory**: ~4GB for Reth process
- **Storage**: 500GB+ for chain data

## Future Integration

See [RETH_NODE_INTEGRATION_TASKS.md](../docs/RETH_NODE_INTEGRATION_TASKS.md) for detailed integration steps:

1. Reth node setup and configuration
2. Engine API integration
3. Process lifecycle management
4. Block building and execution
5. Performance optimization

## Engine API Specification

Following Ethereum's Engine API specification:
- [Engine API Specs](https://github.com/ethereum/execution-apis/tree/main/src/engine)
- JWT authentication required
- JSON-RPC 2.0 protocol
- Async request/response pattern

## License

Licensed under either Apache 2.0 or MIT license at your option.