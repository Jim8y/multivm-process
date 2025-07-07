# MultiVM Consensus Module

Production-grade Byzantine Fault Tolerant consensus layer for the MultiVM blockchain platform, powered by Malachite BFT from Informal Systems.

## 🏛️ Overview

The MultiVM Consensus module provides a robust, scalable consensus mechanism that ensures consistency across multiple virtual machine implementations (EVM and SVM) while maintaining Byzantine fault tolerance.

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────┐
│                 Consensus Manager                        │
│            (MultiVMConsensusManager)                     │
├─────────────────────────────────────────────────────────┤
│              Malachite BFT Engine                        │
│         (Byzantine Fault Tolerant Core)                  │
├─────────────────────────────────────────────────────────┤
│  Transaction  │   State      │   Block    │    Fork     │
│     Pool      │  Management  │  Producer  │  Detection  │
├───────────────┴──────────────┴────────────┴─────────────┤
│                    P2P Network Layer                     │
│              (Message Broadcasting)                      │
└─────────────────────────────────────────────────────────┘
```

## 🔑 Key Features

### **Malachite BFT Consensus**
- Production-ready Byzantine Fault Tolerant consensus from [Informal Systems](https://github.com/informalsystems/malachite)
- Tolerates up to f < n/3 Byzantine validators
- Implements PBTS (Proposer-Based Timestamp) system
- Formally verified for safety and liveness

### **Cross-VM State Management**
- Unified state root across EVM and SVM
- Merkle tree-based state verification
- Persistent state storage with RocksDB
- Checkpoint and recovery mechanisms

### **High-Performance Transaction Pool**
- Concurrent, lock-free transaction pool
- Priority-based transaction ordering
- VM-specific transaction handling
- Configurable pool size and eviction policies

### **Advanced Features**
- Fork detection and resolution
- Network partition recovery
- State synchronization
- Comprehensive metrics and monitoring

## 📦 Core Components

### **Consensus Manager** (`manager.rs`)
Central orchestrator that coordinates all consensus operations:
```rust
pub struct MultiVMConsensusManager {
    consensus_engine: MalachiteConsensus,
    state_coordinator: Arc<RwLock<PersistentCrossVMStateManager>>,
    p2p_network: Option<Arc<RwLock<P2PNetwork>>>,
    transaction_pool: Arc<ConcurrentTransactionPool>,
    // ...
}
```

### **Malachite Integration** (`malachite/`)
- **Engine**: Core consensus engine implementation
- **Config**: Malachite BFT configuration parameters
- **Validator**: Validator management and key handling
- **Types**: Consensus-specific types and messages

### **State Management** (`state/`)
- **CrossVMStateCoordinator**: Manages state across VMs
- **PersistentStateManager**: RocksDB-based persistence
- **MerkleTree**: State verification and proofs
- **Checkpoint**: State snapshot management

### **Transaction Pool** (`transaction_pool.rs`)
- **ConcurrentTransactionPool**: Lock-free pool implementation
- **PriorityQueue**: Transaction prioritization
- **VMRouter**: Routes transactions to appropriate VMs
- **Mempool**: In-memory transaction storage

## 🚀 Usage

### **Basic Setup**

```rust
use multivm_consensus::{
    ConsensusConfig, MultiVMConsensusManager,
    malachite::ConsensusParams,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create consensus configuration
    let mut config = ConsensusConfig::default();
    config.malachite.consensus_params = ConsensusParams {
        block_time_ms: 5000,
        timeout_propose_ms: 3000,
        timeout_prevote_ms: 1000,
        timeout_precommit_ms: 1000,
        timeout_commit_ms: 5000,
        max_block_size: 1024 * 1024, // 1MB
        validator_set_size: 3,
    };

    // Initialize consensus manager
    let mut consensus_manager = MultiVMConsensusManager::new(config).await?;
    
    // Start consensus
    consensus_manager.start().await?;
    
    println!("Malachite BFT consensus started!");
    
    Ok(())
}
```

### **Transaction Submission**

```rust
use multivm_consensus::transaction_pool::TransactionPriority;

// Submit EVM transaction
let evm_tx = vec![/* transaction data */];
consensus_manager.submit_transaction(
    evm_tx,
    TransactionPriority::High
).await?;

