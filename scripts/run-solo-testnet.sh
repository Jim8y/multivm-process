#!/bin/bash
set -euo pipefail

# MultiVM Solo Node Testnet Runner
# This script sets up and runs a single-node test network for development and testing

echo "🚀 Starting MultiVM Solo Node Testnet"
echo "===================================="

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

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ] || [ ! -d "multivm-consensus" ]; then
    echo "❌ Error: Please run this script from the multivm-process root directory"
    exit 1
fi

# Configuration
TESTNET_DIR="${TESTNET_DIR:-/tmp/multivm-testnet}"
LOG_DIR="${LOG_DIR:-$TESTNET_DIR/logs}"
DATA_DIR="${DATA_DIR:-$TESTNET_DIR/data}"
CONFIG_DIR="${CONFIG_DIR:-$TESTNET_DIR/config}"

# Ports
P2P_PORT="${P2P_PORT:-8801}"
CONSENSUS_PORT="${CONSENSUS_PORT:-8802}"
ETHEREUM_RPC_PORT="${ETHEREUM_RPC_PORT:-8545}"
SOLANA_RPC_PORT="${SOLANA_RPC_PORT:-8899}"
METRICS_PORT="${METRICS_PORT:-9090}"
HEALTH_PORT="${HEALTH_PORT:-8090}"
REST_API_PORT="${REST_API_PORT:-8080}"
GRAPHQL_PORT="${GRAPHQL_PORT:-8081}"
WS_PORT="${WS_PORT:-8082}"
ADMIN_PORT="${ADMIN_PORT:-8083}"

# Clean up function
cleanup() {
    print_status "Cleaning up..."
    
    # Kill any running processes
    if [ -f "$TESTNET_DIR/multivm.pid" ]; then
        PID=$(cat "$TESTNET_DIR/multivm.pid")
        if kill -0 $PID 2>/dev/null; then
            print_status "Stopping MultiVM process (PID: $PID)..."
            kill -TERM $PID || true
            sleep 2
            kill -KILL $PID 2>/dev/null || true
        fi
        rm -f "$TESTNET_DIR/multivm.pid"
    fi
    
    # Clean up any background processes
    pkill -f "multivm-node" || true
    pkill -f "solana-validator" || true
    pkill -f "reth" || true
}

# Set up trap for cleanup
trap cleanup EXIT INT TERM

# Initialize testnet directories
init_directories() {
    print_status "Initializing testnet directories..."
    
    # Clean existing data if requested
    if [ "${CLEAN_START:-false}" = "true" ]; then
        print_warning "Cleaning existing testnet data..."
        rm -rf "$TESTNET_DIR"
    fi
    
    # Create directories
    mkdir -p "$LOG_DIR"
    mkdir -p "$DATA_DIR"/{consensus,ethereum,solana,account-mapping}
    mkdir -p "$CONFIG_DIR"
    
    print_success "Directories initialized at $TESTNET_DIR"
}

# Generate validator keypair for solo node
generate_validator_keys() {
    print_status "Generating validator keys..."
    
    # For a solo testnet, we'll use a simple validator identity
    VALIDATOR_ID="solo-validator-$(date +%s)"
    VALIDATOR_PUBKEY="ed25519:$(openssl rand -hex 32)"
    
    # Save validator info
    cat > "$CONFIG_DIR/validator.json" << EOF
{
    "id": "$VALIDATOR_ID",
    "public_key": "$VALIDATOR_PUBKEY",
    "voting_power": 100,
    "address": "127.0.0.1:$CONSENSUS_PORT"
}
EOF
    
    print_success "Validator keys generated"
}

