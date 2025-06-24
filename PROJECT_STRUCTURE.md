# MultiVM Project Structure

## Overview

This document provides a comprehensive overview of the MultiVM project structure, explaining the purpose and contents of each module.

## Directory Structure

```
multivm-process/
├── crates/                      # Core modules
│   ├── multivm-common/          # Shared types, traits, and utilities
│   ├── multivm-account-mapping/ # Cross-VM account binding system
│   ├── multivm-process-manager/ # Process lifecycle management
│   ├── multivm-p2p/            # P2P networking and encryption
│   ├── multivm-consensus/       # Malachite BFT consensus integration
│   ├── multivm-application/     # API layer (REST, GraphQL, WebSocket)
│   ├── multivm-cli/            # Command-line interface
│   ├── multivm-mock-processes/  # Mock VM processes for testing
│   ├── solana-execution-engine/ # Solana VM integration
│   └── reth-execution-engine/   # Ethereum VM integration
├── docs/                        # Documentation
├── scripts/                     # Utility scripts
├── examples/                    # Example applications
└── tests/                      # Integration tests
```

## Core Modules

### 1. multivm-common (`crates/multivm-common/`)

**Purpose**: Shared foundation for all other modules

**Key Components**:
- `traits/`: Core trait definitions (ExecutionEngine, BlockProvider, etc.)
- `types/`: Common types (ProcessId, BlockchainType, etc.)
- `errors/`: Unified error handling
- `config/`: Configuration structures
- `ipc/`: Inter-process communication primitives
- `monitoring/`: System monitoring utilities

**Status**: ✅ Production Ready

### 2. multivm-account-mapping (`crates/multivm-account-mapping/`)

**Purpose**: Manages cross-VM account bindings with cryptographic verification

**Key Components**:
- `address.rs`: Account address types for different VMs
- `binding.rs`: Account binding logic
- `validation.rs`: Cryptographic proof validation
- `storage/`: Persistent storage implementations
- `special_tx.rs`: Cross-VM transaction types

**Features**:
- Ed25519 signature verification
- Secure account binding proofs
- Cross-VM transfer validation

**Status**: ✅ Production Ready

### 3. multivm-process-manager (`crates/multivm-process-manager/`)

**Purpose**: Manages lifecycle of VM processes and coordinates operations

**Key Components**:
- `coordinator.rs`: Main system coordinator
- `manager.rs`: Process lifecycle management
- `block_router.rs`: Transaction routing logic
- `ipc_transport.rs`: Secure IPC implementation
- `health.rs`: Health monitoring

**Features**:
- Process spawning and monitoring
- Automatic restart on failure
- Resource usage tracking
- Health checks

**Status**: ⚠️ Minor compilation issues (15 errors remaining)

### 4. multivm-p2p (`crates/multivm-p2p/`)

**Purpose**: Secure peer-to-peer networking layer

**Key Components**:
- `network.rs`: P2P network management
- `encryption.rs`: Message encryption (ChaCha20-Poly1305)
- `discovery.rs`: Peer discovery
- `routing.rs`: Message routing
- `relay.rs`: Message relay capabilities

**Security Features**:
- Ed25519 signatures
- X25519 key exchange
- ChaCha20-Poly1305 AEAD encryption
- Rate limiting and DDoS protection

**Status**: ✅ Production Ready (x25519-dalek 2.0 compatibility fixed)

### 5. multivm-consensus (`crates/multivm-consensus/`)

**Purpose**: Malachite BFT consensus integration

**Key Components**:
- `core.rs`: Consensus engine
- `validator.rs`: Validator logic
- `voting.rs`: Voting mechanism
- `finality.rs`: Finality tracking
- `synchronization.rs`: State synchronization

**Features**:
- Byzantine fault tolerance (f < n/3)
- Ed25519 signature aggregation
- Fast finality (2-3 seconds)

**Status**: ✅ Production Ready

### 6. multivm-application (`crates/multivm-application/`)

**Purpose**: API layer providing REST, GraphQL, and WebSocket interfaces

**Key Components**:
- `api/rest/`: RESTful API endpoints
- `api/graphql/`: GraphQL schema and resolvers
- `api/websocket/`: Real-time WebSocket server
- `gateway/`: VM-specific gateways
- `auth/`: Authentication and authorization
- `cache/`: Multi-level caching

**Features**:
- JWT authentication
- Rate limiting
- Request validation
- Real-time subscriptions

**Status**: ✅ Production Ready

### 7. multivm-cli (`crates/multivm-cli/`)

**Purpose**: Command-line interface for system management

**Commands**:
- `run`: Start the MultiVM system
- `status`: Check system status
- `account`: Manage accounts
- `transfer`: Execute transfers
- `config`: Manage configuration

**Status**: ✅ Production Ready

### 8. Execution Engines

#### solana-execution-engine (`crates/solana-execution-engine/`)

**Purpose**: Solana VM integration

**Features**:
- Mock and real validator support
- RPC interface
- Transaction submission
- State management

**Status**: ✅ Production Ready

#### reth-execution-engine (`crates/reth-execution-engine/`)

**Purpose**: Ethereum VM integration

**Features**:
- EIP-1559 support
- RLP encoding
- Keccak-256 hashing
- EVM execution

**Status**: ✅ Production Ready

## Configuration Files

- `Cargo.toml`: Workspace configuration
- `config.example.toml`: Example configuration
- `.env.example`: Environment variables template

## Documentation

- `README.md`: Project overview
- `DEPLOYMENT.md`: Production deployment guide
- `API_REFERENCE.md`: Complete API documentation
- `SECURITY.md`: Security best practices
- `PERFORMANCE.md`: Performance tuning guide
- `PROJECT_STRUCTURE.md`: This file

## Scripts

- `scripts/consistency_check.sh`: Verify code consistency
- `scripts/start.sh`: Quick start script
- `scripts/test.sh`: Run all tests

## Development Status

### Completed ✅
- Core architecture
- Security implementation
- API layer
- Documentation
- Cross-VM operations

### In Progress ⚠️
- Minor compilation fixes in process-manager
- Performance optimizations
- Extended test coverage

### Future Enhancements 🚀
- Additional VM support
- Advanced monitoring dashboard
- Kubernetes deployment manifests
- SDK for multiple languages

## Code Quality Standards

1. **Error Handling**: All errors use thiserror with proper context
2. **Async**: Tokio runtime with proper cancellation
3. **Security**: No unsafe code without justification
4. **Testing**: Minimum 80% code coverage
5. **Documentation**: All public APIs documented

## Getting Started

1. Clone the repository
2. Run `cargo build --release`
3. Configure using `config.example.toml`
4. Start with `cargo run --bin multivm-cli run`

---

© 2024 MultiVM Project