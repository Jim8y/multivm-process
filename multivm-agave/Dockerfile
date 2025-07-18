# MultiVM Agave (Solana Fork) Dockerfile
FROM rust:1.75 as builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    protobuf-compiler \
    clang \
    cmake \
    libudev-dev \
    && rm -rf /var/lib/apt/lists/*

# Clone and build multivm-agave
WORKDIR /build
RUN git clone https://github.com/vm-multiverse/multivm-agave.git . && \
    git checkout master

# Build the validator
RUN cargo build --release --bin solana-validator

# Runtime stage
FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libudev1 \
    && rm -rf /var/lib/apt/lists/*

# Copy the built binary
COPY --from=builder /build/target/release/solana-validator /usr/local/bin/

# Create data directory
RUN mkdir -p /var/solana

WORKDIR /var/solana

# Entry point
ENTRYPOINT ["solana-validator"]