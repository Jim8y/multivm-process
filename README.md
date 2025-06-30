# MultiVM Blockchain

A unified blockchain platform supporting multiple virtual machines (EVM, SVM) with cross-VM execution capabilities.

## 🚀 Features

- **Multi-VM Support**: Execute transactions on Ethereum (EVM) and Solana (SVM) virtual machines
- **Cross-VM Transactions**: Enable interoperability between different blockchain ecosystems
- **Malachite BFT Consensus**: Byzantine Fault Tolerant consensus for secure block production
- **High Performance**: Process 60+ transactions per block with 3-second block times
- **Production-Ready APIs**: REST, GraphQL, and WebSocket interfaces
- **Real-Time Monitoring**: Comprehensive metrics and health monitoring

## 📋 Current Status

The MultiVM blockchain is running in production mode with:
- ✅ Full consensus implementation with Malachite BFT
- ✅ Transaction pool management with priority handling
- ✅ REST API, GraphQL, and WebSocket endpoints
- ✅ Metrics and monitoring infrastructure
- ⚠️ Mock execution engines (Reth and Solana engines temporarily disabled due to dependency conflicts)

## 🏗️ Architecture

The MultiVM system follows a **coordinator-relay architecture** where the MultiVM process coordinates consensus and relays transactions to external execution engines:

```
┌─────────────────────────────────────────────────────────────┐
│                    MultiVM Process                          │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              MultiVM Application                        ││
│  │    ┌─────────────┬─────────────┬─────────────────────┐  ││
│  │    │ REST/GraphQL│  WebSocket  │      Admin UI       │  ││
│  │    └─────────────┴─────────────┴─────────────────────┘  ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │           Transaction Router & Coordinator              ││
│  │    ┌─────────────┬─────────────┬─────────────────────┐  ││
│  │    │ EVM Relay   │  SVM Relay  │   Cross-VM Handler  │  ││
│  │    └─────────────┴─────────────┴─────────────────────┘  ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │              Malachite BFT Consensus                   ││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │                P2P Network Layer                       ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
                             │
                 ┌───────────┴───────────┐
                 │                       │
                 ▼                       ▼
┌─────────────────────────┐    ┌─────────────────────────┐
│    External Reth        │    │   External Solana       │
│    Process              │    │   Validator Process     │
│                         │    │                         │
│  ┌─────────────────────┐│    │ ┌─────────────────────┐ │
│  │   EVM Execution     ││    │ │   SVM Execution     │ │
│  │   Engine            ││    │ │   Engine            │ │
│  └─────────────────────┘│    │ └─────────────────────┘ │
│                         │    │                         │
│  ┌─────────────────────┐│    │ ┌─────────────────────┐ │
│  │   Ethereum State    ││    │ │   Solana State      │ │
│  │   Database          ││    │ │   Database          │ │
│  └─────────────────────┘│    │ └─────────────────────┘ │
└─────────────────────────┘    └─────────────────────────┘

Communication: IPC/RPC between MultiVM ↔ Reth/Solana processes
```

**Directory Structure:**
```
multivm-blockchain/
├── multivm-application/     # Main coordinator with APIs and transaction routing
├── multivm-cli/            # Command-line interface
├── multivm-common/         # Shared types and utilities
├── multivm-consensus/      # Malachite BFT consensus implementation
├── multivm-p2p/           # Peer-to-peer networking layer
├── multivm-process-manager/# Process coordination and IPC management
├── multivm-account-mapping/# Cross-VM account coordination
├── reth-execution-engine/  # [DISABLED] Reth process integration
├── solana-execution-engine/# [DISABLED] Solana process integration
└── scripts/               # Deployment and testing scripts
```

## 🚦 Quick Start

### Prerequisites

- Rust 1.75+ (stable)
- 8GB+ RAM recommended
- Linux/macOS (Windows via WSL2)

### Build

```bash
cargo build --release
```

### Run a Solo Testnet

```bash
./scripts/run-testnet.sh
```

This starts a single-node testnet with:
- REST API: http://localhost:8080
- GraphQL: http://localhost:8081
- WebSocket: ws://localhost:8082
- Metrics: http://localhost:9090
- Health: http://localhost:8090

### Monitor Activity

```bash
./scripts/dashboard.sh
```

### Run Tests

