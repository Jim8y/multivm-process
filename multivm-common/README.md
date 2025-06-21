# MultiVM Common

Shared types, traits, and abstractions for the multi-VM blockchain processing system.

## Overview

This crate provides the common interface that both Solana and Ethereum execution engines implement, enabling unified management and communication between different blockchain execution environments.

## Core Abstractions

### `ExecutionEngine` Trait

The main trait that both blockchain engines implement:

```rust
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    type Block;
    type State;
    type Transaction;
    type Receipt;
    type Error: std::error::Error + Send + Sync + 'static;

    async fn process_block(&mut self, block: Self::Block) -> Result<BlockResult<Self::Receipt>, Self::Error>;
    async fn get_state(&self) -> Result<Self::State, Self::Error>;
    async fn start_rpc_server(&self, config: RpcConfig) -> Result<(), Self::Error>;
}
```

### Block Processing Types

- `BlockType`: Enum distinguishing between Solana and Ethereum blocks
- `BlockResult`: Standardized result format for block processing
- `ProcessingMetrics`: Performance and resource usage metrics

### IPC Communication

- `IpcMessage`: Inter-process communication message format
- `CommandType`: Available commands for engine control
- `ResponseType`: Standardized response format

### Configuration

- `EngineConfig`: Base configuration for all engines
- `RpcConfig`: RPC server configuration
- `StorageConfig`: Storage backend configuration

## API Reference

### Core Types

#### `BlockType`
```rust
pub enum BlockType {
    Solana(solana_ledger::shred::Shred),
    Ethereum(reth_primitives::SealedBlock),
}
```

#### `BlockResult<R>`
```rust
pub struct BlockResult<R> {
    pub receipts: Vec<R>,
    pub state_root: Hash,
    pub gas_used: u64,
    pub processing_time: Duration,
    pub metrics: ProcessingMetrics,
}
```

#### `ProcessingMetrics`
```rust
pub struct ProcessingMetrics {
    pub transactions_processed: u64,
    pub compute_units_used: u64,
    pub memory_usage: u64,
    pub cpu_time: Duration,
}
```

### IPC Types

#### `IpcMessage`
```rust
pub struct IpcMessage {
    pub id: MessageId,
    pub command: CommandType,
    pub payload: Vec<u8>,
    pub timestamp: SystemTime,
}
```

#### `CommandType`
```rust
pub enum CommandType {
    ProcessBlock(BlockType),
    GetState,
    GetMetrics,
    Shutdown,
    HealthCheck,
}
```

### Configuration Types

#### `EngineConfig`
```rust
pub struct EngineConfig {
    pub chain_id: u64,
    pub data_dir: PathBuf,
    pub rpc_config: Option<RpcConfig>,
    pub storage_config: StorageConfig,
    pub metrics_enabled: bool,
}
```

#### `RpcConfig`
```rust
pub struct RpcConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
    pub request_timeout: Duration,
    pub cors_origins: Vec<String>,
}
```

## Usage Examples

### Implementing an Execution Engine

```rust
use multivm_common::{ExecutionEngine, BlockResult, RpcConfig};
use async_trait::async_trait;

pub struct MyEngine {
    // Engine state
}

#[async_trait]
impl ExecutionEngine for MyEngine {
    type Block = MyBlock;
    type State = MyState;
    type Transaction = MyTransaction;
    type Receipt = MyReceipt;
    type Error = MyError;

    async fn process_block(&mut self, block: Self::Block) -> Result<BlockResult<Self::Receipt>, Self::Error> {
        // Process block and return results
        let receipts = self.execute_transactions(&block.transactions).await?;
        let state_root = self.compute_state_root().await?;
        
        Ok(BlockResult {
            receipts,
            state_root,
            gas_used: block.gas_limit,
            processing_time: start_time.elapsed(),
            metrics: self.collect_metrics(),
        })
    }

    async fn get_state(&self) -> Result<Self::State, Self::Error> {
        // Return current state
        Ok(self.current_state.clone())
    }

    async fn start_rpc_server(&self, config: RpcConfig) -> Result<(), Self::Error> {
        // Start RPC server
        self.rpc_server.start(config).await
    }
}
```

### Using IPC Communication

```rust
use multivm_common::{IpcMessage, CommandType, BlockType};

async fn send_block_to_engine(block: BlockType) -> Result<(), Box<dyn std::error::Error>> {
    let message = IpcMessage {
        id: MessageId::new(),
        command: CommandType::ProcessBlock(block),
        payload: bincode::serialize(&block)?,
        timestamp: SystemTime::now(),
    };
    
    // Send message to engine process
    send_ipc_message(message).await?;
    Ok(())
}
```

## Design Patterns

### Error Handling
- All operations return `Result` types
- Errors are properly typed and serializable for IPC
- Error context is preserved across process boundaries

### State Management
- Immutable state snapshots for consistency
- Copy-on-write semantics for efficient state updates
- State root computation for integrity verification

### Resource Management
- RAII patterns for automatic cleanup
- Graceful shutdown handling
- Resource monitoring and limits

## Testing

### Unit Tests
```bash
cargo test --package multivm-common
```

### Integration Tests
```bash
cargo test --package multivm-common --tests
```

### Benchmarks
```bash
cargo bench --package multivm-common
``` 