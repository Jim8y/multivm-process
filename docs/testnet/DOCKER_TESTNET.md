# Docker Testnet Guide

This guide explains how to run a 7-node MultiVM testnet using Docker and Docker Compose.

## Prerequisites

- Docker Engine 20.10+
- Docker Compose v2.0+
- At least 8GB RAM available
- 10GB free disk space

## Quick Start

```bash
# 1. Build and start the testnet
./scripts/docker-testnet.sh start

# 2. Check node status
./scripts/docker-testnet.sh status

# 3. Follow logs
./scripts/docker-testnet.sh logs
```

## Testnet Architecture

The testnet consists of:
- **5 Validator Nodes** (node1-node5): Participate in consensus and validate blocks
- **2 Full Nodes** (node6-node7): Sync the blockchain but don't validate
- **Monitoring Stack**: Prometheus and Grafana for metrics

### Network Topology

```
┌─────────────────────────────────────────────────────────────┐
│                    MultiVM Testnet                          │
├─────────────────────────────────────────────────────────────┤
│  Node 1 (Genesis)  ←→  Node 2  ←→  Node 3                  │
│       ↕                  ↕           ↕                      │
│    Node 4         ←→   Node 5   ←   Node 6 (Full)          │
│       ↕                              ↕                      │
│    Node 7 (Full)  ←─────────────────┘                      │
└─────────────────────────────────────────────────────────────┘
```

## Node Configuration

### Validator Nodes (1-5)
- Participate in consensus
- Propose and validate blocks
- Stake tokens (in production)

### Full Nodes (6-7)
- Sync blockchain state
- Serve API requests
- Don't participate in consensus

## Accessing the Nodes

### REST API Endpoints
- Node 1: http://localhost:8080
- Node 2: http://localhost:8180
- Node 3: http://localhost:8280
- Node 4: http://localhost:8380
- Node 5: http://localhost:8480
- Node 6: http://localhost:8580
- Node 7: http://localhost:8680

### WebSocket Endpoints
- Node 1: ws://localhost:8082
- Node 2: ws://localhost:8182
- (Pattern continues for other nodes)

### Admin Endpoints
- Node 1: http://localhost:8083
- Node 2: http://localhost:8183
- (Pattern continues for other nodes)

### P2P Ports
- Node 1: 26656
- Node 2: 26756
- Node 3: 26856
- Node 4: 26956
- Node 5: 27056
- Node 6: 27156
- Node 7: 27256

## Monitoring

### Prometheus
- URL: http://localhost:9090
- Collects metrics from all nodes
- Query examples:
  ```
  # Connected peers per node
  multivm_p2p_connected_peers

  # Block height
  multivm_consensus_height

  # Transaction pool size
  multivm_tx_pool_size
  ```

### Grafana
- URL: http://localhost:3000
- Default credentials: admin/admin
- Pre-configured dashboards for:
  - Node health overview
  - P2P network metrics
  - Consensus performance
  - Transaction throughput

## Common Operations

### Start Specific Nodes
```bash
docker-compose -f docker-compose.testnet.yml up -d node1 node2 node3
```

### Stop Specific Nodes
```bash
docker-compose -f docker-compose.testnet.yml stop node4 node5
```

### View Logs for Specific Node
```bash
./scripts/docker-testnet.sh logs node1
```

### Execute Commands in Container
```bash
docker exec -it multivm-node1 bash
```

### Check Node Health
```bash
curl http://localhost:8080/health
```

### Submit a Transaction
```bash
# EVM transaction
curl -X POST http://localhost:8080/api/v1/evm/transaction \
  -H "Content-Type: application/json" \
  -d '{
    "from": "0x...",
    "to": "0x...",
    "value": "1000000000000000000",
    "gas": 21000,
    "gasPrice": "1000000000"
  }'

# SVM transaction
curl -X POST http://localhost:8080/api/v1/svm/transaction \
  -H "Content-Type: application/json" \
  -d '{
    "instructions": [...],
    "signatures": [...]
  }'
```

### Query Block Information
```bash
# Latest block
curl http://localhost:8080/api/v1/blocks/latest

# Specific block
curl http://localhost:8080/api/v1/blocks/100
```

## Troubleshooting

### Node Won't Start
1. Check if ports are already in use:
   ```bash
   docker-compose -f docker-compose.testnet.yml ps
   ```

2. Check node logs:
   ```bash
   ./scripts/docker-testnet.sh logs node1
   ```

3. Ensure Docker has enough resources

### Nodes Can't Connect
1. Verify network is created:
   ```bash
   docker network ls | grep multivm-testnet
   ```

2. Check P2P connectivity:
   ```bash
   docker exec multivm-node1 nc -zv 172.20.0.12 26656
   ```

### High Resource Usage
1. Limit CPU/Memory per container:
   ```yaml
   deploy:
     resources:
       limits:
         cpus: '1.0'
         memory: 2G
   ```

2. Reduce block time or transaction pool size

## Advanced Configuration

### Custom Genesis File
Place your genesis.json in `testnet/configs/genesis.json` and mount it:
```yaml
volumes:
  - ./testnet/configs/genesis.json:/opt/multivm/config/genesis.json
```

### Enable External Peers
Modify P2P settings to allow external connections:
```yaml
environment:
  - MULTIVM_P2P_EXTERNAL_ADDRESS=your.public.ip:26656
  - MULTIVM_P2P_PRIVATE_PEER_IDS=
```

### Persistent Data
Data is stored in `./testnet/node{1-7}/`. To reset:
```bash
./scripts/docker-testnet.sh clean
```

## Performance Testing

### Transaction Throughput Test
```bash
# Run from any node
docker exec multivm-node1 /opt/multivm/bin/benchmark \
  --duration 60s \
  --tx-rate 100 \
  --tx-size 250
```

### Network Latency Test
```bash
# Test P2P latency between nodes
docker exec multivm-node1 /opt/multivm/bin/p2p-test \
  --target node2@172.20.0.12:26656 \
  --messages 1000
```

## Security Considerations

⚠️ **This testnet configuration is for development only!**

- JWT secrets are hardcoded
- TLS is disabled
- Authentication is minimal
- Ports are exposed to host

For production:
1. Use strong, unique JWT secrets
2. Enable TLS for all endpoints
3. Implement proper authentication
4. Use Docker secrets management
5. Restrict network access

## Cleanup

To completely remove the testnet:
```bash
# Stop and remove containers, networks, volumes
./scripts/docker-testnet.sh clean

# Remove Docker images
docker rmi multivm-testnet:latest
```