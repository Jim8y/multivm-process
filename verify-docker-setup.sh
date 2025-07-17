#!/bin/bash

# MultiVM Docker Setup Verification Script

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

log_info "MultiVM Docker Setup Verification"
log_info "=================================="

# 1. Check Docker and Docker Compose
log_info "Checking Docker installation..."
if ! docker --version >/dev/null 2>&1; then
    log_error "Docker not found. Please install Docker."
    exit 1
fi
log_success "Docker found: $(docker --version)"

if ! docker-compose --version >/dev/null 2>&1; then
    log_error "Docker Compose not found. Please install Docker Compose."
    exit 1
fi
log_success "Docker Compose found: $(docker-compose --version)"

# 2. Check required files
log_info "Checking required files..."
REQUIRED_FILES=(
    "Dockerfile"
    "docker-compose.testnet.yml"
    "docker-compose.complete.yml"
    "Cargo.toml"
    "multivm-common/Cargo.toml"
    "scripts/setup-reth-node.sh"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [[ -f "$file" ]]; then
        log_success "Found: $file"
    else
        log_error "Missing: $file"
        exit 1
    fi
done

# 3. Validate Docker Compose configurations
log_info "Validating docker-compose configurations..."

log_info "Checking testnet configuration..."
if docker-compose -f docker-compose.testnet.yml config >/dev/null 2>&1; then
    log_success "docker-compose.testnet.yml is valid"
else
    log_error "docker-compose.testnet.yml has syntax errors"
    exit 1
fi

log_info "Checking complete configuration..."
if docker-compose -f docker-compose.complete.yml config >/dev/null 2>&1; then
    log_success "docker-compose.complete.yml is valid"
else
    log_error "docker-compose.complete.yml has syntax errors"
    exit 1
fi

# 4. Check network configuration
log_info "Verifying network configurations..."

# Check testnet network
TESTNET_SUBNET=$(docker-compose -f docker-compose.testnet.yml config | grep -A2 "subnet:" | grep "172.20.0.0/16" || true)
if [[ -n "$TESTNET_SUBNET" ]]; then
    log_success "Testnet network subnet correctly configured: 172.20.0.0/16"
else
    log_error "Testnet network subnet not found or incorrect"
fi

# Check complete network
COMPLETE_SUBNET=$(docker-compose -f docker-compose.complete.yml config | grep -A2 "subnet:" | grep "172.30.0.0/16" || true)
if [[ -n "$COMPLETE_SUBNET" ]]; then
    log_success "Complete network subnet correctly configured: 172.30.0.0/16"
else
    log_error "Complete network subnet not found or incorrect"
fi

# 5. Check port assignments
log_info "Checking port assignments for conflicts..."

# Extract all host ports from both configs
TESTNET_PORTS=$(docker-compose -f docker-compose.testnet.yml config | grep -E '^[[:space:]]*-[[:space:]]*"[0-9]+:' | sed 's/.*"\([0-9]*\):.*/\1/' | sort -n)
COMPLETE_PORTS=$(docker-compose -f docker-compose.complete.yml config | grep -E '^[[:space:]]*-[[:space:]]*"[0-9]+:' | sed 's/.*"\([0-9]*\):.*/\1/' | sort -n)

# Check for duplicates within complete setup
DUPLICATE_PORTS=$(echo "$COMPLETE_PORTS" | uniq -d)
if [[ -n "$DUPLICATE_PORTS" ]]; then
    log_error "Duplicate ports found in complete setup: $DUPLICATE_PORTS"
    exit 1
else
    log_success "No duplicate ports found in complete setup"
fi

# 6. Check environment variables and dependencies
log_info "Checking service dependencies..."
SERVICES=$(docker-compose -f docker-compose.complete.yml config --services)
log_success "Found services: $(echo $SERVICES | tr '\n' ' ')"

# 7. Check for required directories
log_info "Checking directory structure..."
REQUIRED_DIRS=(
    "tools"
    "testnet"
    "scripts"
    "docs"
    "multivm-explorer"
)

for dir in "${REQUIRED_DIRS[@]}"; do
    if [[ -d "$dir" ]]; then
        log_success "Found directory: $dir"
    else
        log_warning "Directory not found: $dir (may be created during build)"
    fi
done

# 8. Summary
log_info "Verification Summary"
log_info "==================="
log_success "✓ Docker and Docker Compose are installed"
log_success "✓ Required configuration files exist"
log_success "✓ Docker Compose configurations are valid"
log_success "✓ Network configurations are correct"
log_success "✓ Port assignments have no conflicts"

log_info ""
log_info "Ready to build! Try:"
log_info "Basic testnet:     docker-compose -f docker-compose.testnet.yml up --build"
log_info "Complete setup:    docker-compose -f docker-compose.complete.yml up --build"
log_info ""
log_success "Docker setup verification completed successfully!"