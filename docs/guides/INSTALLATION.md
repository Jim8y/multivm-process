# Installation Guide

This guide covers the installation and setup of the MultiVM Process system across different platforms.

## Table of Contents

- [System Requirements](#system-requirements)
- [Platform-Specific Installation](#platform-specific-installation)
- [Dependency Installation](#dependency-installation)
- [Building from Source](#building-from-source)
- [Docker Installation](#docker-installation)
- [Verification](#verification)
- [Troubleshooting](#troubleshooting)

## System Requirements

### Minimum Requirements
- **CPU**: 4 cores (x86_64 or ARM64)
- **RAM**: 8GB
- **Storage**: 100GB SSD
- **OS**: Linux, macOS, or Windows (WSL2)
- **Rust**: 1.70 or later

### Recommended for Production
- **CPU**: 8+ cores (x86_64)
- **RAM**: 18GB+
- **Storage**: 600GB+ NVMe SSD
- **OS**: Linux (Ubuntu 22.04+ or similar)
- **Network**: High-bandwidth, low-latency connection

## Platform-Specific Installation

### Ubuntu/Debian Linux

```bash
# Update system packages
sudo apt update && sudo apt upgrade -y

# Install essential build tools
sudo apt install -y build-essential curl git pkg-config libssl-dev

# Install additional dependencies
sudo apt install -y clang libclang-dev libudev-dev protobuf-compiler

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify installation
rustc --version
cargo --version
```

### CentOS/RHEL/Fedora

```bash
# Update system packages
sudo dnf update -y  # or sudo yum update -y for older versions

# Install development tools
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y curl git pkg-config openssl-devel

# Install additional dependencies
sudo dnf install -y clang clang-devel systemd-devel protobuf-compiler

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### macOS

```bash
# Install Xcode command line tools
xcode-select --install

# Install Homebrew (if not already installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install dependencies
brew install git pkg-config openssl protobuf

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Set environment variables for OpenSSL (if needed)
export PKG_CONFIG_PATH="/usr/local/opt/openssl/lib/pkgconfig"
export OPENSSL_DIR="/usr/local/opt/openssl"
```

### Windows (WSL2)

```bash
# Install WSL2 Ubuntu from Microsoft Store
# Open WSL2 terminal and follow Ubuntu instructions above

# Additional Windows-specific setup
sudo apt install -y gcc-mingw-w64

# Set up Rust for cross-compilation (optional)
rustup target add x86_64-pc-windows-gnu
```

## Dependency Installation

### Core Dependencies

The MultiVM Process system requires several core dependencies:

```bash
# Rust toolchain components
rustup component add rustfmt clippy

# Install cargo tools
cargo install cargo-audit cargo-outdated

# Platform-specific tools (Linux)
sudo apt install -y htop iotop nethogs  # monitoring tools
```

### Optional Dependencies

For full functionality and development:

```bash
# Docker (for containerized deployment)
curl -fsSL https://get.docker.com -o get-docker.sh
sh get-docker.sh

# Docker Compose
sudo apt install -y docker-compose-plugin

# Development tools
cargo install cargo-watch cargo-tarpaulin
```

## Building from Source

### Quick Build

```bash
# Clone the repository
git clone https://github.com/vm-multiverse/multivm.git
cd multivm-process

# Build in release mode
cargo build --release

# Run tests to verify build
cargo test --release
```

### Development Build

```bash
# Build with development optimizations
cargo build

# Run with debug logging
RUST_LOG=debug cargo run

# Watch for changes during development
cargo install cargo-watch
cargo watch -x check -x test
```

### Feature Selection

The system supports several feature flags:

```bash
# Build with specific features
cargo build --release --features "full"

# Available features:
# - default: Core functionality
# - metrics: Enable metrics collection
# - docker: Docker integration support
# - dev-tools: Development and debugging tools
```

## Docker Installation

### Using Pre-built Images

```bash
# Pull the latest image
docker pull your-org/multivm-process:latest

# Run with default configuration
docker run -d \
  --name multivm-process \
  -p 8080:8080 \
  -p 26657:26657 \
  your-org/multivm-process:latest
```

### Building Docker Image

```bash
# Build from source
docker build -t multivm-process .

# Build with specific features
docker build --build-arg FEATURES="metrics,docker" -t multivm-process .
```

### Docker Compose Setup

```bash
# Copy example configuration
cp docker-compose.yml.example docker-compose.yml

# Edit configuration as needed
vim docker-compose.yml

# Start services
docker-compose up -d

# Check status
docker-compose ps
docker-compose logs -f
```

## Verification

### Basic Functionality Test

```bash
# Run the verification script
./validate_core_functionality.sh

# Expected output:
# ✅ multivm-common compiles successfully
# ✅ multivm-account-mapping compiles successfully  
# ✅ multivm-p2p compiles successfully
# 🎉 SUCCESS: Core MultiVM architecture compiles and is functional!
```

### Full System Test

```bash
# Run comprehensive tests
make test

# Run specific test suites
cargo test -p multivm-common
cargo test -p multivm-account-mapping
cargo test -p multivm-p2p

# Run integration tests (requires full setup)
cargo test --test integration_tests
```

### Performance Verification

```bash
# Run performance benchmarks
cargo bench

# System performance test
./scripts/benchmark_system.sh
```

## Post-Installation Configuration

### Environment Variables

Create a `.env` file for local configuration:

```bash
# Core settings
RUST_LOG=info
MULTIVM_CONFIG_PATH=./config/multivm.toml

# Consensus settings
CONSENSUS_VALIDATOR_ID=validator-0
CONSENSUS_LISTEN_ADDR=127.0.0.1:26657

# Security settings
ENABLE_AUTHENTICATION=true
ENABLE_ENCRYPTION=false

# Performance settings
MAX_CONCURRENT_BLOCKS=10
BLOCK_TIMEOUT_MS=60000
```

### Configuration File

Copy and customize the example configuration:

```bash
# Copy example configuration
cp config.example.toml config/multivm.toml

# Edit configuration
vim config/multivm.toml
```

## Troubleshooting

### Common Issues

#### Rust Compilation Errors

```bash
# Update Rust toolchain
rustup update stable

# Clear cargo cache
cargo clean

# Rebuild with fresh dependencies
cargo build --release
```

#### Missing System Dependencies

```bash
# Ubuntu/Debian: Install missing packages
sudo apt install -y build-essential pkg-config libssl-dev

# macOS: Update Xcode tools
xcode-select --install
brew doctor
```

#### Permission Issues

```bash
# Fix cargo permissions
sudo chown -R $USER:$USER ~/.cargo

# Fix Docker permissions (Linux)
sudo usermod -aG docker $USER
newgrp docker
```

#### Memory Issues During Build

```bash
# Limit parallel jobs
cargo build --release -j 2

# Or set environment variable
export CARGO_BUILD_JOBS=2
```

### Performance Issues

#### Slow Compilation

```bash
# Use faster linker (Linux)
sudo apt install -y lld
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"

# Enable incremental compilation
export CARGO_INCREMENTAL=1
```

#### Runtime Performance

```bash
# Ensure release build
cargo build --release

# Check system resources
htop
iostat -x 1
```

### Getting Help

If you encounter issues not covered here:

1. **Check Logs**: Look at system logs and application output
2. **Search Issues**: Check [GitHub Issues](https://github.com/vm-multiverse/multivm/issues)
3. **Documentation**: Review the [Documentation Index](DOCUMENTATION_INDEX.md)
4. **Community**: Join discussions in [GitHub Discussions](https://github.com/vm-multiverse/multivm/discussions)

### Reporting Issues

When reporting installation issues, please include:

- Operating system and version
- Rust version (`rustc --version`)
- Full error messages and logs
- Steps to reproduce the issue
- Hardware specifications

---

Next: [Quick Start Guide](QUICK_START.md) | [Configuration Reference](CONFIGURATION.md)