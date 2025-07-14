# MultiVM Process

A production-ready multi-blockchain execution platform supporting EVM and SVM with Malachite BFT consensus.

## 🚀 Overview

MultiVM is a blockchain platform that:
- **Multi-VM Support**: Executes transactions on Ethereum (EVM) via Reth integration
- **Malachite BFT Consensus**: Byzantine Fault Tolerant consensus with Ed25519 signatures
- **Account System**: SHA256-based account mapping for cross-chain identity
- **Production APIs**: REST, GraphQL, WebSocket, and Ethereum RPC interfaces
- **Real-Time Monitoring**: Comprehensive blockchain explorer and metrics

## 📋 Current Status

✅ **Production Ready**:
- Full Malachite BFT consensus implementation
- Reth integration via Engine API with JWT authentication
- 7-node production testnet configuration
- Real-time blockchain explorer
- Automated validator funding and transaction generation

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    MultiVM Process                          │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              Core Components                            ││
│  │    ┌─────────────┬─────────────┬─────────────────────┐  ││
│  │    │   APIs      │  Consensus  │  Account System     │  ││
│  │    │ REST/RPC/WS │ Malachite   │  SHA256 Mapping    │  ││
│  │    └─────────────┴─────────────┴─────────────────────┘  ││
│  └─────────────────────────────────────────────────────────┘│
│                            │                                 │
│                     Engine API (JWT)                         │
│                            │                                 │
└────────────────────────────┼─────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                    Reth Execution Engine                    │
│                   (Ethereum State & EVM)                    │
└─────────────────────────────────────────────────────────────┘
```

## 🚦 Quick Start

### Prerequisites

- Docker and Docker Compose
- Python 3.8+ (for scripts)
- 8GB+ RAM recommended

### 1. Clone Repository

```bash
git clone <repository>
cd multivm-process
```

### 2. Start Production Testnet

```bash
# Start the 7-node production testnet
cd testnet-production-complete
docker compose up -d

# Verify all services are running
docker compose ps
```

### 3. Access Services

- **Blockchain Explorer**: http://localhost:3000
- **RPC Endpoints**: http://localhost:8545-8551
- **MultiVM APIs**: http://localhost:8080-8086

### 4. Fund Validators & Send Transactions

```bash
# Setup Python environment
python3 -m venv venv
source venv/bin/activate
pip install web3 eth-account

# Fund validators automatically
python3 auto-fund-validators.py

# Send real transactions
python3 send-real-transactions-final.py
```

## 📡 API Reference

### Ethereum RPC

Standard Ethereum JSON-RPC available at `http://localhost:8545-8551`

```bash
# Get balance
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_getBalance","params":["0xADDRESS","latest"],"id":1}' \
  http://localhost:8545

# Get block
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["latest",true],"id":1}' \
  http://localhost:8545
```

### MultiVM APIs

- **REST**: `http://localhost:8080-8086/api/v1/`
- **GraphQL**: `http://localhost:8080-8086/graphql`
- **WebSocket**: `ws://localhost:8080-8086/ws`

### Blockchain Explorer

Real-time monitoring at http://localhost:3000 with:
- Network overview and node health
- Block and transaction history
- Account balances
- WebSocket live updates

## 🔧 Configuration

### Network Parameters

- **Chain ID**: 1337
- **Block Time**: 3 seconds
- **Consensus**: Malachite BFT with Ed25519
- **Gas Price**: 25 Gwei default

### Account System

- **Node Operators**: Ethereum accounts (primary identity)
- **Consensus Keys**: Ed25519 (separate from account)
- **MultiVM Account**: `SHA256(ethereum_address)`
- **Cross-Chain**: Users can bind Solana accounts to same MultiVM ID

## 📚 Production Scripts

### Core Scripts

| Script | Purpose |
|--------|---------|
| `generate-validator-accounts.py` | Generate validator credentials |
| `auto-fund-validators.py` | Automated validator funding |
| `send-real-transactions-final.py` | Send real signed transactions |
| `fund-accounts.py` | Check account balances |

### Setup Scripts

| Script | Purpose |
|--------|---------|
| `setup-production-testnet.sh` | Initialize testnet |
| `complete-funding-setup.sh` | Complete funding automation |
| `fund-validators-complete.sh` | Funding with fallbacks |
| `run-complete-setup.sh` | Master automation script |

## 🛠️ Development

### Building from Source

```bash
# Build MultiVM
cargo build --release

# Build Docker images
docker build -t multivm:latest .
docker build -t multivm-explorer:latest multivm-explorer/
```

### Running Tests

```bash
# Rust tests
cargo test

# Integration tests
./scripts/test-multivm-reth-integration.sh
```

## 📊 Monitoring

### Logs

```bash
# MultiVM node logs
docker logs multivm-node1 --follow

# Reth execution logs
docker logs reth-node1 --follow

# Explorer logs
docker logs multivm-explorer --follow
```

### Metrics

- Node health: `http://localhost:8080-8086/health`
- Blockchain stats: `http://localhost:3000/api/status`

## 🔐 Security

- JWT authentication for Engine API
- Ed25519 signatures for consensus
- No hardcoded private keys in production
- Secure validator key generation

## 📄 Documentation

- [API Reference](API_REFERENCE.md) - Detailed API documentation
- [Contributing](CONTRIBUTING.md) - Development guidelines
- [Security](SECURITY.md) - Security policies
- [Production Deployment](PRODUCTION_DEPLOYMENT.md) - Deployment guide

## ⚠️ Troubleshooting

### Common Issues

1. **Validator funding fails**
   - Ensure testnet is running: `docker compose ps`
   - Check Python environment: `source venv/bin/activate`

2. **Transactions not confirming**
   - Verify gas price is sufficient (25+ Gwei)
   - Check node synchronization status

3. **Explorer not updating**
   - Verify WebSocket connection
   - Check browser console for errors

## 🤝 Contributing

Contributions welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

MIT License - see LICENSE file for details.