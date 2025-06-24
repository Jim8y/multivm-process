# MultiVM Production Deployment Guide

## Overview

This guide provides comprehensive instructions for deploying the MultiVM blockchain system in a production environment. MultiVM enables atomic cross-VM operations between Solana (SVM) and Ethereum (EVM) blockchains.

## System Requirements

### Hardware Requirements (Minimum)
- **CPU**: 16 cores (32 threads recommended)
- **RAM**: 64GB (128GB recommended)
- **Storage**: 2TB NVMe SSD (4TB recommended)
- **Network**: 1Gbps dedicated connection

### Software Requirements
- **OS**: Ubuntu 22.04 LTS or later
- **Rust**: 1.75.0 or later
- **Docker**: 24.0 or later (optional)
- **PostgreSQL**: 15.0 or later
- **Redis**: 7.0 or later

## Pre-Deployment Checklist

- [ ] Hardware requirements met
- [ ] Software dependencies installed
- [ ] Network configuration complete
- [ ] SSL certificates obtained
- [ ] Database servers running
- [ ] Monitoring infrastructure ready

## Installation Steps

### 1. Clone and Build

```bash
# Clone the repository
git clone https://github.com/your-org/multivm-process.git
cd multivm-process

# Build release binaries
cargo build --release --all

# Run tests
cargo test --all
```

### 2. Configuration

Create the main configuration file:

```toml
# config/production.toml
[system]
name = "multivm-prod-01"
network_id = "mainnet"
data_directory = "/var/lib/multivm"

[networking]
p2p_listen_addr = "0.0.0.0:30303"
rpc_listen_addr = "0.0.0.0:8545"
ws_listen_addr = "0.0.0.0:8546"

[security]
enable_tls = true
tls_cert_path = "/etc/multivm/certs/server.crt"
tls_key_path = "/etc/multivm/certs/server.key"

[consensus]
validator_key_path = "/etc/multivm/keys/validator.key"
min_validators = 4

[ethereum]
network = "mainnet"
data_dir = "/var/lib/multivm/ethereum"
rpc_port = 8545
ws_port = 8546

[solana]
network = "mainnet-beta"
data_dir = "/var/lib/multivm/solana"
rpc_port = 8899
ws_port = 8900

[database]
url = "postgresql://multivm:password@localhost/multivm_prod"
max_connections = 100

[redis]
url = "redis://localhost:6379"
```

### 3. Database Setup

```sql
-- Create database and user
CREATE DATABASE multivm_prod;
CREATE USER multivm WITH ENCRYPTED PASSWORD 'your-secure-password';
GRANT ALL PRIVILEGES ON DATABASE multivm_prod TO multivm;

-- Run migrations
cd multivm-process
cargo run --bin multivm-cli migrate
```

### 4. TLS/SSL Configuration

```bash
# Generate self-signed certificate (for testing)
openssl req -x509 -newkey rsa:4096 -keyout server.key -out server.crt -days 365 -nodes

# For production, use Let's Encrypt
certbot certonly --standalone -d your-domain.com
```

### 5. System Service Setup

Create systemd service file:

```ini
# /etc/systemd/system/multivm.service
[Unit]
Description=MultiVM Blockchain System
After=network.target postgresql.service redis.service

[Service]
Type=simple
User=multivm
Group=multivm
WorkingDirectory=/opt/multivm
ExecStart=/opt/multivm/target/release/multivm-cli run --config /etc/multivm/production.toml
Restart=always
RestartSec=10
StandardOutput=append:/var/log/multivm/multivm.log
StandardError=append:/var/log/multivm/multivm-error.log

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/multivm /var/log/multivm

[Install]
WantedBy=multi-user.target
```

### 6. Start Services

```bash
# Enable and start service
sudo systemctl enable multivm
sudo systemctl start multivm

# Check status
sudo systemctl status multivm
```

## Security Configuration

### 1. Firewall Rules

```bash
# Allow P2P communication
sudo ufw allow 30303/tcp

# Allow RPC (restrict to specific IPs in production)
sudo ufw allow from 10.0.0.0/8 to any port 8545
sudo ufw allow from 10.0.0.0/8 to any port 8899

# Allow monitoring
sudo ufw allow from 10.0.0.0/8 to any port 9090
```

