# MultiVM Testnet with Reth Integration Guide

This guide explains how to run a 7-node MultiVM testnet with Reth as the Ethereum execution engine.

## Overview

The testnet includes:
- **1 Reth node**: Ethereum execution engine with Engine API
- **7 MultiVM nodes**: 5 validators and 2 full nodes
- **Transaction generator**: Sends mock EVM, SVM, and cross-VM transactions
- **Monitoring stack**: Prometheus and Grafana for metrics

## Prerequisites

- Docker and Docker Compose installed
- At least 8GB of RAM available
- Ports 8080-8086, 8545, 8551, 9090, and 3000 available

## Quick Start

1. **Start the testnet**:
   ```bash
   ./scripts/start-testnet.sh
   ```

   This script will:
   - Stop any existing testnet
   - Generate JWT secret for secure Engine API communication
   - Build necessary Docker images
   - Start all services
   - Wait for services to be healthy
   - Verify block production

2. **Verify the testnet**:
   ```bash
   ./scripts/verify-testnet.sh
   ```

   For a quick check:
   ```bash
   ./scripts/verify-testnet.sh --quick
   ```

3. **Monitor the testnet**:
   ```bash
   ./scripts/monitor-testnet.sh
   ```

   This provides a real-time dashboard showing:
   - Reth block height and peers
   - MultiVM node status and consensus
   - Transaction generation statistics
   - System resource usage

## Services and Endpoints

### Reth
- **RPC**: http://localhost:8545
- **Engine API**: http://localhost:8551 (JWT authenticated)
- **IPC**: /tmp/reth.ipc (inside container)

### MultiVM Nodes
- **Node 1** (Validator): http://localhost:8080
- **Node 2** (Validator): http://localhost:8081
- **Node 3** (Validator): http://localhost:8082
- **Node 4** (Validator): http://localhost:8083
- **Node 5** (Validator): http://localhost:8084
- **Node 6** (Full Node): http://localhost:8085
- **Node 7** (Full Node): http://localhost:8086

### Monitoring
- **Prometheus**: http://localhost:9090
- **Grafana**: http://localhost:3000 (login: admin/admin)

## Transaction Generator

The transaction generator continuously sends three types of transactions:
1. **EVM transactions**: Simple ETH transfers between funded accounts
2. **SVM transactions**: Mock Solana VM transactions
3. **Cross-VM transactions**: Simulated cross-chain operations

Configure the generator with environment variables:
- `TX_RATE`: Transactions per second (default: 10)
- `TX_TYPE`: Type of transactions to send: `evm`, `svm`, `cross-vm`, or `mixed` (default)

## Funded Test Accounts

The testnet includes 5 pre-funded accounts with 100 ETH each:

| Account | Address | Private Key |
|---------|---------|-------------|
| Account 1 | 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 | 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 |
| Account 2 | 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 | 0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d |
| Account 3 | 0x3C44CdDdB6a900fa2b585dd299e03d12FA4293BC | 0x5de4111afa1a4b94908f83103eb1f1706367c2e68ca870fc3fb9a804cdab365a |
| Account 4 | 0x23618e81E3f5cdF7f54C3d65f7FBc0aBf5B21E8f | 0xdbda1821b80551c9d65939329250298aa3472ba22feea921c0cf5d620ea67b97 |
| Account 5 | 0xa0Ee7A142d267C1f36714E4a8F75612F20a79720 | 0x2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6 |

## Common Operations

### View logs
```bash
# All services
docker-compose -f docker-compose.testnet-reth.yml logs -f

# Specific service
docker-compose -f docker-compose.testnet-reth.yml logs -f reth
docker-compose -f docker-compose.testnet-reth.yml logs -f multivm-node-1
docker-compose -f docker-compose.testnet-reth.yml logs -f tx-generator
```

### Stop the testnet
```bash
docker-compose -f docker-compose.testnet-reth.yml down
```

### Reset and restart
```bash
docker-compose -f docker-compose.testnet-reth.yml down -v
./scripts/start-testnet.sh
```

### Check specific node status
```bash
# MultiVM node info
curl http://localhost:8080/api/v1/node/info | jq

# Consensus status
curl http://localhost:8080/api/v1/consensus/status | jq

# Reth block number
curl -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
  http://localhost:8545 | jq
```

## Architecture

```
┌─────────────────┐     ┌─────────────────┐
│ Transaction Gen │────>│ MultiVM Nodes   │
└─────────────────┘     │   (1-7)         │
                        └────────┬────────┘
                                 │
                                 v
                        ┌─────────────────┐
                        │   Reth Node     │
                        │                 │
                        │ - Engine API    │
                        │ - JSON-RPC      │
                        └─────────────────┘
```

The MultiVM nodes:
1. Receive transactions from the generator
2. Reach consensus using Malachite BFT
3. Send EVM blocks to Reth via Engine API
4. Reth processes and executes the blocks

## Troubleshooting

### Services not starting
Check Docker logs:
```bash
docker-compose -f docker-compose.testnet-reth.yml logs [service-name]
```

### No blocks being produced
1. Check if all validators are running:
   ```bash
   ./scripts/verify-testnet.sh
   ```

2. Verify consensus is working:
   ```bash
   curl http://localhost:8080/api/v1/consensus/status | jq
   ```

### Reth connection issues
1. Check JWT authentication:
   ```bash
   ls -la testnet/configs/jwt.hex
   ```

2. Verify Engine API is accessible:
   ```bash
   JWT_TOKEN=$(./scripts/generate-jwt-token.sh testnet/configs/jwt.hex --quiet)
   curl -H "Authorization: Bearer $JWT_TOKEN" \
     -X POST -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[["engine_newPayloadV3"]],"id":1}' \
     http://localhost:8551
   ```

### High resource usage
Adjust resource limits in `docker-compose.testnet-reth.yml`:
```yaml
services:
  reth:
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 4G
```

## Development

### Adding more nodes
Edit `docker-compose.testnet-reth.yml` and add new node configurations. Remember to:
1. Update port mappings
2. Add to funded nodes list if validator
3. Update transaction generator node list

### Custom transaction types
Modify `tx-generator/src/main.rs` to add new transaction types or patterns.

### Monitoring dashboards
Import custom Grafana dashboards from `testnet/configs/grafana/dashboards/`.