# MultiVM Process - Multi-stage Docker Build
FROM rustlang/rust:nightly AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    protobuf-compiler \
    clang \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace files (excluding problematic solana dependencies)
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
RUN cp Cargo.toml Cargo.toml.bak && \
    sed -e '/\"solana-execution-engine\"/d' \
        -e '/solana-execution-engine.*path/d' \
        -e '/^\[profile\.release\.package\.solana-execution-engine\]/,/^$/d' \
        -e '/^\[profile\.release\.package\.agave-validator\]/,/^$/d' Cargo.toml.bak > Cargo.toml && \
    echo "Updated Cargo.toml to exclude problematic dependencies"

# Build the node which includes the application
RUN cargo build --release --bin multivm-node

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Create multivm user
RUN useradd -r -s /bin/false multivm

# Create directories
RUN mkdir -p /opt/multivm/{bin,config,data,logs} && \
    chown -R multivm:multivm /opt/multivm

# Copy binary from builder
COPY --from=builder /app/target/release/multivm-node /opt/multivm/bin/multivm

# Copy configuration templates
COPY docker/config/ /opt/multivm/config/

# Copy scripts
COPY docker/scripts/ /opt/multivm/bin/

# Make scripts executable
RUN chmod +x /opt/multivm/bin/*.sh

# Set working directory
WORKDIR /opt/multivm

# Switch to non-root user for security
USER multivm

# Expose ports
EXPOSE 8080 8545 8899 26656 30303

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Default command
ENTRYPOINT ["/opt/multivm/bin/multivm"]
CMD ["--help"]