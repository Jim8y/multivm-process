# MultiVM Reth Fork Dockerfile
FROM rust:1.75 as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    git \
    clang \
    libclang-dev \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Clone and build vm-multiverse/reth
WORKDIR /build
RUN git clone https://github.com/vm-multiverse/reth.git . && \
    git checkout dev

# Build reth with MultiVM modifications
RUN cargo build --release --bin reth

# Runtime stage
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary
COPY --from=builder /build/target/release/reth /usr/local/bin/

# Create data directory
RUN mkdir -p /root/.local/share/reth

WORKDIR /root

# Health check for container
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8545 || exit 1

# Entry point
ENTRYPOINT ["reth"]