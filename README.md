# MultiVM Blockchain Platform

A production-ready multi-blockchain execution platform that unifies EVM and SVM transaction execution under a single Malachite BFT consensus layer.

## 🚀 Overview

MultiVM is a revolutionary blockchain architecture that:
- **Separates Consensus from Execution**: MultiVM handles consensus, networking, and account management while delegating execution to specialized engines
- **Multi-VM Support**: Executes both EVM transactions (via Reth) and SVM transactions (via Solana) 
- **Unified Account System**: SHA256-based cross-chain account mapping with A↔M↔B binding pattern
- **Byzantine Fault Tolerant**: Production-grade Malachite BFT consensus with view changes and safety guarantees
- **Process Isolation**: Execution engines run as isolated processes with restricted networking

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                       MultiVM Core Process                       │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │ • Malachite BFT Consensus (2/3+ majority, view changes)     ││
│  │ • P2P Networking (libp2p with gossipsub)                    ││
│  │ • Account Mapping (SHA256-based cross-chain identity)       ││
│  │ • Request Relay & RPC Wrapping                              ││
│  │ • Block Routing & Transaction Decomposition                 ││
│  └─────────────────────────────────────────────────────────────┘│
│                     ▼ IPC Communication ▼                        │
└──────────────────────┬──────────────┬───────────────────────────┘
                       │              │
    ┌──────────────────▼───┐    ┌────▼──────────────────┐
    │  Reth Process        │    │  Solana Process       │
    │  • EVM Execution     │    │  • SVM Execution      │
    │  • Ethereum State    │    │  • Solana State       │
    │  • No P2P (isolated) │    │  • No P2P (isolated)  │
    └──────────────────────┘    └───────────────────────┘
```

### Key Architectural Principles

1. **Execution Isolation**: Reth and Solana run as separate processes, banned from P2P networking
2. **Consensus Unification**: All transactions go through MultiVM's Malachite BFT consensus
3. **State Sovereignty**: Each execution engine maintains its own state independently
4. **Account Abstraction**: Users have one MultiVM account that can control addresses on both chains

## 🚦 Quick Start

### Prerequisites

- Rust 1.75+ 
- Docker & Docker Compose
- Git
- **Custom Fork Binaries** (REQUIRED):
  - Reth: `git@github.com:vm-multiverse/reth.git` (dev branch)
  - Solana: `git@github.com:vm-multiverse/multivm-agave.git` (master branch)
  
⚠️ **IMPORTANT**: Do NOT use official Reth or Solana binaries. MultiVM requires custom forks with P2P and consensus disabled.

### Installation

```bash
# Clone the repository
git clone https://github.com/your-org/multivm-process
cd multivm-process

# Setup custom fork binaries (REQUIRED)
./scripts/setup-test-binaries.sh

# Build the project
cargo build --release

# Run tests
cargo test
```

### Building Custom Forks

```bash
# Option 1: Automated setup
./scripts/setup-test-binaries.sh

# Option 2: Manual build
# Reth
git clone -b dev git@github.com:vm-multiverse/reth.git
cd reth && cargo build --release --bin reth

# Solana
git clone -b master git@github.com:vm-multiverse/multivm-agave.git
cd multivm-agave && cargo build --release --bin solana-test-validator
```

### Running a Local Testnet

```bash
# Start a 3-node testnet
docker-compose -f docker-compose.yml up

# Or use the production 7-node configuration
docker-compose -f docker-compose.production.yml up
```

### Sending Transactions

```bash
# Send an EVM transaction
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0x..."],"id":1}'

# Send an SVM transaction (when Solana is enabled)
curl -X POST http://localhost:8899 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"sendTransaction","params":["..."],"id":1}'
```

## 📁 Project Structure

```
multivm-process/
├── multivm-consensus/        # Malachite BFT consensus implementation
├── multivm-p2p/             # P2P networking layer
├── multivm-process-manager/  # Process lifecycle and IPC management
├── multivm-account-mapping/  # Cross-chain account binding
├── multivm-common/          # Shared types and utilities
├── reth-execution-engine/   # Reth integration wrapper
├── solana-execution-engine/ # Solana integration wrapper
├── multivm-cli/            # Command-line interface
├── multivm-application/    # REST/GraphQL API server
└── docs/                   # Documentation
```

## 🔧 Configuration

### Network Configuration

Create a `config.toml` file:

```toml
[system]
chain_id = 1337
data_dir = "./data"
log_level = "info"

[consensus]
type = "malachite"
block_time_ms = 1000
view_timeout_ms = 5000

[blockchain.ethereum]
enabled = true
rpc_url = "http://localhost:8545"
chain_id = 1

[blockchain.solana]
enabled = false  # Coming soon
rpc_url = "http://localhost:8899"
```

### Validator Setup

```bash
# Generate validator keys
./multivm-cli validator generate --output validator.json

# Start validator node
./multivm-cli node start --config config.toml --validator validator.json
```

## 🛠️ Development

### Building Components

```bash
# Build specific component
cargo build -p multivm-consensus

# Build with all features
cargo build --all-features

# Build without default features
cargo build --no-default-features
```

### Running Tests

```bash
# Unit tests
cargo test

# Integration tests
cargo test --test integration_tests

# Specific component tests
cargo test -p multivm-consensus
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy -- -D warnings

# Check dependencies
cargo audit
```

## 📊 Monitoring

### Health Endpoints

- MultiVM Health: `http://localhost:8080/health`
- Consensus Status: `http://localhost:8080/consensus/status`
- Execution Engine Status: `http://localhost:8080/engines/status`

### Metrics

Prometheus metrics available at `http://localhost:9090/metrics`:
- `multivm_consensus_height` - Current consensus height
- `multivm_consensus_round` - Current consensus round
- `multivm_transactions_total` - Total transactions processed
- `multivm_block_time` - Block production time

## 🔐 Security

- **JWT Authentication**: Engine API uses JWT tokens for authentication
- **Ed25519 Signatures**: All consensus messages are signed
- **Process Isolation**: Execution engines run with restricted permissions
- **No Hardcoded Keys**: All keys must be provided via configuration

## 📚 Documentation

- [Architecture Overview](docs/ARCHITECTURE.md)
- [Consensus Protocol](docs/CONSENSUS.md)
- [Account System](docs/ACCOUNTS.md)
- [API Reference](docs/API.md)
- [Deployment Guide](docs/DEPLOYMENT.md)

## 🤝 Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- [Malachite BFT](https://github.com/informalsystems/malachite) for the consensus engine
- [Reth](https://github.com/paradigmxyz/reth) for Ethereum execution
- [Solana](https://github.com/solana-labs/solana) for SVM execution