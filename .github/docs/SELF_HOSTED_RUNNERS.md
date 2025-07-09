# Self-Hosted GitHub Actions Runners

This document provides instructions for setting up and configuring self-hosted GitHub Actions runners for the MultiVM project.

## Prerequisites

### System Requirements
- **Operating System**: Ubuntu 20.04+, CentOS 7+, Fedora 30+, or macOS 10.15+
- **CPU**: Minimum 4 cores, recommended 8+ cores
- **Memory**: Minimum 8GB RAM, recommended 16GB+
- **Storage**: Minimum 50GB free space, recommended 100GB+
- **Network**: Stable internet connection

### Required Software

#### For Ubuntu/Debian:
```bash
# Update system
sudo apt-get update && sudo apt-get upgrade -y

# Install required packages
sudo apt-get install -y \
    curl \
    git \
    build-essential \
    pkg-config \
    libudev-dev \
    protobuf-compiler \
    clang \
    lld \
    cmake \
    libssl-dev

# Install Docker (optional, for containerized builds)
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER
```

#### For CentOS/RHEL:
```bash
# Install required packages
sudo yum install -y \
    curl \
    git \
    gcc \
    gcc-c++ \
    pkgconfig \
    libudev-devel \
    protobuf-compiler \
    clang \
    lld \
    cmake \
    openssl-devel

# Or for newer versions with dnf:
sudo dnf install -y \
    curl \
    git \
    gcc \
    gcc-c++ \
    pkgconfig \
    libudev-devel \
    protobuf-compiler \
    clang \
    lld \
    cmake \
    openssl-devel
```

#### For macOS:
```bash
# Install Homebrew if not already installed
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install required packages
brew install \
    git \
    pkg-config \
    protobuf \
    llvm \
    cmake \
    openssl
```

## Installing Rust

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install the specific nightly toolchain used by the project
rustup install nightly-2025-06-15
rustup default nightly-2025-06-15

# Add required components
rustup component add rustfmt clippy
```

## Setting up the GitHub Actions Runner

### 1. Download and Configure the Runner

```bash
# Create a directory for the runner
mkdir -p ~/github-runner
cd ~/github-runner

# Download the runner (replace with latest version)
curl -o actions-runner-linux-x64-2.311.0.tar.gz -L \
    https://github.com/actions/runner/releases/download/v2.311.0/actions-runner-linux-x64-2.311.0.tar.gz

# Extract the runner
tar xzf ./actions-runner-linux-x64-2.311.0.tar.gz
```

### 2. Configure the Runner

1. Go to your repository's Settings > Actions > Runners
2. Click "New self-hosted runner"
3. Follow the instructions to get the registration token
4. Configure the runner:

```bash
# Configure the runner (use the token from GitHub)
./config.sh --url https://github.com/vm-multiverse/multivm --token YOUR_TOKEN_HERE

# When prompted:
# - Enter a name for the runner (e.g., "multivm-runner-1")
# - Enter a runner group (press Enter for default)
# - Enter labels (optional, e.g., "self-hosted,linux,rust")
# - Enter work folder (press Enter for default)
```

### 3. Install the Runner as a Service

#### On Linux (systemd):
```bash
# Install the service
sudo ./svc.sh install

# Start the service
sudo ./svc.sh start

# Check status
sudo ./svc.sh status
```

#### On macOS (launchd):
```bash
# Install the service
./svc.sh install

# Start the service
./svc.sh start

# Check status
./svc.sh status
```

## Configuration for MultiVM Project

### Environment Variables

Create a `.env` file in the runner's work directory:

```bash
# Performance optimizations
export CARGO_TERM_COLOR=always
export RUST_BACKTRACE=1
export CARGO_INCREMENTAL=1
export RUST_LOG=off
export RUSTFLAGS="-C link-arg=-fuse-ld=lld"
export CARGO_NET_RETRY=10
export CARGO_NET_TIMEOUT=60
export CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse
export RUST_MIN_STACK=16777216

# Use all available cores
export CARGO_BUILD_JOBS=0
```

### Storage Optimization

Create a script to manage disk space:

```bash
#!/bin/bash
# ~/github-runner/cleanup.sh

# Clean old build artifacts
find ~/github-runner/_work -name "target" -type d -mtime +7 -exec rm -rf {} + 2>/dev/null || true

