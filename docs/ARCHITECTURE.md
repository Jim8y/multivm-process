# MultiVM Architecture

## Overview

MultiVM is a blockchain platform that separates consensus from execution, allowing multiple virtual machine types (EVM, SVM) to operate under a unified Byzantine Fault Tolerant consensus layer.

## Core Design Principles

### 1. Separation of Concerns
- **Consensus Layer**: Handles block production, validation, and Byzantine fault tolerance
- **Execution Layer**: Processes transactions and maintains blockchain state
- **Network Layer**: Manages peer-to-peer communication and message propagation

### 2. Process Isolation
- Execution engines (Reth, Solana) run as separate OS processes
- Engines are restricted from P2P networking (no external connections)
- All external communication goes through the MultiVM core process

### 3. Unified Account System
- Single account identity across all VMs using SHA256 hashing
- A↔M↔B binding pattern for cross-chain account relationships
- Deterministic address derivation from MultiVM account ID

## System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          External Clients                            │
│         (Wallets, DApps, Block Explorers, RPC Clients)             │
└─────────────────┬─────────────────────┬─────────────────────────────┘
                  │                     │
                  ▼                     ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        MultiVM Core Process                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐   │
│  │   API Layer     │  │ Consensus Layer │  │  P2P Network    │   │
│  │                 │  │                 │  │                 │   │
│  │ • REST API     │  │ • Malachite BFT │  │ • libp2p        │   │
│  │ • GraphQL      │  │ • Block Producer│  │ • Gossipsub     │   │
│  │ • WebSocket    │  │ • Validator Set │  │ • DHT Discovery │   │
│  │ • JSON-RPC     │  │ • View Changes  │  │ • Peer Manager  │   │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘   │
│                                                                     │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐   │
│  │ Account Mapping │  │  Block Router   │  │ Process Manager │   │
│  │                 │  │                 │  │                 │   │
│  │ • SHA256 IDs   │  │ • TX Decompose  │  │ • Lifecycle Mgmt│   │
│  │ • A↔M↔B Binding│  │ • Route to VMs  │  │ • Health Checks │   │
│  │ • Cross-VM Ops │  │ • Result Merge  │  │ • IPC Transport │   │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘   │
│                                                                     │
└────────────────────────────┬───────────┬───────────────────────────┘
                             │    IPC    │
                  ┌──────────┴───────────┴──────────┐
                  ▼                                 ▼
    ┌───────────────────────┐         ┌───────────────────────┐
    │   Reth Process        │         │   Solana Process      │
    │                       │         │                       │
    │ • EVM Execution      │         │ • SVM Execution      │
    │ • Ethereum State     │         │ • Solana State       │
    │ • Engine API (JWT)   │         │ • JSON-RPC API       │
    │ • No External P2P    │         │ • No External P2P    │
    └───────────────────────┘         └───────────────────────┘