// Submit SVM transaction
let svm_tx = vec![/* transaction data */];
consensus_manager.submit_transaction(
    svm_tx,
    TransactionPriority::Normal
).await?;
```

### **Block Production**

```rust
// Consensus automatically produces blocks based on configuration
// Manual block production (for testing)
let block = consensus_manager.produce_block().await?;
println!("Produced block at height: {}", block.header.height);
```

### **State Queries**

```rust
// Get current consensus state
let state = consensus_manager.get_consensus_state().await?;
println!("Current height: {}", state.height);
println!("Current round: {}", state.round);

// Get cross-VM state root
let state_root = consensus_manager.get_state_root().await?;
println!("State root: {}", state_root);
```

## ⚙️ Configuration

### **Consensus Parameters**

```toml
[consensus]
algorithm = "malachite-bft"
block_time_ms = 5000
max_block_size = 1048576  # 1MB

[consensus.timeouts]
propose = "3s"
prevote = "1s"
precommit = "1s"
commit = "5s"

[consensus.validator]
address = "0x1234567890abcdef"
voting_power = 1
```

### **State Management**

```toml
[state]
enable_persistence = true
db_path = "./data/consensus"
checkpoint_interval = 1000
max_checkpoints = 10
cache_size = 1073741824  # 1GB
```

### **Transaction Pool**

```toml
[transaction_pool]
max_size = 10000
eviction_batch_size = 100
ttl_seconds = 300
enable_priority_queue = true
```

## 🧪 Testing

```bash
# Run all tests
cargo test -p multivm-consensus

# Run integration tests
cargo test -p multivm-consensus --test integration

# Run with logging
RUST_LOG=debug cargo test -p multivm-consensus

# Run benchmarks
cargo bench -p multivm-consensus
```

## 📊 Metrics

The consensus module provides comprehensive metrics:

- **Consensus Metrics**: Block height, round, voting statistics
- **Performance Metrics**: Block time, transaction throughput, latency
- **State Metrics**: State size, checkpoint frequency, sync status
- **Network Metrics**: Peer count, message rates, bandwidth usage

```rust
let stats = consensus_manager.get_stats().await;
println!("Blocks processed: {}", stats.consensus_stats.total_blocks);
println!("Average block time: {}ms", stats.consensus_stats.avg_block_time);
println!("Transaction pool size: {}", stats.transaction_pool_size);
```

## 🔒 Security

### **Byzantine Fault Tolerance**
- Tolerates up to f malicious validators where f < n/3
- Cryptographic signatures on all messages
- Fork detection and prevention
- Slashing for malicious behavior

### **State Security**
- Merkle tree verification for all state changes
- Cryptographic commitments for cross-VM state
- Audit trail for all consensus decisions

## 🛠️ Advanced Features

### **Fork Detection**
```rust
use multivm_consensus::fork_detection::ForkDetector;

let fork_detector = consensus_manager.get_fork_detector();
if let Some(fork) = fork_detector.detect_fork().await? {
    println!("Fork detected at height: {}", fork.height);
    fork_detector.resolve_fork(fork).await?;
}
```

### **Network Recovery**
```rust
use multivm_consensus::network_recovery::NetworkRecovery;

let recovery = consensus_manager.get_network_recovery();
if recovery.is_partitioned().await {
    recovery.initiate_recovery().await?;
}
```

### **State Synchronization**
```rust
use multivm_consensus::synchronization::BlockSynchronizer;

let sync = consensus_manager.get_synchronizer();
sync.sync_to_latest().await?;
```

## 📝 Development

### **Adding New Consensus Algorithms**

1. Implement the `ConsensusEngine` trait
2. Add configuration support
3. Update the consensus manager
4. Add comprehensive tests

### **Contributing**

1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## 📚 References

- [Malachite BFT Paper](https://github.com/informalsystems/malachite/blob/main/docs/paper.pdf)
- [Byzantine Fault Tolerance](https://en.wikipedia.org/wiki/Byzantine_fault)
- [PBFT Algorithm](https://pmg.csail.mit.edu/papers/osdi99.pdf)

## 📄 License

This module is part of the MultiVM project and follows the same license.