# Create testnet configuration
create_testnet_config() {
    print_status "Creating testnet configuration..."
    
    # Set test environment variables
    export ETHEREUM_RPC_URL="http://localhost:$ETHEREUM_RPC_PORT"
    export SOLANA_RPC_URL="http://localhost:$SOLANA_RPC_PORT"
    export REDIS_URL="redis://localhost:6379"
    export METRICS_ENDPOINT="0.0.0.0:$METRICS_PORT"
    export JAEGER_ENDPOINT="http://localhost:14268/api/traces"
    export JWT_SECRET="$(openssl rand -hex 32)"
    export EXTERNAL_IP="127.0.0.1"
    
    # Save environment for reuse
    cat > "$CONFIG_DIR/testnet.env" << EOF
export ETHEREUM_RPC_URL="$ETHEREUM_RPC_URL"
export SOLANA_RPC_URL="$SOLANA_RPC_URL"
export REDIS_URL="$REDIS_URL"
export METRICS_ENDPOINT="$METRICS_ENDPOINT"
export JAEGER_ENDPOINT="$JAEGER_ENDPOINT"
export JWT_SECRET="$JWT_SECRET"
export EXTERNAL_IP="$EXTERNAL_IP"
EOF
    
    # Create testnet configuration file
    cat > "$CONFIG_DIR/testnet.toml" << EOF
# MultiVM Solo Node Testnet Configuration

# System-wide settings
[system]
data_dir = "$DATA_DIR"
log_level = "debug"  # More verbose for testing
enable_metrics = true
enable_tracing = false  # Disabled for simple testnet
max_processes = 20
shutdown_timeout_seconds = 30

# Server configuration for all HTTP/WebSocket endpoints
[server]

[server.rest]
host = "0.0.0.0"
port = $REST_API_PORT
enable_cors = true  # Enable for testnet
request_timeout_seconds = 30
max_request_size_bytes = 16777216  # 16MB

[server.graphql]
host = "0.0.0.0"
port = $GRAPHQL_PORT
enable_playground = true  # Enable for testnet
query_timeout_seconds = 30
query_complexity_limit = 1000

[server.websocket]
host = "0.0.0.0"
port = $WS_PORT
connection_timeout_seconds = 60
max_connections = 100

[server.admin]
host = "127.0.0.1"
port = $ADMIN_PORT
enable_ui = true  # Enable for testnet
require_auth = false  # Disable for testnet

# Database configuration
[database]
connection_url = "postgresql://multivm:multivm@localhost:5432/multivm_testnet"
max_connections = 50  # Lower for testnet
min_connections = 5
connection_timeout_seconds = 30
enable_ssl = false  # Disable for testnet

# Cache configuration
[cache]
strategy = "WriteThrough"
default_ttl_seconds = 300
max_memory_bytes = 536870912  # 512MB for testnet

[cache.redis]
connection_url = "redis://localhost:6379/0"
key_prefix = "multivm:testnet:"
connection_timeout_seconds = 5
command_timeout_seconds = 10

[cache.memory]
max_items = 50000  # Lower for testnet
eviction_policy = "Lru"

# Blockchain clients configuration
[blockchain]

[blockchain.solana]
rpc_url = "http://localhost:$SOLANA_RPC_PORT"
timeout_seconds = 30
max_retries = 3
retry_backoff_seconds = 2
enable_health_checks = true

[blockchain.ethereum]
rpc_url = "http://localhost:$ETHEREUM_RPC_PORT"
timeout_seconds = 30
max_retries = 3
retry_backoff_seconds = 2
enable_health_checks = true

# Consensus mechanism configuration
[consensus]
algorithm = "malachite"
block_time_milliseconds = 5000  # 5 second blocks for testing
validator_count = 1  # Solo testnet
enable_single_node = true  # Enable for solo testnet
finality_depth = 3  # Lower for testnet

# Network P2P configuration
[network]
enable_p2p = false  # Disabled for solo testnet
listen_host = "127.0.0.1"
listen_port = $P2P_PORT
max_connections = 10
connection_timeout_seconds = 10

# Inter-process communication configuration
[ipc]
enable_encryption = false  # Disable for testnet
message_timeout = { secs = 30, nanos = 0 }  # Duration struct

[ipc.transport]
UnixSocket = { path = "$TESTNET_DIR/multivm.sock" }

# Security configuration
[security]
enable_authentication = false  # Disabled for testnet
enable_encryption = true
enable_rate_limiting = false  # Disabled for testnet
max_requests_per_minute = 1000
jwt_expiration_hours = 24
jwt_secret = "$JWT_SECRET"

# Monitoring and observability configuration
[monitoring]
enable_prometheus = true
prometheus_host = "0.0.0.0"
prometheus_port = $METRICS_PORT
enable_jaeger = false  # Disabled for simple testnet
jaeger_endpoint = "http://localhost:14268/api/traces"
service_name = "multivm-testnet"

[monitoring.health_check]
enable_endpoint = true
host = "0.0.0.0"
port = $HEALTH_PORT
path = "/health"

# Logging configuration
[logging]
level = "debug"  # More verbose for testing
format = "Pretty"  # Human-readable for testnet
enable_file_logging = true
log_directory = "$LOG_DIR"
max_file_size_bytes = 104857600  # 100MB
max_log_files = 5  # Lower for testnet
EOF
    
    print_success "Testnet configuration created"
}

