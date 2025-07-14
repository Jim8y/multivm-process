#!/bin/bash

# MultiVM Reth Node Setup Script
# This script sets up and starts a Reth node for communication with MultiVM

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "${SCRIPT_DIR}")"
RETH_DATA_DIR="${RETH_DATA_DIR:-${PROJECT_ROOT}/reth-data}"
RETH_HTTP_PORT="${RETH_HTTP_PORT:-8545}"
RETH_ENGINE_PORT="${RETH_ENGINE_PORT:-8551}"
RETH_CHAIN_ID="${RETH_CHAIN_ID:-1337}"
MULTIVM_IPC_PATH="${MULTIVM_IPC_PATH:-/tmp/multivm-reth.sock}"
JWT_SECRET_PATH="${JWT_SECRET_PATH:-${RETH_DATA_DIR}/jwt.hex}"
JWT_EXPIRY_SECONDS="${JWT_EXPIRY_SECONDS:-300}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Help function
show_help() {
    cat << EOF
MultiVM Reth Node Setup Script

USAGE:
    $0 [COMMAND] [OPTIONS]

COMMANDS:
    install     Install Reth if not present
    setup       Initialize Reth node and JWT authentication
    start       Start Reth node for MultiVM communication
    stop        Stop running Reth node
    restart     Restart Reth node
    status      Check Reth node status
    logs        Show Reth node logs
    clean       Clean all Reth data (WARNING: destroys blockchain state)
    help        Show this help message

OPTIONS:
    --data-dir DIR          Reth data directory (default: ${RETH_DATA_DIR})
    --rpc-port PORT         RPC port (default: ${RETH_HTTP_PORT})
    --engine-port PORT      Engine API port (default: ${RETH_ENGINE_PORT})
    --chain-id ID           Chain ID (default: ${RETH_CHAIN_ID})
    --jwt-path PATH         JWT secret file path (default: ${JWT_SECRET_PATH})
    --dev                   Development mode with pre-funded accounts
    --no-discovery          Disable P2P discovery
    --mining                Enable mining (development mode)

ENVIRONMENT VARIABLES:
    RETH_DATA_DIR           Data directory for Reth
    RETH_HTTP_PORT          HTTP RPC port
    RETH_ENGINE_PORT        Engine API port
    RETH_CHAIN_ID           Blockchain chain ID
    MULTIVM_IPC_PATH        IPC socket path for MultiVM communication
    JWT_SECRET_PATH         Path to JWT secret file
    JWT_EXPIRY_SECONDS      JWT token expiry time
    RUST_LOG                Logging level (default: info)

EXAMPLES:
    # Quick start for development
    $0 install && $0 setup --dev && $0 start

    # Production setup
    $0 setup --chain-id 1 --no-discovery
    $0 start

    # Custom configuration
    $0 setup --data-dir /var/lib/reth --rpc-port 8545 --chain-id 1337
    $0 start

    # Clean restart
    $0 stop && $0 clean && $0 setup --dev && $0 start
EOF
}

