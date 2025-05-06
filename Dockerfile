# Build stage
FROM rust:latest AS builder

WORKDIR /usr/src/app

# Copy entire project directory
COPY . .

# Build release binary
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
