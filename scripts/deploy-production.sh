#!/bin/bash
# Production deployment script for MultiVM

set -e

# Configuration
DEPLOYMENT_ENV=${1:-production}
DOCKER_REGISTRY=${DOCKER_REGISTRY:-"docker.io/multivm"}
VERSION=${VERSION:-"latest"}

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Functions
print_header() {
    echo -e "${BLUE}======================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}======================================${NC}"
}

print_status() {
    local status=$1
    local message=$2
    case $status in
        "info") echo -e "${YELLOW}[INFO]${NC} $message" ;;
        "success") echo -e "${GREEN}[SUCCESS]${NC} $message" ;;
        "error") echo -e "${RED}[ERROR]${NC} $message" ;;
    esac
}

# Main execution
main() {
    print_header "MultiVM Production Deployment"
    print_status "info" "Deployment environment: $DEPLOYMENT_ENV"
    print_status "info" "Version: $VERSION"
    
    # Create necessary directories
    mkdir -p data/{node1,node2,node3}
    mkdir -p configs/{prometheus,grafana,nginx,certs}
    
    # Deploy with docker-compose
    print_status "info" "Starting services..."
    docker-compose -f docker-compose.production.yml up -d
    
    print_status "success" "Deployment complete!"
}

# Run main function
main "$@"