```bash
# Run comprehensive test suite
./scripts/test-suite.sh all

# Individual test commands
./scripts/test-suite.sh health      # Check service health
./scripts/test-suite.sh api         # Test REST endpoints
./scripts/test-suite.sh submit-tx   # Submit test transactions
./scripts/test-suite.sh monitor     # Monitor real-time activity
./scripts/test-suite.sh stress      # Run stress tests
```

## 📡 API Reference

### REST API

- `GET /api/v1/health` - Health check
- `GET /api/v1/node/info` - Node information
- `GET /api/v1/explorer/stats` - Blockchain statistics
- `POST /api/v1/transactions` - Submit transaction
- `GET /api/v1/blocks/{height}` - Get block by height

### GraphQL

Access the GraphQL playground at http://localhost:8081/graphql

Example query:
```graphql
{
  nodeInfo {
    version
    nodeId
    consensusAlgorithm
  }
  blockchainStats {
    totalBlocks
    totalTransactions
    transactionsPerSecond
  }
}
```

### WebSocket

Connect to `ws://localhost:8082` for real-time updates:
- Block notifications
- Transaction confirmations
- Network events

## 🔧 Configuration

The testnet configuration is located at `/tmp/multivm-testnet/config/testnet.toml`. Key settings:

- `block_time_milliseconds`: Block generation interval (default: 3000ms)
- `max_transactions_per_block`: Maximum transactions per block (default: 1000)
- `transaction_pool_size`: Maximum pending transactions (default: 10000)

## 📊 Performance

Current performance characteristics:
- Block Time: 3 seconds
- Transactions per Block: 60 (configurable)
- Theoretical TPS: 20 transactions/second
- Transaction Types: 40% EVM, 40% SVM, 20% Cross-VM

## 🛠️ Development

### Project Structure

- `multivm-application/`: Core application logic and API servers
- `multivm-consensus/`: Malachite BFT consensus implementation
- `multivm-common/`: Shared types, traits, and utilities
- `multivm-p2p/`: Network layer for peer communication
- `scripts/`: Utility scripts for deployment and testing

### Building Components

```bash
# Build all components
cargo build --release

# Build specific component
cargo build -p multivm-application --release

# Run tests
cargo test --all

# Check code quality
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

## 📚 Documentation

- [API Reference](API_REFERENCE.md) - Detailed API documentation
- [Contributing](CONTRIBUTING.md) - Contribution guidelines
- [Security](SECURITY.md) - Security policies and practices
- [Production Deployment](PRODUCTION_DEPLOYMENT.md) - Deployment guide

### Archived Documentation

Historical documentation has been moved to `docs/archive/` for reference.

## 🔐 Security

See [SECURITY.md](SECURITY.md) for security policies and how to report vulnerabilities.

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## ⚠️ Known Issues

1. **Execution Engines**: Reth and Solana execution engines are temporarily disabled due to dependency conflicts. The system currently uses mock implementations.

2. **P2P Networking**: The P2P layer is fully implemented with modular architecture (discovery, security, monitoring). The current deployment supports both single-node and multi-node consensus modes.

3. **State Persistence**: State management uses RocksDB but requires additional work for production deployments.

## 👥 Development Team Task Breakdown

The MultiVM project is organized for a 3-person specialist team. Below are detailed, module-based tasks for each specialist to work independently while maintaining integration points.

### **🟦 RETH/EVM SPECIALIST - External Reth Process Integration**

**Primary Focus**: Enable MultiVM to coordinate with external Reth processes via IPC/RPC

#### **Phase 1: Foundation & Re-enablement**

##### **Task 1.1: Dependency Resolution & Build System**
- [ ] **Fix ed25519-dalek conflicts** in `reth-execution-engine/Cargo.toml`
  - Resolve v2.1 (libp2p) vs v1.6 (reth) version conflicts
  - Update `c-kzg` linking issues preventing compilation
  - Test compatibility with MultiVM type system

- [ ] **Re-enable Reth in workspace**
  - Remove `#[cfg(feature = "disabled")]` guards
  - Update root `Cargo.toml` to include `reth-execution-engine`
  - Verify clean compilation with existing MultiVM infrastructure

##### **Task 1.2: Reth Process Coordination**
- [ ] **Complete `reth-execution-engine/src/real_engine.rs` (797 lines)**
  - Implement external Reth node process spawning and management
  - Complete `create_jwt_token()` method for secure Engine API authentication
  - Implement `get_chain_name()` and `rlp_encode_transaction()` methods
  - Add Reth node process lifecycle management (start/stop/restart)

