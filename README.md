# MultiVM Process

A production-ready distributed VM orchestration system built with Rust, featuring Raft consensus, persistent storage, and secure networking.

## Architecture

This project implements a distributed system for managing multiple VM processes with strong consistency guarantees. See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

### Crates

- **core**: Common types, error handling, retry logic, and rate limiting
- **consensus**: Raft consensus algorithm implementation  
- **network**: TCP/TLS transport layer with connection pooling
- **storage**: RocksDB-based persistent storage with WAL and snapshots
- **vm-runtime** (TODO): VM process isolation and management
- **rpc** (TODO): External API for client communication
- **orchestrator** (TODO): Cluster coordination and VM lifecycle management

## Current Status

✅ **Completed:**
- Core infrastructure and common utilities
- Raft consensus algorithm with pluggable transport/storage
- Production-grade TCP networking with TLS support
- Persistent storage with crash recovery
- All crates compile successfully

🚧 **In Progress:**
- VM runtime implementation
- RPC server for external APIs
- Orchestrator for cluster management
- Integration testing

See [PROGRESS.md](PROGRESS.md) for detailed implementation status.

## Building

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Build with optimizations
cargo build --release --workspace
```

## Example Usage

See [consensus/examples/real_implementation.rs](consensus/examples/real_implementation.rs) for an example of using the consensus system with real network and storage implementations.

```rust
// Create storage backend
let storage = RocksDbStorage::new(storage_config).await?;

// Create network transport
let transport = TcpTransport::new(node_id, transport_config, peers).await?;

// Create and start Raft node
let raft_node = RaftNode::new(
    node_id,
    config,
    storage,
    transport,
    state_machine,
)?;
```

## Production Features

- **High Availability**: Raft consensus ensures system remains available with N/2+1 nodes
- **Data Durability**: RocksDB with write-ahead logging and periodic snapshots
- **Security**: TLS encryption for all network communication
- **Reliability**: Connection pooling, automatic retries, and circuit breakers
- **Performance**: Batching, pipelining, and zero-copy optimizations
- **Observability**: Structured logging with tracing

## Examples

The consensus crate includes examples:
- `simple_cluster` - 3-node cluster demo (mock implementations)
- `seven_node_testnet` - 7-node testnet (mock implementations)
- `real_implementation` - Example using real network/storage

## Development

This is an active project under development. Key areas of focus:

1. **Security**: Authentication, authorization, and audit logging
2. **Operations**: Metrics, monitoring, health checks
3. **Testing**: Comprehensive integration and chaos testing
4. **Documentation**: API docs, deployment guides, best practices

## License

Licensed under either MIT or Apache-2.0 at your option.