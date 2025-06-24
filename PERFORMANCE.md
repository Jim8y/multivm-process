# MultiVM Performance Tuning Guide

## Overview

This guide provides comprehensive performance optimization strategies for the MultiVM blockchain system, covering hardware optimization, software configuration, and monitoring best practices.

## Hardware Optimization

### CPU Configuration

1. **CPU Governor**
   ```bash
   # Set performance governor
   echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
   
   # Disable CPU frequency scaling
   systemctl disable ondemand
   ```

2. **CPU Affinity**
   ```bash
   # Pin validator processes to specific cores
   taskset -c 0-15 multivm-validator
   
   # Reserve cores for critical processes
   isolcpus=16-31  # Add to kernel boot parameters
   ```

3. **NUMA Optimization**
   ```bash
   # Check NUMA topology
   numactl --hardware
   
   # Run with NUMA awareness
   numactl --interleave=all multivm-cli run
   ```

### Memory Optimization

1. **Huge Pages**
   ```bash
   # Enable transparent huge pages
   echo always > /sys/kernel/mm/transparent_hugepage/enabled
   
   # Configure huge pages
   echo 4096 > /proc/sys/vm/nr_hugepages
   
   # In config
   [system.memory]
   use_huge_pages = true
   huge_page_size = "2MB"
   ```

2. **Memory Allocation**
   ```toml
   [system.memory]
   jemalloc_enabled = true
   pre_allocate_memory = true
   memory_pool_size = "32GB"
   ```

### Storage Optimization

1. **NVMe Configuration**
   ```bash
   # Set I/O scheduler to none for NVMe
   echo none > /sys/block/nvme0n1/queue/scheduler
   
   # Increase queue depth
   echo 1024 > /sys/block/nvme0n1/queue/nr_requests
   ```

2. **Filesystem Tuning**
   ```bash
   # Mount options for ext4
   mount -o noatime,nodiratime,nobarrier /dev/nvme0n1p1 /var/lib/multivm
   
   # XFS recommended for large databases
   mkfs.xfs -f -d agcount=32 /dev/nvme0n1p1
   ```

3. **Database Storage**
   ```toml
   [storage]
   # Separate devices for different workloads
   state_db_path = "/mnt/nvme0/state"
   transaction_db_path = "/mnt/nvme1/transactions"
   snapshot_path = "/mnt/ssd/snapshots"
   
   # Write-ahead log on fastest device
   wal_path = "/mnt/optane/wal"
   ```

## Network Optimization

### Kernel Parameters

```bash
# /etc/sysctl.conf
# Network buffer sizes
net.core.rmem_max = 134217728
net.core.wmem_max = 134217728
net.ipv4.tcp_rmem = 4096 87380 134217728
net.ipv4.tcp_wmem = 4096 65536 134217728

# Connection handling
net.core.somaxconn = 65535
net.ipv4.tcp_max_syn_backlog = 65535
net.core.netdev_max_backlog = 65535

# TCP optimization
net.ipv4.tcp_fin_timeout = 30
net.ipv4.tcp_keepalive_time = 600
net.ipv4.tcp_keepalive_probes = 3
net.ipv4.tcp_keepalive_intvl = 30
net.ipv4.tcp_tw_reuse = 1

# Congestion control
net.ipv4.tcp_congestion_control = bbr
net.core.default_qdisc = fq
```

### Network Interface Tuning

```bash
# Increase ring buffer
ethtool -G eth0 rx 4096 tx 4096

# Enable offloading
ethtool -K eth0 gso on gro on tso on

# Set interrupt coalescing
ethtool -C eth0 adaptive-rx on adaptive-tx on
```

## Application-Level Optimization

### Transaction Processing

1. **Batch Configuration**
   ```toml
   [transaction_pool]
   max_pending_transactions = 50000
   batch_size = 1000
   batch_timeout_ms = 100
   parallel_validation = true
   validation_threads = 16
   ```

2. **Pipeline Optimization**
   ```toml
   [execution]
   pipeline_stages = 4
   prefetch_distance = 100
   parallel_execution = true
   execution_threads = 32
   ```

### State Management

1. **Caching Strategy**
   ```toml
   [cache]
   # Multi-level caching
   l1_cache_size = "2GB"
   l2_cache_size = "16GB"
   l3_cache_size = "64GB"
   
   # Cache policies
   eviction_policy = "lru"
   prefetch_enabled = true
   compression_enabled = true
   ```

2. **State Pruning**
   ```toml
   [state.pruning]
   enabled = true
   keep_recent_states = 1000
   archive_old_states = true
   prune_interval = "1h"
   ```

### Consensus Optimization

1. **Block Production**
   ```toml
   [consensus]
   block_time_target = "1s"
   max_block_size = "8MB"
   max_transactions_per_block = 10000
   
   # Parallel proposal validation
   parallel_proposal_validation = true
   proposal_validation_threads = 8
   ```

