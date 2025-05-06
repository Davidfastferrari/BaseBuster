# Build stage
FROM rust:latest AS builder

WORKDIR /usr/src/app

# Copy root manifest files
COPY Cargo.toml Cargo.lock ./

# Copy workspace members' manifest files
COPY NodeDB/Cargo.toml NodeDB/
COPY PoolSync/Cargo.toml PoolSync/

# Create empty src directories for workspace members to allow cargo to fetch dependencies
RUN mkdir -p src NodeDB/src PoolSync/src

# Cache dependencies
RUN cargo fetch

# Copy source code
COPY src ./src
COPY NodeDB ./NodeDB
COPY PoolSync ./PoolSync
COPY contracts ./contracts

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install necessary runtime dependencies (if any)
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/BaseBuster .

# Expose port if needed (optional, user can modify)
# EXPOSE 8080

# Set the entrypoint to the binary
ENTRYPOINT ["./BaseBuster"]
