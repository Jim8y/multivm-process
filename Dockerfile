# MultiVM Process - Multi-stage Docker Build
FROM rust:1.70-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Build the application
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
COPY --from=builder /app/target/release/multivm-node /opt/multivm/bin/

# Copy configuration templates
COPY docker/config/ /opt/multivm/config/

# Copy scripts
COPY docker/scripts/ /opt/multivm/bin/

# Make scripts executable
RUN chmod +x /opt/multivm/bin/*.sh

# Set working directory
WORKDIR /opt/multivm

# Switch to multivm user
USER multivm

# Expose ports
EXPOSE 8080 8545 8899 26656 30303

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Default command
CMD ["/opt/multivm/bin/start.sh"]