# Build the project
build_multivm() {
    print_status "Building MultiVM..."
    
    # Check if binary already exists and is recent
    if [ -f "target/release/multivm-node" ]; then
        BINARY_AGE=$(($(date +%s) - $(stat -f%m "target/release/multivm-node" 2>/dev/null || stat -c%Y "target/release/multivm-node" 2>/dev/null || echo 0)))
        if [ $BINARY_AGE -lt 300 ]; then  # Less than 5 minutes old
            print_success "Using existing binary (built ${BINARY_AGE}s ago)"
            return 0
        fi
    fi
    
    # Build in release mode
    RUST_LOG=warn cargo build --release --all
    
    if [ $? -eq 0 ]; then
        print_success "Build completed successfully"
    else
        print_error "Build failed"
        exit 1
    fi
}

# Start background services (Redis alternative for testnet)
start_background_services() {
    print_status "Starting background services..."
    
    # For testnet, we'll skip Redis and use in-memory caching
    # If you want Redis, uncomment below:
    # if ! pgrep -x "redis-server" > /dev/null; then
    #     print_warning "Redis not running. Starting Redis..."
    #     redis-server --daemonize yes --dir "$DATA_DIR" --logfile "$LOG_DIR/redis.log"
    #     sleep 2
    # fi
    
    print_success "Background services ready"
}

# Start the MultiVM node
start_multivm_node() {
    print_status "Starting MultiVM solo node..."
    
    # Ensure we're using the testnet environment
    source "$CONFIG_DIR/testnet.env"
    
    # Start MultiVM with testnet configuration
    RUST_LOG=${RUST_LOG:-info,multivm=debug} \
    RUST_BACKTRACE=1 \
    ./target/release/multivm-node \
        --config "$CONFIG_DIR/testnet.toml" \
        --data-dir "$DATA_DIR" \
        --log-level debug \
        2>&1 | tee "$LOG_DIR/multivm.log" &
    
    # Save PID
    echo $! > "$TESTNET_DIR/multivm.pid"
    
    print_status "MultiVM starting with PID $(cat $TESTNET_DIR/multivm.pid)..."
    print_status "Logs: tail -f $LOG_DIR/multivm.log"
    
    # Wait for startup
    print_status "Waiting for services to start..."
    sleep 10
    
    # Check if process is still running
    if ! kill -0 $(cat "$TESTNET_DIR/multivm.pid") 2>/dev/null; then
        print_error "MultiVM failed to start. Check logs at $LOG_DIR/multivm.log"
        tail -n 50 "$LOG_DIR/multivm.log"
        exit 1
    fi
}