#### **Phase 2: Core EVM Integration**

##### **Task 2.1: Engine API v3 Communication**
- [ ] **Complete `reth-execution-engine/src/engine_api.rs`**
  - Implement RPC client for `engine_newPayloadV3` calls to external Reth
  - Complete `engine_forkchoiceUpdatedV3` relay with withdrawal processing
  - Add `engine_getPayloadV3` request/response handling with blob bundles
  - Implement proper error handling and retry logic for external process communication

##### **Task 2.2: Transaction Relay Pipeline**
- [ ] **Enhance `reth-execution-engine/src/engine.rs` (1616 lines)**
  - Replace mock transaction execution with transaction forwarding to Reth process
  - Implement transaction receipt collection from external Reth
  - Add transaction validation by querying Reth's transaction pool
  - Connect MultiVM transaction format to Reth-compatible format conversion

##### **Task 2.3: Block Coordination Integration**
- [ ] **Connect to `multivm-process-manager/src/consensus_block_generator.rs`**
  - Replace mock EVM block generation with block requests to external Reth
  - Implement block header collection from Reth process
  - Add state root verification through Reth RPC calls
  - Coordinate block timing between MultiVM consensus and Reth process

#### **Phase 3: IPC & State Management**

##### **Task 3.1: IPC Client Implementation**
- [ ] **Complete `reth-execution-engine/src/ipc_client.rs`**
  - Implement secure IPC transport to external Reth process
  - Add encrypted communication channel setup between MultiVM and Reth
  - Implement message queuing and response handling for Reth RPC calls
  - Add connection recovery and health monitoring for Reth process

##### **Task 3.2: Account State Relay Integration**
- [ ] **Connect to `multivm-account-mapping/src/vm_engines/ethereum_engine.rs`**
  - Implement Ethereum address validation by querying external Reth
  - Add EVM account state queries through Reth RPC calls
  - Connect to cross-VM account binding system via Reth state queries
  - Implement balance and nonce tracking through Reth process

##### **Task 3.3: State Coordination & Monitoring**
- [ ] **Implement state management in `reth-execution-engine/src/state_manager.rs`**
  - Connect to external Reth's state database via RPC
  - Implement state snapshot requests from Reth process
  - Add state monitoring and synchronization with external Reth
  - Implement proper state root verification through Reth queries

#### **Phase 4: API & Testing**

##### **Task 4.1: JSON-RPC Relay Compatibility**
- [ ] **Complete Ethereum RPC method relaying**
  - Implement standard `eth_*` method forwarding to external Reth
  - Add transaction submission relay via `eth_sendRawTransaction` to Reth
  - Implement block query forwarding (`eth_getBlockByNumber`, etc.) to Reth
  - Add account query relay (`eth_getBalance`, `eth_getTransactionCount`) to Reth

##### **Task 4.2: Integration Testing**
- [ ] **Comprehensive EVM relay testing suite**
  - Unit tests for Reth process communication and RPC forwarding
  - Integration tests with MultiVM consensus and external Reth process
  - Smart contract deployment and execution tests through Reth relay
  - Performance benchmarking with realistic workloads via external Reth

---

### **🟨 SOLANA/SVM SPECIALIST - External Solana Process Integration**

**Primary Focus**: Enable MultiVM to coordinate with external Solana validator processes via RPC

#### **Phase 1: Foundation & Re-enablement**

##### **Task 1.1: Dependency Resolution**
- [x] **Fix Solana SDK conflicts** in `solana-execution-engine/Cargo.toml`
  - Resolve circular dependency issues with MultiVM common types
  - Update Solana SDK to compatible version with existing infrastructure
  - Fix tokio runtime version conflicts

- [x] **Re-enable Solana engine in workspace**
  - Remove disabled feature flags
  - Update root `Cargo.toml` to include `solana-execution-engine`
  - Verify clean compilation with MultiVM infrastructure

##### **Task 1.2: Solana Process Coordination**
- [ ] **Complete `solana-execution-engine/src/real_engine.rs` (665 lines)**
  - Implement external Solana validator process spawning and management (lines 148-260)
  - Set up execution-only mode validator coordination (P2P/consensus disabled)
  - Complete `submit_block_to_validator()` method for external process communication
  - Add proper validator process lifecycle management and health monitoring

