# MultiVM Configuration

This directory contains configuration files for different environments and components.

## Directory Structure

```
config/
├── environments/          # Environment-specific configurations
│   ├── development.toml   # Development environment
│   ├── staging.toml       # Staging environment
│   └── production.toml    # Production environment
├── schemas/              # Configuration schemas and validation
└── templates/            # Configuration templates
```

## Configuration Hierarchy

Configuration is loaded in the following order (later sources override earlier ones):

1. **Default values** defined in the code
2. **Environment-specific config files** (e.g., `environments/production.toml`)
3. **Environment variables** (e.g., `MULTIVM_LOG_LEVEL=debug`)
4. **Command-line arguments** (e.g., `--log-level debug`)

## Environment Variables

All configuration values can be overridden with environment variables using the pattern:
`MULTIVM_<SECTION>_<KEY>`

Examples:
```bash
MULTIVM_SYSTEM_DATA_DIR=/custom/data/path
MULTIVM_LOGGING_LEVEL=debug
MULTIVM_CONSENSUS_BLOCK_TIME_MS=1500
MULTIVM_NETWORK_LISTEN_PORT=8080
```

## Configuration Sections

### System Configuration
- **data_dir**: Directory for persistent data storage
- **max_processes**: Maximum number of concurrent processes
- **process_restart_delay**: Delay before restarting failed processes
- **health_check_interval**: Interval between health checks
- **shutdown_timeout**: Maximum time to wait for graceful shutdown
- **enable_metrics**: Enable Prometheus metrics collection
- **metrics_port**: Port for metrics endpoint

### Logging Configuration
- **level**: Log level (trace, debug, info, warn, error)
- **format**: Log format (pretty, json, compact)
- **enable_file_logging**: Enable logging to files
- **log_directory**: Directory for log files
- **max_log_files**: Maximum number of log files to retain

### Consensus Configuration
- **algorithm**: Consensus algorithm ("malachite")
- **block_time_ms**: Target block time in milliseconds
- **validator_count**: Number of validators in the network
- **enable_single_node**: Enable single-node mode for development

### Network Configuration
- **enable_p2p**: Enable peer-to-peer networking
- **listen_port**: Port to listen for P2P connections
- **bootstrap_nodes**: List of bootstrap nodes for network discovery

### IPC Configuration
- **transport**: IPC transport type ("unix_socket", "tcp")
- **socket_path**: Path for Unix socket (if using unix_socket transport)
- **timeout_ms**: IPC operation timeout in milliseconds

### Mock Processes Configuration
- **enable_mock_solana**: Use mock Solana process instead of real one
- **enable_mock_reth**: Use mock Reth process instead of real one
- **mock_processing_delay_ms**: Artificial delay for mock processing

## Environment-Specific Settings

### Development
- Debug logging enabled
- Single-node consensus
- Mock processes enabled
- Fast health checks
- Local data directory

### Staging
- Info-level logging
- Multi-node consensus
- Real execution engines
- Production-like settings
- Shared data directory

### Production
- Error-level logging
- Full multi-node consensus
- Real execution engines
- Optimized performance settings
- Secure data directory

## Creating Custom Configurations

1. Copy an existing environment config:
   ```bash
   cp config/environments/development.toml config/environments/custom.toml
   ```

2. Modify the settings as needed

3. Use the custom config:
   ```bash
   multivm-node --config config/environments/custom.toml
   ```

## Configuration Validation

Configuration files are validated at startup. Common validation errors:

- **Invalid TOML syntax**: Check for typos and proper formatting
- **Missing required fields**: Ensure all required configuration values are present
- **Invalid values**: Check that numeric values are within acceptable ranges
- **Path errors**: Ensure directory paths exist and are writable

## Security Considerations

- **File permissions**: Configuration files should be readable only by the MultiVM process user
- **Secret management**: Sensitive values (keys, passwords) should be provided via environment variables, not config files
- **Path validation**: Ensure all file and directory paths are within expected locations

## Examples

### Basic Development Setup
```toml
[system]
data_dir = "./data"
enable_metrics = true

[logging]
level = "debug"
format = "pretty"

[consensus]
enable_single_node = true
block_time_ms = 2000
```

### Production Cluster Setup
```toml
[system]
data_dir = "/opt/multivm/data"
max_processes = 20

[logging]
level = "info"
format = "json"
log_directory = "/var/log/multivm"

[consensus]
validator_count = 7
block_time_ms = 1000

[network]
enable_p2p = true
bootstrap_nodes = [
    "node1.multivm.cluster:8000",
    "node2.multivm.cluster:8000",
    "node3.multivm.cluster:8000"
]
```