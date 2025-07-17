# MultiVM Scripts

This directory contains utility scripts for building, testing, deploying, and managing the MultiVM blockchain platform.

## 📁 Script Categories

### 🚀 Quick Start
- **`quickstart.sh`** - One-command setup for local development
- **`start-testnet.sh`** - Start a local testnet

### 🏗️ Setup & Configuration
- **`setup-validators.sh`** - Configure multi-validator network
- **`setup-reth-node.sh`** - Setup Reth execution engine
- **`generate-jwt-token.sh`** - Generate JWT tokens for Engine API

### 🧪 Testing
- **`test-suite.sh`** - Comprehensive test runner
- **`test-consensus-features.sh`** - Consensus-specific tests
- **`test-multivm-reth-integration.sh`** - Integration tests with Reth
- **`run-integration-tests.sh`** - Full integration test suite
- **`run-performance-tests.sh`** - Performance benchmarks
- **`run-consensus-scenarios.sh`** - Consensus scenario tests

### 📊 Monitoring & Administration
- **`dashboard.sh`** - Real-time monitoring dashboard
- **`monitor-testnet.sh`** - Testnet monitoring
- **`validator-admin.sh`** - Validator management tools
- **`p2p-admin.sh`** - P2P network administration
- **`check-health.sh`** - Health check utilities

### 🚢 Deployment
- **`deploy-production.sh`** - Production deployment script

### 🔍 Verification
- **`verify-testnet.sh`** - Verify testnet setup
- **`verify-testnet-config.sh`** - Validate configurations
- **`verify-reth-isolation.sh`** - Ensure Reth isolation
- **`run-checks.sh`** - Pre-deployment checks

## 🎯 Common Workflows

### Local Development Setup
```bash
# Quick start for development
./quickstart.sh

# Or manual setup
./setup-validators.sh 3        # Setup 3 validators
./start-testnet.sh             # Start the testnet
./dashboard.sh                 # Monitor in real-time
```

### Running Tests
```bash
# Run all tests
./test-suite.sh all

# Run specific test categories
./test-suite.sh unit          # Unit tests only
./test-suite.sh integration   # Integration tests
./test-suite.sh consensus     # Consensus tests
./test-suite.sh performance   # Performance tests
```

### Production Deployment
```bash
# Pre-deployment checks
./run-checks.sh

# Deploy to production
./deploy-production.sh --config production.toml
```

## 🔧 Script Details

### quickstart.sh
One-command setup for developers:
- Checks dependencies
- Builds the project
- Generates validator keys
- Starts a 3-node testnet
- Opens monitoring dashboard

### start-testnet.sh
Starts a configurable testnet:
```bash
./start-testnet.sh [OPTIONS]
  --nodes N        Number of validator nodes (default: 3)
  --port P         Base port number (default: 8080)
  --reset          Clean existing data before start
  --background     Run in background
```

### test-suite.sh
Comprehensive testing framework:
```bash
./test-suite.sh [CATEGORY] [OPTIONS]
  all              Run all tests
  unit             Unit tests only
  integration      Integration tests
  consensus        Consensus tests
  performance      Performance benchmarks
  --verbose        Show detailed output
  --coverage       Generate coverage report
```

### dashboard.sh
Real-time monitoring with:
- Block height and consensus round
- Transaction pool status
- Validator health
- Network topology
- Performance metrics

### validator-admin.sh
Validator management:
```bash
./validator-admin.sh [COMMAND]
  status           Show validator status
  start NODE_ID    Start specific validator
  stop NODE_ID     Stop specific validator
  restart NODE_ID  Restart validator
  logs NODE_ID     Show validator logs
  rotate-leader    Trigger view change
```

## 📋 Environment Variables

Scripts respect these environment variables:
- `MULTIVM_HOME` - MultiVM installation directory
- `MULTIVM_DATA` - Data directory (default: /tmp/multivm)
- `MULTIVM_LOG_LEVEL` - Logging level (trace/debug/info/warn/error)
- `MULTIVM_CONFIG` - Custom config file path

## ⚙️ Requirements

- **Rust** 1.75+ (for building)
- **bash** 4.0+
- **jq** - JSON processing
- **curl** - HTTP requests
- **docker** - Container runtime (optional)
- **python3** - For some test scripts

## 🐛 Troubleshooting

### Port Already in Use
Scripts automatically detect and offer to clean up conflicting processes.

### Permission Denied
Some scripts may need elevated permissions:
```bash
sudo ./script-name.sh
```

### Missing Dependencies
Install required tools:
```bash
# Ubuntu/Debian
sudo apt-get install jq curl netcat

# macOS
brew install jq curl netcat
```

## 🤝 Contributing

When adding new scripts:
1. Use consistent naming (kebab-case)
2. Add proper documentation headers
3. Include in this README
4. Add error handling and validation
5. Follow existing patterns for output formatting