# Clean cargo cache if it's too large (>10GB)
if [ -d ~/.cargo ]; then
    CARGO_SIZE=$(du -s ~/.cargo 2>/dev/null | cut -f1)
    if [ "$CARGO_SIZE" -gt 10485760 ]; then  # 10GB in KB
        echo "Cleaning cargo cache..."
        rm -rf ~/.cargo/registry/cache/*
        rm -rf ~/.cargo/git/db/*
    fi
fi

# Clean old log files
find ~/github-runner/_diag -name "*.log" -mtime +30 -delete 2>/dev/null || true
```

Make it executable and add to cron:
```bash
chmod +x ~/github-runner/cleanup.sh
(crontab -l 2>/dev/null; echo "0 2 * * * ~/github-runner/cleanup.sh") | crontab -
```

## Security Considerations

### 1. SSH Key Management

The workflows use SSH keys for accessing private repositories. Ensure:

- SSH keys are properly configured in GitHub repository secrets
- Keys have minimal required permissions
- Keys are regularly rotated

### 2. Runner Security

- Run the runner as a non-root user
- Regularly update the runner software
- Monitor runner logs for suspicious activity
- Use firewall rules to restrict network access

### 3. Network Security

```bash
# Example firewall rules (adjust as needed)
sudo ufw allow out 443/tcp  # HTTPS
sudo ufw allow out 22/tcp   # SSH
sudo ufw allow out 9418/tcp # Git protocol
```

## Monitoring and Maintenance

### 1. Runner Health Check

Create a health check script:

```bash
#!/bin/bash
# ~/github-runner/health-check.sh

# Check if runner service is running
if ! systemctl is-active --quiet actions.runner.vm-multiverse-multivm.*.service; then
    echo "Runner service is not running"
    exit 1
fi

# Check disk space
DISK_USAGE=$(df -h . | tail -1 | awk '{print $5}' | sed 's/%//')
if [ "$DISK_USAGE" -gt 85 ]; then
    echo "Disk usage is high: ${DISK_USAGE}%"
    exit 1
fi

# Check memory usage
MEM_USAGE=$(free | grep Mem | awk '{printf "%.0f", $3/$2 * 100.0}')
if [ "$MEM_USAGE" -gt 90 ]; then
    echo "Memory usage is high: ${MEM_USAGE}%"
    exit 1
fi

echo "Runner health check passed"
```

### 2. Log Rotation

Configure log rotation for runner logs:

```bash
# /etc/logrotate.d/github-runner
/home/runner/github-runner/_diag/*.log {
    daily
    rotate 30
    compress
    delaycompress
    missingok
    notifempty
    create 644 runner runner
}
```

## Troubleshooting

### Common Issues

1. **Runner Not Starting**
   - Check service status: `sudo systemctl status actions.runner.*`
   - Check logs: `journalctl -u actions.runner.* -f`

2. **Build Failures**
   - Ensure all dependencies are installed
   - Check disk space and memory usage
   - Verify Rust toolchain version

3. **SSH Authentication Issues**
   - Verify SSH key is properly configured
   - Check SSH agent is running
   - Ensure known_hosts includes github.com

4. **Performance Issues**
   - Monitor CPU and memory usage during builds
   - Adjust `CARGO_BUILD_JOBS` based on available cores
   - Consider using SSD storage for better I/O performance

### Debug Mode

To run the runner in debug mode:

```bash
# Stop the service
sudo ./svc.sh stop

# Run in foreground with debug output
./run.sh --debug
```

## Scaling

### Multiple Runners

For high-throughput projects, consider running multiple runners:

1. Set up multiple runner instances on the same machine
2. Use different work directories for each runner
3. Configure appropriate resource limits

### Auto-scaling

For dynamic scaling based on demand:

1. Use cloud provider auto-scaling groups
2. Configure runners to self-register and deregister
3. Monitor queue length and scale accordingly

## Updates

### Updating the Runner

```bash
# Stop the service
sudo ./svc.sh stop

# Download new version
curl -o actions-runner-linux-x64-2.311.0.tar.gz -L \
    https://github.com/actions/runner/releases/download/v2.311.0/actions-runner-linux-x64-2.311.0.tar.gz

# Extract over existing installation
tar xzf ./actions-runner-linux-x64-2.311.0.tar.gz

# Start the service
sudo ./svc.sh start
```

### Updating Dependencies

```bash
# Update system packages
sudo apt-get update && sudo apt-get upgrade -y

# Update Rust toolchain
rustup update nightly-2025-06-15

# Update other tools as needed
```

## Support

For issues specific to self-hosted runners:

1. Check the [GitHub Actions documentation](https://docs.github.com/en/actions/hosting-your-own-runners)
2. Review runner logs in `_diag` directory
3. Contact the development team with specific error messages

Remember to regularly monitor and maintain your self-hosted runners to ensure optimal performance and security.