#### **Phase 2: Core SVM Integration**

##### **Task 2.1: Validator RPC Communication**
- [ ] **Complete `solana-execution-engine/src/validator_api.rs`**
  - Implement RPC client for external Solana validator slot management
  - Add state queries through external validator RPC endpoints
  - Implement account lookup and balance queries via validator RPC
  - Add proper error handling and retry logic for external validator communication

##### **Task 2.2: Transaction Relay Processing**
- [ ] **Enhance `solana-execution-engine/src/engine.rs` (1118 lines)**
  - Replace mock transaction handling with transaction forwarding to external Solana validator
  - Implement proper transaction deserialization via `deserialize_solana_transaction()`
  - Complete `submit_raw_transaction_data()` with external validator submission
  - Add transaction confirmation and status tracking from external validator

##### **Task 2.3: Slot Coordination with External Validator**
- [ ] **Connect to `multivm-process-manager/src/consensus_block_generator.rs`**
  - Replace mock SVM transaction generation with transaction requests to external validator
  - Coordinate slot progression between MultiVM consensus and external Solana validator
  - Implement proper slot finalization callbacks from external validator
  - Add slot health monitoring and recovery for external validator process

#### **Phase 3: RPC & State Management**

##### **Task 3.1: RPC Client Implementation**
- [ ] **Complete `solana-execution-engine/src/rpc_client.rs`**
  - Implement connection pooling to Solana validator RPC
  - Add proper request/response handling
  - Implement retry logic and error recovery
  - Add RPC method batching for efficiency

##### **Task 3.2: RPC Server Implementation**
- [ ] **Complete `solana-execution-engine/src/rpc_server.rs`**
  - Implement Solana-compatible RPC methods
  - Add transaction submission endpoints
  - Implement account and program queries
  - Add proper authentication and rate limiting

##### **Task 3.3: Account State Relay Integration**
- [ ] **Connect to `multivm-account-mapping/src/vm_engines/solana_engine.rs`**
  - Implement Solana address validation by querying external validator
  - Add SVM account state queries through external validator RPC
  - Connect to cross-VM account binding system via validator state queries
  - Implement SOL balance and account data tracking through external validator

#### **Phase 4: Testing & Optimization**

##### **Task 4.1: Solana RPC Relay Compatibility**
- [ ] **Complete standard Solana RPC method relaying**
  - Implement `getAccountInfo`, `getBalance`, `getTransaction` forwarding to external validator
  - Add `sendTransaction` and `simulateTransaction` relay to external validator
  - Implement `getSlot`, `getBlockHeight`, `getRecentBlockhash` forwarding to validator
  - Add program account queries and filtering relay to external validator

##### **Task 4.2: Integration Testing**
- [ ] **Comprehensive SVM relay testing suite**
  - Unit tests for external validator communication and RPC forwarding
  - Integration tests with MultiVM consensus and external Solana validator
  - Solana program deployment and execution tests through validator relay
  - Performance benchmarking with realistic transaction loads via external validator

---

### **🟩 MULTIVM COORDINATION SPECIALIST - Consensus & Cross-Process Coordination**

**Primary Focus**: Complete MultiVM consensus layer and coordinate external Reth/Solana processes

#### **Phase 1: Consensus Integration**

##### **Task 1.1: Transaction Pool Enhancement**
- [ ] **Complete `multivm-consensus/src/transaction_pool.rs` (431 lines)**
  - Replace mock transaction validation with external Reth/Solana validation requests
  - Connect to actual Reth and Solana processes for transaction validation
  - Implement proper transaction priority and fee handling across VMs
  - Add transaction pool persistence and recovery with external process coordination

##### **Task 1.2: Block Generator Integration**
- [ ] **Enhance `multivm-process-manager/src/consensus_block_generator.rs` (256 lines)**
  - Replace mock transaction generation with real transaction requests from external processes
  - Coordinate EVM/SVM transaction collection from external Reth/Solana processes
  - Implement proper block timing (3-second intervals) with external process synchronization
  - Add block validation through external Reth/Solana process verification

#### **Phase 2: Cross-VM Coordination**

