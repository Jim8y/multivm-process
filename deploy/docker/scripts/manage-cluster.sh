#!/bin/bash
# MultiVM Cluster Management Script

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DOCKER_DIR="$PROJECT_ROOT/docker"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${BLUE}[$(date -Iseconds)]${NC} $*"
}

success() {
    echo -e "${GREEN}✅${NC} $*"
}

warning() {
    echo -e "${YELLOW}⚠️${NC} $*"
}

error() {
    echo -e "${RED}❌${NC} $*"
}

# Help function
show_help() {
    cat << EOF
MultiVM Cluster Management Script

Usage: $0 [COMMAND] [OPTIONS]

Commands:
    build           Build MultiVM Docker images
    start           Start the multi-node cluster
    stop            Stop the cluster
    restart         Restart the cluster
    status          Show cluster status
    logs [NODE]     Show logs (optional node filter)
    test            Run multi-node tests
    clean           Clean up all containers and volumes
    reset           Reset the entire environment
    scale [N]       Scale to N validator nodes
    monitor         Start monitoring stack
    shell [NODE]    Connect to a node shell

Examples:
    $0 build
    $0 start
    $0 logs multivm-node-1
    $0 test
    $0 scale 5
    $0 shell multivm-node-2

EOF
}

# Build images
build_images() {
    log "Building MultiVM Docker images..."
    
    cd "$PROJECT_ROOT"
    
    # Build main image
    docker build -t multivm-node:latest -f Dockerfile .
    success "Main MultiVM image built"
    
    # Build test image
    docker build -t multivm-test:latest -f docker/Dockerfile.test .
    success "Test runner image built"
    
    success "All images built successfully"
}

