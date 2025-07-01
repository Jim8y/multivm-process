#!/bin/bash
# Docker Testnet Management Script for MultiVM

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
COMPOSE_FILE="docker-compose.testnet.yml"
PROJECT_NAME="multivm-testnet"
TESTNET_DIR="./testnet"
COMPOSE_CMD=""  # Will be set by check_dependencies

# Function to print colored output
print_info() {
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

# Function to show usage
usage() {
    echo "Usage: $0 {start|stop|restart|status|logs|clean|build|init}"
    echo
    echo "Commands:"
    echo "  start   - Start the 7-node testnet"
    echo "  stop    - Stop the testnet"
    echo "  restart - Restart the testnet"
    echo "  status  - Show testnet status"
    echo "  logs    - Follow logs from all nodes"
    echo "  clean   - Clean testnet data"
    echo "  build   - Build Docker images"
    echo "  init    - Initialize testnet directories and configs"
    echo
    echo "Examples:"
    echo "  $0 start        # Start testnet"
    echo "  $0 logs node1   # Follow logs for node1"
    echo "  $0 stop         # Stop testnet"
}

# Function to check Docker and Docker Compose
check_dependencies() {
    if ! command -v docker &> /dev/null; then
        print_error "Docker is not installed. Please install Docker first."
        exit 1
    fi

    # Check for Docker Compose v2 (preferred) or v1
    if command -v docker &> /dev/null && docker compose version &> /dev/null; then
        COMPOSE_CMD="docker compose"
    elif command -v $COMPOSE_CMD &> /dev/null; then
        COMPOSE_CMD="$COMPOSE_CMD"
    else
        print_error "Docker Compose is not installed. Please install Docker Compose first."
        exit 1
    fi
}

# Function to initialize testnet directories
init_testnet() {
    print_info "Initializing testnet directories..."
    
    # Create base directories
    for i in {1..7}; do
        mkdir -p "$TESTNET_DIR/node$i/data"
        mkdir -p "$TESTNET_DIR/node$i/logs"
        mkdir -p "$TESTNET_DIR/configs/node$i"
    done
    
    # Create monitoring directories
    mkdir -p "$TESTNET_DIR/prometheus"
    mkdir -p "$TESTNET_DIR/grafana/dashboards"
    mkdir -p "$TESTNET_DIR/grafana/datasources"
    
    # Create Prometheus config
    cat > "$TESTNET_DIR/prometheus/prometheus.yml" <<EOF
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'multivm-nodes'
    static_configs:
      - targets: 
        - '172.20.0.11:9464'  # node1
        - '172.20.0.12:9464'  # node2
        - '172.20.0.13:9464'  # node3
        - '172.20.0.14:9464'  # node4
        - '172.20.0.15:9464'  # node5
        - '172.20.0.16:9464'  # node6
        - '172.20.0.17:9464'  # node7
EOF

    # Create Grafana datasource config
    cat > "$TESTNET_DIR/grafana/datasources/prometheus.yml" <<EOF
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
EOF

    print_success "Testnet directories initialized"
}

# Function to build Docker images
build_images() {
    print_info "Building Docker images..."
    $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" build
    print_success "Docker images built successfully"
}

# Function to start the testnet
start_testnet() {
    print_info "Starting 7-node MultiVM testnet..."
    
    # Check if testnet is already running
    if $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" ps | grep -q "Up"; then
        print_warning "Testnet is already running. Use 'restart' to restart it."
        return
    fi
    
    # Start the services
    $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" up -d
    
    print_success "Testnet started successfully!"
    print_info "Nodes are starting up in sequence..."
    echo
    echo "Node endpoints:"
    echo "  Node 1 (Genesis Validator): http://localhost:8080"
    echo "  Node 2 (Validator):         http://localhost:8180"
    echo "  Node 3 (Validator):         http://localhost:8280"
    echo "  Node 4 (Validator):         http://localhost:8380"
    echo "  Node 5 (Validator):         http://localhost:8480"
    echo "  Node 6 (Full Node):         http://localhost:8580"
    echo "  Node 7 (Full Node):         http://localhost:8680"
    echo
    echo "Monitoring:"
    echo "  Prometheus: http://localhost:9091"
    echo "  Grafana:    http://localhost:3001 (admin/admin)"
    echo
    echo "Use '$0 logs' to follow the logs"
    echo "Use '$0 status' to check node status"
}

# Function to stop the testnet
stop_testnet() {
    print_info "Stopping testnet..."
    $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down
    print_success "Testnet stopped"
}

# Function to restart the testnet
restart_testnet() {
    stop_testnet
    sleep 2
    start_testnet
}

# Function to show testnet status
show_status() {
    print_info "Testnet status:"
    $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" ps
    
    echo
    print_info "Node health checks:"
    for i in {1..7}; do
        port=$((8080 + (i-1)*100))
        if [ $i -eq 1 ]; then
            port=8080
        fi
        
        if curl -s -f "http://localhost:$port/health" > /dev/null 2>&1; then
            echo -e "  Node $i: ${GREEN}Healthy${NC}"
        else
            echo -e "  Node $i: ${RED}Unhealthy${NC}"
        fi
    done
}

# Function to follow logs
follow_logs() {
    if [ -n "$1" ]; then
        # Follow logs for specific service
        print_info "Following logs for $1..."
        $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" logs -f "$1"
    else
        # Follow logs for all services
        print_info "Following logs for all nodes..."
        $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" logs -f
    fi
}

# Function to clean testnet data
clean_testnet() {
    print_warning "This will remove all testnet data. Are you sure? (y/N)"
    read -r response
    if [[ "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
        print_info "Cleaning testnet data..."
        
        # Stop testnet if running
        $COMPOSE_CMD -f "$COMPOSE_FILE" -p "$PROJECT_NAME" down 2>/dev/null || true
        
        # Remove data directories
        rm -rf "$TESTNET_DIR"
        
        # Remove Docker volumes
        docker volume rm "${PROJECT_NAME}_prometheus_data" 2>/dev/null || true
        docker volume rm "${PROJECT_NAME}_grafana_data" 2>/dev/null || true
        
        print_success "Testnet data cleaned"
    else
        print_info "Clean cancelled"
    fi
}

# Main script
main() {
    check_dependencies
    
    case "$1" in
        init)
            init_testnet
            ;;
        build)
            build_images
            ;;
        start)
            if [ ! -d "$TESTNET_DIR" ]; then
                init_testnet
            fi
            start_testnet
            ;;
        stop)
            stop_testnet
            ;;
        restart)
            restart_testnet
            ;;
        status)
            show_status
            ;;
        logs)
            follow_logs "$2"
            ;;
        clean)
            clean_testnet
            ;;
        *)
            usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"