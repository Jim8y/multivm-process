# Getting Started with MultiVM

This guide will help you get MultiVM up and running on your local machine.

## Prerequisites

Before you begin, ensure you have the following installed:

- **Rust** 1.75 or higher
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **Git**
  ```bash
  # Ubuntu/Debian
  sudo apt-get install git
  
  # macOS
  brew install git
  ```

- **Build essentials**
  ```bash
  # Ubuntu/Debian
  sudo apt-get install build-essential pkg-config libssl-dev
  
  # macOS
  xcode-select --install
  ```

- **Docker** (optional, for containerized deployment)
  - [Docker Desktop](https://www.docker.com/products/docker-desktop/)

## Installation

### 1. Clone the Repository

```bash
git clone https://github.com/your-org/multivm-process
cd multivm-process
```

### 2. Build the Project

```bash
# Build in release mode (recommended)
cargo build --release

# Or build in debug mode (faster compilation, slower runtime)
cargo build
```

### 3. Run Tests

```bash
# Run all tests
cargo test

# Run tests for a specific component
cargo test -p multivm-consensus
```

## Quick Start

### Option 1: Using the Quickstart Script (Recommended)

```bash
cd scripts
./quickstart.sh
```

This will:
- ✅ Check all dependencies
- ✅ Build the project
- ✅ Generate validator keys
- ✅ Start a 3-node testnet
- ✅ Open the monitoring dashboard

### Option 2: Manual Setup

#### Step 1: Generate Validator Keys

```bash
# Generate keys for 3 validators
./target/release/multivm-cli validator generate --count 3 --output validators/
```

#### Step 2: Create Configuration

Create a `testnet.toml` file:

```toml
[system]
chain_id = 1337
data_dir = "./data"
log_level = "info"

[consensus]
type = "malachite"
block_time_ms = 1000
view_timeout_ms = 5000
validator_threshold = 0.67

[blockchain.ethereum]
enabled = true
chain_id = 1
rpc_url = "http://localhost:8545"

[blockchain.solana]
enabled = false
rpc_url = "http://localhost:8899"

[p2p]
listen_addresses = ["/ip4/0.0.0.0/tcp/9000"]
enable_mdns = true

[api]
rest_port = 8080
graphql_port = 8081
ws_port = 8082
```

#### Step 3: Start the Network

```bash
# Start validator 1
./target/release/multivm-node \
  --config testnet.toml \
  --validator validators/validator_1.json \
  --api-port 8080

# Start validator 2 (in another terminal)
./target/release/multivm-node \
  --config testnet.toml \
  --validator validators/validator_2.json \
  --api-port 8090

# Start validator 3 (in another terminal)
./target/release/multivm-node \
  --config testnet.toml \
  --validator validators/validator_3.json \
  --api-port 8100
```

## Interacting with MultiVM

### Check Node Health

```bash
curl http://localhost:8080/health
```

Expected response:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "chain_id": 1337,
  "block_height": 42
}
```

### Get Blockchain Status

```bash
curl http://localhost:8080/api/v1/status
```

### Submit a Transaction

#### EVM Transaction
```bash
# Using eth_sendRawTransaction
curl -X POST http://localhost:8080/rpc \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "eth_sendRawTransaction",
    "params": ["0x...signed_transaction_hex..."],
    "id": 1
  }'
```

#### Using MultiVM API
```bash
curl -X POST http://localhost:8080/api/v1/transactions \
  -H "Content-Type: application/json" \
  -d '{
    "from": "0x...",
    "to": "0x...",
    "value": "1000000000000000000",
    "data": "0x",
    "type": "evm"
  }'
```

### Query Blocks

```bash
# Get latest block
curl http://localhost:8080/api/v1/blocks/latest

# Get specific block
curl http://localhost:8080/api/v1/blocks/100
```

### WebSocket Subscription

```javascript
// Connect to WebSocket
const ws = new WebSocket('ws://localhost:8082');

// Subscribe to new blocks
ws.send(JSON.stringify({
  type: 'subscribe',
  channel: 'blocks'
}));

// Handle messages
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('New block:', data);
};
```

## Using Docker

### Build Docker Images

```bash
# Build MultiVM image
docker build -t multivm:latest .

# Build with specific features
docker build --build-arg FEATURES="ethereum,monitoring" -t multivm:latest .
```

### Run with Docker Compose

```bash
# Start a 3-node testnet
docker-compose up

# Run in background
docker-compose up -d

# View logs
docker-compose logs -f

# Stop the network
docker-compose down
```

## Configuration Options

### Environment Variables

- `MULTIVM_HOME` - Base directory for MultiVM data
- `MULTIVM_CONFIG` - Path to configuration file
- `MULTIVM_LOG_LEVEL` - Logging level (trace/debug/info/warn/error)
- `RUST_LOG` - Rust logging configuration

### Command Line Options

```bash
multivm-node --help

Options:
  --config <FILE>        Configuration file path
  --validator <FILE>     Validator key file
  --data-dir <DIR>       Data directory
  --api-port <PORT>      API server port
  --p2p-port <PORT>      P2P network port
  --log-level <LEVEL>    Logging level
  --enable-metrics       Enable Prometheus metrics
```

## Monitoring

### Prometheus Metrics

Metrics are exposed at `http://localhost:9090/metrics`:

```bash
# Check metrics
curl http://localhost:9090/metrics | grep multivm
```

Key metrics:
- `multivm_block_height` - Current blockchain height
- `multivm_consensus_round` - Current consensus round
- `multivm_peers_connected` - Number of connected peers
- `multivm_transactions_processed` - Total transactions processed

### Dashboard

Use the built-in dashboard:

```bash
cd scripts
./dashboard.sh
```

## Troubleshooting

### Common Issues

#### Port Already in Use
```bash
# Find process using port
lsof -i :8080

# Kill process
kill -9 <PID>
```

#### Build Failures
```bash
# Clean build cache
cargo clean

# Update dependencies
cargo update

# Rebuild
cargo build --release
```

#### Node Not Syncing
- Check network connectivity
- Verify P2P ports are open
- Ensure system time is synchronized

### Debug Mode

Run with debug logging:
```bash
RUST_LOG=debug ./target/release/multivm-node --config testnet.toml
```

### Getting Help

- Check the [documentation](../README.md)
- Review [common issues](./TROUBLESHOOTING.md)
- Join our [Discord community](https://discord.gg/multivm)
- Submit an [issue on GitHub](https://github.com/your-org/multivm-process/issues)

## Next Steps

- 📚 Read the [Architecture Overview](./ARCHITECTURE.md)
- 🔧 Learn about [Configuration](./CONFIGURATION.md)
- 🚀 Deploy to [Production](./DEPLOYMENT.md)
- 🤝 [Contribute](../CONTRIBUTING.md) to the project