# Check if Reth is installed
check_reth_installation() {
    if ! command -v reth &> /dev/null; then
        log_error "Reth is not installed or not in PATH"
        log_info "Please install Reth using one of these methods:"
        echo "  1. cargo install --git https://github.com/paradigmxyz/reth.git --bin reth"
        echo "  2. Download from: https://github.com/paradigmxyz/reth/releases"
        echo "  3. Run: $0 install"
        return 1
    fi

    local version=$(reth --version 2>/dev/null || echo "unknown")
    log_success "Reth found: $version"
    
    # Check for required tools
    local missing_tools=()
    command -v openssl &> /dev/null || missing_tools+=("openssl")
    command -v xxd &> /dev/null || missing_tools+=("xxd")
    command -v curl &> /dev/null || missing_tools+=("curl")
    
    if [ ${#missing_tools[@]} -gt 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        log_info "Please install the missing tools before continuing"
        return 1
    fi
    
    return 0
}

# Install Reth
install_reth() {
    log_info "Installing Reth..."
    
    if command -v cargo &> /dev/null; then
        log_info "Installing Reth via Cargo..."
        cargo install --git https://github.com/paradigmxyz/reth.git --bin reth --force
        log_success "Reth installed successfully"
    else
        log_error "Cargo not found. Please install Rust and Cargo first:"
        echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        return 1
    fi
}

# Check port availability
check_port() {
    local port=$1
    local name=$2
    
    if netstat -tuln 2>/dev/null | grep -q ":${port} " || ss -tuln 2>/dev/null | grep -q ":${port} "; then
        log_error "$name port $port is already in use"
        log_info "Please choose a different port or stop the service using port $port"
        return 1
    fi
    log_success "$name port $port is available"
    return 0
}

# Generate JWT secret
generate_jwt_secret() {
    log_info "Generating JWT secret for Engine API authentication..."
    
    mkdir -p "$(dirname "$JWT_SECRET_PATH")"
    
    # Generate 32-byte (256-bit) random secret and convert to hex
    openssl rand -hex 32 > "$JWT_SECRET_PATH"
    chmod 600 "$JWT_SECRET_PATH"
    
    log_success "JWT secret generated: $JWT_SECRET_PATH"
    log_info "Secret: $(cat "$JWT_SECRET_PATH")"
}

# Initialize Reth database
init_reth_database() {
    local chain_name="dev"
    if [ "$RETH_CHAIN_ID" != "1337" ]; then
        case "$RETH_CHAIN_ID" in
            1) chain_name="mainnet" ;;
            11155111) chain_name="sepolia" ;;
            17000) chain_name="holesky" ;;
            *) chain_name="dev" ;;
        esac
    fi

    log_info "Initializing Reth database for chain: $chain_name"
    
    if [ -d "$RETH_DATA_DIR/db" ]; then
        log_warning "Database already exists at $RETH_DATA_DIR/db"
        # Check if we need to reinitialize for different chain
        if [ -f "$RETH_DATA_DIR/chain_info" ]; then
            local stored_chain=$(cat "$RETH_DATA_DIR/chain_info" 2>/dev/null)
            if [ "$stored_chain" != "$chain_name" ]; then
                log_warning "Chain mismatch! Stored: $stored_chain, Expected: $chain_name"
                log_warning "Consider running 'clean' command before setup"
            fi
        fi
        return 0
    fi

    mkdir -p "$RETH_DATA_DIR"
    
    # Run init command
    log_info "Running: reth init --datadir $RETH_DATA_DIR --chain $chain_name"
    reth init \
        --datadir "$RETH_DATA_DIR" \
        --chain "$chain_name" || {
        log_error "Failed to initialize Reth database"
        return 1
    }
    
    # Store chain info
    echo "$chain_name" > "$RETH_DATA_DIR/chain_info"
    
    log_success "Reth database initialized successfully"
}

# Setup Reth node
setup_reth() {
    local dev_mode=false
    local no_discovery=false
    local mining=false
    
    # Parse setup options
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dev)
                dev_mode=true
                shift
                ;;
            --no-discovery)
                no_discovery=true
                shift
                ;;
            --mining)
                mining=true
                shift
                ;;
            --data-dir)
                RETH_DATA_DIR="$2"
                JWT_SECRET_PATH="${RETH_DATA_DIR}/jwt.hex"
                shift 2
                ;;
            --rpc-port)
                RETH_HTTP_PORT="$2"
                shift 2
                ;;
            --engine-port)
                RETH_ENGINE_PORT="$2"
                shift 2
                ;;
            --chain-id)
                RETH_CHAIN_ID="$2"
                shift 2
                ;;
            --jwt-path)
                JWT_SECRET_PATH="$2"
                shift 2
                ;;
            *)
                log_error "Unknown setup option: $1"
                return 1
                ;;
        esac
    done

    log_info "Setting up Reth node..."
    log_info "Data directory: $RETH_DATA_DIR"
    log_info "RPC port: $RETH_HTTP_PORT"
    log_info "Engine port: $RETH_ENGINE_PORT"
    log_info "Chain ID: $RETH_CHAIN_ID"
    log_info "Development mode: $dev_mode"

    # Check Reth installation
    check_reth_installation || return 1

    # Check port availability
    check_port "$RETH_HTTP_PORT" "RPC" || return 1
    check_port "$RETH_ENGINE_PORT" "Engine API" || return 1

    # Generate JWT secret
    generate_jwt_secret || return 1

    # Initialize database
    init_reth_database || return 1

    # Create systemd service file if running as root
    if [ "$(id -u)" -eq 0 ]; then
        create_systemd_service "$dev_mode" "$no_discovery" "$mining"
    fi

    log_success "Reth node setup completed!"
    log_info "You can now start the node with: $0 start"
}

