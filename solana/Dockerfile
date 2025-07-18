# Dockerfile for custom Solana build from vm-multiverse/multivm-agave
FROM rust:1.75 as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    git \
    clang \
    cmake \
    make \
    libssl-dev \
    pkg-config \
    libudev-dev \
    && rm -rf /var/lib/apt/lists/*

# Clone the custom Solana repository
WORKDIR /build
RUN git clone -b master https://github.com/vm-multiverse/multivm-agave.git .

# Build Solana
RUN cargo build --release --bin solana-test-validator

# Runtime stage
FROM ubuntu:22.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libudev1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary
COPY --from=builder /build/target/release/solana-test-validator /usr/local/bin/

# Create data directory
RUN mkdir -p /var/solana/ledger

# Expose ports (internal only)
EXPOSE 8899 8900 9900

WORKDIR /var/solana

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/solana-test-validator"]