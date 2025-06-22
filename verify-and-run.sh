#!/bin/bash
# Verification and run script for MultiVM single node

set -e

echo "======================================"
echo "MultiVM Single Node Verification"
echo "======================================"
echo ""

# Check if user can run Docker
echo "1. Checking Docker access..."
if docker info >/dev/null 2>&1; then
    echo "✓ Docker is accessible"
    DOCKER_AVAILABLE=true
else
    echo "✗ Docker requires sudo or user is not in docker group"
    echo "  To fix: sudo usermod -aG docker $USER && newgrp docker"
    DOCKER_AVAILABLE=false
fi

# Check if Docker compose is available
echo ""
echo "2. Checking Docker Compose..."
if command -v docker-compose >/dev/null 2>&1; then
    echo "✓ docker-compose command available"
    COMPOSE_CMD="docker-compose"
elif docker compose version >/dev/null 2>&1; then
    echo "✓ docker compose command available"
    COMPOSE_CMD="docker compose"
else
    echo "✗ Docker Compose not found"
    exit 1
fi

# Build the Rust project first
echo ""
echo "3. Building MultiVM project..."
echo "This may take a few minutes on first build..."
cd /home/neo/git/multivm-process

# Check if we need to build
if [ ! -f "target/release/multivm-node" ]; then
    echo "Building in release mode..."
    cargo build --release --bin multivm-node
else
    echo "✓ Binary already built"
fi

# Verify the build
if [ -f "target/release/multivm-node" ]; then
    echo "✓ MultiVM node binary built successfully"
else
    echo "✗ Failed to build MultiVM node"
    exit 1
fi

# Check configuration
echo ""
echo "4. Verifying configuration..."
if [ -f "docker-compose.single.yml" ]; then
    echo "✓ Docker Compose configuration found"
    echo ""
    echo "Environment configuration:"
    grep -E "(NODE_ID|VALIDATOR|BLOCK_GENERATION)" docker-compose.single.yml | grep -v "#" | sed 's/^/  /'
else
    echo "✗ docker-compose.single.yml not found"
    exit 1
fi

# If Docker is available, offer to run
if [ "$DOCKER_AVAILABLE" = true ]; then
    echo ""
    echo "======================================"
    echo "Ready to run MultiVM single node!"
    echo "======================================"
    echo ""
    echo "Would you like to:"
    echo "1) Run with Docker (recommended)"
    echo "2) Run locally without Docker"
    echo "3) Exit"
    echo ""
    read -p "Enter choice (1-3): " choice

    case $choice in
        1)
            echo ""
            echo "Starting MultiVM with Docker..."
            ./run-single-node.sh
            ;;
        2)
            echo ""
            echo "Starting MultiVM locally..."
            export NODE_ID=single-node
            export NODE_TYPE=bootstrap
            export CONSENSUS_ROLE=validator
            export VALIDATOR_KEY=single-validator-key
            export BLOCK_GENERATION_ENABLED=true
            export BLOCK_INTERVAL_MS=2000
            export SVM_TX_PER_BLOCK=3
            export EVM_TX_PER_BLOCK=3
            export RUST_LOG=info,multivm=debug
            
            echo "Environment variables set:"
            env | grep -E "(NODE_|BLOCK_|VALIDATOR|RUST_LOG)" | sort
            echo ""
            echo "Starting node..."
            ./target/release/multivm-node
            ;;
        3)
            echo "Exiting..."
            exit 0
            ;;
        *)
            echo "Invalid choice"
            exit 1
            ;;
    esac
else
    echo ""
    echo "======================================"
    echo "Docker not accessible"
    echo "======================================"
    echo ""
    echo "You can still run locally without Docker:"
    echo ""
    echo "export NODE_ID=single-node"
    echo "export NODE_TYPE=bootstrap"
    echo "export CONSENSUS_ROLE=validator"
    echo "export VALIDATOR_KEY=single-validator-key"
    echo "export BLOCK_GENERATION_ENABLED=true"
    echo "export BLOCK_INTERVAL_MS=2000"
    echo "export RUST_LOG=info,multivm=debug"
    echo "./target/release/multivm-node"
    echo ""
    echo "Or to use Docker, add yourself to the docker group:"
    echo "sudo usermod -aG docker $USER && newgrp docker"
fi