# Solana Execution Engine

A high-performance Solana execution engine integration for the MultiVM framework, providing seamless blockchain execution capabilities with leader-follower consensus support.

## Overview

The Solana Execution Engine is a core component of the MultiVM ecosystem that enables:

- **Leader-Follower Architecture**: Support for both leader nodes (block creation) and follower nodes (block replay)
- **Private Validator Integration**: Built-in Solana private validator for isolated blockchain execution
- **RPC Proxy Server**: Secure RPC forwarding with method filtering and access control
- **Block Processing**: Sequential block creation and replay with cryptographic verification
- **Transaction Management**: Complete transaction lifecycle management including creation, submission, and confirmation

## Features

### Core Functionality
- ✅ **Solana Private Validator**: Embedded validator for deterministic blockchain execution
- ✅ **Block Creation**: Create blocks with multiple transactions as a leader node
- ✅ **Block Replay**: Replay blocks in sequential order as a follower node
- ✅ **Hash Verification**: Cryptographic block hash verification for integrity
- ✅ **RPC Integration**: Full Solana RPC client integration with health monitoring
- ✅ **Configuration Management**: Flexible configuration with builder patterns

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Leader Node   │    │  Follower Node   │    │  RPC Clients    │
│                 │    │                  │    │                 │
│ ┌─────────────┐ │    │ ┌──────────────┐ │    │ ┌─────────────┐ │
│ │ Block       │ │    │ │ Block        │ │    │ │ External    │ │
│ │ Creation    │ │────┼─│ Replay       │ │    │ │ Apps        │ │
│ └─────────────┘ │    │ └──────────────┘ │    │ └─────────────┘ │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 │
                    ┌─────────────────────────┐
                    │   Solana Engine Core    │
                    │                         │
                    │ ┌─────────────────────┐ │
                    │ │ Private Validator   │ │
                    │ └─────────────────────┘ │
                    │ ┌─────────────────────┐ │
                    │ │ RPC Proxy Server    │ │
                    │ └─────────────────────┘ │
                    │ ┌─────────────────────┐ │
                    │ │ Transaction Pool    │ │
                    │ └─────────────────────┘ │
                    └─────────────────────────┘
```

## Quick Start

### Installation

1. **Clone the repository**:
```bash
git clone <repository-url>
cd multivm/solana-execution-engine
```

2. **Build the project**:
```bash
cargo build --release
```

3. **Run tests**:
```bash
cargo test
```

### Basic Usage

#### Leader Node Example

```rust
use solana_execution_engine::{SolanaEngine, create_transfer_transaction};
use solana_sdk::{signature::Keypair, signer::Signer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the engine
    let mut engine = SolanaEngine::new_default().await?;
    engine.initialize().await?;
    
    // Create transactions
    let recent_blockhash = engine.get_latest_blockhash().await?;
    let tx = create_transfer_transaction(
        &Keypair::new(),
        &Keypair::new().pubkey(),
        1_000_000, // 0.001 SOL
        recent_blockhash,
    );
    
    // Create a block
    let mut transactions = vec![tx];
    let block = engine.create_block(&mut transactions).await?;
    
    println!("Created block: slot={}, hash={:?}", block.slot, block.block_hash);
    
    // Cleanup
    engine.shutdown(Some(std::time::Duration::from_secs(10))).await?;
    Ok(())
}
```

#### Follower Node Example

```rust
use solana_execution_engine::SolanaEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize follower engine
    let mut follower = SolanaEngine::new_default().await?;
    follower.initialize().await?;
    
    // Replay received block (from network/leader)
    let success = follower.replay_block(received_block).await?;
    
    if success {
        println!("Block replayed successfully!");
    }
    
    // Cleanup
    follower.shutdown(Some(std::time::Duration::from_secs(10))).await?;
    Ok(())
}
```

## Configuration

### Engine Configuration

```rust
use solana_execution_engine::config::{SolanaEngineConfig, SolanaConfig, SolanaConnectionConfig};
use std::time::Duration;

// Custom engine configuration
let engine_config = SolanaEngineConfig::new_with_config(
    "127.0.0.1".to_string(),  // RPC server host
    8888,                     // RPC server port
);

// Custom Solana validator configuration
let solana_config = SolanaConfig::builder()
    .rpc_port(8899)
    .gossip_port(1024)
    .ledger_path("/tmp/custom-ledger")
    .ticks_per_slot(4)
    .deterministic(true)
    .reset(true)
    .build();

// Custom connection configuration
let connection_config = SolanaConnectionConfig::builder()
    .max_retries(5)
    .retry_delay(Duration::from_millis(500))
    .request_timeout(Duration::from_secs(60))
    .build();

// Create engine with custom configuration
let engine = SolanaEngine::new_with_config(
    engine_config,
    connection_config,
    solana_config,
).await?;
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `RUST_LOG` | Logging level | `info` |
| `SOLANA_RPC_PORT` | Solana RPC port | `8899` |
| `SOLANA_GOSSIP_PORT` | Gossip port | `1024` |
| `ENGINE_RPC_PORT` | Engine RPC proxy port | `8888` |

## API Reference

### Core Methods

#### `SolanaEngine::new_default()`
Creates a new engine instance with default configuration.

#### `SolanaEngine::initialize()`
Initializes the engine, starts the validator, and sets up RPC connections.

#### `SolanaEngine::create_block(transactions: &mut [Transaction])`
Creates a new block with the provided transactions (Leader mode).

#### `SolanaEngine::replay_block(block: SolanaBlockData)`
Replays a block in sequential order (Follower mode).

#### `SolanaEngine::shutdown(timeout: Option<Duration>)`
Gracefully shuts down the engine and all associated processes.

### RPC Methods

The engine provides a proxy RPC server that forwards requests to the internal Solana validator with security filtering:

- **Allowed**: `getBalance`, `getBlockHeight`, `getLatestBlockhash`, etc.
- **Blocked**: `requestAirdrop`, `sendTransaction`, `simulateTransaction`

## Examples

The project includes comprehensive examples:

- [`basic_engine_usage.rs`](examples/basic_engine_usage.rs) - Basic engine operations
- [`leader_engine.rs`](examples/leader_engine.rs) - Leader node implementation
- [`follower_engine.rs`](examples/follower_engine.rs) - Follower node with block replay

Run examples:
```bash
cargo run --example follower_engine
```

## Testing

### Unit Tests
```bash
cargo test
```

### Integration Tests
```bash
cargo test --test engine_tests
cargo test --test engine_rpc_tests
```

### Test Coverage
```bash
cargo tarpaulin --out Html
```

## Troubleshooting

### Common Issues

1. **Port Conflicts**
   ```
   Error: Address already in use
   Solution: Change RPC ports in configuration
   ```

2. **Validator Startup Timeout**
   ```
   Error: Failed to start Solana Private Validator
   Solution: Increase initialization timeout or check system resources
   ```

3. **RPC Connection Failed**
   ```
   Error: RPC client not initialized
   Solution: Ensure initialize() is called before other operations
   ```

### Debug Mode

Enable debug logging:
```bash
RUST_LOG=debug cargo run --example follower_engine
```

### Health Checks

The engine provides built-in health monitoring:
```rust
// Health checks run automatically every 10 seconds
// Check logs for health status updates
```

### Development Setup

```bash
# Install development dependencies
cargo install cargo-tarpaulin cargo-audit

# Run full test suite
cargo test --all-features

# Check code quality
cargo clippy -- -D warnings
cargo fmt --check
```