# Create systemd service
create_systemd_service() {
    local dev_mode=$1
    local no_discovery=$2
    local mining=$3
    
    log_info "Creating systemd service..."

    local service_file="/etc/systemd/system/multivm-reth.service"
    local reth_args=""
    
    if [ "$dev_mode" = true ]; then
        reth_args="$reth_args --dev"
    fi
    
    if [ "$no_discovery" = true ]; then
        reth_args="$reth_args --no-discovery"
    fi

    # Determine chain name for systemd service
    local chain_name="dev"
    if [ "$RETH_CHAIN_ID" != "1337" ]; then
        case "$RETH_CHAIN_ID" in
            1) chain_name="mainnet" ;;
            11155111) chain_name="sepolia" ;;
            17000) chain_name="holesky" ;;
            *) chain_name="dev" ;;
        esac
    fi

    cat > "$service_file" << EOF
[Unit]
Description=MultiVM Reth Node
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=${PROJECT_ROOT}
Environment=RUST_LOG=info
Environment=RETH_DATA_DIR=${RETH_DATA_DIR}
Environment=RETH_HTTP_PORT=${RETH_HTTP_PORT}
Environment=RETH_ENGINE_PORT=${RETH_ENGINE_PORT}
Environment=RETH_CHAIN_ID=${RETH_CHAIN_ID}
Environment=JWT_SECRET_PATH=${JWT_SECRET_PATH}
Environment=MULTIVM_IPC_PATH=${MULTIVM_IPC_PATH}
ExecStart=$(which reth) node \\
    --datadir ${RETH_DATA_DIR} \\
    --chain ${chain_name} \\
    --http \\
    --http.addr 0.0.0.0 \\
    --http.port ${RETH_HTTP_PORT} \\
    --http.api eth,net,web3,debug,trace \\
    --http.corsdomain "*" \\
    --authrpc.addr 0.0.0.0 \\
    --authrpc.port ${RETH_ENGINE_PORT} \\
    --authrpc.jwtsecret ${JWT_SECRET_PATH} \\
    --full ${reth_args}
ExecReload=/bin/kill -HUP \$MAINPID
KillMode=process
Restart=on-failure
RestartSec=5
TimeoutStopSec=60
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
EOF

    systemctl daemon-reload
    log_success "Systemd service created: $service_file"
    log_info "Enable with: systemctl enable multivm-reth"
    log_info "Start with: systemctl start multivm-reth"
}

