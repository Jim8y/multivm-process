#!/bin/bash

# Quick Start Script for MultiVM Reth Integration
# This script provides a one-command setup for development

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}=== MultiVM Reth Quick Start ===${NC}"
echo ""

# Step 1: Install Reth if needed
if ! command -v reth &> /dev/null; then
    echo -e "${BLUE}Step 1: Installing Reth...${NC}"
    "${SCRIPT_DIR}/setup-reth-node.sh" install
else
    echo -e "${GREEN}Step 1: Reth already installed ✓${NC}"
fi

# Step 2: Setup Reth for development
echo -e "${BLUE}Step 2: Setting up Reth for development...${NC}"
"${SCRIPT_DIR}/setup-reth-node.sh" setup --dev

# Step 3: Start Reth
echo -e "${BLUE}Step 3: Starting Reth node...${NC}"
"${SCRIPT_DIR}/setup-reth-node.sh" start

# Step 4: Run integration tests
echo -e "${BLUE}Step 4: Running integration tests...${NC}"
sleep 5  # Give Reth a moment to fully start
"${SCRIPT_DIR}/test-multivm-reth-integration.sh"

echo ""
echo -e "${GREEN}=== Quick Start Complete! ===${NC}"
echo ""
echo "Reth is now running and ready for MultiVM integration."
echo ""
echo "Useful commands:"
echo "  ./scripts/setup-reth-node.sh status   # Check status"
echo "  ./scripts/setup-reth-node.sh logs     # View logs"
echo "  ./scripts/setup-reth-node.sh stop     # Stop Reth"
echo ""
echo "To use in your Rust code:"
echo "  cargo run --bin multivm-node --features ethereum"
echo ""