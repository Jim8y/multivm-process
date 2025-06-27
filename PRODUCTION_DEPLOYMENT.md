# MultiVM Production Deployment Guide

## Prerequisites

- Linux server (Ubuntu 20.04+ or similar)
- Docker and Docker Compose installed
- PostgreSQL 13+ database
- Redis 6+ cache server
- Minimum 16GB RAM, 8 CPU cores
- SSD storage with at least 500GB free space

## Security Checklist

### 1. Environment Variables
- [ ] Generate strong JWT secret: `openssl rand -hex 32`
- [ ] Set unique Redis password
- [ ] Configure database credentials
- [ ] Never commit secrets to version control

### 2. Network Security
- [ ] Configure firewall rules
- [ ] Use TLS/SSL for all external connections
- [ ] Restrict admin interface to internal IPs
- [ ] Enable rate limiting

### 3. Validator Configuration
- [ ] Configure at least 3 validators for production
- [ ] Set unique validator keys
- [ ] Enable validator persistence
- [ ] Configure proper validator endpoints

## Deployment Steps

### 1. Clone and Build

```bash
git clone https://github.com/vm-multiverse/multivm.git
cd multivm
cargo build --release --all-features
```

### 2. Configure Environment

Copy the production template:
```bash
cp configs/production.toml.example configs/production.toml
```

Set environment variables:
```bash
export JWT_SECRET=$(openssl rand -hex 32)
export REDIS_PASSWORD=$(openssl rand -hex 16)
export EXTERNAL_IP=$(curl -s ifconfig.me)
```

### 3. Database Setup

```sql
CREATE DATABASE multivm_production;
CREATE USER multivm WITH ENCRYPTED PASSWORD 'your-secure-password';
GRANT ALL PRIVILEGES ON DATABASE multivm_production TO multivm;
```

### 4. Configure Validators

Edit `configs/production.toml`:
```toml
[consensus]
validators = [
    "validator1-pubkey@10.0.1.10:8801",
    "validator2-pubkey@10.0.1.11:8801",
    "validator3-pubkey@10.0.1.12:8801"
]
min_validators = 3
```

### 5. Start Services

Using Docker Compose:
```bash
docker-compose -f docker-compose.production.yml up -d
```

Or manually:
```bash
# Start Redis
redis-server --requirepass $REDIS_PASSWORD

# Start PostgreSQL
# (Use your distribution's service manager)

# Start MultiVM
./target/release/multivm-cli start --config configs/production.toml
```

### 6. Health Checks

Verify all services are healthy:
```bash
curl http://localhost:8080/health
curl http://localhost:9090/metrics
```

## Monitoring

### Prometheus Configuration

Add to `prometheus.yml`:
```yaml
scrape_configs:
  - job_name: 'multivm'
    static_configs:
      - targets: ['localhost:9090']
```

### Grafana Dashboards

Import the MultiVM dashboard from `monitoring/grafana-dashboard.json`

### Alerts

Configure alerts for:
- Service health status
- High error rates
- Resource usage
- Validator participation

## Backup and Recovery

### Automated Backups

Create backup script:
```bash
#!/bin/bash
# /opt/multivm/scripts/backup.sh

BACKUP_DIR="/var/backups/multivm"
DATE=$(date +%Y%m%d_%H%M%S)

# Backup database
pg_dump multivm_production > $BACKUP_DIR/db_$DATE.sql

# Backup consensus state
tar -czf $BACKUP_DIR/consensus_$DATE.tar.gz /var/lib/multivm/consensus

# Backup configuration
cp /opt/multivm/configs/production.toml $BACKUP_DIR/config_$DATE.toml

# Keep only last 7 days
find $BACKUP_DIR -type f -mtime +7 -delete
```

Add to crontab:
```bash
0 2 * * * /opt/multivm/scripts/backup.sh
```

### Recovery Procedure

1. Stop all services
2. Restore database: `psql multivm_production < backup.sql`
3. Restore consensus state: `tar -xzf consensus_backup.tar.gz -C /`
4. Restart services

## Troubleshooting

### Common Issues

1. **Consensus not starting**
   - Check validator configuration
   - Ensure all validators can communicate
   - Verify validator keys are unique

2. **High memory usage**
   - Adjust cache sizes in configuration
   - Enable memory limits for processes
   - Monitor for memory leaks

3. **Connection errors**
   - Check firewall rules
   - Verify RPC endpoints are accessible
   - Check TLS certificates

### Debug Mode

Enable debug logging:
```toml
[monitoring]
log_level = "debug"
```

### Support

For production support:
- Documentation: https://docs.multivm.io
- Issues: https://github.com/vm-multiverse/multivm/issues
- Security: security@multivm.io

## Performance Tuning

### Database Optimization

```sql
-- Add indexes for common queries
CREATE INDEX idx_transactions_timestamp ON transactions(timestamp);
CREATE INDEX idx_accounts_address ON accounts(address);

-- Configure PostgreSQL
max_connections = 200
shared_buffers = 4GB
effective_cache_size = 12GB
```

### Redis Optimization

```conf
maxmemory 4gb
maxmemory-policy allkeys-lru
tcp-keepalive 60
```

### System Tuning

```bash
# Increase file descriptors
ulimit -n 65536

# TCP tuning
echo 'net.core.somaxconn = 65536' >> /etc/sysctl.conf
echo 'net.ipv4.tcp_max_syn_backlog = 65536' >> /etc/sysctl.conf
sysctl -p
```

## Security Updates

Subscribe to security notifications and regularly update:
```bash
# Check for updates
cargo update
cargo audit

# Apply system updates
apt update && apt upgrade -y
```

Remember: Security is an ongoing process. Regularly review logs, update dependencies, and follow security best practices.