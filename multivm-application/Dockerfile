# Multi-stage build for MultiVM Application Layer
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies
RUN cargo build --release
RUN rm src/main.rs

# Copy source code
COPY src ./src
COPY examples ./examples
COPY tests ./tests

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false multivm

# Create app directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /usr/src/app/target/release/multivm-application /usr/local/bin/multivm-application

# Copy configuration template
COPY config.example.toml /app/config.example.toml

# Create logs directory
RUN mkdir -p /app/logs && chown multivm:multivm /app/logs

# Change ownership
RUN chown -R multivm:multivm /app

# Switch to app user
USER multivm

# Expose ports
EXPOSE 8080 8081 8082 8083 9090 9091

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Default command
CMD ["multivm-application", "--config", "/app/config.toml"] 