### 2. API Authentication

Configure JWT authentication:

```toml
[auth]
jwt_secret = "your-256-bit-secret-key"
jwt_expiry = "24h"
api_key_required = true
```

### 3. Rate Limiting

```toml
[rate_limiting]
enabled = true
requests_per_minute = 100
burst_size = 20
```

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
```

### 2. Grafana Dashboards

Import the provided dashboards from `monitoring/grafana/`:
- System Overview
- Transaction Metrics
- Cross-VM Operations
- Network Health

### 3. Alerting Rules

```yaml
# alerts.yml
groups:
  - name: multivm
    rules:
      - alert: HighCPUUsage
        expr: cpu_usage_percent > 80
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High CPU usage detected"
          
      - alert: LowDiskSpace
        expr: disk_free_percent < 10
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "Low disk space"
```

## Performance Tuning

### 1. System Limits

```bash
# /etc/security/limits.conf
multivm soft nofile 65536
multivm hard nofile 65536
multivm soft nproc 32768
multivm hard nproc 32768
```

### 2. Kernel Parameters

```bash
# /etc/sysctl.conf
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.core.netdev_max_backlog = 65535
vm.swappiness = 10
```

### 3. Database Optimization

```sql
-- PostgreSQL configuration
ALTER SYSTEM SET shared_buffers = '16GB';
ALTER SYSTEM SET effective_cache_size = '48GB';
ALTER SYSTEM SET maintenance_work_mem = '2GB';
ALTER SYSTEM SET max_connections = 200;
```

## Backup and Recovery

### 1. Automated Backups

```bash
#!/bin/bash
# /opt/multivm/scripts/backup.sh
BACKUP_DIR="/backup/multivm"
DATE=$(date +%Y%m%d_%H%M%S)

# Backup database
pg_dump multivm_prod > "$BACKUP_DIR/db_$DATE.sql"

# Backup state
tar -czf "$BACKUP_DIR/state_$DATE.tar.gz" /var/lib/multivm/

# Backup configuration
tar -czf "$BACKUP_DIR/config_$DATE.tar.gz" /etc/multivm/

# Keep only last 7 days
find "$BACKUP_DIR" -name "*.sql" -mtime +7 -delete
find "$BACKUP_DIR" -name "*.tar.gz" -mtime +7 -delete
```

### 2. Recovery Procedure

```bash
# Stop services
sudo systemctl stop multivm

# Restore database
psql multivm_prod < backup.sql

# Restore state
tar -xzf state_backup.tar.gz -C /

# Restart services
sudo systemctl start multivm
```

## Maintenance

### Rolling Updates

```bash
# 1. Build new version
git pull
cargo build --release

# 2. Stop service gracefully
sudo systemctl stop multivm

# 3. Backup current version
cp /opt/multivm/target/release/multivm-cli /opt/multivm/target/release/multivm-cli.backup

# 4. Deploy new version
cp target/release/multivm-cli /opt/multivm/target/release/

# 5. Start service
sudo systemctl start multivm
```

### Health Checks

```bash
# Check system health
curl http://localhost:9090/health

# Check cross-VM operations
multivm-cli status --detailed

# Check consensus
multivm-cli consensus status
```

## Troubleshooting

### Common Issues

1. **Connection Refused**
   - Check firewall rules
   - Verify service is running
   - Check logs: `journalctl -u multivm -f`

2. **High Memory Usage**
   - Adjust cache settings
   - Check for memory leaks
   - Review transaction pool size

3. **Slow Performance**
   - Check disk I/O: `iostat -x 1`
   - Review database queries
   - Check network latency

### Debug Commands

```bash
# Enable debug logging
export RUST_LOG=debug

# Get detailed metrics
multivm-cli metrics --verbose

# Export state for analysis
multivm-cli export-state --output state.json
```

## Support

- Documentation: https://docs.multivm.org
- Issues: https://github.com/your-org/multivm-process/issues
- Discord: https://discord.gg/multivm
- Email: support@multivm.org

---

© 2024 MultiVM Project. Licensed under MIT/Apache-2.0.