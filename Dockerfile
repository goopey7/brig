# syntax=docker/dockerfile:1

FROM rust:1.85 AS builder

WORKDIR /usr/src/app
COPY . .
RUN cargo build --release -p brig_server

# Final minimal image
FROM debian:bookworm-slim
WORKDIR /app

# Install SSH client
RUN apt-get update && \
    apt-get install -y --no-install-recommends openssh-client && \
    rm -rf /var/lib/apt/lists/*

# Copy the release binary
COPY --from=builder /usr/src/app/target/release/brig_server .

# Run the binary
CMD ["./brig_server", "-c", "/app/config.json"]

