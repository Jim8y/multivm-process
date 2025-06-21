# MultiVM Application Layer

A comprehensive API interface layer for the MultiVM blockchain system, providing unified access to both Solana Virtual Machine (SVM) and Ethereum Virtual Machine (EVM) operations.

## Features

- **🔗 Unified API**: Single interface for both SVM and EVM operations
- **🚀 Multiple Protocols**: REST, GraphQL, WebSocket, and Admin interfaces
- **🔒 Security**: JWT authentication, API keys, and role-based access control
- **⚡ Performance**: Multi-level caching, connection pooling, and async operations
- **📊 Monitoring**: Prometheus metrics, health checks, and distributed tracing
- **🐳 Container Ready**: Docker and Kubernetes deployment support

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Application Layer                        │
├─────────────────────────────────────────────────────────────┤
│ REST API │ GraphQL │ WebSocket │ Admin Interface            │
├─────────────────────────────────────────────────────────────┤
│ SVM Gateway │ EVM Gateway │ MultiVM Gateway │ Monitor       │
├─────────────────────────────────────────────────────────────┤
│ Auth Manager │ Rate Limiter │ Cache Layer │ Validation     │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.75+ 
- PostgreSQL 13+
- Redis 6+
- Docker (optional)

### Development Setup

1. **Clone and build**:
   ```bash
   git clone <repository-url>
   cd multivm-application
   cargo build
   ```

2. **Setup configuration**:
   ```bash
   cp config.example.toml config.toml
   # Edit config.toml with your settings
   ```

3. **Setup database**:
   ```bash
   # Create database and run migrations
   createdb multivm
   sqlx migrate run
   ```

4. **Start services**:
   ```bash
   # Start Redis
   redis-server
   
   # Start the application
   cargo run
   ```

### Docker Setup

1. **Using Docker Compose**:
   ```bash
   # Copy configuration
   cp config.example.toml config.toml
   
   # Start all services
   docker-compose up -d
   ```

2. **Manual Docker build**:
   ```bash
   # Build image
   docker build -t multivm-application .
   
   # Run container
   docker run -p 8080:8080 -p 8081:8081 -p 8082:8082 multivm-application
   ```

## API Endpoints

### REST API (`http://localhost:8080`)

- **Health**: `GET /health`
- **SVM Operations**: `GET|POST /api/v1/svm/*`
- **EVM Operations**: `GET|POST /api/v1/evm/*`
- **MultiVM Operations**: `GET|POST /api/v1/multivm/*`

Example requests:
```bash
# Health check
curl http://localhost:8080/health

# Get SVM account
curl http://localhost:8080/api/v1/svm/accounts/{address}

# Get EVM account  
curl http://localhost:8080/api/v1/evm/accounts/{address}

# Bind accounts across VMs
curl -X POST http://localhost:8080/api/v1/multivm/accounts/bind \
  -H "Content-Type: application/json" \
  -d '{"svm_account": "...", "evm_account": "0x..."}'
```

### GraphQL API (`http://localhost:8081`)

Access the GraphQL Playground at `http://localhost:8081` for interactive queries.

Example queries:
```graphql
# System information
query {
  systemInfo {
    version
    name
    uptime
    nodeCount
  }
}

# Get SVM account
query {
  svmAccount(address: "11111111111111111111111111111112") {
    address
    lamports
    owner
    executable
  }
}

# Bind accounts
mutation {
  bindAccounts(input: {
    svmAccount: "11111111111111111111111111111112"
    evmAccount: "0x742d35Cc6632C0532" 
  }) {
    success
    multivmAccount
    bindingId
  }
}
```

### WebSocket API (`ws://localhost:8082`)

Real-time event subscriptions:

```javascript
const ws = new WebSocket('ws://localhost:8082/ws');

// Subscribe to new blocks
ws.send(JSON.stringify({
  type: 'subscribe',
  events: ['new_block', 'new_transaction']
}));

// Handle events
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Received event:', data);
};
```

### Admin Interface (`http://localhost:8083`)

