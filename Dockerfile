# Multi-stage build for optimal image size
FROM rust:1.82.0-slim as builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    sqlite3 \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release
RUN rm src/main.rs

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build the actual application
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    sqlite3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd --create-home --shell /bin/bash app

# Create app directory and set permissions
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/trading-system /app/trading-system
COPY --from=builder /app/migrations /app/migrations

# Create directories for data and logs with proper permissions
RUN mkdir -p /app/data /app/logs /app/config && \
    chown -R app:app /app && \
    chmod -R 755 /app

# Switch to app user
USER app

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Expose ports
EXPOSE 8080 9090

# Set environment variables
ENV RUST_LOG=info
ENV DATABASE_URL=sqlite:/app/data/trading.db

# Run the application
CMD ["./trading-system"]