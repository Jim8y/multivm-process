# MultiVM Docker Deployment

Complete Docker-based deployment for the MultiVM multi-blockchain execution system.

## Overview

This directory contains everything needed to deploy a production-ready MultiVM cluster using Docker Compose:

- **4-Node Cluster**: 1 bootstrap node + 3 validator nodes
- **P2P Networking**: Custom bridge network with service discovery
- **Monitoring Stack**: Prometheus metrics and Loki log aggregation
- **Health Checks**: Automated health monitoring and recovery
- **Volume Management**: Persistent data storage for all nodes

## Quick Start

### Prerequisites

```bash
# Install Docker and Docker Compose
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh

# Add user to docker group (logout/login required)
sudo usermod -aG docker $USER
```

### Deploy Cluster

```bash
# Navigate to docker directory
cd docker

# Start the full 4-node cluster
./scripts/manage-cluster.sh start

# Check cluster status
./scripts/manage-cluster.sh status

# View logs
./scripts/manage-cluster.sh logs

# Stop cluster
./scripts/manage-cluster.sh stop
```

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                External Network                     │
│  Ports: 8080-8083 (API), 26656-26659 (P2P)       │
└─────────────────────────────────────────────────────┘
                            │
┌─────────────────────────────────────────────────────┐
│             MultiVM Bridge Network                  │
│                172.20.0.0/16                       │
│                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │   Node 1    │  │   Node 2    │  │   Node 3    │ │
│  │ (Bootstrap) │  │ (Validator) │  │ (Validator) │ │
│  │172.20.0.10  │  │172.20.0.11  │  │172.20.0.12  │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│                                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │   Node 4    │  │Prometheus   │  │    Loki     │ │
│  │ (Observer)  │  │(Monitoring) │  │  (Logs)     │ │
│  │172.20.0.13  │  │    :9090    │  │   :3100     │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
└─────────────────────────────────────────────────────┘
```

## Services

### MultiVM Nodes

| Service | Type | Ports | Role | IP |
|---------|------|-------|------|-----|
| multivm-node-1 | Bootstrap | 8080, 26656, 8545, 8899 | Validator | 172.20.0.10 |
| multivm-node-2 | Validator | 8081, 26657, 8546, 8900 | Validator | 172.20.0.11 |
| multivm-node-3 | Validator | 8082, 26658, 8547, 8901 | Validator | 172.20.0.12 |
| multivm-node-4 | Observer | 8083, 26659, 8548, 8902 | Observer | 172.20.0.13 |

### Monitoring Services

| Service | Purpose | Port | Profile |
|---------|---------|------|---------|
| prometheus | Metrics collection | 9090 | monitoring |
| log-aggregator | Log aggregation | 3100 | monitoring |

### Test Services

| Service | Purpose | Profile |
|---------|---------|---------|
| test-runner | Integration tests | testing |

## Configuration

### Node Configuration

Each node has its own configuration file in `config/`:

- `node-1.toml` - Bootstrap validator
- `node-2.toml` - Regular validator  
- `node-3.toml` - Regular validator
- `node-4.toml` - Observer node

Key configuration sections:
```toml
[node]
id = "node-1"
type = "bootstrap"  # bootstrap, validator, full
role = "validator"  # validator, observer

[consensus]
algorithm = "malachite"
validator_key = "validator-1-key"
timeout_propose_ms = 3000

[execution]
solana_mode = "mock"  # Change to "native" for production
reth_mode = "mock"    # Change to "native" for production
```

### Network Configuration

- **Subnet**: 172.20.0.0/16
- **Gateway**: 172.20.0.1
- **DNS**: Automatic service discovery
- **Health Checks**: 30s interval with 3 retries

## Management Commands

### Cluster Management

```bash
# Build Docker images
./scripts/manage-cluster.sh build

# Start all nodes
./scripts/manage-cluster.sh start

# Stop all nodes
./scripts/manage-cluster.sh stop

# Restart cluster
./scripts/manage-cluster.sh restart

# Check status
./scripts/manage-cluster.sh status

