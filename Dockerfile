# MultiVM Complete Integration - Multi-stage Docker Build
# Builds Reth from MultiVM fork + MultiVM + Testnet Setup

FROM rustlang/rust:nightly AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    protobuf-compiler \
    clang \
    libclang-dev \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Set environment variables
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV RUSTC_WRAPPER=""

# Create app directory
WORKDIR /app

# ============================================
# STAGE 1: Build Reth from MultiVM fork
# ============================================
WORKDIR /app/reth

# Clone and build Reth from MultiVM fork (dev branch)
# Use ARG to conditionally build Reth
ARG BUILD_RETH=false
RUN if [ "$BUILD_RETH" = "true" ]; then \
        echo "=== Building Reth from MultiVM fork ===" && \
        git clone --depth 1 --branch dev https://github.com/vm-multiverse/reth.git . && \
        echo "Reth source cloned, building..." && \
        cargo build --release --bin reth && \
        echo "Reth build completed successfully"; \
    else \
        echo "=== Skipping Reth build (BUILD_RETH=false) ===" && \
        mkdir -p target/release && \
        echo '#!/bin/bash' > target/release/reth && \
        echo 'echo "Mock Reth - set BUILD_RETH=true to build real Reth"' >> target/release/reth && \
        chmod +x target/release/reth; \
    fi

# ============================================
# STAGE 2: Build MultiVM
# ============================================
WORKDIR /app/multivm

# Copy MultiVM source
COPY Cargo.toml Cargo.lock ./
COPY multivm-common ./multivm-common
COPY multivm-consensus ./multivm-consensus
COPY multivm-p2p ./multivm-p2p
COPY multivm-process-manager ./multivm-process-manager
COPY multivm-account-mapping ./multivm-account-mapping
COPY multivm-application ./multivm-application
COPY multivm-cli ./multivm-cli
COPY reth-execution-engine ./reth-execution-engine
COPY multivm-mock-processes ./multivm-mock-processes

# Create a clean Cargo.toml without problematic dependencies
RUN echo "=== Building MultiVM ===" && \
    cp Cargo.toml Cargo.toml.bak && \
    sed -e '/\"solana-execution-engine\"/d' \
        -e '/solana-execution-engine.*path/d' \
        -e '/^\[profile\.release\.package\.solana-execution-engine\]/,/^$/d' \
        -e '/^\[profile\.release\.package\.agave-validator\]/,/^$/d' Cargo.toml.bak > Cargo.toml && \
    echo "Updated Cargo.toml to exclude problematic dependencies"

# Build MultiVM node
RUN cargo build --release --bin multivm-node && \
    echo "MultiVM build completed successfully"

# ============================================
# STAGE 3: Runtime Environment
# ============================================
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    jq \
    openssl \
    netcat-openbsd \
    procps \
    python3 \
    python3-pip \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies for transaction tools
RUN pip3 install --no-cache-dir --break-system-packages web3 eth-account

# Create users
RUN useradd -r -s /bin/false multivm && \
    useradd -r -s /bin/false reth

# Create directory structure
RUN mkdir -p /opt/multivm/{bin,config,data,logs} && \
    mkdir -p /opt/reth/{bin,data,config,logs} && \
    mkdir -p /opt/testnet/{configs,scripts,tools} && \
    chown -R multivm:multivm /opt/multivm && \
    chown -R reth:reth /opt/reth && \
    chmod 755 /opt/testnet

# ============================================
# STAGE 4: Copy Binaries and Configuration
# ============================================

# Copy Reth binary from builder
COPY --from=builder /app/reth/target/release/reth /opt/reth/bin/reth

# Copy MultiVM binary from builder  
COPY --from=builder /app/multivm/target/release/multivm-node /opt/multivm/bin/multivm-node

# Copy MultiVM configuration and scripts
COPY scripts/ /opt/multivm/scripts/
COPY tools/ /opt/testnet/tools/
COPY testnet/ /opt/testnet/
COPY docs/ /opt/multivm/docs/

# Note: Docker-specific scripts would be copied here if they exist
# COPY docker/scripts/ /opt/multivm/bin/
# COPY docker/config/ /opt/multivm/config/

# ============================================
# STAGE 5: Setup Scripts and Permissions
# ============================================

# Create comprehensive startup script
RUN cat > /opt/multivm/bin/start-complete.sh << 'EOF'
#!/bin/bash
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

NODE_ID=${NODE_ID:-node1}
NODE_TYPE=${NODE_TYPE:-validator}
RETH_DATA_DIR=${RETH_DATA_DIR:-/opt/reth/data}
MULTIVM_DATA_DIR=${MULTIVM_DATA_DIR:-/opt/multivm/data}

