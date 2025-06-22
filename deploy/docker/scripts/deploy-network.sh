#!/bin/bash
# Deployment script for MultiVM network with continuous block generation

set -e

# Configuration
COMPOSE_FILE="docker-compose.yml"
PROJECT_NAME="multivm"
VERIFY_TIMEOUT=120
LOGS_FOLLOW_TIME=30

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log() {
    echo -e "[$(date -Iseconds)] $*"
}

success() {
    echo -e "${GREEN}✓ $*${NC}"
}

warning() {
    echo -e "${YELLOW}⚠ $*${NC}"
}

error() {
    echo -e "${RED}✗ $*${NC}"
}

info() {
    echo -e "${BLUE}ℹ $*${NC}"
}

# Cleanup function
cleanup() {
    log "Cleaning up previous deployment..."
    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down --volumes --remove-orphans 2>/dev/null || true
    docker system prune -f 2>/dev/null || true
}

# Deploy the network
deploy() {
    log "Starting MultiVM network deployment..."
    
    # Build and start services
    info "Building MultiVM containers..."
    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" build --no-cache
    
    info "Starting MultiVM network..."
    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" up -d
    
    # Wait for services to start
    info "Waiting for services to initialize..."
    sleep 20
    
    # Check service status
    info "Checking service status..."
    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" ps
}

# Verify deployment
verify() {
    log "Verifying MultiVM network..."
    
    # Set environment variables for verification script
    export MULTIVM_NODES="localhost:8080,localhost:8081,localhost:8082,localhost:8083"
    export VERIFY_TIMEOUT="$VERIFY_TIMEOUT"
    
    # Run verification
    if ./scripts/verify-blocks.sh; then
        success "Network verification passed!"
        return 0
    else
        error "Network verification failed!"
        return 1
    fi
}

# Show logs
show_logs() {
    local duration=${1:-$LOGS_FOLLOW_TIME}
    
    info "Showing logs for $duration seconds..."
    timeout "$duration" docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" logs -f || true
}

# Show network status
show_status() {
    log "MultiVM Network Status:"
    echo
    
    # Service status
    info "Service Status:"
    docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" ps
    echo
    
    # Network info
    info "Network Access Points:"
    echo "  Node 1 (Bootstrap): http://localhost:8080"
    echo "  Node 2 (Validator): http://localhost:8081" 
    echo "  Node 3 (Validator): http://localhost:8082"
    echo "  Node 4 (Full Node): http://localhost:8083"
    echo
    
    # Block generation info
    info "Block Generation Configuration:"
    echo "  Generator Node: Node 1 (Bootstrap)"
    echo "  Block Interval: 2000ms (2 seconds)"
    echo "  SVM Transactions per Block: 3"
    echo "  EVM Transactions per Block: 3"
    echo
    
    # Health check
    info "Health Status:"
    for port in 8080 8081 8082 8083; do
        if curl -sf "http://localhost:$port/health" > /dev/null 2>&1; then
            success "Node on port $port: Healthy"
        else
            error "Node on port $port: Unhealthy"
        fi
    done
}

# Main function
main() {
    local command=${1:-"deploy"}
    
    case "$command" in
        "deploy")
            cleanup
            deploy
            if verify; then
                show_status
                success "MultiVM network deployed successfully! 🚀"
                info "Use '$0 logs' to view real-time logs"
                info "Use '$0 status' to check network status"
                info "Use '$0 stop' to stop the network"
            else
                error "Deployment verification failed. Check logs with '$0 logs'"
                exit 1
            fi
            ;;
        "verify")
            verify
            ;;
        "status")
            show_status
            ;;
        "logs")
            show_logs "${2:-60}"
            ;;
        "stop")
            log "Stopping MultiVM network..."
            docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down
            success "MultiVM network stopped"
            ;;
        "cleanup")
            cleanup
            success "Cleanup completed"
            ;;
        "restart")
            log "Restarting MultiVM network..."
            docker-compose -f "$COMPOSE_FILE" -p "$PROJECT_NAME" restart
            sleep 10
            verify
            show_status
            ;;
        *)
            echo "Usage: $0 {deploy|verify|status|logs|stop|cleanup|restart}"
            echo
            echo "Commands:"
            echo "  deploy   - Deploy the complete MultiVM network"
            echo "  verify   - Verify network is working correctly"
            echo "  status   - Show network status and endpoints"
            echo "  logs     - Show real-time logs (optional: duration in seconds)"
            echo "  stop     - Stop the network"
            echo "  cleanup  - Clean up containers and volumes"
            echo "  restart  - Restart the network and verify"
            exit 1
            ;;
    esac
}

# Check if we're in the right directory
if [ ! -f "$COMPOSE_FILE" ]; then
    error "docker-compose.yml not found. Please run this script from the deploy/docker directory."
    exit 1
fi

# Run main function
main "$@"