2. **Network Topology**
   ```toml
   [p2p.topology]
   # Optimize for low latency
   prefer_geographic_proximity = true
   max_peers = 100
   outbound_peers = 25
   
   # Fast block propagation
   compact_blocks = true
   sendheaders = true
   ```

## Database Optimization

### PostgreSQL Tuning

```sql
-- Memory settings
ALTER SYSTEM SET shared_buffers = '32GB';
ALTER SYSTEM SET effective_cache_size = '96GB';
ALTER SYSTEM SET work_mem = '256MB';
ALTER SYSTEM SET maintenance_work_mem = '2GB';

-- Write performance
ALTER SYSTEM SET checkpoint_completion_target = 0.9;
ALTER SYSTEM SET wal_buffers = '64MB';
ALTER SYSTEM SET max_wal_size = '4GB';

-- Query optimization
ALTER SYSTEM SET random_page_cost = 1.1;  -- For SSD
ALTER SYSTEM SET effective_io_concurrency = 200;
ALTER SYSTEM SET max_parallel_workers_per_gather = 8;
ALTER SYSTEM SET max_parallel_workers = 32;

-- Connection pooling
ALTER SYSTEM SET max_connections = 500;
```

### Redis Optimization

```conf
# redis.conf
# Memory management
maxmemory 16gb
maxmemory-policy allkeys-lru

# Persistence tuning
save ""  # Disable RDB
appendonly yes
appendfsync everysec

# Performance
io-threads 8
io-threads-do-reads yes

# Network
tcp-backlog 511
tcp-keepalive 300
```

## Monitoring and Profiling

### Performance Metrics

1. **System Metrics**
   ```bash
   # CPU profiling
   perf record -g multivm-validator
   perf report
   
   # Memory profiling
   valgrind --tool=massif multivm-validator
   
   # I/O profiling
   iotop -p $(pgrep multivm)
   ```

2. **Application Metrics**
   ```toml
   [monitoring.metrics]
   enabled = true
   export_interval = "10s"
   
   # Detailed metrics
   track_transaction_latency = true
   track_state_access_patterns = true
   track_network_bandwidth = true
   ```

### Benchmarking

1. **Transaction Throughput**
   ```bash
   # Run benchmark
   multivm-bench --duration 300 --threads 32 --tx-rate 10000
   
   # Analyze results
   multivm-bench analyze --input bench-results.json
   ```

2. **Latency Testing**
   ```bash
   # P99 latency test
   multivm-bench latency --percentile 99 --duration 3600
   ```

## Optimization Strategies

### Vertical Scaling

1. **Hardware Upgrades**
   - CPU: AMD EPYC or Intel Xeon (high core count)
   - RAM: DDR5 with ECC (minimum 128GB)
   - Storage: Enterprise NVMe with power-loss protection
   - Network: 25Gbps or higher

2. **Resource Allocation**
   ```toml
   [resources]
   validator_memory = "64GB"
   rpc_memory = "32GB"
   api_memory = "16GB"
   
   validator_cpu_cores = 32
   rpc_cpu_cores = 16
   api_cpu_cores = 8
   ```

### Horizontal Scaling

1. **Load Balancing**
   ```nginx
   upstream multivm_rpc {
       least_conn;
       server rpc1.internal:8545 weight=10;
       server rpc2.internal:8545 weight=10;
       server rpc3.internal:8545 weight=10;
   }
   ```

2. **Sharding Strategy**
   ```toml
   [sharding]
   enabled = true
   shard_count = 4
   replication_factor = 3
   
   # Account-based sharding
   sharding_method = "account_hash"
   ```

## Performance Troubleshooting

### Common Issues

1. **High CPU Usage**
   - Check for infinite loops in smart contracts
   - Verify transaction validation isn't bottlenecked
   - Profile hot functions with flamegraphs

2. **Memory Leaks**
   - Monitor with `htop` and `smem`
   - Use heap profiling
   - Check for unbounded caches

3. **Disk I/O Bottlenecks**
   - Monitor with `iostat -x 1`
   - Check for excessive logging
   - Verify database vacuum is running

### Performance Testing

```bash
# Comprehensive performance test
./scripts/performance-test.sh --full

# Generate performance report
multivm-cli performance-report --output perf-report.html
```

## Best Practices Summary

1. **Regular Maintenance**
   - Weekly performance reviews
   - Monthly database optimization
   - Quarterly hardware assessment

2. **Capacity Planning**
   - Monitor growth trends
   - Plan for 2x current load
   - Test scaling strategies

3. **Continuous Optimization**
   - A/B test configuration changes
   - Profile before and after updates
   - Document all optimizations

---

© 2024 MultiVM Project. Performance Matters.