# MultiVM Consensus

Malachite BFT consensus integration for the MultiVM blockchain execution platform.

## Overview

The MultiVM Consensus module integrates the Malachite Byzantine Fault Tolerant consensus engine to provide unified consensus across both Solana VM and Ethereum VM transactions. It ensures consistent block production and state agreement across the distributed system.

## Features

### 🔐 Byzantine Fault Tolerance
- **Malachite Integration**: Production-grade BFT consensus
- **3f+1 Tolerance**: Handles up to f Byzantine validators
- **Leader-based**: Efficient single-leader consensus rounds
- **View Changes**: Automatic leader rotation on failures

### 🔄 Cross-VM Consensus
- **Unified Blocks**: Single consensus for both VMs
- **Transaction Ordering**: Deterministic cross-VM ordering
- **State Coordination**: Synchronized state updates
- **Special Transaction Support**: Consensus for cross-VM operations

### 📊 Performance
- **High Throughput**: Optimized for blockchain workloads
- **Low Latency**: Sub-second block times
- **Efficient Communication**: Minimal message overhead
- **Parallel Validation**: Concurrent transaction validation

## Architecture

```
┌─────────────────────────────────────────┐
│          Consensus Layer                │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │  Malachite  │    │  Consensus  │   │
│   │   Engine    │    │   Manager   │   │
│   └─────────────┘    └─────────────┘   │
├─────────────────────────────────────────┤
│   ┌─────────────┐    ┌─────────────┐   │
│   │    State    │    │   Message   │   │
│   │ Coordinator │    │  Protocol   │   │
│   └─────────────┘    └─────────────┘   │
└─────────────────────────────────────────┘
```

## Usage

### Basic Consensus Setup

```rust
use multivm_consensus::*;

// Create Malachite configuration
let config = MalachiteConfig {
    node_id: "validator-0".to_string(),
    network_config: NetworkConfig {
        listen_addr: "127.0.0.1:26657".parse()?,
        peers: vec![
            "validator-1:26657".parse()?,
            "validator-2:26657".parse()?,
        ],
    },
    consensus_params: ConsensusParams {
        timeout_ms: 5000,
        max_block_size: 1_000_000,
        max_transaction_size: 100_000,
    },
    validators: vec![validator_info_1, validator_info_2, validator_info_3],
};

// Initialize consensus
let consensus = MalachiteConsensus::new(config).await?;

// Start consensus engine
consensus.start().await?;
```

### Block Production

```rust
// Create a new block
let block = MultiVMBlock {
    header: BlockHeader {
        height: 100,
        previous_hash: prev_hash,
        timestamp: SystemTime::now(),
        proposer: validator_id,
    },
    svm_transactions: vec![svm_tx1, svm_tx2],
    evm_transactions: vec![evm_tx1, evm_tx2],
    special_transactions: vec![cross_vm_tx],
    state_changes: vec![],
};

// Propose block for consensus
let result = consensus.propose_block(block).await?;
```

### State Management

```rust
// Create consensus manager
let manager = ConsensusManager::new(
    consensus_engine,
    state_coordinator,
    network_layer,
).await?;

// Process incoming messages
manager.handle_consensus_message(message).await?;

// Get consensus status
let status = manager.get_consensus_status().await?;
println!("Current height: {}", status.current_height);
println!("Current leader: {}", status.current_leader);
```

## Configuration

### Validator Configuration

```toml
[consensus]
# Node identity
node_id = "validator-0"

# Network settings
listen_addr = "0.0.0.0:26657"
external_addr = "validator-0.example.com:26657"

# Consensus parameters
block_time_ms = 1000
timeout_propose_ms = 3000
timeout_prevote_ms = 1000
timeout_precommit_ms = 1000

# Validator set
[[consensus.validators]]
public_key = "Ed25519:..."
voting_power = 10

[[consensus.validators]]
public_key = "Ed25519:..."
voting_power = 10
```

## Implementation Status

✅ **Complete and Functional**
- Malachite consensus engine integration
- Block production and validation
- State coordination across VMs
- Network message handling
- Leader election and view changes

## Testing

```bash
# Run unit tests
cargo test -p multivm-consensus

# Run consensus simulation
cargo test -p multivm-consensus --test consensus_simulation

# Run with debug logging
RUST_LOG=debug cargo test -p multivm-consensus
```

## Performance Considerations

- **Validator Count**: Optimal performance with 4-7 validators
- **Network Latency**: Sub-100ms latency recommended
- **Block Size**: Configure based on transaction throughput
- **Timeout Tuning**: Adjust timeouts based on network conditions

## License

Licensed under either Apache 2.0 or MIT license at your option.