#!/bin/bash

# MultiVM Application Layer Start Script
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Configuration
CONFIG_FILE="${PROJECT_ROOT}/config.toml"
EXAMPLE_CONFIG="${PROJECT_ROOT}/config.example.toml"
LOG_DIR="${PROJECT_ROOT}/logs"

# Functions
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

# Check prerequisites
check_prerequisites() {
    log_info "Checking prerequisites..."
    
    # Check Rust
    if ! command -v cargo &> /dev/null; then
        log_error "Rust/Cargo not found. Please install Rust: https://rustup.rs/"
        exit 1
    fi
    
    # Check Redis (optional for development)
    if ! command -v redis-cli &> /dev/null; then
        log_warning "Redis not found. Cache will use in-memory storage."
    else
        if ! redis-cli ping &> /dev/null; then
            log_warning "Redis server not running. Starting Redis or configure in-memory cache."
        fi
    fi
    
    # Check PostgreSQL (optional for development)
    if ! command -v psql &> /dev/null; then
        log_warning "PostgreSQL not found. Some features may be limited."
    fi
    
    log_success "Prerequisites check completed"
}

# Setup configuration
setup_config() {
    log_info "Setting up configuration..."
    
    if [[ ! -f "$CONFIG_FILE" ]]; then
        if [[ -f "$EXAMPLE_CONFIG" ]]; then
            cp "$EXAMPLE_CONFIG" "$CONFIG_FILE"
            log_info "Created config.toml from example"
            log_warning "Please edit config.toml with your settings"
        else
            log_error "Example configuration not found: $EXAMPLE_CONFIG"
            exit 1
        fi
    else
        log_info "Configuration file already exists: $CONFIG_FILE"
    fi
}

# Setup logging directory
setup_logging() {
    log_info "Setting up logging directory..."
    
    if [[ ! -d "$LOG_DIR" ]]; then
        mkdir -p "$LOG_DIR"
        log_info "Created logs directory: $LOG_DIR"
    fi
}

# Build application
build_application() {
    log_info "Building application..."
    cd "$PROJECT_ROOT"
    
    if [[ "${1:-}" == "--release" ]]; then
        cargo build --release
        log_success "Application built in release mode"
    else
        cargo build
        log_success "Application built in debug mode"
    fi
}

# Run tests
run_tests() {
    log_info "Running tests..."
    cd "$PROJECT_ROOT"
    
    # Unit tests
    cargo test --lib
    
    # Integration tests (if available)
    if cargo test --test integration --no-run &> /dev/null; then
        cargo test --test integration
    fi
    
    log_success "All tests passed"
}

# Start application
start_application() {
    log_info "Starting MultiVM Application Layer..."
    cd "$PROJECT_ROOT"
    
    # Set environment variables
    export RUST_LOG="${RUST_LOG:-info}"
    export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
    
    # Start application
    if [[ "${1:-}" == "--release" ]]; then
        ./target/release/multivm-application --config "$CONFIG_FILE"
    else
        cargo run -- --config "$CONFIG_FILE"
    fi
}

# Docker operations
docker_build() {
    log_info "Building Docker image..."
    cd "$PROJECT_ROOT"
    docker build -t multivm-application:latest .
    log_success "Docker image built successfully"
}

docker_start() {
    log_info "Starting with Docker Compose..."
    cd "$PROJECT_ROOT"
    
    # Ensure config exists
    setup_config
    
    # Start services
    docker-compose up -d
    log_success "Services started with Docker Compose"
    
    # Show service status
    docker-compose ps
}

docker_stop() {
    log_info "Stopping Docker services..."
    cd "$PROJECT_ROOT"
    docker-compose down
    log_success "Docker services stopped"
}

# Health check
health_check() {
    log_info "Performing health check..."
    
    local max_attempts=30
    local attempt=1
    
    while [[ $attempt -le $max_attempts ]]; do
        if curl -s http://localhost:8080/health > /dev/null; then
            log_success "Application is healthy"
            
            # Show API endpoints
            echo
            log_info "API Endpoints:"
            echo "  REST API:     http://localhost:8080"
            echo "  GraphQL:      http://localhost:8081"
            echo "  WebSocket:    ws://localhost:8082"
            echo "  Admin UI:     http://localhost:8083"
            echo "  Metrics:      http://localhost:9090"
            echo "  Health:       http://localhost:9091"
            
            return 0
        fi
        
        log_info "Waiting for application to start... ($attempt/$max_attempts)"
        sleep 2
        ((attempt++))
    done
    
    log_error "Application failed to start or is unhealthy"
    return 1
}

# Show usage
show_usage() {
    echo "Usage: $0 [COMMAND] [OPTIONS]"
    echo
    echo "Commands:"
    echo "  check           Check prerequisites"
    echo "  build           Build the application"
    echo "  build --release Build in release mode"
    echo "  test            Run tests"
    echo "  start           Start the application (development)"
    echo "  start --release Start the application (release mode)"
    echo "  docker-build    Build Docker image"
    echo "  docker-start    Start with Docker Compose"
    echo "  docker-stop     Stop Docker services"
    echo "  health          Check application health"
    echo "  setup           Setup configuration and directories"
    echo "  dev             Full development setup and start"
    echo "  help            Show this help message"
    echo
    echo "Examples:"
    echo "  $0 dev                    # Full development setup"
    echo "  $0 build --release        # Build for production"
    echo "  $0 docker-start           # Start with Docker"
}

# Full development setup
dev_setup() {
    log_info "Starting full development setup..."
    
    check_prerequisites
    setup_config
    setup_logging
    build_application
    run_tests
    
    log_success "Development setup completed"
    log_info "Starting application..."
    
    start_application &
    
    # Wait a moment and check health
    sleep 5
    health_check
}

# Main script logic
main() {
    case "${1:-help}" in
        "check")
            check_prerequisites
            ;;
        "build")
            check_prerequisites
            setup_config
            setup_logging
            build_application "${2:-}"
            ;;
        "test")
            check_prerequisites
            run_tests
            ;;
        "start")
            check_prerequisites
            setup_config
            setup_logging
            start_application "${2:-}"
            ;;
        "docker-build")
            docker_build
            ;;
        "docker-start")
            docker_start
            ;;
        "docker-stop")
            docker_stop
            ;;
        "health")
            health_check
            ;;
        "setup")
            check_prerequisites
            setup_config
            setup_logging
            ;;
        "dev")
            dev_setup
            ;;
        "help"|"--help"|"-h")
            show_usage
            ;;
        *)
            log_error "Unknown command: $1"
            echo
            show_usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@" 