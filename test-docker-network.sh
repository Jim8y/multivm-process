#!/bin/bash
# Test script for MultiVM Docker network

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

log() {
    echo -e "${BLUE}[$(date '+%Y-%m-%d %H:%M:%S')]${NC} $*"
}

success() {
    echo -e "${GREEN}✓${NC} $*"
}

warning() {
    echo -e "${YELLOW}⚠${NC} $*"
}

error() {
    echo -e "${RED}✗${NC} $*"
}

info() {
    echo -e "${PURPLE}ℹ${NC} $*"
}

# Function to test single node
test_single_node() {
    log "Testing Single Node MultiVM Network"
    echo "===================================="
    
    # Clean up any existing containers
    log "Cleaning up existing containers..."
    docker-compose -f docker-compose.single.yml down -v 2>/dev/null || true
    
    # Build the image
    log "Building MultiVM Docker image..."
    docker-compose -f docker-compose.single.yml build
    
    # Start single node
    log "Starting single node network..."
    docker-compose -f docker-compose.single.yml up -d
    
    # Wait for node to be healthy
    log "Waiting for node to be healthy..."
    local retries=30
    while [ $retries -gt 0 ]; do
        if docker-compose -f docker-compose.single.yml ps | grep -q "healthy"; then
            success "Node is healthy!"
            break
        fi
        echo -n "."
        sleep 2
        retries=$((retries - 1))
    done
    
    if [ $retries -eq 0 ]; then
        error "Node failed to become healthy"
        docker-compose -f docker-compose.single.yml logs
        return 1
    fi
    
    # Check node status
    log "Checking node status..."
    curl -sf http://localhost:8080/health && success "Health endpoint working" || error "Health endpoint failed"
    
    # Show logs for 20 seconds
    log "Showing logs for 20 seconds..."
    echo "----------------------------------------"
    timeout 20 docker-compose -f docker-compose.single.yml logs -f || true
    echo "----------------------------------------"
    
    # Check if blocks are being generated
    log "Checking block generation..."
    sleep 5
    
    # Stop single node
    log "Stopping single node network..."
    docker-compose -f docker-compose.single.yml down -v
    
    success "Single node test completed!"
    echo
}

# Function to test multi-node network
test_multi_node() {
    log "Testing Multi-Node MultiVM Network"
    echo "===================================="
    
    # Clean up any existing containers
    log "Cleaning up existing containers..."
    docker-compose -f docker-compose.multi.yml down -v 2>/dev/null || true
    
    # Build the image
    log "Building MultiVM Docker image..."
    docker-compose -f docker-compose.multi.yml build
    
    # Start multi-node network
    log "Starting multi-node network..."
    docker-compose -f docker-compose.multi.yml up -d
    
    # Wait for all nodes to be healthy
    log "Waiting for all nodes to be healthy..."
    local retries=60
    while [ $retries -gt 0 ]; do
        healthy_count=$(docker-compose -f docker-compose.multi.yml ps | grep -c "healthy" || true)
        if [ "$healthy_count" -eq 3 ]; then
            success "All 3 nodes are healthy!"
            break
        fi
        echo -n "."
        sleep 2
        retries=$((retries - 1))
    done
    
    if [ $retries -eq 0 ]; then
        error "Not all nodes became healthy"
        docker-compose -f docker-compose.multi.yml ps
        docker-compose -f docker-compose.multi.yml logs
        return 1
    fi
    
    # Check each node
    log "Checking individual nodes..."
    for port in 8080 8081 8082; do
        if curl -sf http://localhost:$port/health > /dev/null; then
            success "Node on port $port is healthy"
        else
            error "Node on port $port is not healthy"
        fi
    done
    
    # Show status monitor output
    log "Network Status Monitor Output:"
    echo "----------------------------------------"
    docker logs multivm-status-monitor --tail 50
    echo "----------------------------------------"
    
    # Show aggregated logs
    log "Showing aggregated logs for 30 seconds..."
    echo "----------------------------------------"
    timeout 30 docker logs -f multivm-log-aggregator || true
    echo "----------------------------------------"
    
    # Final status check
    log "Final network status:"
    docker-compose -f docker-compose.multi.yml ps
    
    # Stop multi-node network
    log "Stopping multi-node network..."
    docker-compose -f docker-compose.multi.yml down -v
    
    success "Multi-node test completed!"
    echo
}

# Main execution
main() {
    log "Starting MultiVM Docker Network Tests"
    echo "====================================="
    echo
    
    # Check Docker is running
    if ! docker info > /dev/null 2>&1; then
        error "Docker is not running. Please start Docker first."
        exit 1
    fi
    
    # Create logs directory
    mkdir -p logs
    
    # Test based on argument
    case "${1:-both}" in
        single)
            test_single_node
            ;;
        multi)
            test_multi_node
            ;;
        both)
            test_single_node
            echo
            info "Waiting 5 seconds before multi-node test..."
            sleep 5
            echo
            test_multi_node
            ;;
        *)
            echo "Usage: $0 [single|multi|both]"
            exit 1
            ;;
    esac
    
    success "All tests completed!"
    info "Check the ./logs directory for detailed logs"
}

# Run main function
main "$@"