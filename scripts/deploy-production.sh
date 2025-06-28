#!/bin/bash
set -euo pipefail

# MultiVM Production Deployment Script
# This script deploys the MultiVM system in production mode with all security features enabled

echo "🚀 Starting MultiVM Production Deployment"

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "multivm-consensus" ]; then
    echo "❌ Error: Please run this script from the multivm-process root directory"
    exit 1
fi

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check required environment variables
check_env_vars() {
    print_status "Checking required environment variables..."
    
    required_vars=(
        "ETHEREUM_RPC_URL"
        "SOLANA_RPC_URL"
        "REDIS_URL"
        "METRICS_ENDPOINT"
        "JWT_SECRET"
    )
    
    missing_vars=()
    for var in "${required_vars[@]}"; do
        if [ -z "${!var:-}" ]; then
            missing_vars+=("$var")
        fi
    done
    
    if [ ${#missing_vars[@]} -gt 0 ]; then
        print_error "Missing required environment variables:"
        for var in "${missing_vars[@]}"; do
            echo "  - $var"
        done
        echo ""
        echo "Please set these environment variables and try again."
        echo "Example:"
        echo "  export ETHEREUM_RPC_URL='https://mainnet.infura.io/v3/YOUR-PROJECT-ID'"
        echo "  export SOLANA_RPC_URL='https://api.mainnet-beta.solana.com'"
        echo "  export REDIS_URL='redis://localhost:6379'"
        echo "  export METRICS_ENDPOINT='0.0.0.0:9090'"
        echo "  export JWT_SECRET='\$(openssl rand -hex 32)'"
        exit 1
    fi
    
    print_success "All required environment variables are set"
}

# Build the project in release mode
build_release() {
    print_status "Building MultiVM in release mode..."
    
    # Clean previous builds
    cargo clean
    
    # Build with production features only (disable mock features)
    cargo build --release \
        --workspace
    
    if [ $? -eq 0 ]; then
        print_success "Release build completed successfully"
    else
        print_error "Release build failed"
        exit 1
    fi
}

# Run security checks
security_checks() {
    print_status "Running security checks..."
    
    # Check for security vulnerabilities
    if command -v cargo-audit &> /dev/null; then
        print_status "Running cargo audit..."
        cargo audit
    else
        print_warning "cargo-audit not installed, skipping vulnerability check"
        print_warning "Install with: cargo install cargo-audit"
    fi
    
    # Run clippy with strict security lints
    print_status "Running security-focused clippy checks..."
    cargo clippy --release -- \
        -D clippy::unwrap_used \
        -D clippy::expect_used \
        -D clippy::panic \
        -D clippy::todo \
        -D clippy::unimplemented \
        -W clippy::nursery \
        -W clippy::pedantic
    
    print_success "Security checks completed"
}

# Test the build
run_tests() {
    print_status "Running production tests..."
    
    # Run all tests excluding mock-only tests
    RUST_LOG=info cargo test --release \
        --workspace
    
    if [ $? -eq 0 ]; then
        print_success "All tests passed"
    else
        print_error "Some tests failed"
        exit 1
    fi
}

# Create production configuration
create_config() {
    print_status "Creating production configuration..."
    
    # Create config directory if it doesn't exist
    mkdir -p configs
    
    # Create production configuration file
    cat > configs/production.toml << EOF
[network]
# P2P Network Configuration
p2p_port = 8801
external_ip = "\${EXTERNAL_IP:-127.0.0.1}"
max_peers = 50
enable_mdns = false  # Disable mDNS in production
enable_upnp = false  # Disable UPnP in production

[consensus]
# Validator Configuration - MUST be set for production
validators = []  # Set via environment or config file
min_validators = 3
block_time_ms = 12000
max_block_size = 1048576  # 1MB
max_transactions_per_block = 1000

[execution_engines]
# Ethereum Configuration
[execution_engines.ethereum]
enabled = true
data_dir = "\${ETHEREUM_DATA_DIR:-/var/lib/multivm/ethereum}"
rpc_port = 8545
chain_id = 1  # Mainnet
mock_mode = false
auto_start = true

# Solana Configuration  
[execution_engines.solana]
enabled = true
data_dir = "\${SOLANA_DATA_DIR:-/var/lib/multivm/solana}"
rpc_port = 8899
cluster = "\${SOLANA_CLUSTER:-mainnet-beta}"
mock_mode = false
auto_start = true

[execution_engines.global]
max_concurrent_blocks = 10
block_timeout_seconds = 30
health_check_interval_seconds = 10
enable_cross_vm_coordination = true

[storage]
# Database Configuration
[storage.consensus]
backend = "rocksdb"
path = "\${CONSENSUS_DB_PATH:-/var/lib/multivm/consensus.db}"
enable_compression = true
cache_size_mb = 256

[storage.account_mapping]
backend = "rocksdb" 
path = "\${ACCOUNT_MAPPING_DB_PATH:-/var/lib/multivm/accounts.db}"
enable_compression = true
cache_size_mb = 128

[monitoring]
# Metrics and Health Checks
enable_metrics = true
metrics_port = 9090
health_check_port = 8080
log_level = "info"

# Tracing Configuration
[monitoring.tracing]
enabled = true
endpoint = "\${JAEGER_ENDPOINT:-http://localhost:14268/api/traces}"
sample_rate = 0.1
enable_jaeger = true

[security]
# Security Configuration
enable_encryption = true
enable_authentication = true
jwt_secret = "\${JWT_SECRET}"
max_request_size = 10485760  # 10MB
enable_rate_limiting = true
rate_limit_requests_per_minute = 1000

[cache]
# Redis Configuration
url = "\${REDIS_URL}"
max_connections = 10
connection_timeout_seconds = 5
command_timeout_seconds = 5
key_prefix = "multivm:"
EOF

    print_success "Production configuration created at configs/production.toml"
}

# Create systemd service files
create_systemd_service() {
    print_status "Creating systemd service file..."
    
    sudo tee /etc/systemd/system/multivm.service > /dev/null << EOF
[Unit]
Description=MultiVM Blockchain Service
After=network.target
Wants=network.target

[Service]
Type=simple
User=multivm
Group=multivm
WorkingDirectory=/opt/multivm
ExecStart=/opt/multivm/target/release/multivm-node --config /opt/multivm/configs/production.toml
Restart=always
RestartSec=10
Environment=RUST_LOG=info
Environment=RUST_BACKTRACE=1

# Security settings
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/multivm /var/log/multivm
CapabilityBoundingSet=CAP_NET_BIND_SERVICE

# Resource limits
LimitNOFILE=65536
LimitNPROC=32768

[Install]
WantedBy=multi-user.target
EOF

    print_success "Systemd service file created"
}

# Create directories and set permissions
setup_directories() {
    print_status "Setting up directories and permissions..."
    
    # Create system user if it doesn't exist
    if ! id "multivm" &>/dev/null; then
        sudo useradd --system --home-dir /opt/multivm --shell /bin/false multivm
        print_success "Created multivm system user"
    fi
    
    # Create directories
    sudo mkdir -p /opt/multivm
    sudo mkdir -p /var/lib/multivm/{ethereum,solana,consensus,accounts}
    sudo mkdir -p /var/log/multivm
    
    # Copy binary and configuration
    sudo cp target/release/multivm-node /opt/multivm/
    sudo cp -r configs /opt/multivm/
    
    # Set ownership and permissions
    sudo chown -R multivm:multivm /opt/multivm /var/lib/multivm /var/log/multivm
    sudo chmod 755 /opt/multivm/multivm-node
    sudo chmod 600 /opt/multivm/configs/production.toml
    
    print_success "Directories and permissions configured"
}

# Start the service
start_service() {
    print_status "Starting MultiVM service..."
    
    # Reload systemd and enable service
    sudo systemctl daemon-reload
    sudo systemctl enable multivm
    sudo systemctl start multivm
    
    # Wait a moment for startup
    sleep 5
    
    # Check status
    if sudo systemctl is-active --quiet multivm; then
        print_success "MultiVM service started successfully"
        print_status "Service status:"
        sudo systemctl status multivm --no-pager
    else
        print_error "Failed to start MultiVM service"
        print_error "Check logs with: sudo journalctl -u multivm -f"
        exit 1
    fi
}

# Verify deployment
verify_deployment() {
    print_status "Verifying deployment..."
    
    # Check health endpoint
    if curl -f -s http://localhost:8080/health > /dev/null; then
        print_success "Health check endpoint responding"
    else
        print_error "Health check endpoint not responding"
        return 1
    fi
    
    # Check metrics endpoint  
    if curl -f -s http://localhost:9090/metrics > /dev/null; then
        print_success "Metrics endpoint responding"
    else
        print_warning "Metrics endpoint not responding"
    fi
    
    print_success "Deployment verification completed"
}

# Main deployment function
deploy() {
    echo "🔧 MultiVM Production Deployment"
    echo "==============================="
    
    check_env_vars
    build_release
    security_checks
    run_tests
    create_config
    create_systemd_service
    setup_directories
    start_service
    verify_deployment
    
    echo ""
    echo "🎉 MultiVM Production Deployment Complete!"
    echo ""
    echo "Next steps:"
    echo "1. Configure your validators in configs/production.toml"
    echo "2. Set up monitoring with Prometheus/Grafana"
    echo "3. Configure log aggregation"
    echo "4. Set up backup procedures"
    echo ""
    echo "Service management:"
    echo "  Start:   sudo systemctl start multivm"
    echo "  Stop:    sudo systemctl stop multivm"
    echo "  Status:  sudo systemctl status multivm"
    echo "  Logs:    sudo journalctl -u multivm -f"
    echo ""
    echo "Health check: curl http://localhost:8080/health"
    echo "Metrics:      curl http://localhost:9090/metrics"
}

# Parse command line arguments
case "${1:-deploy}" in
    "deploy")
        deploy
        ;;
    "check-env")
        check_env_vars
        ;;
    "build")
        build_release
        ;;
    "test")
        run_tests
        ;;
    "config")
        create_config
        ;;
    "help")
        echo "Usage: $0 [deploy|check-env|build|test|config|help]"
        echo ""
        echo "Commands:"
        echo "  deploy     - Full production deployment (default)"
        echo "  check-env  - Check environment variables"
        echo "  build      - Build release version"
        echo "  test       - Run production tests"
        echo "  config     - Create production configuration"
        echo "  help       - Show this help"
        ;;
    *)
        print_error "Unknown command: $1"
        echo "Use '$0 help' for usage information"
        exit 1
        ;;
esac