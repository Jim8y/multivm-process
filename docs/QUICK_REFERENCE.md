# MultiVM Quick Reference

## Project Structure

```
multivm-process/
├── multivm-consensus/        # BFT consensus implementation
├── multivm-p2p/             # P2P networking
├── multivm-process-manager/  # Process lifecycle management
├── multivm-account-mapping/  # Cross-chain accounts
├── multivm-common/          # Shared types
├── multivm-application/     # API server
├── multivm-cli/            # Command-line interface
├── multivm-rpc-service/    # RPC proxy
├── reth-execution-engine/   # Ethereum execution
├── solana-execution-engine/ # Solana execution
├── configs/                # Configuration examples
├── scripts/                # Utility scripts
├── docs/                   # Documentation
└── tests/                  # Integration tests
```

## Common Commands

### Building
```bash
# Build everything
cargo build --release

# Build specific component
cargo build -p multivm-consensus

# Build without default features
cargo build --no-default-features
```

### Testing
```bash
# Run all tests
cargo test

# Run specific component tests
cargo test -p multivm-consensus

# Run with output
cargo test -- --nocapture
```

### Running
```bash
# Quick start (3-node testnet)
./scripts/quickstart.sh

# Start single validator
./target/release/multivm-node --config config.toml --validator validator.json

# Start with custom ports
./target/release/multivm-node --api-port 8090 --p2p-port 9090
```

## Configuration

### Minimal Config
```toml
[system]
chain_id = 1337
data_dir = "./data"

[consensus]
type = "malachite"
block_time_ms = 1000

[blockchain.ethereum]
enabled = true
rpc_url = "http://localhost:8545"
```

### Key Configuration Files
- `configs/example.toml` - Full configuration reference
- `configs/production.toml` - Production-ready config
- `configs/testnet.toml` - Testnet configuration

## API Endpoints

### REST API
- `GET /health` - Health check
- `GET /api/v1/status` - Chain status
- `GET /api/v1/blocks/latest` - Latest block
- `POST /api/v1/transactions` - Submit transaction

### JSON-RPC
- `eth_blockNumber` - Current block number
- `eth_sendRawTransaction` - Submit transaction
- `eth_getBalance` - Get account balance
- `eth_call` - Execute call

### WebSocket
- `ws://localhost:8082` - Real-time updates
- Subscribe to: `blocks`, `transactions`, `logs`

## Environment Variables

```bash
export MULTIVM_HOME=/path/to/multivm
export MULTIVM_DATA=/var/lib/multivm
export MULTIVM_LOG_LEVEL=info
export RUST_LOG=multivm=debug
```

## Key Scripts

### Development
- `quickstart.sh` - One-command setup
- `start-testnet.sh` - Start testnet
- `dashboard.sh` - Monitoring dashboard
- `test-suite.sh` - Run tests

### Administration
- `validator-admin.sh` - Manage validators
- `check-health.sh` - Health checks
- `monitor-testnet.sh` - Network monitoring

### Deployment
- `deploy-production.sh` - Production deployment
- `setup-validators.sh` - Configure validators

## Docker Commands

```bash
# Build image
docker build -t multivm:latest .

# Run container
docker run -p 8080:8080 -p 9000:9000 multivm:latest

# Docker Compose
docker-compose up -d
docker-compose logs -f
docker-compose down
```

## Debugging

### Enable Debug Logging
```bash
RUST_LOG=debug ./target/release/multivm-node
```

### Check Logs
```bash
# Application logs
tail -f /tmp/multivm/logs/multivm.log

# Consensus logs
grep consensus /tmp/multivm/logs/multivm.log

# Error logs
grep ERROR /tmp/multivm/logs/multivm.log
```

### Common Issues

1. **Port in use**
   ```bash
   lsof -i :8080
   kill -9 <PID>
   ```

2. **Build fails**
   ```bash
   cargo clean
   cargo update
   ```

3. **Node not syncing**
   - Check P2P connectivity
   - Verify bootstrap nodes
   - Check system time

## Performance Tuning

### System Settings
```bash
# Increase file descriptors
ulimit -n 65535

# Increase memory limits
ulimit -m unlimited
```

### Configuration Tuning
```toml
[system.resource_limits]
max_memory_mb = 32768
max_file_descriptors = 1048576

[storage]
cache_size_mb = 4096

[p2p]
max_connections = 1000
```

## Security Checklist

- [ ] Generate unique validator keys
- [ ] Set strong JWT secret
- [ ] Enable TLS for production
- [ ] Configure firewall rules
- [ ] Set resource limits
- [ ] Enable rate limiting
- [ ] Regular backups
- [ ] Monitor logs

## Useful Links

- [Architecture](./ARCHITECTURE.md)
- [Getting Started](./GETTING_STARTED.md)
- [API Reference](./API.md)
- [Troubleshooting](./TROUBLESHOOTING.md)