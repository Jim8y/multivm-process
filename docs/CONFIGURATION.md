# Configuration Reference

Complete reference for configuring the MultiVM Process system for development, testing, and production environments.

## Table of Contents

- [Configuration Overview](#configuration-overview)
- [Configuration File Format](#configuration-file-format)
- [Core Settings](#core-settings)
- [Component Configuration](#component-configuration)
- [Environment Variables](#environment-variables)
- [Production Configuration](#production-configuration)
- [Security Configuration](#security-configuration)
- [Performance Tuning](#performance-tuning)
- [Validation and Testing](#validation-and-testing)

## Configuration Overview

MultiVM Process uses a hierarchical configuration system:

1. **Default Values**: Built-in sensible defaults
2. **Configuration File**: TOML format configuration
3. **Environment Variables**: Override specific settings
4. **Command Line Arguments**: Highest priority overrides

### Configuration Precedence

```
Command Line Args > Environment Variables > Config File > Defaults
```

## Configuration File Format

### Basic Structure

```toml
# config/multivm.toml
[coordinator]
# System coordinator settings

[consensus]
# Malachite consensus configuration

[account_mapping]
# Cross-VM account mapping settings

[p2p]
# Network layer configuration

[security]
# Security and authentication

[logging]
# Logging and tracing

[monitoring]
# Metrics and health monitoring
```

### Complete Example Configuration

```toml
# MultiVM Process Configuration
# Production-ready example configuration

[coordinator]
# System coordination settings
health_check_interval = "30s"
block_timeout = "60s"
max_concurrent_blocks = 10
enable_recovery = true
recovery_timeout = "300s"
startup_timeout = "120s"
shutdown_timeout = "60s"

# Resource limits
max_memory_usage = "16GB"
max_cpu_usage = 80.0  # percentage
max_disk_usage = "500GB"

[consensus]
# Malachite consensus configuration
validator_id = "validator-0"
listen_addr = "0.0.0.0:26657"
timeout_ms = 5000
max_block_size = "2MB"
block_time = "6s"

# Consensus participants
validators = [
    { id = "validator-0", address = "127.0.0.1:26657", weight = 1 },
    { id = "validator-1", address = "127.0.0.1:26658", weight = 1 },
    { id = "validator-2", address = "127.0.0.1:26659", weight = 1 },
    { id = "validator-3", address = "127.0.0.1:26660", weight = 1 }
]

# Byzantine fault tolerance settings
min_validators = 3
byzantine_fault_tolerance = 1  # f = 1, supports 3f+1 = 4 validators

[account_mapping]
# Cross-VM account mapping configuration
storage_backend = "memory"  # "memory" | "file" | "database"
cache_size = 10000
enable_cache = true
validation_timeout = "10s"

# Cryptographic settings
enable_signature_verification = true
signature_cache_size = 1000
signature_cache_ttl = "3600s"

# Storage settings (when using file/database backend)
storage_path = "./data/account_mapping"
backup_interval = "1h"
backup_retention = "7d"

[p2p]
# Network layer configuration (currently simplified)
listen_addresses = ["0.0.0.0:4001"]
bootstrap_peers = []
enable_dht = false        # Disabled for current architecture
enable_gossipsub = false  # Disabled for current architecture
max_connections = 50
connection_timeout = "30s"

[security]
# Security and authentication settings
enable_authentication = true
enable_encryption = false  # Set to true for production
enable_rate_limiting = true

# Authentication settings
auth_token_ttl = "24h"
auth_token_refresh_threshold = "4h"
max_auth_attempts = 5
auth_lockout_duration = "15m"

# Rate limiting
rate_limit_messages = 100
rate_limit_window = "60s"
rate_limit_burst = 10

# TLS settings (when encryption enabled)
tls_cert_path = "./certs/server.crt"
tls_key_path = "./certs/server.key"
tls_ca_path = "./certs/ca.crt"

[logging]
# Logging and tracing configuration
level = "info"          # "error" | "warn" | "info" | "debug" | "trace"
format = "json"         # "json" | "pretty" | "compact"
target = "stdout"       # "stdout" | "stderr" | "file"

# File logging (when target = "file")
file_path = "./logs/multivm.log"
max_file_size = "100MB"
max_files = 10
rotation = "daily"      # "hourly" | "daily" | "never"

# Component-specific logging levels
[logging.levels]
multivm_consensus = "debug"
multivm_account_mapping = "info"
multivm_p2p = "warn"
multivm_process_manager = "info"

[monitoring]
# Metrics and health monitoring
enable_metrics = true
metrics_bind_addr = "0.0.0.0:9090"
metrics_path = "/metrics"

# Health check settings
health_check_bind_addr = "0.0.0.0:8080"
health_check_path = "/health"
detailed_health_path = "/health/detailed"

# Metrics collection intervals
metrics_collection_interval = "10s"
health_check_interval = "30s"
system_metrics_interval = "60s"

# Resource monitoring thresholds
[monitoring.thresholds]
cpu_usage_warning = 70.0
cpu_usage_critical = 90.0
memory_usage_warning = 80.0
memory_usage_critical = 95.0
disk_usage_warning = 85.0
disk_usage_critical = 95.0

[performance]
# Performance tuning settings
worker_threads = 8              # Number of async runtime threads
blocking_threads = 16           # Number of blocking task threads
max_blocking_threads = 512      # Maximum blocking threads

# Memory settings
stack_size = "2MB"              # Thread stack size
gc_threshold = "1GB"            # Garbage collection threshold

# I/O settings
io_buffer_size = "64KB"
max_io_events = 1024
io_timeout = "30s"

# Networking performance
tcp_nodelay = true
tcp_keepalive = true
socket_buffer_size = "256KB"

[development]
# Development-only settings
enable_debug_endpoints = false
enable_profiling = false
hot_reload = false
mock_external_services = false

# Testing overrides
[development.testing]
fast_mode = false               # Faster but less accurate for testing
deterministic_mode = false      # Deterministic randomness for tests
enable_test_utils = false       # Additional test utilities
```

## Core Settings

### Coordinator Configuration

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `health_check_interval` | Duration | `"30s"` | How often to check system health |
| `block_timeout` | Duration | `"60s"` | Maximum time to process a block |
| `max_concurrent_blocks` | Integer | `10` | Maximum blocks processed simultaneously |
| `enable_recovery` | Boolean | `true` | Enable automatic error recovery |
| `recovery_timeout` | Duration | `"300s"` | Timeout for recovery operations |

### Duration Format

Durations can be specified using these units:
- `ns` - nanoseconds
- `us` - microseconds  
- `ms` - milliseconds
- `s` - seconds
- `m` - minutes
- `h` - hours
- `d` - days

Examples: `"30s"`, `"5m"`, `"2h"`, `"1d"`

### Size Format

Sizes can be specified using these units:
- `B` - bytes
- `KB` - kilobytes (1,000 bytes)
- `MB` - megabytes
- `GB` - gigabytes
- `TB` - terabytes
- `KiB` - kibibytes (1,024 bytes)
- `MiB` - mebibytes
- `GiB` - gibibytes

Examples: `"1GB"`, `"512MB"`, `"2GiB"`

## Component Configuration

### Consensus Configuration

```toml
[consensus]
# Basic settings
validator_id = "validator-0"        # Unique validator identifier
listen_addr = "0.0.0.0:26657"      # Address to listen on
timeout_ms = 5000                   # Consensus round timeout

# Block settings
max_block_size = "2MB"              # Maximum block size
block_time = "6s"                   # Target block time
max_transactions_per_block = 10000  # Transaction limit per block

# Validator set configuration
validators = [
    { id = "validator-0", address = "127.0.0.1:26657", weight = 1 },
    { id = "validator-1", address = "127.0.0.1:26658", weight = 1 }
]

# Fault tolerance
min_validators = 3                  # Minimum validators for operation
byzantine_fault_tolerance = 1       # Number of Byzantine faults tolerated
```

### Account Mapping Configuration

```toml
[account_mapping]
# Storage backend
storage_backend = "memory"          # "memory" | "file" | "database"
storage_path = "./data/accounts"    # Path for persistent storage
cache_size = 10000                  # Number of cached mappings
enable_cache = true                 # Enable in-memory caching

# Validation settings
enable_signature_verification = true
signature_cache_size = 1000
signature_cache_ttl = "3600s"
validation_timeout = "10s"

# Performance settings
batch_size = 100                    # Batch operations for efficiency
max_pending_operations = 1000       # Queue limit for operations
```

### Security Configuration

```toml
[security]
# Authentication
enable_authentication = true
auth_token_ttl = "24h"
auth_token_refresh_threshold = "4h"
max_auth_attempts = 5
auth_lockout_duration = "15m"

# Encryption (TLS)
enable_encryption = false           # Enable for production
tls_cert_path = "./certs/server.crt"
tls_key_path = "./certs/server.key"
tls_ca_path = "./certs/ca.crt"
tls_verify_client = true

# Rate limiting
enable_rate_limiting = true
rate_limit_messages = 100           # Messages per window
rate_limit_window = "60s"           # Time window
rate_limit_burst = 10               # Burst allowance
```

## Environment Variables

Override configuration using environment variables:

### Core Variables

```bash
# Logging
export RUST_LOG=info                            # Global log level
export MULTIVM_LOG_LEVEL=debug                  # Override log level
export MULTIVM_LOG_FORMAT=json                  # Log format

# Configuration
export MULTIVM_CONFIG_PATH=./config/multivm.toml    # Config file path
export MULTIVM_DATA_DIR=./data                      # Data directory

# Consensus
export CONSENSUS_VALIDATOR_ID=validator-0           # Validator ID
export CONSENSUS_LISTEN_ADDR=127.0.0.1:26657       # Listen address
export CONSENSUS_TIMEOUT_MS=5000                    # Timeout

# Security
export ENABLE_AUTHENTICATION=true                   # Enable auth
export ENABLE_ENCRYPTION=false                      # Enable TLS
export AUTH_TOKEN_TTL=24h                          # Token TTL

# Performance
export MAX_CONCURRENT_BLOCKS=10                     # Concurrency limit
export WORKER_THREADS=8                            # Runtime threads
```

### Environment Variable Naming

Environment variables follow this pattern:
- Convert section and setting to uppercase
- Replace dots with underscores
- Prefix with `MULTIVM_`

Examples:
- `coordinator.health_check_interval` → `MULTIVM_COORDINATOR_HEALTH_CHECK_INTERVAL`
- `consensus.validator_id` → `MULTIVM_CONSENSUS_VALIDATOR_ID`
- `security.enable_authentication` → `MULTIVM_SECURITY_ENABLE_AUTHENTICATION`

## Production Configuration

### High-Performance Production Setup

```toml
[coordinator]
health_check_interval = "10s"
block_timeout = "30s"
max_concurrent_blocks = 20
enable_recovery = true
recovery_timeout = "120s"

[consensus]
validator_id = "prod-validator-1"
listen_addr = "0.0.0.0:26657"
timeout_ms = 3000
max_block_size = "4MB"
block_time = "3s"

[security]
enable_authentication = true
enable_encryption = true
enable_rate_limiting = true
rate_limit_messages = 1000
rate_limit_window = "60s"

[monitoring]
enable_metrics = true
metrics_bind_addr = "0.0.0.0:9090"
health_check_interval = "10s"

[performance]
worker_threads = 16
blocking_threads = 32
io_buffer_size = "128KB"
tcp_nodelay = true
socket_buffer_size = "512KB"

[logging]
level = "warn"
format = "json"
target = "file"
file_path = "/var/log/multivm/multivm.log"
max_file_size = "500MB"
max_files = 20
```

### High-Availability Setup

```toml
[consensus]
# 7-validator setup for high availability
validators = [
    { id = "validator-0", address = "node-0.internal:26657", weight = 1 },
    { id = "validator-1", address = "node-1.internal:26657", weight = 1 },
    { id = "validator-2", address = "node-2.internal:26657", weight = 1 },
    { id = "validator-3", address = "node-3.internal:26657", weight = 1 },
    { id = "validator-4", address = "node-4.internal:26657", weight = 1 },
    { id = "validator-5", address = "node-5.internal:26657", weight = 1 },
    { id = "validator-6", address = "node-6.internal:26657", weight = 1 }
]
min_validators = 5
byzantine_fault_tolerance = 2  # Tolerates 2 Byzantine failures

[coordinator]
max_concurrent_blocks = 50
health_check_interval = "5s"
enable_recovery = true

[monitoring]
health_check_interval = "5s"
metrics_collection_interval = "5s"

[monitoring.thresholds]
cpu_usage_warning = 60.0
memory_usage_warning = 70.0
```

## Security Configuration

### Production Security Settings

```toml
[security]
# Enable all security features
enable_authentication = true
enable_encryption = true
enable_rate_limiting = true

# Strong authentication
auth_token_ttl = "8h"               # Shorter token lifetime
auth_token_refresh_threshold = "2h"  # More frequent refresh
max_auth_attempts = 3               # Stricter attempt limit
auth_lockout_duration = "30m"       # Longer lockout

# Aggressive rate limiting
rate_limit_messages = 50            # Lower message limit
rate_limit_window = "60s"
rate_limit_burst = 5               # Lower burst limit

# TLS configuration
tls_verify_client = true            # Require client certificates
tls_min_version = "1.3"            # TLS 1.3 minimum
tls_cipher_suites = [               # Allowed cipher suites
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256"
]
```

### Certificate Setup

Generate TLS certificates for production:

```bash
# Generate CA certificate
openssl genrsa -out ca-key.pem 4096
openssl req -new -x509 -key ca-key.pem -out ca.pem -days 3650

# Generate server certificate
openssl genrsa -out server-key.pem 4096
openssl req -new -key server-key.pem -out server.csr
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca-key.pem -out server.pem -days 365

# Generate client certificate
openssl genrsa -out client-key.pem 4096
openssl req -new -key client-key.pem -out client.csr
openssl x509 -req -in client.csr -CA ca.pem -CAkey ca-key.pem -out client.pem -days 365
```

## Performance Tuning

### CPU Optimization

```toml
[performance]
# Match CPU cores
worker_threads = 16                 # Set to number of CPU cores
blocking_threads = 32               # 2x worker threads

# Optimize for CPU-bound workloads
max_blocking_threads = 1024
stack_size = "1MB"                 # Smaller stacks for more threads
```

### Memory Optimization

```toml
[performance]
# Memory settings
gc_threshold = "2GB"               # Higher threshold for less GC
stack_size = "4MB"                 # Larger stacks for complex operations

[account_mapping]
cache_size = 50000                 # Larger cache for better hit rate
batch_size = 500                   # Larger batches for efficiency

[consensus]
max_block_size = "8MB"             # Larger blocks for throughput
```

### Network Optimization

```toml
[performance]
# Network performance
tcp_nodelay = true                 # Disable Nagle's algorithm
tcp_keepalive = true               # Enable keepalive
socket_buffer_size = "1MB"         # Large socket buffers
io_buffer_size = "256KB"           # Large I/O buffers
max_io_events = 4096               # More I/O events per poll

[p2p]
max_connections = 200              # More concurrent connections
connection_timeout = "10s"         # Faster connection timeout
```

## Validation and Testing

### Configuration Validation

Validate your configuration:

```bash
# Validate configuration file
cargo run --bin multivm-cli -- config validate ./config/multivm.toml

# Test configuration with dry run
cargo run --bin multivm-cli -- --config ./config/multivm.toml --dry-run

# Check configuration precedence
cargo run --bin multivm-cli -- config show
```

### Testing Configuration

```toml
# config/test.toml - Configuration for testing
[coordinator]
health_check_interval = "1s"       # Faster for tests
block_timeout = "5s"               # Shorter timeout
max_concurrent_blocks = 5          # Lower concurrency

[consensus]
timeout_ms = 1000                  # Fast consensus for tests
block_time = "1s"                  # Quick blocks

[logging]
level = "debug"                    # Verbose logging for debugging
format = "pretty"                  # Human-readable format

[development.testing]
fast_mode = true                   # Enable test optimizations
deterministic_mode = true          # Predictable behavior
enable_test_utils = true           # Additional test features
```

### Environment-Specific Configs

Organize configurations by environment:

```
config/
├── multivm.toml          # Base configuration
├── development.toml      # Development overrides
├── testing.toml          # Testing overrides
├── staging.toml          # Staging environment
└── production.toml       # Production configuration
```

Load environment-specific config:

```bash
# Development
export MULTIVM_CONFIG_PATH=./config/development.toml

# Production
export MULTIVM_CONFIG_PATH=./config/production.toml
```

---

Next: [Security Guide](SECURITY.md) | [Deployment Guide](DEPLOYMENT.md)