# Verify the node is running
verify_node() {
    print_status "Verifying node status..."
    
    # Check health endpoint
    if curl -f -s http://localhost:$HEALTH_PORT/health > /dev/null 2>&1; then
        print_success "Health check passed"
        HEALTH_STATUS=$(curl -s http://localhost:$HEALTH_PORT/health | jq -r '.status' 2>/dev/null || echo "unknown")
        print_status "Health status: $HEALTH_STATUS"
    else
        print_warning "Health check endpoint not responding yet"
    fi
    
    # Check metrics endpoint
    if curl -f -s http://localhost:$METRICS_PORT/metrics > /dev/null 2>&1; then
        print_success "Metrics endpoint responding"
    else
        print_warning "Metrics endpoint not responding yet"
    fi
    
    # Check API endpoints
    if curl -f -s http://localhost:$REST_API_PORT/api/v1/status > /dev/null 2>&1; then
        print_success "REST API responding"
    else
        print_warning "REST API not responding yet"
    fi
}

# Display connection information
display_info() {
    echo ""
    echo "🎉 MultiVM Solo Node Testnet Running!"
    echo "===================================="
    echo ""
    echo "📊 Service Endpoints:"
    echo "  Health Check:  http://localhost:$HEALTH_PORT/health"
    echo "  Metrics:       http://localhost:$METRICS_PORT/metrics"
    echo "  REST API:      http://localhost:$REST_API_PORT/api/v1"
    echo "  GraphQL:       http://localhost:$GRAPHQL_PORT/graphql"
    echo "  WebSocket:     ws://localhost:$WS_PORT"
    echo "  Admin UI:      http://localhost:$ADMIN_PORT"
    echo ""
    echo "🔗 Blockchain Endpoints:"
    echo "  Ethereum RPC:  http://localhost:$ETHEREUM_RPC_PORT"
    echo "  Solana RPC:    http://localhost:$SOLANA_RPC_PORT"
    echo ""
    echo "📁 Data & Logs:"
    echo "  Data Dir:      $DATA_DIR"
    echo "  Logs:          $LOG_DIR/multivm.log"
    echo "  Config:        $CONFIG_DIR/testnet.toml"
    echo ""
    echo "🔑 Validator Info:"
    echo "  ID:            $VALIDATOR_ID"
    echo "  P2P Port:      $P2P_PORT"
    echo "  Consensus:     $CONSENSUS_PORT"
    echo ""
    echo "📝 Useful Commands:"
    echo "  View logs:     tail -f $LOG_DIR/multivm.log"
    echo "  Stop node:     kill \$(cat $TESTNET_DIR/multivm.pid)"
    echo "  Clean data:    CLEAN_START=true $0"
    echo ""
    echo "🧪 Test the network:"
    echo "  # Check node status"
    echo "  curl http://localhost:$REST_API_PORT/api/v1/status"
    echo ""
    echo "  # Get latest block"
    echo "  curl http://localhost:$REST_API_PORT/api/v1/blocks/latest"
    echo ""
    echo "  # Submit test transaction"
    echo "  curl -X POST http://localhost:$REST_API_PORT/api/v1/transactions \\"
    echo "    -H 'Content-Type: application/json' \\"
    echo "    -d '{\"type\":\"test\",\"data\":\"hello\"}'"
    echo ""
}

# Monitor the node
monitor_node() {
    print_status "Monitoring node... (Press Ctrl+C to stop)"
    
    # Follow logs
    tail -f "$LOG_DIR/multivm.log" | while read line; do
        # Color errors in red
        if echo "$line" | grep -E "(ERROR|error|Error)" > /dev/null; then
            echo -e "${RED}$line${NC}"
        # Color warnings in yellow
        elif echo "$line" | grep -E "(WARN|warn|Warning)" > /dev/null; then
            echo -e "${YELLOW}$line${NC}"
        # Color success/info in green
        elif echo "$line" | grep -E "(SUCCESS|success|started|Started)" > /dev/null; then
            echo -e "${GREEN}$line${NC}"
        # Normal output
        else
            echo "$line"
        fi
    done
}

# Main execution
main() {
    case "${1:-start}" in
        "start")
            init_directories
            generate_validator_keys
            create_testnet_config
            build_multivm
            start_background_services
            start_multivm_node
            verify_node
            display_info
            
            # Monitor if requested
            if [ "${MONITOR:-true}" = "true" ]; then
                monitor_node
            fi
            ;;
        "stop")
            cleanup
            print_success "Testnet stopped"
            ;;
        "clean")
            CLEAN_START=true
            cleanup
            rm -rf "$TESTNET_DIR"
            print_success "Testnet data cleaned"
            ;;
        "status")
            if [ -f "$TESTNET_DIR/multivm.pid" ] && kill -0 $(cat "$TESTNET_DIR/multivm.pid") 2>/dev/null; then
                print_success "Testnet is running (PID: $(cat $TESTNET_DIR/multivm.pid))"
                verify_node
            else
                print_error "Testnet is not running"
            fi
            ;;
        "logs")
            if [ -f "$LOG_DIR/multivm.log" ]; then
                tail -f "$LOG_DIR/multivm.log"
            else
                print_error "No log file found"
            fi
            ;;
        "help")
            echo "Usage: $0 [start|stop|clean|status|logs|help]"
            echo ""
            echo "Commands:"
            echo "  start   - Start the solo node testnet (default)"
            echo "  stop    - Stop the running testnet"
            echo "  clean   - Stop and remove all testnet data"
            echo "  status  - Check if testnet is running"
            echo "  logs    - Follow the testnet logs"
            echo "  help    - Show this help"
            echo ""
            echo "Environment variables:"
            echo "  TESTNET_DIR    - Base directory for testnet data (default: /tmp/multivm-testnet)"
            echo "  CLEAN_START    - Clean existing data before starting (default: false)"
            echo "  MONITOR        - Monitor logs after starting (default: true)"
            echo "  RUST_LOG       - Rust log level (default: info,multivm=debug)"
            ;;
        *)
            print_error "Unknown command: $1"
            echo "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
}

# Run main function
main "$@"