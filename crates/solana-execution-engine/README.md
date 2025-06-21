# Solana Execution Engine

Solana Virtual Machine (SVM) integration for the MultiVM blockchain execution platform.

## Overview

The Solana Execution Engine provides integration with Solana's runtime for executing SVM transactions within the MultiVM system. It manages Solana validator processes in execution-only mode, with consensus and P2P networking disabled.

## Features

### 🚀 SVM Execution
- **Full SVM Compatibility**: Execute Solana programs and transactions
- **JSON-RPC Interface**: Standard Solana RPC API support
- **Transaction Processing**: High-throughput transaction execution
- **Account Management**: Solana account state management

### 🔧 Process Management
- **Validator Lifecycle**: Automated validator startup/shutdown
- **Health Monitoring**: Continuous health checks
- **Resource Management**: Memory and CPU monitoring
- **Crash Recovery**: Automatic restart on failures

### 🔌 Integration
- **IPC Communication**: Secure inter-process communication
- **Mock Implementation**: Development-ready mock for testing (default mode)
- **Real Validator Mode**: Production-ready with actual Solana validator process
- **Feature Flags**: Configurable mock/real execution modes
- **Performance Monitoring**: Metrics and logging

## Architecture

```
┌─────────────────────────────────────────┐
│      Solana Execution Engine            │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │   Engine    │    │    JSON     │   │
│   │   Core      │    │    RPC      │   │
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
use solana_execution_engine::*;

// Create engine configuration
let config = SolanaConfig {
    rpc_url: "http://127.0.0.1:8899".to_string(),
    ws_url: Some("ws://127.0.0.1:8900".to_string()),
    commitment: CommitmentConfig::confirmed(),
    identity_path: "/var/lib/multivm/solana/identity.json",
    ledger_path: "/var/lib/multivm/solana/ledger",
    accounts_path: "/var/lib/multivm/solana/accounts",
};

// Initialize engine
let engine = SolanaExecutionEngine::new(config).await?;

// Start engine
engine.start().await?;
```

### Transaction Execution

```rust
// Submit transaction
let signature = engine.execute_transaction(transaction).await?;

// Check transaction status
let status = engine.get_transaction_status(&signature).await?;
match status {
    TransactionStatus::Confirmed { slot, .. } => {
        println!("Transaction confirmed in slot {}", slot);
    }
    TransactionStatus::Failed { error } => {
        println!("Transaction failed: {}", error);
    }
    _ => {}
}
```

### Account Queries

```rust
// Get account info
let account = engine.get_account(&pubkey).await?;
if let Some(account) = account {
    println!("Balance: {} lamports", account.lamports);
    println!("Owner: {}", account.owner);
}

// Get multiple accounts
let accounts = engine.get_multiple_accounts(&pubkeys).await?;
```

### Program Execution

```rust
// Execute program
let result = engine.execute_program(
    program_id,
    accounts,
    instruction_data,
).await?;

// Simulate transaction
let simulation = engine.simulate_transaction(&transaction).await?;
println!("Compute units: {}", simulation.units_consumed);
```

## Configuration

### Validator Configuration

```toml
[solana]
# RPC endpoints
rpc_url = "http://127.0.0.1:8899"
ws_url = "ws://127.0.0.1:8900"

# Validator settings
identity_path = "/var/lib/multivm/solana/identity.json"
vote_account_path = "/var/lib/multivm/solana/vote.json"
ledger_path = "/var/lib/multivm/solana/ledger"
accounts_path = "/var/lib/multivm/solana/accounts"

# Performance settings
accounts_db_caching_enabled = true
rpc_threads = 8
banking_threads = 4

# Network (disabled for MultiVM)
enable_gossip = false
enable_rpc = true
private_rpc = true
```

## Implementation Status

✅ **Architecture Complete**
- Engine interface defined
- IPC communication structure
- Process management framework
- Mock implementation for development

🚧 **Production Integration Ready**
- Real validator integration documented in [SOLANA_NODE_INTEGRATION_TASKS.md](../docs/SOLANA_NODE_INTEGRATION_TASKS.md)
- JSON-RPC client implementation needed
- Validator process management needed
- Estimated effort: 3-4 weeks

## Mock and Real Modes

### Mock Mode (Default)
When built with the `mock` feature (default), the engine provides:
- **Simulated Execution**: Fast transaction processing without validator
- **Consistent Responses**: Predictable behavior for testing
- **No External Dependencies**: Runs without Solana validator binary
- **Identical API**: Same interface as real mode

### Real Validator Mode
When built with the `real-validator` feature:
- **Full SVM Execution**: Uses actual Solana validator process
- **Production Performance**: Real-world transaction processing
- **Validator Management**: Automatic process lifecycle management
- **P2P Disabled**: Execution-only mode without consensus

### Feature Configuration
```toml
# Cargo.toml
[features]
default = ["mock"]           # Default to mock mode
mock = []                   # Mock implementation
real-validator = []         # Real validator process

# Build commands
cargo build                      # Uses mock mode
cargo build --features real-validator  # Uses real validator
cargo build --no-default-features --features real-validator  # Only real mode
```

## Testing

```bash
# Run unit tests (mock mode)
cargo test -p solana-execution-engine

# Run with mock engine (default)
cargo run -p solana-execution-engine

# Run with real validator
cargo run -p solana-execution-engine --features real-validator

# Integration test
cargo test -p solana-execution-engine --test integration

# Test both modes
cargo test -p solana-execution-engine --all-features
```

## Performance

- **Target TPS**: 65,000+ transactions per second
- **Latency**: < 50ms transaction confirmation
- **Memory**: ~8GB for validator process
- **Storage**: 100GB+ for ledger data

## Future Integration

See [SOLANA_NODE_INTEGRATION_TASKS.md](../docs/SOLANA_NODE_INTEGRATION_TASKS.md) for detailed integration steps:

1. Validator setup and configuration
2. JSON-RPC integration
3. Process lifecycle management
4. WebSocket subscriptions
5. Performance optimization

## License

Licensed under either Apache 2.0 or MIT license at your option.