# Start Reth node
start_reth() {
    log_info "Starting Reth node..."

    # Check if already running
    if get_reth_pid > /dev/null 2>&1; then
        log_warning "Reth node is already running (PID: $(get_reth_pid))"
        return 0
    fi

    # Check setup
    if [ ! -f "$JWT_SECRET_PATH" ]; then
        log_error "JWT secret not found. Please run setup first: $0 setup"
        return 1
    fi

    if [ ! -d "$RETH_DATA_DIR/db" ]; then
        log_error "Reth database not found. Please run setup first: $0 setup"
        return 1
    fi

    # Check ports
    check_port "$RETH_HTTP_PORT" "RPC" || return 1
    check_port "$RETH_ENGINE_PORT" "Engine API" || return 1

    # Determine chain argument
    local chain_arg="dev"
    if [ "$RETH_CHAIN_ID" != "1337" ]; then
        case "$RETH_CHAIN_ID" in
            1) chain_arg="mainnet" ;;
            11155111) chain_arg="sepolia" ;;
            17000) chain_arg="holesky" ;;
        esac
    fi

    # Start Reth node
    log_info "Starting Reth with the following configuration:"
    echo "  Data directory: $RETH_DATA_DIR"
    echo "  Chain: $chain_arg"
    echo "  RPC URL: http://127.0.0.1:$RETH_HTTP_PORT"
    echo "  Engine URL: http://127.0.0.1:$RETH_ENGINE_PORT"
    echo "  JWT secret: $JWT_SECRET_PATH"

    # Create log directory
    mkdir -p "${RETH_DATA_DIR}/logs"
    
    # Create IPC socket directory if needed
    local ipc_dir=$(dirname "$MULTIVM_IPC_PATH")
    if [ ! -d "$ipc_dir" ]; then
        log_info "Creating IPC socket directory: $ipc_dir"
        mkdir -p "$ipc_dir"
    fi
    
    # Clean up old IPC socket if exists
    if [ -S "$MULTIVM_IPC_PATH" ]; then
        log_info "Removing old IPC socket: $MULTIVM_IPC_PATH"
        rm -f "$MULTIVM_IPC_PATH"
    fi

    # Build Reth command based on configuration
    local reth_cmd="reth node"
    reth_cmd="$reth_cmd --datadir \"$RETH_DATA_DIR\""
    reth_cmd="$reth_cmd --chain \"$chain_arg\""
    reth_cmd="$reth_cmd --http"
    reth_cmd="$reth_cmd --http.addr 0.0.0.0"
    reth_cmd="$reth_cmd --http.port $RETH_HTTP_PORT"
    reth_cmd="$reth_cmd --http.api eth,net,web3,debug,trace"
    reth_cmd="$reth_cmd --http.corsdomain \"*\""
    reth_cmd="$reth_cmd --authrpc.addr 0.0.0.0"
    reth_cmd="$reth_cmd --authrpc.port $RETH_ENGINE_PORT"
    reth_cmd="$reth_cmd --authrpc.jwtsecret \"$JWT_SECRET_PATH\""
    reth_cmd="$reth_cmd --full"
    
    # Only add --dev flag for dev chain
    if [ "$chain_arg" = "dev" ]; then
        reth_cmd="$reth_cmd --dev"
    fi
    
    # Add additional flags from environment if set
    if [ -n "${RETH_EXTRA_FLAGS:-}" ]; then
        reth_cmd="$reth_cmd $RETH_EXTRA_FLAGS"
    fi
    
    log_info "Starting Reth with command:"
    log_info "$reth_cmd"
    
    # Start Reth node in background
    RUST_LOG="${RUST_LOG:-info}" eval "nohup $reth_cmd > \"${RETH_DATA_DIR}/logs/reth.log\" 2>&1 &"

    local reth_pid=$!
    echo "$reth_pid" > "${RETH_DATA_DIR}/reth.pid"

    # Wait for startup
    log_info "Waiting for Reth to start..."
    for i in {1..30}; do
        sleep 1
        if curl -s -X POST \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
            "http://127.0.0.1:$RETH_HTTP_PORT" > /dev/null 2>&1; then
            log_success "Reth node started successfully!"
            log_info "PID: $reth_pid"
            log_info "RPC URL: http://127.0.0.1:$RETH_HTTP_PORT"
            log_info "Engine URL: http://127.0.0.1:$RETH_ENGINE_PORT"
            log_info "Log file: ${RETH_DATA_DIR}/logs/reth.log"
            
            # Test Engine API with JWT
            test_engine_api
            
            # Display connection info for MultiVM
            log_info ""
            log_info "=== MultiVM Integration Info ==="
            log_info "RPC Endpoint: http://127.0.0.1:$RETH_HTTP_PORT"
            log_info "Engine API: http://127.0.0.1:$RETH_ENGINE_PORT"
            log_info "JWT Secret: $JWT_SECRET_PATH"
            log_info "IPC Socket: $MULTIVM_IPC_PATH"
            log_info "Chain ID: $RETH_CHAIN_ID"
            log_info "================================"
            
            return 0
        fi
        echo -n "."
    done

    log_error "Reth node failed to start within 30 seconds"
    log_info "Check logs: tail -f ${RETH_DATA_DIR}/logs/reth.log"
    return 1
}