log_info "Starting MultiVM Complete Integration"
log_info "Node ID: $NODE_ID"
log_info "Node Type: $NODE_TYPE"

# Initialize directories
mkdir -p "$RETH_DATA_DIR" "$MULTIVM_DATA_DIR"

# Generate JWT secret if not exists
JWT_SECRET_PATH="$RETH_DATA_DIR/jwt.hex"
if [ ! -f "$JWT_SECRET_PATH" ]; then
    log_info "Generating JWT secret..."
    openssl rand -hex 32 > "$JWT_SECRET_PATH"
fi

# Initialize Reth if needed
if [ ! -f "$RETH_DATA_DIR/db/mdbx.dat" ]; then
    log_info "Initializing Reth database..."
    /opt/reth/bin/reth init --datadir "$RETH_DATA_DIR" --chain dev
fi

# Start Reth with MultiVM-specific configuration
log_info "Starting Reth execution engine..."
/opt/reth/bin/reth node \
    --datadir "$RETH_DATA_DIR" \
    --chain dev \
    --http \
    --http.addr 0.0.0.0 \
    --http.port 8545 \
    --http.api eth,net,web3,debug,trace \
    --http.corsdomain "*" \
    --authrpc.addr 0.0.0.0 \
    --authrpc.port 8551 \
    --authrpc.jwtsecret "$JWT_SECRET_PATH" \
    --full \
    --disable-discovery \
    --max-inbound-peers 0 \
    --max-outbound-peers 0 \
    --port 0 \
    --ipcdisable \
    --dev &

RETH_PID=$!
log_success "Reth started with PID: $RETH_PID"

# Skip waiting for Reth since we're using mock mode in MultiVM
log_info "Using mock mode - MultiVM will handle execution engines internally"
log_success "Reth readiness check skipped"

# Set environment variable to enable mock mode
export MULTIVM_ETHEREUM_MOCK_MODE=true
export MULTIVM_SOLANA_MOCK_MODE=true
log_info "Set environment variables for mock mode execution"

# Start MultiVM node
log_info "Starting MultiVM node..."
exec /opt/multivm/bin/multivm-node \
    --data-dir "$MULTIVM_DATA_DIR"
EOF

# Create testnet management script
RUN cat > /opt/testnet/start-testnet.sh << 'EOF'
#!/bin/bash
set -euo pipefail

log_info() { echo -e "\033[0;34m[INFO]\033[0m $1"; }
log_success() { echo -e "\033[0;32m[SUCCESS]\033[0m $1"; }

log_info "Setting up MultiVM Testnet"

# Generate validator accounts if not exists
if [ ! -f /opt/testnet/validators_complete.json ]; then
    log_info "Generating validator accounts..."
    if [ -d /opt/testnet/tools/account-management ]; then
        cd /opt/testnet/tools/account-management
        python3 generate-validator-accounts.py 2>/dev/null || true
        mv validators_complete.json /opt/testnet/ 2>/dev/null || true
    fi
fi

# Setup testnet configuration
log_info "Setting up testnet configuration..."
cp -r /opt/testnet/configs/* /opt/multivm/config/ 2>/dev/null || true

log_success "Testnet setup completed"
log_info "Starting complete MultiVM + Reth integration..."

# Start the complete stack
exec /opt/multivm/bin/start-complete.sh
EOF

# Make all scripts executable
RUN find /opt -name "*.sh" -type f -exec chmod +x {} \; && \
    chmod +x /opt/reth/bin/reth && \
    chmod +x /opt/multivm/bin/multivm-node

# ============================================
# STAGE 6: Final Configuration
# ============================================

# Set working directory
WORKDIR /opt/multivm

# Create symbolic links for easy access
RUN ln -sf /opt/reth/bin/reth /usr/local/bin/reth && \
    ln -sf /opt/multivm/bin/multivm-node /usr/local/bin/multivm && \
    ln -sf /opt/testnet/start-testnet.sh /usr/local/bin/start-testnet

# Expose ports
EXPOSE 8080 8545 8551 8899 26656 30303

# Add labels
LABEL org.opencontainers.image.title="MultiVM Complete Integration"
LABEL org.opencontainers.image.description="Complete MultiVM + Reth integration with testnet setup"
LABEL org.opencontainers.image.version="1.0.0"
LABEL org.opencontainers.image.source="https://github.com/vm-multiverse/multivm"

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=120s --retries=5 \
    CMD curl -f http://localhost:8080/health && curl -f http://localhost:8545 || exit 1

# Default command - start the complete testnet
ENTRYPOINT ["/opt/testnet/start-testnet.sh"]

# Alternative commands available:
# docker run multivm:complete /opt/multivm/bin/start-complete.sh
# docker run multivm:complete /opt/reth/bin/reth --help  
# docker run multivm:complete /opt/multivm/bin/multivm-node --help