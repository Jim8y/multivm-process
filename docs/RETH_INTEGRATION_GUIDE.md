# Reth Integration Guide for MultiVM

This guide explains how to set up and configure a Reth node for integration with the MultiVM blockchain system.

## Overview

The MultiVM system uses a coordinator-relay architecture where Reth runs as an external process that handles Ethereum Virtual Machine (EVM) execution. The MultiVM coordinator communicates with Reth via:

- **JSON-RPC** for standard Ethereum operations
- **Engine API** for consensus integration with JWT authentication
- **IPC** for secure inter-process communication

## Quick Start

### 1. Install and Setup

```bash
# Install Reth (if not already installed)
./scripts/setup-reth-node.sh install

# Setup Reth for development with MultiVM
./scripts/setup-reth-node.sh setup --dev

# Start Reth node
./scripts/setup-reth-node.sh start
```

### 2. Verify Integration

```bash
# Check node status
./scripts/setup-reth-node.sh status

# View logs
./scripts/setup-reth-node.sh logs

# Test RPC connectivity
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545
```

## Detailed Setup

### Prerequisites

- **Rust 1.70+** - Install from [rustup.rs](https://rustup.rs/)
- **Reth Binary** - Installed via the setup script or manually
- **System Requirements**:
  - 8GB+ RAM (16GB+ recommended for mainnet)
  - 500GB+ storage (2TB+ for mainnet)
  - Stable internet connection

### Configuration Files

The setup uses two main configuration files:

1. **`configs/reth-multivm.toml`** - Main configuration
2. **`./reth-data/jwt.hex`** - JWT secret for authentication

### Environment Variables

```bash
# Required environment variables
export RETH_DATA_DIR="./reth-data"           # Data directory
export RETH_HTTP_PORT="8545"                # RPC port
export RETH_ENGINE_PORT="8551"              # Engine API port
export RETH_CHAIN_ID="1337"                 # Chain ID (dev mode)
export JWT_SECRET_PATH="./reth-data/jwt.hex" # JWT secret file
export MULTIVM_IPC_PATH="/tmp/multivm-reth.sock" # IPC socket
```

## Communication Protocols

### 1. JSON-RPC Interface

Standard Ethereum JSON-RPC for transaction submission and queries:

```bash
# Get latest block number
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545

# Send transaction
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_sendRawTransaction","params":["0x..."],"id":1}' \
  http://localhost:8545
```

### 2. Engine API with JWT Authentication

Used by MultiVM consensus for block production:

```bash
# Generate JWT token (example)
JWT_SECRET=$(cat ./reth-data/jwt.hex)
JWT_TOKEN=$(./scripts/generate-jwt-token.sh "$JWT_SECRET")

# Call Engine API
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[],"id":1}' \
  http://localhost:8551
```

### 3. IPC Communication

Secure inter-process communication between MultiVM and Reth:

```rust
// Example IPC client usage
use reth_execution_engine::{IpcClient, JwtCredentials};

let jwt_creds = JwtCredentials::from_secret_file("./reth-data/jwt.hex")?;
let ipc_client = IpcClient::new_with_jwt("/tmp/multivm-reth.sock", jwt_creds).await?;

// Send authenticated message
let response = ipc_client.call_authenticated(
    "submit_transaction",
    json!({"tx_data": "0x..."}),
).await?;
```

## Architecture Integration

### MultiVM ↔ Reth Communication Flow

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
│                             │                               │
│                   ┌─────────┼─────────┐                     │
│                   │ JSON-RPC│Engine API│IPC                 │
│                   │ :8545   │:8551 JWT │Socket              │
└─────────────────────────────┼─────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Reth Process                             │
│  ┌─────────────────────────────────────────────────────────┐│
│  │                  Reth Node                              ││
│  │  ┌─────────────┬─────────────┬─────────────────────────┐││
│  │  │  RPC Server │ Engine API  │    EVM Execution        │││
│  │  │    :8545    │   :8551     │       Engine            │││
│  │  └─────────────┴─────────────┴─────────────────────────┘││
│  └─────────────────────────────────────────────────────────┘│
│  ┌─────────────────────────────────────────────────────────┐│
│  │                Ethereum State                           ││
│  │                 (RocksDB)                               ││
│  └─────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Key Integration Points

1. **Transaction Submission**:
   - MultiVM receives transactions via REST/GraphQL/WebSocket
   - EVM transactions are forwarded to Reth via JSON-RPC
   - Transaction receipts are collected and returned to MultiVM

2. **Block Production**:
   - MultiVM consensus determines when to create blocks
   - Engine API calls coordinate block creation with Reth
   - Block headers and state roots are synchronized

3. **State Queries**:
   - Account balances and state queries routed to Reth
   - Smart contract interactions executed on Reth
   - Results returned through MultiVM APIs

## Security Configuration

### JWT Authentication

JWT tokens secure Engine API communication:

```bash
# JWT secret is automatically generated during setup
cat ./reth-data/jwt.hex
# Example: 1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef

# Tokens expire after 5 minutes by default
export JWT_EXPIRY_SECONDS=300
```

### Network Security

```toml
# In configs/reth-multivm.toml
[security]
enable_admin_api = false
allow_unsafe_methods = false
enable_personal_api = false

[security.rate_limiting]
enabled = true
requests_per_minute = 1000
burst_size = 100
```

### Firewall Configuration

```bash
# Allow only necessary ports
sudo ufw allow 8545/tcp  # RPC (internal only)
sudo ufw allow 8551/tcp  # Engine API (internal only)
sudo ufw deny 30303/tcp  # P2P (disabled in coordination mode)
```

## Development Mode

For development and testing:

```bash
# Setup development mode
./scripts/setup-reth-node.sh setup --dev --mining

# This configures:
# - Pre-funded accounts with test ETH
# - Fast block times
# - Mining enabled
# - Admin APIs enabled
# - Unsafe methods allowed
```

### Pre-funded Development Accounts

```javascript
// Account 1 (100 ETH)
Private Key: 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80
Address: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266

// Account 2 (100 ETH)  
Private Key: 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d
Address: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8

// Account 3 (100 ETH)
Private Key: 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a
Address: 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC
```

## Production Deployment

### System Service Setup

```bash
# Setup as systemd service (requires root)
sudo ./scripts/setup-reth-node.sh setup --env production

# Enable and start service
sudo systemctl enable multivm-reth
sudo systemctl start multivm-reth

# Monitor service
sudo systemctl status multivm-reth
sudo journalctl -u multivm-reth -f
```

### Production Configuration

```toml
# configs/reth-multivm.toml - Production overrides
[environments.production]
[environments.production.reth]
dev_mode = false
chain = "mainnet"  # or "sepolia" for testnet
log_level = "warn"

[environments.production.security]
enable_admin_api = false
allow_unsafe_methods = false

[environments.production.rpc]
cors_origins = ["https://multivm.example.com"]
max_connections = 1000
```

### Resource Requirements

| Environment | RAM | Storage | CPU | Network |
|-------------|-----|---------|-----|---------|
| Development | 4GB | 100GB | 2 cores | 10 Mbps |
| Staging | 8GB | 500GB | 4 cores | 50 Mbps |
| Production | 16GB | 2TB | 8 cores | 100 Mbps |

## Monitoring and Maintenance

### Health Checks

```bash
# Automated health monitoring
./scripts/setup-reth-node.sh status

# Manual health checks
curl -f http://localhost:8545/health || echo "RPC unhealthy"
curl -f http://localhost:8551/health || echo "Engine API unhealthy"
```

### Log Management

```bash
# View real-time logs
./scripts/setup-reth-node.sh logs

# Structured log analysis
grep "ERROR" ./reth-data/logs/reth.log
grep "engine_" ./reth-data/logs/reth.log | tail -20
```

### Performance Monitoring

```bash
# Resource usage
ps aux | grep reth
df -h ./reth-data
netstat -tulpn | grep -E ":8545|:8551"

# Metrics endpoint (if enabled)
curl http://localhost:9090/metrics
```

## Troubleshooting

### Common Issues

1. **Port Conflicts**:
   ```bash
   # Check what's using the ports
   lsof -i :8545
   lsof -i :8551
   
   # Use different ports
   ./scripts/setup-reth-node.sh setup --rpc-port 8546 --engine-port 8552
   ```

2. **JWT Authentication Failures**:
   ```bash
   # Regenerate JWT secret
   rm ./reth-data/jwt.hex
   ./scripts/setup-reth-node.sh setup
   
   # Check token validity
   openssl rand -hex 32 > ./reth-data/jwt.hex
   ```

3. **Database Corruption**:
   ```bash
   # Clean and reinitialize
   ./scripts/setup-reth-node.sh clean
   ./scripts/setup-reth-node.sh setup
   ```

4. **High Memory Usage**:
   ```toml
   # Reduce cache sizes in config
   [performance.memory]
   state_cache_size_mb = 256
   transaction_cache_size = 5000
   block_cache_size = 500
   ```

### Debug Mode

```bash
# Enable debug logging
export RUST_LOG="reth=debug,reth_execution_engine=debug"
./scripts/setup-reth-node.sh restart

# Check debug logs
tail -f ./reth-data/logs/reth.log | grep DEBUG
```

## Integration Testing

### Test Suite

```bash
# Run Reth integration tests
cd reth-execution-engine
cargo test --features real-node

# Run specific test
cargo test test_real_engine_integration --features real-node

# Run with logging
RUST_LOG=debug cargo test --features real-node
```

### Manual Testing

```bash
# 1. Start Reth
./scripts/setup-reth-node.sh start

# 2. Test RPC
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_chainId","params":[],"id":1}' \
  http://localhost:8545

# 3. Test Engine API
JWT_TOKEN=$(./scripts/generate-jwt-token.sh $(cat ./reth-data/jwt.hex))
curl -X POST \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $JWT_TOKEN" \
  -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[],"id":1}' \
  http://localhost:8551

# 4. Submit test transaction
curl -X POST -H "Content-Type: application/json" \
  -d '{
    "jsonrpc":"2.0",
    "method":"eth_sendTransaction",
    "params":[{
      "from":"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
      "to":"0x70997970C51812dc3A010C7d01b50e0d17dc79C8",
      "value":"0x1000000000000000000"
    }],
    "id":1
  }' \
  http://localhost:8545
```

## Advanced Configuration

### Custom Network Configuration

```toml
# For custom networks
[reth]
chain = "custom"
genesis_file = "./configs/genesis.json"
network_id = 12345
chain_id = 12345

[rpc]
# Enable additional APIs for development
api_modules = ["eth", "net", "web3", "debug", "trace", "admin", "personal", "miner"]
```

### Performance Tuning

```toml
[performance]
# Increase workers for high throughput
transaction_pool_workers = 8
block_processing_workers = 4
network_workers = 16

# Optimize caches
[performance.memory]
transaction_cache_size = 20000
block_cache_size = 2000
state_cache_size_mb = 1024
```

### Database Optimization

```toml
[reth.database]
# Tune RocksDB for SSD
cache_size_mb = 2048
max_open_files = 20000
compression = "lz4"
enable_statistics = true
```

## Support and Resources

- **MultiVM Documentation**: [../README.md](../README.md)
- **Reth Documentation**: [https://reth.rs](https://reth.rs)
- **Configuration Reference**: [./configs/reth-multivm.toml](../configs/reth-multivm.toml)
- **Script Reference**: [./scripts/setup-reth-node.sh](../scripts/setup-reth-node.sh)

For issues or questions:
1. Check the troubleshooting section above
2. Review logs in `./reth-data/logs/reth.log`
3. Open an issue in the MultiVM repository
4. Consult the Reth community resources