```

## Component Details

### 1. MultiVM Core Process

The main process that coordinates all blockchain operations:

#### API Layer
- **REST API**: RESTful endpoints for blockchain queries and transaction submission
- **GraphQL**: Rich query interface for complex data retrieval
- **WebSocket**: Real-time updates and event subscriptions
- **JSON-RPC**: Ethereum-compatible RPC for wallet integration

#### Consensus Layer (Malachite BFT)
- **Byzantine Fault Tolerance**: Tolerates up to 1/3 malicious validators
- **View-Based Protocol**: Rotating leaders with view change mechanism
- **Safety**: Never produces conflicting blocks at the same height
- **Liveness**: Guaranteed progress with 2/3+ honest validators

#### P2P Network
- **libp2p**: Modern, modular networking stack
- **Gossipsub**: Efficient message propagation
- **DHT**: Distributed peer discovery
- **Security**: TLS encryption, peer authentication

#### Account Mapping
- **Unified Identity**: Single MultiVM account ID (SHA256 hash)
- **Cross-Chain Binding**: Link EVM and SVM addresses to one identity
- **Special Transactions**: Account binding operations in consensus

#### Block Router
- **Transaction Classification**: Identify EVM vs SVM transactions
- **Decomposition**: Split MultiVM blocks into VM-specific batches
- **Result Aggregation**: Combine execution results into unified response

#### Process Manager
- **Lifecycle Management**: Start, stop, restart execution engines
- **Health Monitoring**: Regular health checks and automatic recovery
- **IPC Communication**: Secure inter-process communication

### 2. Execution Engines

#### Reth (Ethereum)
- **Full EVM Implementation**: Complete Ethereum Virtual Machine
- **State Management**: MPT (Merkle Patricia Trie) state storage
- **Engine API**: Authenticated JSON-RPC for consensus communication
- **Isolation**: No P2P connections, only IPC with MultiVM

#### Solana (planned)
- **SVM Execution**: Solana Virtual Machine for parallel execution
- **Account Model**: Solana's account-based state model
- **Program Deployment**: Support for Solana programs
- **Isolation**: Restricted networking, IPC-only communication

## Data Flow

### Transaction Lifecycle

1. **Client Submission**
   - Client sends transaction to MultiVM API
   - Transaction validated and added to mempool

2. **Block Production**
   - Leader proposes block with transactions
   - Block broadcast to validator network

3. **Consensus**
   - Validators vote on proposed block
   - Block committed with 2/3+ votes

4. **Execution Routing**
   - Block router classifies transactions
   - EVM transactions → Reth via IPC
   - SVM transactions → Solana via IPC

5. **State Update**
   - Execution engines process transactions
   - State roots returned to MultiVM
   - Block finalized with execution results

### Consensus Protocol

```
┌─────────┐     Propose      ┌─────────┐
│ Leader  │ ─────────────▶   │Validator│
│   V0    │                   │   V1    │
└─────────┘                   └─────────┘
     │                             │
     │         Vote               │
     │ ◀───────────────────────── │
     │                             │
     │         Commit             │
     │ ─────────────────────────▶ │
     │                             │
     ▼                             ▼
[Block N+1]                   [Block N+1]
```

## Security Model

### Byzantine Fault Tolerance
- Assumes up to 1/3 of validators may be malicious
- Requires 2/3+ majority for all decisions
- Cryptographic signatures on all messages

### Process Isolation
- Execution engines run with restricted permissions
- No network access except IPC with parent
- Resource limits enforced by OS

### Authentication
- Ed25519 signatures for consensus messages
- JWT tokens for Engine API authentication
- TLS for P2P connections

## Performance Characteristics

### Consensus
- **Block Time**: Configurable (1-5 seconds typical)
- **Throughput**: Limited by execution engine capacity
- **Finality**: Immediate with BFT consensus

### Execution
- **EVM**: ~15M gas/second (Reth performance)
- **SVM**: ~50k TPS potential (Solana performance)
- **Parallelism**: Independent execution per VM type

### Network
- **Message Complexity**: O(n²) for n validators
- **Gossip Efficiency**: O(log n) message hops
- **Bandwidth**: ~100KB/s per peer typical

## Deployment Considerations

### Hardware Requirements
- **CPU**: 8+ cores recommended
- **RAM**: 16GB minimum, 32GB recommended
- **Storage**: 500GB+ SSD for state storage
- **Network**: 100Mbps+ symmetric connection

### Validator Set
- **Minimum**: 4 validators (1 can fail)
- **Recommended**: 7+ validators for production
- **Maximum**: 100+ validators supported

### Configuration
- **Genesis**: Define initial validator set
- **Parameters**: Block time, gas limits, etc.
- **Networking**: P2P ports, API endpoints

## Future Enhancements

### Additional VMs
- **WASM**: WebAssembly runtime support
- **Move**: Move VM for Libra/Diem compatibility
- **Custom**: Plugin system for new VMs

### Scalability
- **Sharding**: Horizontal scaling with multiple shards
- **State Channels**: Off-chain transaction processing
- **Rollups**: Layer 2 scaling solutions

### Interoperability
- **IBC**: Inter-blockchain communication
- **Bridges**: Asset transfers between chains
- **Light Clients**: Efficient chain verification