# View logs
./scripts/manage-cluster.sh logs [node]

# Scale cluster (1-4 nodes)
./scripts/manage-cluster.sh scale 3

# Clean everything
./scripts/manage-cluster.sh clean
```

### Monitoring

```bash
# Start monitoring stack
./scripts/manage-cluster.sh monitoring

# View Prometheus: http://localhost:9090
# View Loki: http://localhost:3100
```

### Testing

```bash
# Run integration tests
./scripts/manage-cluster.sh test

# Connect to node shell
./scripts/manage-cluster.sh shell node-1
```

## Networking

### Port Mapping

| Internal Port | External Port | Service |
|---------------|---------------|---------|
| 8080 | 8080-8083 | REST API |
| 26656 | 26656-26659 | P2P Communication |
| 8545 | 8545-8548 | EVM RPC |
| 8899 | 8899-8902 | SVM RPC |

### Service Discovery

Nodes can reach each other using hostnames:
- `multivm-node-1` → Bootstrap node
- `multivm-node-2` → Validator 2
- `multivm-node-3` → Validator 3
- `multivm-node-4` → Observer node

## Storage

### Persistent Volumes

Each node has persistent storage:
```
multivm-node-X-data:/opt/multivm/data
```

Stored data:
- Blockchain state
- Consensus data
- Account mappings
- IPC sockets

### Log Management

Logs are stored in:
- Container: `/opt/multivm/logs/`
- Host: `./logs/`

Log rotation is handled automatically.

## Health Checks

### Node Health

Health checks run every 30 seconds:
```bash
curl -f http://localhost:8080/health
```

Response:
```json
{
  "status": "healthy",
  "components": {
    "consensus": "healthy",
    "p2p": "healthy",
    "execution": "healthy"
  },
  "uptime": "3600s"
}
```

### Cluster Health

Check overall cluster health:
```bash
./scripts/manage-cluster.sh health
```

## Production Deployment

### Security Considerations

1. **Change Default Keys**: Generate new validator keys
2. **Network Security**: Use firewall rules
3. **TLS Encryption**: Enable HTTPS for APIs
4. **Authentication**: Enable JWT authentication

### Resource Requirements

**Minimum per node:**
- CPU: 2 cores
- RAM: 4GB
- Storage: 100GB SSD
- Network: 1Gbps

**Recommended for production:**
- CPU: 4 cores
- RAM: 8GB
- Storage: 500GB NVMe SSD
- Network: 10Gbps

### Scaling

The current setup supports up to 4 nodes. For larger deployments:

1. Add new node configurations
2. Update docker-compose.yml
3. Configure additional bootstrap nodes
4. Update monitoring targets

## Troubleshooting

### Common Issues

**Container won't start:**
```bash
# Check logs
docker logs multivm-node-1

# Check resource usage
docker stats

# Restart specific node
docker restart multivm-node-1
```

**Network connectivity:**
```bash
# Test internal connectivity
docker exec multivm-node-1 ping multivm-node-2

# Check bridge network
docker network inspect docker_multivm-net
```

**Health check failures:**
```bash
# Manual health check
curl http://localhost:8080/health

# Check service status
./scripts/manage-cluster.sh status
```

### Log Analysis

```bash
# View recent logs
./scripts/manage-cluster.sh logs node-1

# Follow logs in real-time
docker logs -f multivm-node-1

# Filter logs by level
docker logs multivm-node-1 2>&1 | grep ERROR
```

## Integration

### API Access

REST API endpoints are available at:
- Node 1: http://localhost:8080
- Node 2: http://localhost:8081  
- Node 3: http://localhost:8082
- Node 4: http://localhost:8083

### Monitoring Integration

Prometheus metrics at:
- `/metrics` endpoint on each node
- Prometheus UI: http://localhost:9090

### External Integration

To integrate with external systems:

1. **Load Balancer**: Point to multiple API ports
2. **Monitoring**: Scrape metrics endpoints
3. **Logging**: Forward logs to external systems
4. **Backup**: Schedule volume backups

---

**Status**: ✅ Production Ready | **Last Updated**: 2025-06-21