Web-based administration dashboard for:
- System monitoring and metrics
- Node management and configuration
- Log viewing and maintenance
- API analytics and debugging

## Configuration

Key configuration sections in `config.toml`:

```toml
[server]
[server.rest]
port = 8080

[server.graphql] 
port = 8081
enable_playground = true

[server.websocket]
port = 8082

[server.admin]
port = 8083
enable_ui = true

[database]
host = "localhost"
name = "multivm"

[cache]
cache_type = "Redis"
default_ttl = "300s"

[vm_clients]
[vm_clients.solana]
rpc_url = "http://localhost:8899"

[vm_clients.reth]
rpc_url = "http://localhost:8545"

[auth]
require_auth = false
jwt_secret = "your-secret"

[monitoring]
enable_metrics = true
```

## Security

### Authentication

The application supports multiple authentication methods:

1. **API Keys**: Include `X-API-Key` header
2. **JWT Tokens**: Include `Authorization: Bearer <token>` header
3. **Public Access**: Some endpoints are publicly accessible

### Rate Limiting

Configurable rate limiting per client/endpoint:
- IP-based limiting
- API key-based limiting  
- User-based limiting

### CORS

Cross-Origin Resource Sharing is configurable for web applications.

## Monitoring

### Metrics (`http://localhost:9090`)

Prometheus metrics including:
- Request counts and latencies
- System resource usage
- VM node health status
- Cache hit/miss rates

### Health Checks (`http://localhost:9091`)

Health check endpoints for load balancers:
- Application health
- Database connectivity
- Cache availability
- VM node status

### Tracing

Distributed tracing with Jaeger integration for request flow tracking.

## Development

### Running Tests

```bash
# Unit tests
cargo test --lib

# Integration tests  
cargo test --test integration

# All tests
cargo test
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check for security issues
cargo audit
```

### Database Migrations

```bash
# Create migration
sqlx migrate add <name>

# Run migrations
sqlx migrate run

# Revert migration
sqlx migrate revert
```

## Production Deployment

### Kubernetes

Example Kubernetes deployment:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: multivm-application
spec:
  replicas: 3
  selector:
    matchLabels:
      app: multivm-application
  template:
    metadata:
      labels:
        app: multivm-application
    spec:
      containers:
      - name: multivm-application
        image: multivm-application:latest
        ports:
        - containerPort: 8080
        - containerPort: 8081
        - containerPort: 8082
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: multivm-secrets
              key: database-url
```

### Load Balancing

Configure load balancer health checks:
- Health endpoint: `/health`
- Readiness endpoint: `/api/v1/system/health` 
- Metrics endpoint: `/metrics`

### Scaling

The application is designed for horizontal scaling:
- Stateless design
- External session storage (Redis)
- Database connection pooling
- Cache layer for performance

## Troubleshooting

### Common Issues

1. **Database Connection Failed**
   ```bash
   # Check PostgreSQL is running
   pg_isready -h localhost -p 5432
   
   # Check configuration
   grep database config.toml
   ```

2. **Redis Connection Failed**
   ```bash
   # Check Redis is running
   redis-cli ping
   
   # Check configuration  
   grep redis config.toml
   ```

3. **VM Node Connection Failed**
   ```bash
   # Check Solana node
   curl http://localhost:8899 -X POST -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
   
   # Check Reth node
   curl http://localhost:8545 -X POST -H "Content-Type: application/json" \
     -d '{"jsonrpc":"2.0","method":"web3_clientVersion","params":[],"id":1}'
   ```

### Logs

Application logs are available:
- Console output (development)
- File logs: `logs/multivm-application.log`
- Structured JSON format for production

### Debug Mode

Enable debug logging:
```bash
RUST_LOG=debug cargo run
```

## Contributing

1. Fork the repository
2. Create feature branch: `git checkout -b feature/amazing-feature`
3. Commit changes: `git commit -m 'Add amazing feature'`
4. Push to branch: `git push origin feature/amazing-feature`
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Support

For support and questions:
- Create an issue in the repository
- Check the [documentation](docs/)
- Review the [FAQ](docs/FAQ.md)

---

**MultiVM Architecture Team** 