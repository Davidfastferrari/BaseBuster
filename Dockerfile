# Stage 1: Builder
FROM rust:latest AS builder

# Set the working directory for the application
WORKDIR /usr
#/src/app

# Copy the entire project context
# This will place NodeDB at /usr/src/app/NodeDB, PoolSync at /usr/src/app/PoolSync,
# the main Cargo.toml at /usr/src/app/Cargo.toml, and src at /usr/src/app/src.
COPY . .

# Build the release binary
# This assumes your BaseBuster/Cargo.toml (now at /usr/src/app/Cargo.toml)
# defines dependencies like: node-db = { path = "NodeDB" }
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install necessary runtime dependencies
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/BaseBuster .

# Expose port if needed (optional)
# EXPOSE 8080

# Set the entrypoint to the binary
ENTRYPOINT ["./BaseBuster"]