# Start cluster
start_cluster() {
    log "Starting MultiVM cluster..."
    
    cd "$DOCKER_DIR"
    
    # Create network if it doesn't exist
    docker network create multivm-net 2>/dev/null || true
    
    # Start core nodes
    docker compose up -d multivm-node-1 multivm-node-2 multivm-node-3 multivm-node-4
    
    log "Waiting for nodes to become healthy..."
    
    # Wait for nodes to be healthy
    local nodes=("multivm-node-1" "multivm-node-2" "multivm-node-3" "multivm-node-4")
    local max_wait=120
    local waited=0
    
    while [ $waited -lt $max_wait ]; do
        local healthy_count=0
        
        for node in "${nodes[@]}"; do
            if docker compose ps "$node" | grep -q "healthy"; then
                healthy_count=$((healthy_count + 1))
            fi
        done
        
        if [ $healthy_count -eq ${#nodes[@]} ]; then
            success "All nodes are healthy!"
            break
        fi
        
        log "Waiting for nodes to be healthy ($healthy_count/${#nodes[@]})..."
        sleep 5
        waited=$((waited + 5))
    done
    
    if [ $waited -ge $max_wait ]; then
        error "Timeout waiting for nodes to become healthy"
        return 1
    fi
    
    success "MultiVM cluster started successfully"
    show_cluster_info
}

# Stop cluster
stop_cluster() {
    log "Stopping MultiVM cluster..."
    
    cd "$DOCKER_DIR"
    docker compose down
    
    success "Cluster stopped"
}

# Restart cluster
restart_cluster() {
    log "Restarting MultiVM cluster..."
    stop_cluster
    sleep 5
    start_cluster
}

# Show cluster status
show_status() {
    log "MultiVM Cluster Status:"
    
    cd "$DOCKER_DIR"
    
    # Show container status
    echo ""
    echo "Containers:"
    docker compose ps
    
    # Show network info
    echo ""
    echo "Network Information:"
    docker network inspect multivm-net --format '{{range .Containers}}{{.Name}}: {{.IPv4Address}}{{"\n"}}{{end}}' 2>/dev/null || echo "Network not found"
    
    # Show node health
    echo ""
    echo "Node Health:"
    local nodes=("multivm-node-1:8080" "multivm-node-2:8080" "multivm-node-3:8080" "multivm-node-4:8080")
    
    for node_addr in "${nodes[@]}"; do
        local node_name=$(echo "$node_addr" | cut -d':' -f1)
        if curl -sf "http://localhost:${node_addr#*:}/health" > /dev/null 2>&1; then
            success "$node_name: HEALTHY"
        else
            error "$node_name: UNHEALTHY"
        fi
    done
}

# Show logs
show_logs() {
    local node=$1
    
    cd "$DOCKER_DIR"
    
    if [ -n "$node" ]; then
        log "Showing logs for $node:"
        docker compose logs -f "$node"
    else
        log "Showing logs for all nodes:"
        docker compose logs -f
    fi
}

# Run tests
run_tests() {
    log "Running multi-node tests..."
    
    cd "$DOCKER_DIR"
    
    # Ensure test results directory exists
    mkdir -p test-results
    
    # Run tests
    docker compose --profile testing run --rm test-runner
    
    # Show test results
    if [ -f "test-results/test_summary.json" ]; then
        log "Test Summary:"
        cat test-results/test_summary.json | jq '.'
        success "Tests completed! Check test-results/ directory for detailed results."
    else
        warning "No test summary found"
    fi
}

# Clean up
clean_up() {
    log "Cleaning up MultiVM environment..."
    
    cd "$DOCKER_DIR"
    
    # Stop and remove containers
    docker compose down -v --remove-orphans
    
    # Remove images
    docker rmi multivm-node:latest multivm-test:latest 2>/dev/null || true
    
    # Remove network
    docker network rm multivm-net 2>/dev/null || true
    
    # Remove test results
    rm -rf test-results logs
    
    success "Environment cleaned up"
}

# Reset environment
reset_environment() {
    warning "This will completely reset the MultiVM environment"
    read -p "Are you sure? (y/N): " -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        clean_up
        build_images
        success "Environment reset complete"
    else
        log "Reset cancelled"
    fi
}

# Scale cluster
scale_cluster() {
    local target_nodes=$1
    
    if [ -z "$target_nodes" ] || [ "$target_nodes" -lt 1 ] || [ "$target_nodes" -gt 10 ]; then
        error "Invalid node count. Please specify 1-10 nodes."
        return 1
    fi
    
    log "Scaling cluster to $target_nodes validator nodes..."
    
    # For now, we support up to 4 nodes (as defined in docker-compose.yml)
    if [ "$target_nodes" -gt 4 ]; then
        warning "Scaling beyond 4 nodes requires additional configuration"
        warning "Current implementation supports up to 4 nodes"
        return 1
    fi
    
    cd "$DOCKER_DIR"
    
    # Start the requested number of nodes
    local services=()
    for i in $(seq 1 "$target_nodes"); do
        services+=("multivm-node-$i")
    done
    
    docker compose up -d "${services[@]}"
    
    success "Cluster scaled to $target_nodes nodes"
}

# Start monitoring
start_monitoring() {
    log "Starting monitoring stack..."
    
    cd "$DOCKER_DIR"
    docker compose --profile monitoring up -d monitoring log-aggregator
    
    success "Monitoring started"
    log "Prometheus: http://localhost:9090"
    log "Loki: http://localhost:3100"
}

# Connect to node shell
connect_shell() {
    local node=$1
    
    if [ -z "$node" ]; then
        node="multivm-node-1"
    fi
    
    log "Connecting to $node shell..."
    docker exec -it "$node" /bin/bash
}

# Show cluster information
show_cluster_info() {
    echo ""
    log "MultiVM Cluster Information:"
    echo ""
    echo "API Endpoints:"
    echo "  Node 1: http://localhost:8080"
    echo "  Node 2: http://localhost:8081"
    echo "  Node 3: http://localhost:8082"
    echo "  Node 4: http://localhost:8083"
    echo ""
    echo "P2P Endpoints:"
    echo "  Node 1: localhost:26656"
    echo "  Node 2: localhost:26657"
    echo "  Node 3: localhost:26658"
    echo "  Node 4: localhost:26659"
    echo ""
    echo "Health Checks:"
    for i in {1..4}; do
        echo "  curl http://localhost:808$((i-1))/health"
    done
    echo ""
}

# Main command dispatcher
main() {
    case "$1" in
        build)
            build_images
            ;;
        start)
            start_cluster
            ;;
        stop)
            stop_cluster
            ;;
        restart)
            restart_cluster
            ;;
        status)
            show_status
            ;;
        logs)
            show_logs "$2"
            ;;
        test)
            run_tests
            ;;
        clean)
            clean_up
            ;;
        reset)
            reset_environment
            ;;
        scale)
            scale_cluster "$2"
            ;;
        monitor)
            start_monitoring
            ;;
        shell)
            connect_shell "$2"
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            error "Unknown command: $1"
            echo ""
            show_help
            exit 1
            ;;
    esac
}

# Check if Docker and Docker Compose are available
check_dependencies() {
    if ! command -v docker &> /dev/null; then
        error "Docker is not installed or not in PATH"
        exit 1
    fi
    
    if ! docker compose version &> /dev/null; then
        error "Docker Compose is not installed or not available"
        exit 1
    fi
}

# Entry point
if [ $# -eq 0 ]; then
    show_help
    exit 1
fi

check_dependencies
main "$@"