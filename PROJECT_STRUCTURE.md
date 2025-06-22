# MultiVM Project Structure

This document describes the professional organization of the MultiVM project.

## Directory Structure

```
multivm/
├── .github/                    # GitHub workflows and templates
│   ├── workflows/             # CI/CD workflows
│   ├── ISSUE_TEMPLATE/        # Issue templates
│   └── PULL_REQUEST_TEMPLATE/ # PR templates
├── crates/                    # Rust workspace crates
│   ├── multivm-cli/          # Command-line interface
│   ├── multivm-common/       # Shared types and utilities
│   ├── multivm-consensus/    # Malachite consensus integration
│   ├── multivm-p2p/          # Peer-to-peer networking
│   ├── multivm-process-manager/ # Process management
│   ├── multivm-application/  # Web application layer
│   ├── multivm-account-mapping/ # Cross-VM account mapping
│   ├── multivm-mock-processes/  # Mock execution engines
│   ├── solana-execution-engine/ # Solana VM integration
│   └── reth-execution-engine/   # Ethereum VM integration
├── scripts/                   # Build, test, and deployment scripts
│   ├── build/                # Build automation
│   ├── test/                 # Test automation
│   ├── deploy/               # Deployment automation
│   ├── dev/                  # Development utilities
│   └── maintenance/          # Maintenance scripts
├── docs/                      # Comprehensive documentation
│   ├── architecture/         # System architecture docs
│   ├── guides/               # User and developer guides
│   ├── references/           # API and technical references
│   └── reports/              # Project status reports
├── config/                    # Configuration files
├── examples/                  # Usage examples and demos
├── tests/                     # Integration tests
├── deploy/                    # Deployment configurations
│   ├── docker/               # Docker deployment
│   └── kubernetes/           # Kubernetes deployment
├── tools/                     # Development and analysis tools
├── logs/                      # Log files (gitignored)
├── temp/                      # Temporary files (gitignored)
├── build/                     # Build artifacts (gitignored)
└── data/                      # Runtime data (gitignored)
```

## Build System

### Make Targets

The project uses a professional Makefile with these targets:

```bash
make help      # Show available targets
make build     # Build the project
make test      # Run all tests
make check     # Run all quality checks
make format    # Format code
make lint      # Run linting
make clean     # Clean build artifacts
make dev       # Start development environment
make docker    # Build Docker image
make deploy    # Deploy to environment
make all       # Run everything
```

### Scripts

Professional shell scripts are organized by purpose:

- **scripts/build/build.sh**: Comprehensive build automation
- **scripts/test/run-tests.sh**: Test suite automation
- **scripts/deploy/deploy.sh**: Deployment automation
- **scripts/dev/**: Development utilities

## Documentation Organization

### Architecture Documentation
- System overview and design principles
- Component interaction diagrams
- IPC protocol specifications
- Consensus mechanism details

### User Guides
- Installation and setup
- Configuration reference
- Deployment guides
- Troubleshooting

### Developer Guides
- Contribution guidelines
- Code style standards
- Testing procedures
- Release processes

### API References
- Rust API documentation
- REST API specifications
- WebSocket API documentation
- Configuration schemas

## Code Organization

### Workspace Structure

The project follows Rust workspace conventions:

```toml
[workspace]
members = [
    "crates/multivm-*",
    "examples",
    "tests"
]
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `multivm-cli` | Command-line interface and node management |
| `multivm-common` | Shared types, traits, and utilities |
| `multivm-consensus` | Malachite BFT consensus integration |
| `multivm-p2p` | Network layer and peer discovery |
| `multivm-process-manager` | Process lifecycle and IPC management |
| `multivm-application` | Web application and API layer |
| `multivm-account-mapping` | Cross-VM account binding |
| `multivm-mock-processes` | Testing and development mocks |
| `solana-execution-engine` | Solana VM integration |
| `reth-execution-engine` | Ethereum VM integration |

## Configuration Management

### Environment-Specific Configs

```
config/
├── development.toml    # Development environment
├── staging.toml       # Staging environment
├── production.toml    # Production environment
└── test.toml         # Test environment
```

### Configuration Hierarchy

1. **Default values** in code
2. **Configuration files** (environment-specific)
3. **Environment variables** (override config files)
4. **Command-line arguments** (override everything)

## Deployment Strategy

### Docker Support

```dockerfile
# Multi-stage build for production efficiency
FROM rust:1.70 as builder
# ... build stage ...

FROM debian:bullseye-slim as runtime
# ... runtime stage ...
```

### Container Orchestration

- **docker-compose.single.yml**: Single-node development
- **docker-compose.multi.yml**: Multi-node production
- **deploy/kubernetes/**: Kubernetes manifests

### Environment Management

```bash
# Development
make deploy ENVIRONMENT=development

# Staging
make deploy ENVIRONMENT=staging

# Production
make deploy ENVIRONMENT=production
```

## Quality Assurance

### Code Quality Tools

- **rustfmt**: Code formatting
- **clippy**: Linting and best practices
- **cargo-audit**: Security vulnerability scanning
- **cargo-tarpaulin**: Code coverage

### Testing Strategy

1. **Unit Tests**: Individual component testing
2. **Integration Tests**: Component interaction testing
3. **End-to-End Tests**: Full system workflow testing
4. **Performance Tests**: Benchmarking and profiling

### CI/CD Pipeline

```yaml
# .github/workflows/ci.yml
- Format checking
- Lint checking
- Security audit
- Test execution
- Build verification
- Docker image building
```

## Development Workflow

### Getting Started

```bash
# 1. Clone and setup
git clone https://github.com/vm-multiverse/multivm.git
cd multivm
make setup

# 2. Start development
make dev

# 3. Run tests
make test

# 4. Check code quality
make check
```

### Contribution Process

1. **Fork** the repository
2. **Create** a feature branch
3. **Implement** changes following code standards
4. **Test** thoroughly with `make all`
5. **Submit** a pull request

## Security Considerations

### Process Isolation
- Execution engines run in separate processes
- IPC communication via Unix sockets
- Resource monitoring and limits

### Cryptographic Security
- Ed25519 signatures for consensus
- ECDSA signatures for Ethereum compatibility
- Secure key generation and storage

### Network Security
- TLS encryption for P2P communication
- Rate limiting and DoS protection
- Input validation and sanitization

## Monitoring and Observability

### Structured Logging
```rust
use tracing::{info, warn, error};

info!(block_height = 123, "Block processed successfully");
```

### Metrics Collection
- Prometheus-compatible metrics
- Custom business metrics
- Performance monitoring

### Health Checks
- Component health endpoints
- Automatic recovery mechanisms
- Alerting and notifications

## Maintenance

### Dependency Management
```bash
# Check for updates
cargo outdated

# Update dependencies
cargo update

# Security audit
cargo audit
```

### Performance Monitoring
```bash
# Run benchmarks
make bench

# Profile application
perf record target/release/multivm-node
```

This professional structure ensures:
- **Maintainability** through clear organization
- **Scalability** through modular design
- **Reliability** through comprehensive testing
- **Security** through defense-in-depth
- **Observability** through monitoring and logging