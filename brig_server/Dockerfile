# syntax=docker/dockerfile:1

FROM rust:1.85 AS builder

# Create app directory
WORKDIR /usr/src/app

# Copy everything and build in release mode
COPY . .
RUN cargo build --release

# Final minimal image
FROM debian:bullseye-slim
WORKDIR /app

# Copy the release binary
COPY --from=builder /usr/src/app/target/release/brig_server .

# Run the binary
CMD ["./brig_server", "-c", "/config.json"]

