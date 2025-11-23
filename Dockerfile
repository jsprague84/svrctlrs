# Build stage
FROM rust:1.83-bookworm as builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY core ./core
COPY server ./server
COPY scheduler ./scheduler
COPY database ./database
COPY plugins ./plugins
COPY assets ./assets

# Build release binary
RUN cargo build --release --bin server

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    openssh-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/server /app/server
COPY --from=builder /app/assets /app/assets

# Create data directory
RUN mkdir -p /app/data

# Expose port
EXPOSE 8080

# Run server
CMD ["/app/server"]
