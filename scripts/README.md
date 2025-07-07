# MultiVM Scripts

This directory contains utility scripts for running, testing, and managing the MultiVM blockchain.

## Available Scripts

### Single Node Testing

#### 🚀 `run-testnet.sh`

Starts a single-node MultiVM testnet with all services enabled.

```bash
./run-testnet.sh
```

Features:
- Automatically cleans up existing processes on required ports
- Creates necessary directories and configuration
- Displays colored output for easy monitoring
- Runs in foreground (press Ctrl+C to stop)

#### 📊 `dashboard.sh`

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

#### 🧪 `test-suite.sh`

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

#### 🔧 `run-solo-testnet.sh`

Alternative script for running a solo testnet (legacy).

```bash
./run-solo-testnet.sh
```

### Multi-Validator Consensus Testing

#### 🏗️ `setup-validators.sh`

**NEW!** Sets up a multi-validator network for testing consensus features.

```bash
# Setup 4 validators (default)
./setup-validators.sh

# Setup custom configuration
./setup-validators.sh [VALIDATOR_COUNT] [BASE_PORT] [VOTING_POWER]

# Examples
./setup-validators.sh 7 9000 100    # 7 validators, port 9000+, 100 voting power each
./setup-validators.sh 4              # 4 validators with defaults
```

Features:
- ✅ Round-robin leader selection setup
- ✅ BFT consensus configuration (2/3 + 1 voting threshold)
- ✅ View change mechanism
- ✅ Automatic validator discovery and networking
- ✅ Individual validator management scripts
- ✅ Master control scripts (start-all, stop-all, monitor)
- ✅ Built-in consensus testing

#### 🧪 `test-consensus-features.sh`

**NEW!** Comprehensive testing suite for consensus features.

```bash
# Run all consensus tests
./test-consensus-features.sh

# Run specific test categories
./test-consensus-features.sh -t unit           # Unit tests only
./test-consensus-features.sh -t integration    # Integration tests
./test-consensus-features.sh -t leader         # Leader selection tests
./test-consensus-features.sh -t bft            # BFT consensus tests
./test-consensus-features.sh -t view-change    # View change tests
./test-consensus-features.sh -t fault-tolerance # Fault tolerance tests
./test-consensus-features.sh -t live           # Live network tests

# Custom validator count
./test-consensus-features.sh -c 7              # Test with 7 validators
```

Test Categories:
- **Unit Tests**: Leader selection, validator set, view change modules
- **Integration Tests**: Multi-validator consensus scenarios
- **Leader Selection**: Round-robin determinism and rotation
- **BFT Consensus**: Byzantine fault tolerance, voting thresholds
- **View Change**: Leader failover and recovery
- **Fault Tolerance**: Network partitions, validator failures
- **Live Network**: Real multi-validator network testing
- **Performance**: Benchmarks and stress testing

#### 🔧 `validator-admin.sh`

**NEW!** Advanced validator network administration and monitoring.

```bash
# Network status and monitoring
./validator-admin.sh status          # Show validator network status
./validator-admin.sh monitor         # Real-time monitoring dashboard
./validator-admin.sh health          # Comprehensive health check

# Leader management
./validator-admin.sh leader          # Show current leader information
./validator-admin.sh rotate          # Trigger view change (leader rotation)

# Transaction testing
./validator-admin.sh submit-tx       # Submit test transaction
./validator-admin.sh stress-test 100 # Run stress test with 100 transactions

# Validator management
./validator-admin.sh logs validator_01      # Show logs for specific validator
./validator-admin.sh config validator_01   # Show configuration
./validator-admin.sh benchmark              # Run performance benchmarks
./validator-admin.sh cleanup               # Stop all validators and cleanup
```

Features:
- 📊 Real-time network monitoring
- 🏥 Health checks with BFT analysis
- 👑 Leader selection verification
- 🔄 Manual view change triggering
- ⚡ Stress testing and benchmarks
- 📋 Individual validator management
- 🧹 Complete network cleanup

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