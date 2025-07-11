# MultiVM Consensus Documentation

## Overview

The MultiVM consensus layer provides a production-ready Byzantine Fault Tolerant (BFT) consensus implementation using the Malachite BFT protocol. This document details the consensus architecture, components, and usage patterns.

## Table of Contents

- [Architecture](#architecture)
- [Core Components](#core-components)
- [Leader Selection](#leader-selection)
- [Validator Set Management](#validator-set-management)
- [View Change Mechanism](#view-change-mechanism)
- [BFT Safety and Liveness](#bft-safety-and-liveness)
- [Configuration](#configuration)
- [API Reference](#api-reference)
- [Production Deployment](#production-deployment)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)

## Architecture

The MultiVM consensus system follows a modular BFT architecture with the following key principles:

- **Byzantine Fault Tolerance**: Tolerates up to 1/3 malicious validators
- **Round-Robin Leader Selection**: Fair and deterministic leader rotation
- **View Change Protocol**: Automatic leader failover on timeouts
- **Cross-VM State Consistency**: Ensures consistency across multiple VMs
- **Production Ready**: Full error handling, logging, and metrics

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 MultiVM Consensus Layer                     │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │   Leader    │  │ Validator   │  │ View Change │         │
│  │ Selection   │  │    Set      │  │   Manager   │         │
│  │             │  │ Management  │  │             │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │ Consensus   │  │ Transaction │  │    State    │         │
│  │  Manager    │  │    Pool     │  │ Management  │         │
│  │             │  │             │  │             │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│              Malachite BFT Engine                          │
├─────────────────────────────────────────────────────────────┤
│                   P2P Network Layer                        │
└─────────────────────────────────────────────────────────────┘
```

## Core Components

### 1. Consensus Manager (`MultiVMConsensusManager`)

The main consensus coordinator that orchestrates all consensus activities.

**Key Features:**
- Manages consensus lifecycle (start, stop, configure)
- Coordinates with P2P network for message handling
- Integrates with transaction pool for block proposals
- Provides APIs for consensus state queries

**Main Methods:**
```rust
// Initialize validator set for BFT consensus
async fn initialize_validators(validators: Vec<(NodeId, u64)>) -> ConsensusResult<()>

// Check if this node is the current block proposer
async fn is_block_proposer() -> ConsensusResult<bool>

// Get the current designated proposer
async fn get_current_proposer() -> ConsensusResult<NodeId>

// Propose a block if this node is the leader
async fn try_propose_block() -> ConsensusResult<bool>

// Handle view change message from network
async fn handle_view_change_message(sender: &NodeId, height: u64, new_round: u32, signature: Vec<u8>) -> ConsensusResult<()>

// Manually trigger view change (for emergencies)
async fn trigger_view_change() -> ConsensusResult<()>
```

### 2. Leader Selection (`LeaderSelector`)

Implements deterministic round-robin leader selection for fair block proposal rotation.

**Algorithm:**
- Uses `(height * 1000 + round) % validator_count` for deterministic selection
- Ensures all validators get equal opportunity to propose blocks
- Validators are sorted alphabetically for consistent ordering

**Usage:**
```rust
use multivm_consensus::leader_selection::{LeaderSelector, LeaderSelectionStrategy};
use multivm_consensus::malachite::types::{ValidatorAddress, Round};

// Create round-robin selector
let validators = vec![
    ValidatorAddress("alice".to_string()),
    ValidatorAddress("bob".to_string()),
    ValidatorAddress("charlie".to_string()),
];

let selector = LeaderSelector::new_round_robin(validators);

// Get leader for specific height and round
let leader = selector.get_leader(height, Round::new(round)).await?;

// Check if a specific validator is the leader
let is_leader = selector.is_leader(&validator_addr, height, Round::new(round)).await?;
```

### 3. Validator Set Management (`ValidatorSetManager`)

Manages the active validator set with voting power and BFT threshold calculations.

**Features:**
- Dynamic validator addition/removal
- Voting power management
- BFT threshold calculation (2/3 + 1)
- Validator activity tracking

**Key Methods:**
```rust
// Initialize validator set
async fn initialize(validators: Vec<Validator>) -> ConsensusResult<()>

// Add new validator
async fn add_validator(validator: Validator) -> ConsensusResult<()>

// Remove validator
async fn remove_validator(address: &ValidatorAddress) -> ConsensusResult<()>

// Get required voting power for consensus (2/3 + 1)
async fn get_required_voting_power() -> u64

// Check if validator set has sufficient power for consensus
async fn has_sufficient_power(validators: &HashSet<ValidatorAddress>) -> bool
```

### 4. View Change Management (`ViewChangeManager`)

Handles leader failures and view changes to maintain liveness.

**Process:**
1. Timeout detection for current leader
2. View change message broadcasting
3. Threshold-based view change completion
4. New leader determination and transition

**Usage:**
```rust
// Start view change for new round
let message = view_change_manager.start_view_change(height, new_round).await?;

// Process view change from other validator
let threshold_reached = view_change_manager.process_view_change(
    &sender_address, height, new_round, signature
).await?;

// Complete view change and get new leader
if threshold_reached {
    let new_leader = view_change_manager.complete_view_change().await?;
}
```

## Leader Selection

### Round-Robin Algorithm

The consensus uses a deterministic round-robin algorithm:

1. **Deterministic Selection**: `leader_index = (height * 1000 + round) % validator_count`
2. **Fair Rotation**: Each validator gets equal opportunity to propose
3. **Consistent Ordering**: Validators sorted alphabetically across all nodes

### Example Rotation

For validators `["alice", "bob", "charlie"]`:

| Height | Round | Leader  | Calculation |
|--------|-------|---------|-------------|
| 0      | 0     | alice   | (0*1000+0) % 3 = 0 |
| 0      | 1     | bob     | (0*1000+1) % 3 = 1 |
| 0      | 2     | charlie | (0*1000+2) % 3 = 2 |
| 1      | 0     | bob     | (1*1000+0) % 3 = 1 |
| 1      | 1     | charlie | (1*1000+1) % 3 = 2 |

## Validator Set Management

### BFT Requirements

- **Minimum Validators**: 4 (to tolerate 1 Byzantine fault)
- **Voting Threshold**: 2/3 + 1 of total voting power
- **Active Validators**: Only active validators participate in consensus

### Validator Structure

```rust
pub struct Validator {
    pub address: ValidatorAddress,      // Unique validator identifier
    pub public_key: String,             // Public key for verification
    pub voting_power: u64,              // Voting weight in consensus
    pub is_active: bool,                // Whether validator is active
    pub last_seen: Option<SystemTime>,  // Last activity timestamp
}
```

### Dynamic Validator Updates

Validators can be added or removed dynamically:

```rust
// Add new validator
let new_validator = Validator {
    address: ValidatorAddress("new_validator".to_string()),
    public_key: "pubkey_new".to_string(),
    voting_power: 100,
    is_active: true,
    last_seen: Some(SystemTime::now()),
};

validator_set_manager.add_validator(new_validator).await?;

// Remove validator
validator_set_manager.remove_validator(&validator_address).await?;
```

## View Change Mechanism

### Trigger Conditions

View changes are triggered when:
1. Leader timeout (no block proposal within timeout period)
2. Manual trigger (emergency situations)
3. Network partition recovery

### View Change Process

1. **Timeout Detection**: Each validator monitors proposal timeouts
2. **View Change Initiation**: Send view change message with new round
3. **Threshold Collection**: Collect view change messages from other validators
4. **Majority Reached**: When 2/3+ validators agree, complete view change
5. **Leader Transition**: New leader determined by round-robin for new round

### Configuration

```rust
// View change timeout configuration
let view_change_timeout = Duration::from_secs(30);

// Create view change manager
let view_change_manager = ViewChangeManager::new(
    node_id,
    leader_selector,
    view_change_timeout
);
```

## BFT Safety and Liveness

### Safety Properties

1. **Agreement**: All honest validators agree on committed blocks
2. **Validity**: Only valid blocks are committed
3. **Integrity**: Blocks are not modified during consensus

### Liveness Properties

1. **Progress**: System continues to make progress with honest majority
2. **Leader Rotation**: Failed leaders are replaced via view changes
3. **Recovery**: System recovers from network partitions

### Fault Tolerance

- **Byzantine Tolerance**: Up to f = ⌊(n-1)/3⌋ Byzantine validators
- **Network Partitions**: Majority partition continues operation
- **Leader Failures**: Automatic failover via view changes

## Configuration

### Basic Configuration

```rust
use multivm_consensus::{ConsensusManagerConfig, ConsensusAlgorithmType, AlgorithmConfig, MalachiteConfig};

let config = ConsensusManagerConfig {
    node_id: Some("validator1".to_string()),
    algorithm: ConsensusAlgorithmType::Malachite,
    algorithm_config: AlgorithmConfig::Malachite(MalachiteConfig {
        timeout_propose_ms: 3000,
        timeout_prevote_ms: 1000,
        timeout_precommit_ms: 1000,
        // ... other Malachite-specific settings
    }),
    block_proposal_interval_ms: 3000,
    max_transactions_per_block: 1000,
    enable_auto_proposal: true,
    // ... network and state configuration
};

let consensus_manager = MultiVMConsensusManager::new(config).await?;
```

### Validator Setup

```rust
// Define validator set with voting power
let validators = vec![
    ("validator1".to_string(), 100),  // 25% voting power
    ("validator2".to_string(), 100),  // 25% voting power
    ("validator3".to_string(), 100),  // 25% voting power
    ("validator4".to_string(), 100),  // 25% voting power
];

// Initialize validators
consensus_manager.initialize_validators(validators).await?;
```

## API Reference

### Consensus Manager APIs

```rust
impl MultiVMConsensusManager {
    // Validator management
    async fn initialize_validators(validators: Vec<(NodeId, u64)>) -> ConsensusResult<()>;
    
    // Leader queries
    async fn is_block_proposer() -> ConsensusResult<bool>;
    async fn get_current_proposer() -> ConsensusResult<NodeId>;
    
    // Block operations
    async fn try_propose_block() -> ConsensusResult<bool>;
    async fn submit_transaction(tx: serde_json::Value, priority: TransactionPriority) -> ConsensusResult<()>;
    
    // View changes
    async fn handle_view_change_message(sender: &NodeId, height: u64, new_round: u32, signature: Vec<u8>) -> ConsensusResult<()>;
    async fn trigger_view_change() -> ConsensusResult<()>;
    
    // State queries
    async fn get_consensus_stats() -> ConsensusResult<ConsensusManagerStats>;
    async fn get_cross_vm_state() -> ConsensusResult<CrossVMState>;
    
    // Lifecycle
    async fn start() -> ConsensusResult<()>;
    async fn stop() -> ConsensusResult<()>;
}
```

### Events and Monitoring

```rust
pub enum ConsensusEvent {
    BlockProposed { block: MultiVMBlock, proposer: String, height: u64 },
    BlockCommitted { block: MultiVMBlock, height: u64, block_hash: String },
    ViewChanged { old_view: u64, new_view: u64, reason: String },
    NodeJoined { node_id: NodeId },
    NodeLeft { node_id: NodeId },
    StateSync { state_root: String, height: u64 },
}
```

## Production Deployment

### Multi-Validator Setup

1. **Validator Configuration**: Each validator needs unique node ID and network config
2. **Network Setup**: Configure P2P network with bootstrap nodes
3. **Storage**: Persistent storage for consensus state and blocks
4. **Monitoring**: Metrics collection and health checks

### Example Production Config

```toml
[consensus]
algorithm = "malachite"
block_time_milliseconds = 3000
max_transactions_per_block = 1000
validator_count = 7

[consensus.malachite]
timeout_propose_ms = 3000
timeout_prevote_ms = 1000
timeout_precommit_ms = 1000

[network]
listen_host = "0.0.0.0"
listen_port = 8080
bootstrap_nodes = [
    "validator1.example.com:8080",
    "validator2.example.com:8080"
]

[state_manager]
rocksdb_path = "/data/consensus_state.db"
max_checkpoints = 1000
checkpoint_interval = 100
```

### Validator Deployment Steps

1. **Generate Validator Keys**: Each validator needs unique cryptographic keys
2. **Configure Networking**: Set up P2P network with proper firewall rules
3. **Initialize Genesis**: All validators start with same genesis validator set
4. **Start Consensus**: Validators join network and begin consensus
5. **Monitor Health**: Set up monitoring for consensus metrics

### Security Considerations

- **Key Management**: Secure storage of validator private keys
- **Network Security**: Encrypted P2P communication
- **Access Control**: Restrict administrative APIs
- **Monitoring**: Real-time detection of Byzantine behavior

## Testing

### Unit Tests

The consensus module includes comprehensive unit tests:

```bash
# Run consensus unit tests
cargo test -p multivm-consensus --lib

# Run specific module tests
cargo test -p multivm-consensus leader_selection::tests
cargo test -p multivm-consensus validator_set::tests  
cargo test -p multivm-consensus view_change::tests
```

### Integration Tests

Multi-validator integration tests are in `/tests/consensus_integration_tests.rs`:

```bash
# Run consensus integration tests
cargo test consensus_integration_tests

# Run specific integration test
cargo test test_multi_validator_leader_selection
```

### Test Scenarios

Key test scenarios include:
- Multi-validator leader selection and rotation
- BFT voting thresholds with various validator counts
- View change coordination across validators
- Fault tolerance with Byzantine validators
- Network partition simulation
- Performance testing with large validator sets

## Troubleshooting

### Common Issues

1. **Validator Not Proposing**
   - Check if node is current leader: `is_block_proposer()`
   - Verify validator is in active set
   - Check network connectivity

2. **View Change Timeouts**
   - Increase view change timeout duration
   - Check network latency between validators
   - Verify validator set consistency

3. **Consensus Stalled**
   - Check if sufficient validators are online (> 2/3)
   - Verify network connectivity
   - Check for clock synchronization issues

### Debugging Commands

```rust
// Check current consensus state
let stats = consensus_manager.get_consensus_stats().await?;
println!("Height: {}, Round: {}", stats.current_height, stats.current_round);

// Check current proposer
let proposer = consensus_manager.get_current_proposer().await?;
println!("Current proposer: {}", proposer);

// Check if this node should propose
let is_proposer = consensus_manager.is_block_proposer().await?;
println!("Am I proposer: {}", is_proposer);
```

### Logging Configuration

Enable detailed consensus logging:

```toml
[log]
level = "debug"
targets = [
    "multivm_consensus=debug",
    "multivm_consensus::leader_selection=trace",
    "multivm_consensus::view_change=trace"
]
```

### Metrics Monitoring

Key metrics to monitor:
- `consensus_height`: Current block height
- `consensus_round`: Current consensus round
- `validator_count`: Number of active validators
- `view_changes_total`: Total view changes
- `block_proposals_total`: Total block proposals
- `consensus_duration`: Time to reach consensus

## Conclusion

The MultiVM consensus layer provides a robust, production-ready BFT consensus implementation with:

- **Deterministic leader selection** for fair block proposal rotation
- **Dynamic validator set management** with proper BFT thresholds
- **Automatic view changes** for handling leader failures
- **Comprehensive testing** including multi-validator scenarios
- **Production-ready deployment** with monitoring and security

For additional support or questions, refer to the API documentation or create an issue in the project repository.