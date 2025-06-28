# MultiVM Scripts

This directory contains utility scripts for running, testing, and managing the MultiVM blockchain.

## Available Scripts

### 🚀 `run-testnet.sh`

Starts a single-node MultiVM testnet with all services enabled.

```bash
./run-testnet.sh
```

Features:
- Automatically cleans up existing processes on required ports
- Creates necessary directories and configuration
- Displays colored output for easy monitoring
- Runs in foreground (press Ctrl+C to stop)

### 📊 `dashboard.sh`

Real-time monitoring dashboard for the MultiVM blockchain.

```bash
./dashboard.sh
```

Displays:
- Current block height
- Transaction pool status
- Recent blocks with timestamps
- Processing statistics
- Updates every 2 seconds

### 🧪 `test-suite.sh`

Comprehensive testing utilities for the MultiVM blockchain.

```bash
# Show help
./test-suite.sh --help

# Run all tests
./test-suite.sh all

# Individual commands
./test-suite.sh health      # Check service health
./test-suite.sh api         # Test REST API endpoints
./test-suite.sh graphql     # Test GraphQL API
./test-suite.sh submit-tx   # Submit test transactions
./test-suite.sh monitor     # Monitor blockchain activity
./test-suite.sh stress      # Run stress test (100 transactions)
```

### 🔧 `run-solo-testnet.sh`

Alternative script for running a solo testnet (legacy).

```bash
./run-solo-testnet.sh
```

## Port Configuration

The scripts use the following default ports:

| Service | Port | Description |
|---------|------|-------------|
| REST API | 8080 | Main HTTP API |
| GraphQL | 8081 | GraphQL endpoint |
| WebSocket | 8082 | Real-time updates |
| Admin | 8083 | Admin interface |
| Health | 8090 | Health checks |
| Metrics | 9090 | Prometheus metrics |

## Requirements

- Bash 4.0+
- `jq` for JSON parsing
- `curl` for API requests
- `lsof` for port management (Linux/macOS)

## Tips

1. **First Time Setup**: Run `cargo build --release` before using any scripts

2. **Port Conflicts**: The scripts automatically clean up processes on required ports

3. **Logs**: Check `/tmp/multivm-testnet/logs/` for detailed logs

4. **Configuration**: Testnet config is at `/tmp/multivm-testnet/config/testnet.toml`

5. **Cleanup**: To fully reset, run:
   ```bash
   rm -rf /tmp/multivm-testnet
   ```