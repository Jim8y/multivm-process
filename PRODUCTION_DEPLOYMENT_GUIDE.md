# Production Deployment Guide

## Overview

This guide covers deploying the MultiVM platform in a production environment with proper security, monitoring, and high availability.

## Prerequisites

- Linux servers (Ubuntu 22.04 LTS recommended)
- Docker 24.0+ and Docker Compose 2.20+
- Python 3.8+ for management scripts
- Minimum 8GB RAM per node
- SSD storage (100GB+ recommended)
- Network connectivity between nodes

## Architecture

### Recommended Production Setup

- **Validator Nodes**: 7+ nodes for BFT consensus
- **Load Balancer**: HAProxy or NGINX for RPC distribution
- **Monitoring**: Prometheus + Grafana stack
- **Explorer**: Dedicated server for blockchain explorer

## Deployment Steps

### 1. Server Preparation

```bash
# Update system
sudo apt update && sudo apt upgrade -y

# Install Docker
curl -fsSL https://get.docker.com | sudo sh
sudo usermod -aG docker $USER

# Install Docker Compose
sudo curl -L "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
sudo chmod +x /usr/local/bin/docker-compose

# Install Python dependencies
sudo apt install python3-pip python3-venv -y
```

### 2. Clone and Configure

```bash
# Clone repository
git clone <repository> /opt/multivm
cd /opt/multivm

# Create configuration directory
sudo mkdir -p /etc/multivm
sudo cp -r testnet-production-complete/config/* /etc/multivm/
```

### 3. Generate Validator Credentials

```bash
# Generate new validator accounts
python3 generate-validator-accounts.py

# Backup validator keys securely
sudo cp testnet-production-complete/validators_complete.json /etc/multivm/validators.json
sudo chmod 600 /etc/multivm/validators.json
```

### 4. Configure Network

Edit `/etc/multivm/docker-compose.yml` to update:
- External IP addresses
- P2P ports (30303-30309)
- RPC ports (8545-8551)
- API ports (8080-8086)

### 5. JWT Secret Configuration

```bash
# Generate JWT secret
openssl rand -hex 32 > /etc/multivm/jwt.hex
sudo chmod 600 /etc/multivm/jwt.hex

# Update all node configurations to use same JWT
```

### 6. Deploy Services

```bash
cd /opt/multivm/testnet-production-complete
sudo docker compose -f /etc/multivm/docker-compose.yml up -d

# Verify all services running
sudo docker compose ps
```

### 7. Initial Funding

```bash
# Setup Python environment
python3 -m venv /opt/multivm/venv
source /opt/multivm/venv/bin/activate
pip install web3 eth-account

# Fund validators
python3 /opt/multivm/auto-fund-validators.py
```

## Security Configuration

### 1. Firewall Rules

```bash
# Allow P2P communication
sudo ufw allow 30303:30309/tcp

# Allow RPC (restrict source IPs in production)
sudo ufw allow from <trusted_ip> to any port 8545:8551

# Allow Explorer (public)
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp

# Enable firewall
sudo ufw enable
```

### 2. SSL/TLS Configuration

```nginx
# NGINX configuration for RPC endpoints
server {
    listen 443 ssl http2;
    server_name rpc.multivm.example.com;
    
    ssl_certificate /etc/ssl/multivm/cert.pem;
    ssl_certificate_key /etc/ssl/multivm/key.pem;
    
    location / {
        proxy_pass http://localhost:8545;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    }
}
```

### 3. Access Control

- Use JWT authentication for Engine API
- Implement rate limiting on RPC endpoints
- Whitelist validator node IPs
- Enable audit logging

## Monitoring Setup

### 1. Prometheus Configuration

```yaml
# prometheus.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'multivm'
    static_configs:
      - targets: ['localhost:9090']
        
  - job_name: 'reth'
    static_configs:
      - targets: ['localhost:9091']
```

### 2. Grafana Dashboards

Import provided dashboards:
- MultiVM Network Overview
- Reth Execution Metrics
- Transaction Pool Status
- Node Health Monitoring

### 3. Alerting Rules

```yaml
# alerts.yml
groups:
  - name: multivm
    rules:
      - alert: NodeDown
        expr: up{job="multivm"} == 0
        for: 5m
        
      - alert: LowPeerCount
        expr: p2p_peers < 3
        for: 10m
        
      - alert: TransactionPoolFull
        expr: txpool_pending > 9000
        for: 5m
```

## Backup and Recovery

### 1. Automated Backups

```bash
#!/bin/bash
# backup.sh
BACKUP_DIR="/backup/multivm/$(date +%Y%m%d)"
mkdir -p $BACKUP_DIR

# Backup validator keys
cp /etc/multivm/validators.json $BACKUP_DIR/

# Backup chain data
docker exec reth-node1 reth db snapshot $BACKUP_DIR/reth-snapshot

# Compress and encrypt
tar -czf - $BACKUP_DIR | gpg -c > $BACKUP_DIR.tar.gz.gpg
```

### 2. Recovery Procedure

1. Stop all services: `docker compose down`
2. Restore configuration files
3. Import chain snapshot
4. Restart services
5. Verify synchronization

## Maintenance

### Rolling Updates

```bash
# Update one node at a time
for i in {1..7}; do
    docker compose stop multivm-node$i reth-node$i
    docker compose pull multivm-node$i reth-node$i
    docker compose up -d multivm-node$i reth-node$i
    sleep 60  # Wait for sync
done
```

### Log Management

```bash
# Configure log rotation
cat > /etc/logrotate.d/multivm << EOF
/var/lib/docker/containers/*/*.log {
    rotate 7
    daily
    compress
    missingok
    delaycompress
    notifempty
    maxsize 100M
}
EOF
```

## Performance Tuning

### 1. System Optimization

```bash
# Increase file descriptors
echo "* soft nofile 65536" >> /etc/security/limits.conf
echo "* hard nofile 65536" >> /etc/security/limits.conf

# Network tuning
cat >> /etc/sysctl.conf << EOF
net.core.somaxconn = 1024
net.ipv4.tcp_max_syn_backlog = 2048
net.ipv4.tcp_synack_retries = 2
EOF

sysctl -p
```

### 2. Docker Optimization

```yaml
# docker-compose.yml optimizations
services:
  multivm-node1:
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 8G
        reservations:
          cpus: '2'
          memory: 4G
```

## Troubleshooting

### Common Issues

1. **Consensus Stalls**
   - Check peer connectivity
   - Verify time synchronization
   - Review validator keys

2. **High Memory Usage**
   - Adjust cache sizes
   - Enable state pruning
   - Monitor transaction pool

3. **RPC Timeouts**
   - Scale RPC nodes horizontally
   - Implement caching layer
   - Optimize query patterns

### Health Checks

```bash
# Check consensus health
curl http://localhost:8080/health

# Verify block production
curl http://localhost:8545 -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Monitor peer count
curl http://localhost:8080/api/v1/node/peers
```

## Disaster Recovery

### Emergency Procedures

1. **Network Split**
   - Identify partition
   - Restore connectivity
   - Allow automatic recovery

2. **Data Corruption**
   - Stop affected node
   - Restore from snapshot
   - Resync with network

3. **Key Compromise**
   - Immediately stop validator
   - Generate new keys
   - Update network configuration

## Production Checklist

- [ ] All nodes synchronized
- [ ] Firewall rules configured
- [ ] SSL certificates installed
- [ ] Monitoring alerts active
- [ ] Backup automation verified
- [ ] Load balancer configured
- [ ] Documentation updated
- [ ] Emergency contacts listed
- [ ] Recovery procedures tested