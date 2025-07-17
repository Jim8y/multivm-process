# MultiVM RPC Service

A unified RPC gateway for the MultiVM platform that provides seamless access to Ethereum (via Reth) and Solana nodes through a single endpoint.

## Features

- **Unified API**: Single endpoint for Ethereum and Solana RPC calls
- **Intelligent Routing**: Automatic method detection and routing to appropriate backend
- **Load Balancing**: Multiple backend support with health checking and failover
- **Caching**: Configurable response caching with method-specific TTLs
- **Rate Limiting**: IP and API key-based rate limiting
- **Authentication**: API key-based authentication with method restrictions
- **Health Monitoring**: Built-in health checks and metrics
- **MultiVM Extensions**: Cross-chain balance queries, account binding, and more

## Quick Start

### 1. Configuration

Create a configuration file (see `examples/config.toml`):

```toml
[server]
bind_address = "127.0.0.1:8545"

[backends]
[[backends.ethereum]]
name = "reth-local"
url = "http://localhost:8551"

[[backends.solana]]
name = "solana-local"
url = "http://localhost:8899"
```

### 2. Start the Service

```bash
# With default configuration
cargo run --bin multivm-rpc-service

# With custom configuration
RPC_CONFIG_PATH=./config.toml cargo run --bin multivm-rpc-service
```

### 3. Make RPC Calls

```bash
# Ethereum JSON-RPC
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}'

# Solana JSON-RPC
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"getSlot","params":[],"id":1}'

# MultiVM-specific methods
curl -X POST http://localhost:8545 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"multivm_getVersion","params":null,"id":1}'
```

## Architecture

```
Client → MultiVM RPC Gateway → {Reth Node, Solana Node}
              ↓
       Account Mapping Layer
              ↓
          Cache & Metrics
```

### Components

1. **RPC Router**: Intelligent method routing based on prefixes and rules
2. **Relay Managers**: Backend connection management with health checking
3. **Cache Layer**: Response caching with configurable TTLs
4. **Middleware Stack**: Authentication, rate limiting, CORS, metrics
5. **MultiVM API**: Cross-chain operations and account management

## Supported Methods

### Ethereum (via Reth)

All standard Ethereum JSON-RPC methods:
- `eth_*` (e.g., `eth_blockNumber`, `eth_getBalance`, `eth_sendTransaction`)
- `web3_*` (e.g., `web3_clientVersion`)
- `net_*` (e.g., `net_version`)
- `debug_*` and `trace_*` (if enabled on Reth)

### Solana

All standard Solana JSON-RPC methods:
- Account methods: `getAccountInfo`, `getBalance`, `getMultipleAccounts`
- Block methods: `getBlock`, `getBlockHeight`, `getSlot`
- Transaction methods: `getTransaction`, `sendTransaction`, `simulateTransaction`
- Network methods: `getHealth`, `getVersion`, `getClusterNodes`

### MultiVM Extensions

- `multivm_getVersion`: Get service version and capabilities
- `multivm_getNetworkStats`: Aggregate network statistics
- `multivm_getCrossChainBalance`: Get balances across multiple chains
- `multivm_getAccountBinding`: Get account binding information
- `multivm_resolveMutliVmAccount`: Resolve MultiVM account from address
- `multivm_submitCrossVmTransaction`: Submit cross-chain transaction
- `multivm_getSupportedChains`: Get supported blockchain networks

## Configuration

### Server Configuration

```toml
[server]
bind_address = "127.0.0.1:8545"    # Server bind address
request_timeout = "30s"             # Request timeout
max_connections = 1000              # Maximum concurrent connections
cors_enabled = true                 # Enable CORS
cors_origins = ["*"]               # Allowed CORS origins
websocket_enabled = true           # Enable WebSocket support
```

### Backend Configuration

```toml
[[backends.ethereum]]
name = "reth-local"
url = "http://localhost:8551"
timeout = "10s"
max_concurrent_requests = 100
health_check_interval = "30s"
priority = 100                     # Higher priority = preferred

# Optional authentication
[backends.ethereum.auth]
bearer_token = "your-token"
# or
username = "user"
password = "pass"
```