# Test Engine API with JWT authentication
test_engine_api() {
    log_info "Testing Engine API with JWT authentication..."
    
    # Generate JWT token
    local jwt_secret=$(cat "$JWT_SECRET_PATH")
    local jwt_token=$(generate_jwt_token "$jwt_secret")
    
    # Test Engine API call
    local response=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer $jwt_token" \
        -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[[]],"id":1}' \
        "http://127.0.0.1:$RETH_ENGINE_PORT" 2>/dev/null)
    
    if echo "$response" | grep -q '"result"'; then
        log_success "Engine API authentication working!"
    else
        log_warning "Engine API authentication test failed"
        log_info "Response: $response"
    fi
}

# Generate JWT token
generate_jwt_token() {
    local secret=$1
    local now=$(date +%s)
    local exp=$((now + JWT_EXPIRY_SECONDS))
    
    # Create JWT header and payload
    local header='{"alg":"HS256","typ":"JWT"}'
    local payload="{\"iat\":$now,\"exp\":$exp}"
    
    # Base64 encode (URL-safe, no padding)
    local header_b64=$(echo -n "$header" | base64 -w 0 | tr '+/' '-_' | tr -d '=')
    local payload_b64=$(echo -n "$payload" | base64 -w 0 | tr '+/' '-_' | tr -d '=')
    
    # Create signature
    local data="${header_b64}.${payload_b64}"
    local signature=$(echo -n "$data" | openssl dgst -sha256 -hmac "$(echo -n "$secret" | xxd -r -p)" -binary | base64 -w 0 | tr '+/' '-_' | tr -d '=')
    
    echo "${data}.${signature}"
}

# Get Reth process PID
get_reth_pid() {
    if [ -f "${RETH_DATA_DIR}/reth.pid" ]; then
        local pid=$(cat "${RETH_DATA_DIR}/reth.pid")
        if kill -0 "$pid" 2>/dev/null; then
            echo "$pid"
            return 0
        else
            rm -f "${RETH_DATA_DIR}/reth.pid"
        fi
    fi
    return 1
}

# Stop Reth node
stop_reth() {
    log_info "Stopping Reth node..."

    local pid
    if pid=$(get_reth_pid); then
        kill "$pid"
        
        # Wait for graceful shutdown
        for i in {1..10}; do
            if ! kill -0 "$pid" 2>/dev/null; then
                log_success "Reth node stopped successfully"
                rm -f "${RETH_DATA_DIR}/reth.pid"
                return 0
            fi
            sleep 1
        done
        
        # Force kill if still running
        log_warning "Force killing Reth node..."
        kill -9 "$pid" 2>/dev/null || true
        rm -f "${RETH_DATA_DIR}/reth.pid"
        log_success "Reth node force stopped"
    else
        log_info "Reth node is not running"
    fi
}

