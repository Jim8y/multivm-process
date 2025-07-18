#!/bin/bash
set -e

echo "🔧 Setting up MultiVM test binaries from custom forks"
echo "===================================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
RETH_REPO="git@github.com:vm-multiverse/reth.git"
RETH_BRANCH="dev"
SOLANA_REPO="git@github.com:vm-multiverse/multivm-agave.git"
SOLANA_BRANCH="master"

WORK_DIR="/tmp/multivm-test-binaries"
RETH_DIR="$WORK_DIR/reth"
SOLANA_DIR="$WORK_DIR/multivm-agave"

# Create work directory
mkdir -p "$WORK_DIR"
cd "$WORK_DIR"

# Function to check if a command exists
check_command() {
    if command -v "$1" >/dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Check prerequisites
echo "Checking prerequisites..."
if ! check_command "git"; then
    echo -e "${RED}❌ git is not installed${NC}"
    exit 1
fi

if ! check_command "cargo"; then
    echo -e "${RED}❌ cargo is not installed${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Prerequisites satisfied${NC}"
echo ""

# Setup Reth
echo "📦 Setting up Reth from custom fork..."
echo "Repository: $RETH_REPO"
echo "Branch: $RETH_BRANCH"

if [ -d "$RETH_DIR" ]; then
    echo "Updating existing Reth repository..."
    cd "$RETH_DIR"
    git fetch origin
    git checkout "$RETH_BRANCH"
    git pull origin "$RETH_BRANCH"
else
    echo "Cloning Reth repository..."
    git clone -b "$RETH_BRANCH" "$RETH_REPO" "$RETH_DIR"
    cd "$RETH_DIR"
fi

# Check if we need to build Reth
RETH_BINARY="$RETH_DIR/target/release/reth"
if [ ! -f "$RETH_BINARY" ] || [ "$1" = "--force-build" ]; then
    echo "Building Reth (this may take a while)..."
    cargo build --release --bin reth
    echo -e "${GREEN}✅ Reth built successfully${NC}"
else
    echo -e "${YELLOW}⚠ Using existing Reth binary${NC}"
fi

# Verify Reth binary
if [ -f "$RETH_BINARY" ]; then
    echo "Reth binary location: $RETH_BINARY"
    $RETH_BINARY --version
else
    echo -e "${RED}❌ Reth binary not found${NC}"
fi

echo ""

# Setup Solana
echo "📦 Setting up Solana from custom fork..."
echo "Repository: $SOLANA_REPO"
echo "Branch: $SOLANA_BRANCH"

if [ -d "$SOLANA_DIR" ]; then
    echo "Updating existing Solana repository..."
    cd "$SOLANA_DIR"
    git fetch origin
    git checkout "$SOLANA_BRANCH"
    git pull origin "$SOLANA_BRANCH"
else
    echo "Cloning Solana repository..."
    git clone -b "$SOLANA_BRANCH" "$SOLANA_REPO" "$SOLANA_DIR"
    cd "$SOLANA_DIR"
fi

# Install Solana dependencies
echo "Installing Solana build dependencies..."
if check_command "apt-get"; then
    # Ubuntu/Debian
    sudo apt-get update
    sudo apt-get install -y pkg-config libudev-dev libssl-dev
elif check_command "yum"; then
    # RHEL/CentOS
    sudo yum install -y pkg-config libudev-devel openssl-devel
elif check_command "brew"; then
    # macOS
    brew install pkg-config
fi

# Build Solana
SOLANA_BINARY="$SOLANA_DIR/target/release/solana-test-validator"
if [ ! -f "$SOLANA_BINARY" ] || [ "$1" = "--force-build" ]; then
    echo "Building Solana (this may take a while)..."
    cd "$SOLANA_DIR"
    cargo build --release --bin solana-test-validator
    echo -e "${GREEN}✅ Solana built successfully${NC}"
else
    echo -e "${YELLOW}⚠ Using existing Solana binary${NC}"
fi

# Verify Solana binary
if [ -f "$SOLANA_BINARY" ]; then
    echo "Solana binary location: $SOLANA_BINARY"
    $SOLANA_BINARY --version
else
    echo -e "${RED}❌ Solana binary not found${NC}"
fi

echo ""

# Create symlinks for easy access
echo "Creating symlinks for test execution..."
SYMLINK_DIR="$HOME/.local/bin"
mkdir -p "$SYMLINK_DIR"

if [ -f "$RETH_BINARY" ]; then
    ln -sf "$RETH_BINARY" "$SYMLINK_DIR/reth-multivm"
    echo "Created symlink: $SYMLINK_DIR/reth-multivm -> $RETH_BINARY"
fi

if [ -f "$SOLANA_BINARY" ]; then
    ln -sf "$SOLANA_BINARY" "$SYMLINK_DIR/solana-test-validator-multivm"
    echo "Created symlink: $SYMLINK_DIR/solana-test-validator-multivm -> $SOLANA_BINARY"
fi

# Export paths for current session
echo ""
echo "🎯 Setup complete! To use these binaries in your current session:"
echo ""
echo "export PATH=\"$SYMLINK_DIR:\$PATH\""
echo "export RETH_BINARY=\"$RETH_BINARY\""
echo "export SOLANA_TEST_VALIDATOR_BINARY=\"$SOLANA_BINARY\""
echo ""
echo "Or add the export commands to your ~/.bashrc or ~/.zshrc"

# Create a test config file
CONFIG_FILE="$WORK_DIR/test-binaries.env"
cat > "$CONFIG_FILE" << EOF
# MultiVM Test Binaries Configuration
export RETH_BINARY="$RETH_BINARY"
export SOLANA_TEST_VALIDATOR_BINARY="$SOLANA_BINARY"
export PATH="$SYMLINK_DIR:\$PATH"

# Custom fork information
export RETH_FORK_REPO="$RETH_REPO"
export RETH_FORK_BRANCH="$RETH_BRANCH"
export SOLANA_FORK_REPO="$SOLANA_REPO"
export SOLANA_FORK_BRANCH="$SOLANA_BRANCH"
EOF

echo ""
echo "Configuration saved to: $CONFIG_FILE"
echo "Source it with: source $CONFIG_FILE"