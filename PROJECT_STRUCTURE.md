# MultiVM Project Structure

This document outlines the organized structure of the MultiVM project after cleanup and reorganization.

## Root Directory Structure

```
multivm/
├── README.md                    # Main project documentation
├── Cargo.toml                   # Rust workspace configuration
├── Dockerfile                   # Production Docker configuration
├── .github/                     # GitHub workflows and templates
├── docs/                        # 📁 Comprehensive documentation
├── scripts/                     # 🔧 Production deployment scripts
├── tools/                       # 🛠 Development and testing tools
├── examples/                    # 📝 Integration examples
├── testnet/                     # 🌐 Production testnet configuration
├── multivm-explorer/            # 🔍 Blockchain explorer
└── [core modules...]            # Core Rust modules
```

## Documentation Structure (`docs/`)

```
docs/
├── README.md                           # Documentation index
├── RETH_INTEGRATION_GUIDE.md          # Core integration guide
├── DEPLOYMENT.md                      # Production deployment
├── CONSENSUS.md                       # Consensus layer docs
├── CONFIGURATION.md                   # Configuration reference
├── api/
│   └── API_REFERENCE.md               # Complete API reference
└── testnet/
    └── DOCKER_TESTNET.md              # Testnet setup guide
```

## Tools Structure (`tools/`)

```
tools/
├── README.md                          # Tools overview
├── transaction-generators/            # Transaction testing tools
│   ├── send-transactions-continuous.py    # 1 tx/sec generator
│   ├── send-varied-transactions.py        # Mixed transaction types
│   └── send-fast-transactions.py          # High-throughput testing
└── account-management/
    └── generate-validator-accounts.py     # Validator account generator
```

## Scripts Structure (`scripts/`)

```
scripts/
├── start-testnet.sh                   # 🚀 Main testnet launcher
├── monitor-testnet.sh                 # 📊 Network monitoring
├── verify-testnet.sh                  # ✅ Network verification
├── setup-reth-node.sh                 # ⚙️ Reth node setup
├── generate-jwt-token.sh              # 🔐 JWT management
└── [additional utilities...]           # Other automation scripts
```

## Examples Structure (`examples/`)

```
examples/
├── complete_multivm_integration.rs    # Full integration example
└── simple_integration_test.rs         # Basic integration test
```

## Testnet Structure (`testnet/`)

```
testnet/
├── docker-compose.yml                 # Main orchestration
├── genesis.json                       # Genesis block configuration
├── validators_complete.json           # Validator definitions
├── configs/                           # Per-node configurations
│   ├── production.toml.example        # Production config template
│   ├── node1/ ... node7/              # Individual node configs
└── node1/ ... node7/                  # Runtime directories
    ├── keys/                          # Cryptographic keys
    ├── multivm.toml                   # MultiVM configuration
    ├── reth.toml                      # Reth configuration
    └── genesis.json                   # Node-specific genesis
```

## Explorer Structure (`multivm-explorer/`)

```
multivm-explorer/
├── package.json                       # Node.js dependencies
├── server.js                          # Express server with WebSocket
├── Dockerfile                         # Explorer containerization
└── public/                            # Frontend assets
    ├── index.html                     # Main explorer interface
    └── script.js                      # Frontend JavaScript
```

## Core Modules

### Consensus Layer
- `multivm-consensus/` - Malachite BFT consensus implementation
- `multivm-p2p/` - Peer-to-peer networking layer

### Execution Engines  
- `reth-execution-engine/` - Ethereum execution via Reth
- `solana-execution-engine/` - Solana execution engine

### Common Infrastructure
- `multivm-common/` - Shared types and utilities
- `multivm-application/` - Main application framework
- `multivm-process-manager/` - Process lifecycle management
- `multivm-account-mapping/` - Cross-chain account mapping

### CLI and Tools
- `multivm-cli/` - Command-line interface
- `multivm-mock-processes/` - Testing utilities

## Key Features by Directory

### Production Ready (`scripts/`, `testnet/`)
- Automated deployment and monitoring
- 7-node Byzantine Fault Tolerant testnet
- JWT-secured RPC communication
- Health checking and verification

### Development Tools (`tools/`, `examples/`)
- Transaction generators for testing
- Account management utilities
- Integration examples and templates

### Documentation (`docs/`)
- Comprehensive setup guides
- API references and technical specifications
- Testing and deployment procedures

### Monitoring (`multivm-explorer/`)
- Real-time blockchain explorer
- Network health dashboard
- Transaction and block inspection

## Quick Start

1. **Setup**: Follow [`docs/RETH_INTEGRATION_GUIDE.md`](docs/RETH_INTEGRATION_GUIDE.md)
2. **Deploy**: Run `scripts/start-testnet.sh`
3. **Monitor**: Visit `http://localhost:3000` for explorer
4. **Test**: Use tools in `tools/transaction-generators/`

This structure provides clear separation of concerns while maintaining ease of use for both development and production deployment.