##### **Task 2.1: Cross-VM Coordinator Implementation**
- [ ] **Complete `multivm-account-mapping/src/cross_vm_coordinator.rs` (1023 lines)**
  - Connect `VmOperation` execution to external Reth/Solana processes
  - Implement `OperationType::Lock`, `OperationType::Transfer`, `OperationType::Mint` via external processes
  - Add atomic cross-VM transaction coordination across external processes
  - Implement proper rollback mechanisms for failed operations across processes

##### **Task 2.2: Account Mapping External Process Integration**
- [ ] **Connect `multivm-account-mapping/src/mapping.rs` to external processes**
  - Integrate Ethereum address validation with external Reth process
  - Connect Solana address validation with external Solana validator process
  - Implement real-time account balance synchronization across external processes
  - Add account binding persistence and recovery with external process coordination

##### **Task 2.3: Cross-Process Validation Layer Enhancement**
- [ ] **Complete `multivm-account-mapping/src/validation.rs`**
  - Add real signature validation using external process cryptography
  - Implement cross-VM transaction format validation across external processes
  - Add proper error handling and validation reporting for external process failures
  - Implement security checks for cross-VM operations across processes

#### **Phase 3: API & Process Coordination**

##### **Task 3.1: MultiVM API Implementation**
- [ ] **Complete `multivm-application/src/api/rest/handlers/multivm.rs` (268 lines)**
  - Replace mock responses with real cross-VM coordinator integration via external processes
  - Implement `send_cross_vm_transaction()` with actual execution across external Reth/Solana
  - Add proper transaction status tracking and reporting across external processes
  - Implement WebSocket updates for cross-VM transaction progress across processes

##### **Task 3.2: External Process Coordination Pipeline**
- [ ] **Complete `multivm-process-manager/src/coordinator.rs`**
  - Implement secure IPC coordination with external Reth/Solana processes
  - Add external process health monitoring and recovery (Reth/Solana restart)
  - Implement proper message routing and response handling to external processes
  - Add encryption layer for MultiVM ↔ Reth/Solana communication

##### **Task 3.3: External Process IPC Transport Layer**
- [ ] **Complete `multivm-process-manager/src/ipc_transport.rs`**
  - Implement encryption for IPC communications with external Reth/Solana (currently TODO)
  - Add proper key management and rotation for external process communication
  - Implement message queuing and delivery guarantees to external processes
  - Add connection pooling and load balancing for external Reth/Solana connections

#### **Phase 4: Production Readiness**

##### **Task 4.1: Configuration Management**
- [ ] **Enhance configuration system**
  - Remove hardcoded validator sets in consensus
  - Add environment-based VM engine configuration
  - Implement configuration validation and error reporting
  - Add hot-reload capabilities for production settings

##### **Task 4.2: External Process Monitoring & Health Checks**
- [ ] **Replace mock implementations with real external process monitoring**
  - Implement real health checks for external Reth/Solana processes
  - Add comprehensive metrics collection for cross-VM operations across external processes
  - Implement alerting for failed transactions and external process errors
  - Add performance monitoring and bottleneck detection for external process communication

##### **Task 4.3: Multi-Process Testing Infrastructure**
- [ ] **Comprehensive external process integration testing**
  - Multi-process stress testing framework for Reth/Solana coordination
  - End-to-end transaction flow testing across external processes
  - External process failure recovery and rollback testing
  - Performance benchmarking under realistic loads with external processes

---

## **🎯 Critical Integration Checkpoints**

### **Milestone 1: Foundation Complete**
- All engines building and basic IPC functional
- Mock mode still operational for development

### **Milestone 2: Core Engines Functional**
- Reth executing real Ethereum transactions
- Solana processing real SVM transactions
- Basic consensus coordination working

### **Milestone 3: Full Integration**
- Cross-VM coordinator operational with real engines
- Complete API functionality
- Encrypted IPC operational

### **Milestone 4: Production Ready**
- Comprehensive testing complete
- Performance optimized
- Production deployment ready

## 🚧 Roadmap

- [ ] **Phase 1**: Resolve execution engine dependency conflicts
- [ ] **Phase 2**: Complete individual VM implementations
- [ ] **Phase 3**: Enable full cross-VM coordination
- [ ] **Phase 4**: Production deployment readiness
- [ ] Add support for more VM types
- [ ] Enhanced cross-VM transaction capabilities
- [ ] Multi-node P2P networking

---

For questions or support, please open an issue on GitHub.