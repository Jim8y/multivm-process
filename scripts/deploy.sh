#!/bin/bash

# Multi-VM Blockchain Execution System Deployment Script
# This script helps deploy and manage the multi-blockchain execution environment

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default configuration
RETH_DATA_DIR="./data/reth"
SOLANA_DATA_DIR="./data/solana"
RETH_RPC_PORT="8545"
SOLANA_RPC_PORT="8899"
LOG_LEVEL="info"

print_header() {
    echo -e "${BLUE}"
    echo "=================================================="
    echo "  Multi-VM Blockchain Execution System"
    echo "  Professional Grade Deployment Tool"
    echo "=================================================="
    echo -e "${NC}"
}

print_section() {
    echo -e "\n${YELLOW}▶ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

check_dependencies() {
    print_section "Checking Dependencies"
    
    # Check Rust and Cargo
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo is not installed. Please install Rust: https://rustup.rs/"
        exit 1
    fi
    print_success "Rust/Cargo found"
    
    # Check for optional binaries
    if command -v reth &> /dev/null; then
        print_success "Reth binary found: $(which reth)"
    else
        print_warning "Reth binary not found. Engine will try to use built binary."
    fi
    
    if command -v solana-validator &> /dev/null; then
        print_success "Solana validator found: $(which solana-validator)"
    else
        print_warning "Solana validator not found. Engine will try to use built binary."
    fi
    
    if command -v solana-genesis &> /dev/null; then
        print_success "Solana genesis tool found: $(which solana-genesis)"
    else
        print_warning "Solana genesis not found. Engine will try to use built binary."
    fi
}

build_system() {
    print_section "Building Multi-VM System"
    
    echo "Building in release mode for optimal performance..."
    cargo build --release
    
    if [ $? -eq 0 ]; then
        print_success "System built successfully"
    else
        print_error "Build failed"
        exit 1
    fi
}

setup_directories() {
    print_section "Setting Up Data Directories"
    
    mkdir -p "$RETH_DATA_DIR"
    mkdir -p "$SOLANA_DATA_DIR"
    mkdir -p "./logs"
    mkdir -p "/tmp/multivm"
    
    print_success "Data directories created"
}

run_health_check() {
    print_section "Running System Health Check"
    
    # Basic compilation check
    if cargo check &> /dev/null; then
        print_success "Code compilation check passed"
    else
        print_error "Code compilation check failed"
        return 1
    fi
    
    # Check available ports
    if nc -z localhost "$RETH_RPC_PORT" 2>/dev/null; then
        print_warning "Port $RETH_RPC_PORT is already in use"
    else
        print_success "Reth RPC port $RETH_RPC_PORT is available"
    fi
    
    if nc -z localhost "$SOLANA_RPC_PORT" 2>/dev/null; then
        print_warning "Port $SOLANA_RPC_PORT is already in use"
    else
        print_success "Solana RPC port $SOLANA_RPC_PORT is available"
    fi
    
    # Check disk space (warn if less than 1GB available)
    available_space=$(df . | awk 'NR==2 {print $4}')
    if [ "$available_space" -lt 1048576 ]; then # 1GB in KB
        print_warning "Low disk space available: $(df -h . | awk 'NR==2 {print $4}')"
    else
        print_success "Sufficient disk space available"
    fi
}

start_reth_engine() {
    print_section "Starting Reth Execution Engine"
    
    echo "Starting Reth engine with:"
    echo "  - Data directory: $RETH_DATA_DIR"
    echo "  - RPC port: $RETH_RPC_PORT"
    echo "  - P2P: DISABLED"
    echo "  - Consensus: DISABLED"
    
    RUST_LOG="$LOG_LEVEL" ./target/release/reth-execution-engine \
        --data-dir "$RETH_DATA_DIR" \
        --rpc-port "$RETH_RPC_PORT" &
    
    RETH_PID=$!
    echo "$RETH_PID" > ./data/reth.pid
    print_success "Reth engine started (PID: $RETH_PID)"
}

start_solana_engine() {
    print_section "Starting Solana Execution Engine"
    
    echo "Starting Solana engine with:"
    echo "  - Data directory: $SOLANA_DATA_DIR"
    echo "  - RPC port: $SOLANA_RPC_PORT"
    echo "  - P2P: DISABLED"
    echo "  - Consensus: DISABLED"
    
    RUST_LOG="$LOG_LEVEL" ./target/release/solana-execution-engine \
        --data-dir "$SOLANA_DATA_DIR" \
        --rpc-port "$SOLANA_RPC_PORT" &
    
    SOLANA_PID=$!
    echo "$SOLANA_PID" > ./data/solana.pid
    print_success "Solana engine started (PID: $SOLANA_PID)"
}

start_process_manager() {
    print_section "Starting Process Manager"
    
    echo "Starting process manager..."
    RUST_LOG="$LOG_LEVEL" ./target/release/multivm-process-manager &
    
    MANAGER_PID=$!
    echo "$MANAGER_PID" > ./data/manager.pid
    print_success "Process manager started (PID: $MANAGER_PID)"
}

stop_system() {
    print_section "Stopping Multi-VM System"
    
    # Stop process manager
    if [ -f "./data/manager.pid" ]; then
        PID=$(cat ./data/manager.pid)
        if kill -0 "$PID" 2>/dev/null; then
            kill "$PID"
            print_success "Process manager stopped"
        fi
        rm -f ./data/manager.pid
    fi
    
    # Stop Reth engine
    if [ -f "./data/reth.pid" ]; then
        PID=$(cat ./data/reth.pid)
        if kill -0 "$PID" 2>/dev/null; then
            kill "$PID"
            print_success "Reth engine stopped"
        fi
        rm -f ./data/reth.pid
    fi
    
    # Stop Solana engine
    if [ -f "./data/solana.pid" ]; then
        PID=$(cat ./data/solana.pid)
        if kill -0 "$PID" 2>/dev/null; then
            kill "$PID"
            print_success "Solana engine stopped"
        fi
        rm -f ./data/solana.pid
    fi
}

show_status() {
    print_section "System Status"
    
    # Check manager
    if [ -f "./data/manager.pid" ]; then
        PID=$(cat ./data/manager.pid)
        if kill -0 "$PID" 2>/dev/null; then
            print_success "Process Manager: Running (PID: $PID)"
        else
            print_error "Process Manager: Not running (stale PID file)"
        fi
    else
        print_warning "Process Manager: Not started"
    fi
    
    # Check Reth
    if [ -f "./data/reth.pid" ]; then
        PID=$(cat ./data/reth.pid)
        if kill -0 "$PID" 2>/dev/null; then
            print_success "Reth Engine: Running (PID: $PID)"
        else
            print_error "Reth Engine: Not running (stale PID file)"
        fi
    else
        print_warning "Reth Engine: Not started"
    fi
    
    # Check Solana
    if [ -f "./data/solana.pid" ]; then
        PID=$(cat ./data/solana.pid)
        if kill -0 "$PID" 2>/dev/null; then
            print_success "Solana Engine: Running (PID: $PID)"
        else
            print_error "Solana Engine: Not running (stale PID file)"
        fi
    else
        print_warning "Solana Engine: Not started"
    fi
    
    # Check ports
    if nc -z localhost "$RETH_RPC_PORT" 2>/dev/null; then
        print_success "Reth RPC: Available on port $RETH_RPC_PORT"
    else
        print_warning "Reth RPC: Not responding on port $RETH_RPC_PORT"
    fi
    
    if nc -z localhost "$SOLANA_RPC_PORT" 2>/dev/null; then
        print_success "Solana RPC: Available on port $SOLANA_RPC_PORT"
    else
        print_warning "Solana RPC: Not responding on port $SOLANA_RPC_PORT"
    fi
}

show_help() {
    echo "Multi-VM Blockchain Execution System Deployment Tool"
    echo ""
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo ""
    echo "Commands:"
    echo "  deploy      - Full deployment (build + setup + start)"
    echo "  build       - Build the system"
    echo "  start       - Start all components"
    echo "  stop        - Stop all components"
    echo "  restart     - Restart all components"
    echo "  status      - Show system status"
    echo "  health      - Run health check"
    echo "  clean       - Clean build artifacts and data"
    echo "  help        - Show this help"
    echo ""
    echo "Environment Variables:"
    echo "  RETH_DATA_DIR    - Reth data directory (default: ./data/reth)"
    echo "  SOLANA_DATA_DIR  - Solana data directory (default: ./data/solana)"
    echo "  RETH_RPC_PORT    - Reth RPC port (default: 8545)"
    echo "  SOLANA_RPC_PORT  - Solana RPC port (default: 8899)"
    echo "  LOG_LEVEL        - Log level (default: info)"
}

# Parse environment variables
[ -n "$RETH_DATA_DIR_ENV" ] && RETH_DATA_DIR="$RETH_DATA_DIR_ENV"
[ -n "$SOLANA_DATA_DIR_ENV" ] && SOLANA_DATA_DIR="$SOLANA_DATA_DIR_ENV"
[ -n "$RETH_RPC_PORT_ENV" ] && RETH_RPC_PORT="$RETH_RPC_PORT_ENV"
[ -n "$SOLANA_RPC_PORT_ENV" ] && SOLANA_RPC_PORT="$SOLANA_RPC_PORT_ENV"
[ -n "$LOG_LEVEL_ENV" ] && LOG_LEVEL="$LOG_LEVEL_ENV"

# Main script logic
case "${1:-help}" in
    "deploy")
        print_header
        check_dependencies
        setup_directories
        build_system
        run_health_check
        start_reth_engine
        sleep 2
        start_solana_engine
        sleep 2
        start_process_manager
        print_success "Multi-VM system deployed successfully!"
        echo ""
        show_status
        ;;
    "build")
        print_header
        check_dependencies
        build_system
        ;;
    "start")
        print_header
        setup_directories
        start_reth_engine
        sleep 2
        start_solana_engine
        sleep 2
        start_process_manager
        print_success "Multi-VM system started!"
        ;;
    "stop")
        print_header
        stop_system
        ;;
    "restart")
        print_header
        stop_system
        sleep 2
        setup_directories
        start_reth_engine
        sleep 2
        start_solana_engine
        sleep 2
        start_process_manager
        print_success "Multi-VM system restarted!"
        ;;
    "status")
        print_header
        show_status
        ;;
    "health")
        print_header
        run_health_check
        ;;
    "clean")
        print_header
        print_section "Cleaning System"
        stop_system
        cargo clean
        rm -rf ./data
        rm -rf ./logs
        print_success "System cleaned"
        ;;
    "help"|*)
        show_help
        ;;
esac 