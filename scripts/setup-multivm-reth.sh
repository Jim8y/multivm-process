#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RETH_DIR="$PROJECT_ROOT/reth"

echo -e "${BLUE}=== Setting up MultiVM Reth Fork ===${NC}"
echo

# Clone or update the Reth fork
if [ -d "$RETH_DIR" ]; then
    echo -e "${YELLOW}Reth directory exists. Updating...${NC}"
    cd "$RETH_DIR"
    git fetch origin
    git checkout dev
    git pull origin dev
else
    echo -e "${YELLOW}Cloning MultiVM Reth fork...${NC}"
    cd "$PROJECT_ROOT"
    git clone git@github.com:vm-multiverse/reth.git
    cd "$RETH_DIR"
    git checkout dev
fi

# Create Dockerfile for Reth if it doesn't exist
if [ ! -f "$RETH_DIR/Dockerfile" ]; then
    echo -e "${YELLOW}Creating Reth Dockerfile...${NC}"
    cat > "$RETH_DIR/Dockerfile" << 'EOF'
# Build stage
FROM rust:1.75-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    clang \
    cmake \
    git \
    libssl-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Set up working directory
WORKDIR /app

# Copy source code
COPY . .

# Build Reth with MultiVM support
RUN cargo build --release --features multivm

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /app/target/release/reth /usr/local/bin/reth

# Create data directory
RUN mkdir -p /root/.local/share/reth

# Expose ports
EXPOSE 8545 8546 8551 30303 30303/udp 9001

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/reth"]
EOF
fi

# Create a MultiVM-specific Reth configuration
echo -e "${YELLOW}Creating Reth MultiVM configuration...${NC}"
cat > "$PROJECT_ROOT/configs/reth-multivm.toml" << 'EOF'
# Reth configuration for MultiVM integration

[network]
# Disable P2P for MultiVM-controlled network
discovery = false
bootnodes = []

[rpc]
# Enable all RPC modules for MultiVM
http_modules = ["eth", "net", "web3", "debug", "trace", "txpool", "admin", "rpc"]
ws_modules = ["eth", "net", "web3", "debug", "trace", "txpool", "admin", "rpc"]

[engine]
# Engine API configuration for MultiVM
enabled = true

[chain]
# Chain configuration
chain_id = 12345  # MultiVM testnet chain ID

[pruning]
# Disable pruning for testing
mode = "archive"

[log]
# Logging configuration
level = "debug"
EOF

echo -e "${GREEN}✓ MultiVM Reth setup complete!${NC}"
echo
echo "Next steps:"
echo "1. Generate JWT secret: ./scripts/generate-jwt-token.sh"
echo "2. Start the testnet: ./scripts/start-testnet.sh"
echo
echo "The Reth fork is located at: $RETH_DIR"
echo "Configuration is at: $PROJECT_ROOT/configs/reth-multivm.toml"