### Cache Configuration

```toml
[cache]
enabled = true
max_size = 10000
persistent = false

[cache.method_ttl]
"eth_blockNumber" = "1s"           # Fast-changing data
"eth_getBlockByHash" = "300s"      # Immutable data
"getSlot" = "1s"                   # Fast-changing data
"getTransaction" = "300s"          # Immutable data
```

### Authentication

```toml
[auth]
enabled = true

[auth.api_keys."your-api-key"]
name = "Production Key"
allowed_methods = ["eth_getBalance", "getBalance"]  # Restrict methods
rate_limit = 1000                  # Override default rate limit
enabled = true
```

### Rate Limiting

```toml
[rate_limit]
enabled = true
requests_per_second = 100
burst_size = 200
by_ip = true                       # Rate limit by IP
by_api_key = true                  # Rate limit by API key
whitelist = ["trusted-ip", "trusted-key"]
```

## Health Monitoring

### Health Check Endpoint

```bash
curl http://localhost:8545/health
```

Response:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T00:00:00Z",
  "backends": {
    "ethereum": {
      "healthy": 2,
      "total": 2,
      "status": "healthy"
    },
    "solana": {
      "healthy": 1,
      "total": 1,
      "status": "healthy"
    }
  }
}
```

### Metrics Endpoint

```bash
curl http://localhost:8545/metrics
```

## Error Handling

The service returns standard JSON-RPC error responses:

```json
{
  "jsonrpc": "2.0",
  "error": {
    "code": -32601,
    "message": "Method not found",
    "data": "Additional error details"
  },
  "id": 1
}
```

Common error codes:
- `-32601`: Method not found
- `-32602`: Invalid params
- `-32000`: Internal error / Backend unavailable
- `-32001`: Authentication failed
- `-32002`: Backend unavailable
- `-32003`: Request timeout

## Load Balancing

The service supports multiple load balancing strategies:

1. **Priority**: Route to highest priority healthy node
2. **Round Robin**: Cycle through healthy nodes
3. **Response Time**: Route to fastest responding node
4. **Least Connections**: Route to node with fewest active connections

Configure in `config.toml`:
```toml
[backends]
strategy = "priority"  # or "round_robin", "response_time", "least_connections"
```

## Security

### Production Deployment

1. **Enable Authentication**:
   ```toml
   [auth]
   enabled = true
   ```

2. **Generate Secure API Keys**:
   ```bash
   openssl rand -hex 32
   ```

3. **Restrict CORS Origins**:
   ```toml
   [server]
   cors_origins = ["https://yourdapp.com"]
   ```

4. **Enable Rate Limiting**:
   ```toml
   [rate_limit]
   enabled = true
   requests_per_second = 100
   ```

5. **Use HTTPS**: Deploy behind a reverse proxy with TLS termination

### API Key Management

- Generate unique keys for each client
- Use descriptive names for tracking
- Implement method restrictions for read-only access
- Monitor usage through metrics
- Rotate keys regularly

## Development

### Building

```bash
cargo build --release
```

### Testing

```bash
# Run all tests
cargo test

# Run integration tests
cargo test --test integration

# Run with logs
RUST_LOG=debug cargo test
```

### Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests
5. Submit a pull request

## Troubleshooting

### Common Issues

1. **Backend Connection Failed**
   - Check backend node URLs in configuration
   - Verify nodes are running and accessible
   - Check firewall settings

2. **High Response Times**
   - Enable caching for read-only methods
   - Add more backend nodes
   - Tune connection pool settings

3. **Rate Limiting Issues**
   - Adjust rate limit configuration
   - Implement proper API key usage
   - Monitor client request patterns

4. **Authentication Errors**
   - Verify API key configuration
   - Check method restrictions
   - Ensure proper header format

### Logging

Enable debug logging:
```bash
RUST_LOG=multivm_rpc_service=debug cargo run
```

Log levels:
- `error`: Critical errors only
- `warn`: Warnings and errors
- `info`: General information (default)
- `debug`: Detailed debugging
- `trace`: Very verbose tracing

## License

This project is licensed under the same terms as the MultiVM project.