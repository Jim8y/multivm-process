# Docker Testing Guide for MultiVM Network

This guide shows how to run and test both single-node and multi-node MultiVM networks using Docker.

## Prerequisites

- Docker and Docker Compose installed
- At least 4GB of free RAM
- Ports 8080-8082, 26656-26658 available

## Quick Start

### 1. Test Everything Automatically

```bash
# Run both single and multi-node tests
./test-docker-network.sh

# Or test individually:
./test-docker-network.sh single   # Single node only
./test-docker-network.sh multi    # Multi-node only
```

### 2. Monitor Network in Real-Time

In a separate terminal, run:

```bash
./monitor-network.sh
```

This shows a live dashboard with:
- Node health status
- Block heights
- Container resource usage
- Recent log activity

## Manual Testing

### Single Node Network

```bash
# Start single node
docker-compose -f docker-compose.single.yml up -d

# View logs
docker-compose -f docker-compose.single.yml logs -f

# Check health
curl http://localhost:8080/health

# Stop
docker-compose -f docker-compose.single.yml down -v
```

### Multi-Node Network

```bash
# Start 3-node network
docker-compose -f docker-compose.multi.yml up -d

# View status monitor
docker logs -f multivm-status-monitor

# View aggregated logs
docker logs -f multivm-log-aggregator

# Check each node
curl http://localhost:8080/health  # Node 1
curl http://localhost:8081/health  # Node 2
curl http://localhost:8082/health  # Node 3

# Stop
docker-compose -f docker-compose.multi.yml down -v
```

## Network Configuration

### Single Node
- **Node ID**: single-node
- **Type**: Bootstrap validator
- **Block Generation**: Enabled (2-second intervals)
- **Transactions**: 3 SVM + 3 EVM per block
- **Ports**: 8080 (API), 26656 (P2P), 8545 (EVM), 8899 (SVM)

### Multi-Node Network
- **Node 1**: Bootstrap validator with block generation
- **Node 2**: Validator (no block generation)
- **Node 3**: Validator (no block generation)
- **Consensus**: Malachite BFT
- **Network**: 172.20.0.0/16

## Logging

All nodes are configured with enhanced logging:
- **Log Level**: DEBUG for single node, INFO for multi-node
- **Rust Log**: Trace logging for multivm modules
- **Log Location**: `./logs/` directory
- **Real-time Monitoring**: Built-in log aggregators

## Verification Steps

1. **Health Check**: All nodes should respond to `/health` endpoint
2. **Block Generation**: Node 1 should generate blocks every 2 seconds
3. **Consensus**: All validators should agree on block height
4. **P2P Network**: Nodes should discover and connect to peers
5. **Resource Usage**: Monitor CPU and memory usage

## Troubleshooting

### Containers won't start
```bash
# Check for port conflicts
sudo lsof -i :8080
sudo lsof -i :26656

# Clean up everything
docker-compose -f docker-compose.multi.yml down -v
docker system prune -f
```

### Nodes not healthy
```bash
# Check individual node logs
docker logs multivm-node-1
docker logs multivm-node-2
docker logs multivm-node-3

# Check detailed health
docker exec multivm-node-1 curl localhost:8080/health
```

### No blocks being generated
- Verify `BLOCK_GENERATION_ENABLED=true` on Node 1
- Check coordinator logs: `docker logs multivm-node-1 | grep -i block`
- Ensure consensus is working between nodes

## Expected Output

### Successful Single Node:
```
✓ Node is healthy!
✓ Health endpoint working
[INFO] Starting MultiVM Node...
[INFO] Block generator started successfully
[INFO] Generated mock block 1 with 3 SVM and 3 EVM transactions
```

### Successful Multi-Node:
```
✓ All 3 nodes are healthy!
Node 1: ✅ Healthy | Block Height: 150
Node 2: ✅ Healthy | Block Height: 150  
Node 3: ✅ Healthy | Block Height: 150
⚡ Block Generation: Active (Node 1)
```

## Clean Up

```bash
# Remove all containers and volumes
docker-compose -f docker-compose.single.yml down -v
docker-compose -f docker-compose.multi.yml down -v

# Clean up logs
rm -rf ./logs/*
```