# Troubleshooting Guide

Comprehensive troubleshooting guide for diagnosing and resolving issues with MultiVM Process deployments.

## Table of Contents

- [Quick Diagnostics](#quick-diagnostics)
- [Common Issues](#common-issues)
- [System-Level Issues](#system-level-issues)
- [Network Issues](#network-issues)
- [Performance Issues](#performance-issues)
- [Security Issues](#security-issues)
- [Consensus Issues](#consensus-issues)
- [Log Analysis](#log-analysis)
- [Recovery Procedures](#recovery-procedures)
- [Support Resources](#support-resources)

## Quick Diagnostics

### Health Check Script

Run this script first to get an overview of system health:

```bash
#!/bin/bash
# quick-diagnosis.sh - Quick system health check

echo "🔍 MultiVM Process Quick Diagnostics"
echo "===================================="
echo "Timestamp: $(date)"
echo ""

# Check if service is running
echo "🔄 Service Status:"
if systemctl is-active --quiet multivm-process; then
    echo "   ✅ Service is running"
    echo "   ⏰ Uptime: $(systemctl show multivm-process --property=ActiveEnterTimestamp --value | cut -d' ' -f2-)"
else
    echo "   ❌ Service is not running"
    echo "   📋 Status: $(systemctl is-active multivm-process)"
fi
echo ""

# Check health endpoint
echo "🏥 Health Endpoint:"
if curl -sf http://localhost:8080/health > /dev/null 2>&1; then
    echo "   ✅ Health endpoint responding"
    echo "   📊 Response:"
    curl -s http://localhost:8080/health | jq '.' 2>/dev/null || echo "   Raw response received"
else
    echo "   ❌ Health endpoint not responding"
fi
echo ""

# Check ports
echo "🔌 Port Status:"
for port in 8080 9090 26657; do
    if ss -tlnp | grep -q ":$port "; then
        echo "   ✅ Port $port is listening"
    else
        echo "   ❌ Port $port is not listening"
    fi
done
echo ""

# Check disk space
echo "💾 Disk Space:"
df -h /var/lib/multivm /var/log/multivm 2>/dev/null | tail -n +2 | while read line; do
    echo "   📁 $line"
done
echo ""

# Check memory usage
echo "🧠 Memory Usage:"
TOTAL_MEM=$(free -h | awk 'NR==2{print $2}')
USED_MEM=$(free -h | awk 'NR==2{print $3}')
echo "   📊 Used: $USED_MEM / $TOTAL_MEM"

# Check recent errors in logs
echo "📝 Recent Errors (last 10 lines):"
journalctl -u multivm-process --since "1 hour ago" -p err --no-pager -n 10 | tail -n +2 | while read line; do
    echo "   ⚠️  $line"
done

echo ""
echo "🔧 For detailed diagnosis, check specific sections below"
```

### System Information Collection

```bash
#!/bin/bash
# collect-system-info.sh - Collect system information for support

OUTPUT_DIR="/tmp/multivm-diagnostics-$(date +%Y%m%d-%H%M%S)"
mkdir -p "$OUTPUT_DIR"

echo "📊 Collecting system information..."

# System info
uname -a > "$OUTPUT_DIR/system-info.txt"
cat /etc/os-release >> "$OUTPUT_DIR/system-info.txt"
lscpu > "$OUTPUT_DIR/cpu-info.txt"
free -h > "$OUTPUT_DIR/memory-info.txt"
df -h > "$OUTPUT_DIR/disk-info.txt"
lsblk > "$OUTPUT_DIR/block-devices.txt"

# Network info
ip addr show > "$OUTPUT_DIR/network-interfaces.txt"
ss -tlnp > "$OUTPUT_DIR/listening-ports.txt"
iptables -L -n > "$OUTPUT_DIR/firewall-rules.txt" 2>/dev/null || echo "No iptables access" > "$OUTPUT_DIR/firewall-rules.txt"

# MultiVM specific
systemctl status multivm-process > "$OUTPUT_DIR/service-status.txt" 2>&1
journalctl -u multivm-process --since "24 hours ago" > "$OUTPUT_DIR/service-logs.txt"
curl -s http://localhost:8080/health > "$OUTPUT_DIR/health-status.json" 2>/dev/null || echo "Health endpoint unavailable" > "$OUTPUT_DIR/health-status.json"
curl -s http://localhost:9090/metrics > "$OUTPUT_DIR/metrics.txt" 2>/dev/null || echo "Metrics endpoint unavailable" > "$OUTPUT_DIR/metrics.txt"

# Configuration
cp /etc/multivm/multivm.toml "$OUTPUT_DIR/config.toml" 2>/dev/null || echo "Config file not found"
ls -la /etc/multivm/ > "$OUTPUT_DIR/config-directory.txt" 2>/dev/null

# Process info
ps aux | grep multivm > "$OUTPUT_DIR/processes.txt"
lsof -p $(pgrep multivm-process) > "$OUTPUT_DIR/open-files.txt" 2>/dev/null

# Create archive
tar -czf "$OUTPUT_DIR.tar.gz" -C "/tmp" "$(basename "$OUTPUT_DIR")"
rm -rf "$OUTPUT_DIR"

echo "✅ System information collected: $OUTPUT_DIR.tar.gz"
echo "📧 Please attach this file when requesting support"
```

## Common Issues

### Issue: Service Won't Start

**Symptoms:**
- `systemctl start multivm-process` fails
- Service shows "failed" status
- No process running

**Diagnosis:**
```bash
# Check service status
systemctl status multivm-process

# Check service logs
journalctl -u multivm-process -n 50

# Check configuration syntax
multivm-process --config /etc/multivm/multivm.toml --validate-config

# Check file permissions
ls -la /etc/multivm/
ls -la /var/lib/multivm/
ls -la /var/log/multivm/
```

**Common Causes & Solutions:**

1. **Configuration Error:**
   ```bash
   # Validate configuration
   multivm-process --config /etc/multivm/multivm.toml --dry-run
   
   # Check for syntax errors
   toml_lint /etc/multivm/multivm.toml
   ```

2. **Permission Issues:**
   ```bash
   # Fix ownership
   sudo chown -R multivm:multivm /var/lib/multivm /var/log/multivm
   sudo chown root:multivm /etc/multivm/multivm.toml
   sudo chmod 640 /etc/multivm/multivm.toml
   ```

3. **Port Already in Use:**
   ```bash
   # Check what's using the port
   sudo ss -tlnp | grep :8080
   sudo ss -tlnp | grep :26657
   
   # Kill conflicting process or change port in config
   sudo kill $(sudo lsof -t -i:8080)
   ```

4. **Missing Dependencies:**
   ```bash
   # Check binary dependencies
   ldd /opt/multivm/bin/multivm-process
   
   # Install missing libraries
   sudo apt-get update && sudo apt-get install -y libssl3
   ```

### Issue: Health Check Fails

**Symptoms:**
- HTTP 500/503 on /health endpoint
- Service running but health check returns unhealthy
- Load balancer marking node as down

**Diagnosis:**
```bash
# Test health endpoint directly
curl -v http://localhost:8080/health

# Check detailed health status
curl -s http://localhost:8080/health/detailed | jq '.'

# Check application logs
journalctl -u multivm-process -f | grep -i health

# Check resource usage
htop
iostat -x 1 5
```

**Common Causes & Solutions:**

1. **Resource Exhaustion:**
   ```bash
   # Check memory usage
   free -h
   # Check if OOM killer activated
   dmesg | grep -i "killed process"
   
   # Increase memory limits in systemd service
   sudo systemctl edit multivm-process
   # Add:
   # [Service]
   # MemoryLimit=4G
   ```

2. **Disk Space Full:**
   ```bash
   # Check disk usage
   df -h /var/lib/multivm /var/log/multivm
   
   # Clean up old logs
   sudo journalctl --vacuum-time=7d
   sudo find /var/log/multivm -name "*.log" -mtime +7 -delete
   ```

3. **Database/Storage Issues:**
   ```bash
   # Check storage backend health
   # For file storage:
   ls -la /var/lib/multivm/storage/
   # For database:
   # Check connection and integrity
   ```

### Issue: High CPU Usage

**Symptoms:**
- CPU usage consistently above 90%
- System becomes unresponsive
- Performance degradation

**Diagnosis:**
```bash
# Monitor CPU usage
top -p $(pgrep multivm-process)
htop

# Check CPU usage over time
sar -u 1 10

# Profile the application
sudo perf top -p $(pgrep multivm-process)

# Check for CPU-intensive operations
journalctl -u multivm-process | grep -i "processing\|consensus\|validation"
```

**Solutions:**

1. **Reduce Worker Threads:**
   ```toml
   [performance]
   worker_threads = 4  # Reduce from higher number
   blocking_threads = 8
   ```

2. **Optimize Consensus Settings:**
   ```toml
   [consensus]
   timeout_ms = 10000  # Increase timeout to reduce retries
   max_block_size = "1MB"  # Reduce block size
   ```

3. **Enable Performance Optimizations:**
   ```toml
   [performance]
   tcp_nodelay = true
   io_buffer_size = "128KB"
   enable_fast_path = true
   ```

### Issue: Memory Leaks

**Symptoms:**
- Memory usage continuously increasing
- Eventually leads to OOM kills
- Performance degradation over time

**Diagnosis:**
```bash
# Monitor memory usage over time
watch -n 5 'ps -p $(pgrep multivm-process) -o pid,ppid,cmd,pmem,rsz,vsz'

# Check for memory leaks in logs
journalctl -u multivm-process | grep -i "memory\|allocation\|leak"

# Use memory profiling tools
valgrind --tool=massif --pages-as-heap=yes multivm-process --config /etc/multivm/multivm.toml
```

**Solutions:**

1. **Increase Memory Limits:**
   ```toml
   [performance]
   gc_threshold = "2GB"  # Force more frequent garbage collection
   ```

2. **Reduce Cache Sizes:**
   ```toml
   [account_mapping]
   cache_size = 1000  # Reduce from higher number
   signature_cache_size = 500
   ```

3. **Enable Memory Monitoring:**
   ```toml
   [monitoring]
   enable_memory_profiling = true
   memory_profile_interval = "5m"
   ```

## Network Issues

### Issue: Consensus Network Partitions

**Symptoms:**
- Frequent leader changes
- High consensus timeouts
- Nodes can't reach consensus

**Diagnosis:**
```bash
# Test connectivity between consensus nodes
for node in 10.0.2.11 10.0.2.12 10.0.2.13; do
    echo "Testing connectivity to $node:26657"
    timeout 5 bash -c "</dev/tcp/$node/26657" && echo "✅ Connected" || echo "❌ Failed"
done

# Check network latency
for node in 10.0.2.11 10.0.2.12 10.0.2.13; do
    ping -c 5 $node
done

# Check consensus logs
journalctl -u multivm-process | grep -i "consensus\|leader\|timeout"

# Monitor network traffic
sudo tcpdump -i any port 26657 -c 20
```

**Solutions:**

1. **Increase Consensus Timeouts:**
   ```toml
   [consensus]
   timeout_ms = 10000  # Increase from 5000
   heartbeat_timeout_ms = 2000
   ```

2. **Check Network Configuration:**
   ```bash
   # Verify routing
   traceroute 10.0.2.11
   
   # Check for packet loss
   mtr --report --report-cycles 100 10.0.2.11
   ```

3. **Firewall Rules:**
   ```bash
   # Allow consensus traffic
   sudo iptables -A INPUT -p tcp --dport 26657 -s 10.0.2.0/24 -j ACCEPT
   sudo iptables -A OUTPUT -p tcp --dport 26657 -d 10.0.2.0/24 -j ACCEPT
   ```

### Issue: Load Balancer Health Checks Failing

**Symptoms:**
- Load balancer removes healthy nodes
- Intermittent connection failures
- 503 errors from load balancer

**Diagnosis:**
```bash
# Test health check from load balancer perspective
curl -H "Host: multivm.example.com" http://10.0.2.10:8080/health

# Check load balancer logs
# For HAProxy:
sudo tail -f /var/log/haproxy.log

# Check response times
curl -w "@curl-format.txt" -o /dev/null -s http://localhost:8080/health
```

**Solutions:**

1. **Adjust Health Check Parameters:**
   ```
   # HAProxy configuration
   server node1 10.0.2.10:8080 check inter 5s fall 5 rise 2
   ```

2. **Optimize Health Check Endpoint:**
   ```toml
   [monitoring]
   health_check_timeout = "5s"
   health_check_cache = "2s"  # Cache health status briefly
   ```

## Performance Issues

### Issue: Slow Block Processing

**Symptoms:**
- Block processing takes longer than expected
- Increasing backlog of pending blocks
- Timeouts during block validation

**Diagnosis:**
```bash
# Check block processing metrics
curl -s http://localhost:9090/metrics | grep block_processing

# Monitor block processing logs
journalctl -u multivm-process -f | grep -i "block\|processing\|validation"

# Check resource utilization during processing
iostat -x 1 10
```

**Solutions:**

1. **Increase Concurrency:**
   ```toml
   [coordinator]
   max_concurrent_blocks = 20  # Increase from 10
   block_timeout = "120s"      # Increase timeout
   ```

2. **Optimize I/O:**
   ```toml
   [performance]
   io_buffer_size = "256KB"
   max_io_events = 4096
   enable_io_uring = true  # If supported
   ```

3. **Database Optimization:**
   ```toml
   [account_mapping]
   batch_size = 1000       # Increase batch size
   enable_write_batching = true
   ```

### Issue: High Latency

**Symptoms:**
- API responses are slow
- Client timeouts
- Poor user experience

**Diagnosis:**
```bash
# Measure API latency
curl -w "@curl-format.txt" -o /dev/null -s http://localhost:8080/health

# Check application latency metrics
curl -s http://localhost:9090/metrics | grep -i latency

# Profile slow requests
journalctl -u multivm-process | grep -i "slow\|timeout\|latency"
```

**Solutions:**

1. **Enable Caching:**
   ```toml
   [account_mapping]
   enable_cache = true
   cache_size = 50000
   cache_ttl = "300s"
   ```

2. **Optimize Network Stack:**
   ```toml
   [performance]
   tcp_nodelay = true
   socket_buffer_size = "1MB"
   keep_alive = true
   ```

## Log Analysis

### Important Log Patterns

```bash
# Error patterns to watch for
journalctl -u multivm-process | grep -E "(ERROR|FATAL|PANIC)"

# Performance issues
journalctl -u multivm-process | grep -E "(timeout|slow|latency|performance)"

# Security issues
journalctl -u multivm-process | grep -E "(auth|security|intrusion|attack)"

# Consensus issues
journalctl -u multivm-process | grep -E "(consensus|leader|election|byzantine)"

# Network issues
journalctl -u multivm-process | grep -E "(connection|network|tcp|udp)"
```

### Log Analysis Script

```bash
#!/bin/bash
# analyze-logs.sh - Automated log analysis

LOG_FILE="${1:-/var/log/multivm/multivm.log}"
SINCE="${2:-1 hour ago}"

echo "📊 Log Analysis Report"
echo "====================="
echo "File: $LOG_FILE"
echo "Since: $SINCE"
echo ""

# Error summary
echo "🚨 Error Summary:"
journalctl -u multivm-process --since "$SINCE" -p err --no-pager | \
    grep -oE "ERROR.*" | sort | uniq -c | sort -nr | head -10
echo ""

# Performance metrics
echo "⏱️  Performance Issues:"
journalctl -u multivm-process --since "$SINCE" --no-pager | \
    grep -i "timeout\|slow\|latency" | wc -l | xargs echo "Timeout/latency events:"
echo ""

# Security events
echo "🔒 Security Events:"
journalctl -u multivm-process --since "$SINCE" --no-pager | \
    grep -i "auth\|security\|unauthorized" | wc -l | xargs echo "Security-related events:"
echo ""

# Consensus health
echo "🤝 Consensus Health:"
journalctl -u multivm-process --since "$SINCE" --no-pager | \
    grep -c "leader.change" | xargs echo "Leader changes:"
journalctl -u multivm-process --since "$SINCE" --no-pager | \
    grep -c "consensus.timeout" | xargs echo "Consensus timeouts:"
echo ""

# Resource usage peaks
echo "📈 Resource Usage:"
journalctl -u multivm-process --since "$SINCE" --no-pager | \
    grep -i "memory\|cpu\|disk" | tail -5
```

## Recovery Procedures

### Emergency Stop Procedure

```bash
#!/bin/bash
# emergency-stop.sh - Emergency shutdown procedure

echo "🚨 Emergency Stop Procedure"
echo "==========================="

# Graceful shutdown first
echo "1. Attempting graceful shutdown..."
sudo systemctl stop multivm-process
sleep 10

# Check if process stopped
if pgrep multivm-process > /dev/null; then
    echo "2. Graceful shutdown failed, forcing termination..."
    sudo pkill -TERM multivm-process
    sleep 5
    
    if pgrep multivm-process > /dev/null; then
        echo "3. Process still running, using SIGKILL..."
        sudo pkill -KILL multivm-process
    fi
fi

echo "✅ Process terminated"

# Block traffic at firewall
echo "4. Blocking incoming traffic..."
sudo iptables -I INPUT -p tcp --dport 8080 -j DROP
sudo iptables -I INPUT -p tcp --dport 26657 -j DROP

echo "🚨 Emergency stop completed"
echo "   - Service stopped"
echo "   - Traffic blocked"
echo "   - Node isolated from network"
```

### Data Recovery Procedure

```bash
#!/bin/bash
# data-recovery.sh - Recover from data corruption

BACKUP_DIR="/mnt/shared/backups"
RECOVERY_DIR="/var/lib/multivm"

echo "💾 Data Recovery Procedure"
echo "========================="

# Stop service
sudo systemctl stop multivm-process

# Backup current state
echo "1. Backing up current state..."
sudo cp -r "$RECOVERY_DIR" "$RECOVERY_DIR.corrupted.$(date +%Y%m%d-%H%M%S)"

# Find latest backup
LATEST_BACKUP=$(ls -t "$BACKUP_DIR"/ | head -1)
echo "2. Using backup: $LATEST_BACKUP"

# Restore from backup
echo "3. Restoring data..."
sudo rm -rf "$RECOVERY_DIR"/*
sudo tar -xzf "$BACKUP_DIR/$LATEST_BACKUP" -C "$RECOVERY_DIR"

# Fix permissions
sudo chown -R multivm:multivm "$RECOVERY_DIR"

# Validate recovery
echo "4. Validating recovery..."
sudo -u multivm multivm-process --config /etc/multivm/multivm.toml --validate-data

# Start service
echo "5. Starting service..."
sudo systemctl start multivm-process

# Verify health
sleep 30
if curl -f http://localhost:8080/health > /dev/null 2>&1; then
    echo "✅ Recovery successful"
else
    echo "❌ Recovery failed - check logs"
    sudo systemctl status multivm-process
fi
```

### Consensus Recovery

```bash
#!/bin/bash
# consensus-recovery.sh - Recover consensus

echo "🤝 Consensus Recovery Procedure"
echo "==============================="

# Check consensus status
echo "1. Checking consensus status..."
HEALTHY_NODES=0
for node in 10.0.2.10 10.0.2.11 10.0.2.12 10.0.2.13; do
    if curl -sf "http://$node:8080/health" > /dev/null; then
        ((HEALTHY_NODES++))
        echo "   ✅ $node is healthy"
    else
        echo "   ❌ $node is unhealthy"
    fi
done

TOTAL_NODES=4
CONSENSUS_THRESHOLD=$((TOTAL_NODES * 2 / 3 + 1))

if [ $HEALTHY_NODES -ge $CONSENSUS_THRESHOLD ]; then
    echo "✅ Consensus can be achieved ($HEALTHY_NODES >= $CONSENSUS_THRESHOLD)"
else
    echo "❌ Insufficient nodes for consensus ($HEALTHY_NODES < $CONSENSUS_THRESHOLD)"
    echo "Manual intervention required"
    exit 1
fi

# Force consensus restart on all nodes
echo "2. Restarting consensus on all nodes..."
for node in 10.0.2.10 10.0.2.11 10.0.2.12 10.0.2.13; do
    echo "   Restarting $node..."
    ssh "multivm@$node" "sudo systemctl restart multivm-process" &
done
wait

# Wait for consensus to stabilize
echo "3. Waiting for consensus to stabilize..."
sleep 60

# Verify consensus
echo "4. Verifying consensus..."
LEADERS=()
for node in 10.0.2.10 10.0.2.11 10.0.2.12 10.0.2.13; do
    LEADER=$(curl -s "http://$node:8080/health/detailed" | jq -r '.consensus.leader' 2>/dev/null)
    if [ "$LEADER" != "null" ] && [ -n "$LEADER" ]; then
        LEADERS+=("$LEADER")
    fi
done

UNIQUE_LEADERS=$(printf '%s\n' "${LEADERS[@]}" | sort -u | wc -l)
if [ $UNIQUE_LEADERS -eq 1 ]; then
    echo "✅ Consensus recovered - single leader elected"
else
    echo "❌ Consensus split - multiple leaders: $UNIQUE_LEADERS"
fi
```

## Support Resources

### Getting Help

1. **Documentation**: Check the [Documentation Index](DOCUMENTATION_INDEX.md)
2. **GitHub Issues**: [Report bugs](https://github.com/your-org/multivm-process/issues)
3. **Discussions**: [Community support](https://github.com/your-org/multivm-process/discussions)
4. **Enterprise Support**: Contact support@your-domain.com

### Before Requesting Support

Please collect the following information:

1. **System Information:**
   ```bash
   ./collect-system-info.sh
   ```

2. **Error Description:**
   - What were you trying to do?
   - What happened instead?
   - When did the issue start?
   - Is it reproducible?

3. **Environment Details:**
   - Operating system and version
   - Hardware specifications
   - Network configuration
   - Deployment method (systemd, Docker, Kubernetes)

4. **Recent Changes:**
   - Recent configuration changes
   - Recent updates or deployments
   - Infrastructure changes

### Emergency Contact

For critical production issues:
- **Email**: emergency@your-domain.com
- **On-call**: +1-800-MULTIVM
- **Slack**: #multivm-emergency (for enterprise customers)

**Response Times:**
- Critical (system down): 15 minutes
- High (significant impact): 2 hours
- Medium (limited impact): 8 hours
- Low (questions/requests): 24 hours

---

Previous: [Deployment Guide](DEPLOYMENT.md) | Next: [API Reference](API_REFERENCE.md)