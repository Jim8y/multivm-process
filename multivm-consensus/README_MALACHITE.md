# Malachite Consensus Integration

This document describes the integration of [Malachite](https://github.com/informalsystems/malachite) BFT consensus from Informal Systems into the MultiVM architecture.

## Overview

Malachite is a state-of-the-art Byzantine Fault Tolerant (BFT) consensus engine that implements the Tendermint consensus algorithm. It provides instant finality and high throughput, making it ideal for the MultiVM architecture.

## Current Implementation Status

✅ **Completed:**
- Basic Malachite consensus wrapper (`MalachiteConsensus`)
- Configuration management (`MalachiteConfig`)
- Integration with MultiVM consensus traits
- Consensus manager integration
- Complete test suite
- Example implementation

🚧 **In Progress / TODO:**
- Full Malachite crate integration (currently stub implementation)
- Context trait implementation for MultiVM
- Network layer integration
- Validator set management
- Cryptographic signature validation
- Persistent storage integration

## Architecture

```
┌─────────────────────────────────────┐
│     MultiVM Consensus Manager       │
├─────────────────────────────────────┤
│        Malachite Consensus          │
├─────────────────────────────────────┤
│    Malachite BFT Engine (Stub)     │
├─────────────────────────────────────┤
│         P2P Network Layer           │
└─────────────────────────────────────┘
```

## Configuration

```rust
use multivm_consensus::malachite::{MalachiteConfig, NetworkConfig, ConsensusParams};

let config = MalachiteConfig {
    node_id: "validator-1".to_string(),
    network_config: NetworkConfig {
        listen_addr: "127.0.0.1:26656".to_string(),
        peers: vec!["127.0.0.1:26657".to_string()],
    },
    consensus_params: ConsensusParams {
        block_time_ms: 1000,
        max_block_size: 1024 * 1024,
        timeout_propose_ms: 3000,
        timeout_prevote_ms: 1000,
        timeout_precommit_ms: 1000,
    },
    validators: vec![
        ValidatorInfo {
            public_key: "validator1_pubkey".to_string(),
            voting_power: 100,
        },
    ],
};
```

## Usage

```rust
use multivm_consensus::{MalachiteConsensus, MalachiteConfig};

// Create Malachite consensus instance
let (mut consensus, block_sender, commit_receiver) = 
    MalachiteConsensus::new(config).await?;

// Initialize and start
consensus.initialize(config).await?;
consensus.start().await?;

// Propose a block
let transactions = vec![vec![1, 2, 3], vec![4, 5, 6]];
let block = consensus.propose_block(transactions).await?;

// Commit a block
consensus.commit_block(block).await?;

// Get consensus statistics
let stats = consensus.get_consensus_stats().await?;
println!("Current height: {}", stats.current_height);
```

## Integration with MultiVM Manager

```rust
use multivm_consensus::{MultiVMConsensusManager, ConsensusManagerConfig};

let manager_config = ConsensusManagerConfig {
    malachite_config: config,
    // ... other configuration
};

let mut manager = MultiVMConsensusManager::new(manager_config).await?;
manager.start().await?;

// Process cross-VM transactions
manager.process_cross_vm_transaction(cross_vm_tx).await?;
```

## Key Features

### 1. **BFT Consensus**
- Byzantine fault tolerance with instant finality
- Optimized for high throughput and low latency
- Tendermint-based consensus algorithm

### 2. **Cross-VM State Management**
- Unified consensus for SVM and EVM transactions
- State synchronization across VMs
- Account binding consensus

### 3. **Network Integration**
- P2P message propagation
- Validator set management
- Consensus message routing

### 4. **Event System**
- Block proposal events
- Block commit events
- View change notifications
- Error handling

## Dependencies

The integration uses the following Malachite crates:

```toml
[dependencies]
informalsystems-malachitebft-engine = "0.2"
informalsystems-malachitebft-core-consensus = "0.2"
informalsystems-malachitebft-core-types = "0.2"
informalsystems-malachitebft-proto = "0.2"
informalsystems-malachitebft-app-channel = "0.2"
informalsystems-malachitebft-codec = "0.2"
informalsystems-malachitebft-config = "0.2"
```

## Full Integration Roadmap

To complete the full Malachite integration, the following steps are needed:

### Phase 1: Core Integration
1. **Context Implementation**
   - Implement Malachite's `Context` trait for MultiVM
   - Define validator selection logic
   - Implement consensus message creation

2. **Value Integration**
   - Implement `Value` trait for `MultiVMBlock`
   - Add block validation logic
   - Integrate with MultiVM state machine

### Phase 2: Network Integration
3. **P2P Layer**
   - Connect Malachite networking with MultiVM P2P
   - Implement consensus message routing
   - Add network discovery integration

4. **Storage Integration**
   - Connect with persistent storage
   - Implement WAL (Write-Ahead Log)
   - Add crash recovery support

### Phase 3: Production Features
5. **Security**
   - Implement cryptographic verification
   - Add validator key management
   - Security audit and testing

6. **Monitoring**
   - Add metrics collection
   - Implement health checks
   - Performance monitoring

## Testing

Run the test suite:

```bash
cargo test -p multivm-consensus
```

Run the example:

```bash
cargo run --example malachite_example
```

## References

- [Malachite GitHub Repository](https://github.com/informalsystems/malachite)
- [Malachite Documentation](https://docs.rs/informalsystems-malachitebft-engine)
- [Tendermint Consensus Specification](https://github.com/tendermint/spec)
- [Malaketh Integration Examples](https://github.com/informalsystems/malaketh-layered)

## Contributing

When contributing to the Malachite integration:

1. Follow the existing code patterns
2. Add comprehensive tests for new functionality
3. Update documentation for API changes
4. Ensure compatibility with the MultiVM architecture
5. Test with multiple validator configurations