# Check Reth node status
check_status() {
    log_info "Checking Reth node status..."

    local pid
    if pid=$(get_reth_pid); then
        log_success "Reth node is running (PID: $pid)"
        
        # Test RPC connectivity
        if curl -s -X POST \
            -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' \
            "http://127.0.0.1:$RETH_HTTP_PORT" > /dev/null 2>&1; then
            log_success "RPC endpoint is responsive"
        else
            log_warning "RPC endpoint is not responding"
        fi
        
        # Test Engine API
        if [ -f "$JWT_SECRET_PATH" ]; then
            local jwt_secret=$(cat "$JWT_SECRET_PATH")
            local jwt_token=$(generate_jwt_token "$jwt_secret")
            
            if curl -s -X POST \
                -H "Content-Type: application/json" \
                -H "Authorization: Bearer $jwt_token" \
                -d '{"jsonrpc":"2.0","method":"engine_exchangeCapabilities","params":[[]],"id":1}' \
                "http://127.0.0.1:$RETH_ENGINE_PORT" | grep -q '"result"'; then
                log_success "Engine API is responsive"
            else
                log_warning "Engine API is not responding"
            fi
        fi
        
        # Show additional info
        echo ""
        echo "Configuration:"
        echo "  Data directory: $RETH_DATA_DIR"
        echo "  RPC URL: http://127.0.0.1:$RETH_HTTP_PORT"
        echo "  Engine URL: http://127.0.0.1:$RETH_ENGINE_PORT"
        echo "  JWT secret: $JWT_SECRET_PATH"
        echo "  Log file: ${RETH_DATA_DIR}/logs/reth.log"
    else
        log_error "Reth node is not running"
        return 1
    fi
}

# Show logs
show_logs() {
    local log_file="${RETH_DATA_DIR}/logs/reth.log"
    
    if [ -f "$log_file" ]; then
        tail -f "$log_file"
    else
        log_error "Log file not found: $log_file"
        return 1
    fi
}

# Clean all data
clean_data() {
    log_warning "This will delete all Reth data including blockchain state!"
    read -p "Are you sure? (y/N): " -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        stop_reth
        
        if [ -d "$RETH_DATA_DIR" ]; then
            rm -rf "$RETH_DATA_DIR"
            log_success "Reth data directory cleaned: $RETH_DATA_DIR"
        else
            log_info "Data directory does not exist: $RETH_DATA_DIR"
        fi
        
        # Clean IPC socket
        if [ -S "$MULTIVM_IPC_PATH" ]; then
            rm -f "$MULTIVM_IPC_PATH"
            log_success "Cleaned IPC socket: $MULTIVM_IPC_PATH"
        fi
    else
        log_info "Clean operation cancelled"
    fi
}

# Parse global options
while [[ $# -gt 0 ]]; do
    case $1 in
        --data-dir)
            RETH_DATA_DIR="$2"
            JWT_SECRET_PATH="${RETH_DATA_DIR}/jwt.hex"
            shift 2
            ;;
        --rpc-port)
            RETH_HTTP_PORT="$2"
            shift 2
            ;;
        --engine-port)
            RETH_ENGINE_PORT="$2"
            shift 2
            ;;
        --chain-id)
            RETH_CHAIN_ID="$2"
            shift 2
            ;;
        --jwt-path)
            JWT_SECRET_PATH="$2"
            shift 2
            ;;
        install)
            install_reth
            exit $?
            ;;
        setup)
            shift
            setup_reth "$@"
            exit $?
            ;;
        start)
            start_reth
            exit $?
            ;;
        stop)
            stop_reth
            exit $?
            ;;
        restart)
            stop_reth
            sleep 2
            start_reth
            exit $?
            ;;
        status)
            check_status
            exit $?
            ;;
        logs)
            show_logs
            exit $?
            ;;
        clean)
            clean_data
            exit $?
            ;;
        help|--help|-h)
            show_help
            exit 0
            ;;
        *)
            log_error "Unknown command: $1"
            echo "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
done

# Default action if no command provided
log_info "MultiVM Reth Node Setup Script"
echo "Use '$0 help' for usage information"
echo ""
echo "Quick start:"
echo "  $0 install    # Install Reth if needed"
echo "  $0 setup --dev # Setup for development"
echo "  $0 start      # Start the node"
echo "  $0 status     # Check status"