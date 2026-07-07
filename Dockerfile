# â”€â”€â”€ praxis Docker image â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
#
# Multi-stage build of the praxis CLI binary (server mode).
# The desktop (Tauri) app is NOT built here â€” it ships as a native
# installer via the release workflow. This image runs `praxis server`.
#
# Build:  docker build -t praxis .
# Run:    docker run -p 8080:8080 -v praxis-data:/data praxis

# â”€â”€â”€ Build stage â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
FROM rustlang/rust:nightly AS builder

# Tauri/WebKit deps are NOT needed â€” we only build the CLI binary,
# which has no GUI dependencies. Keep the image lean.
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the lockfile and workspace manifest first for layer caching.
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Copy all crate sources (the workspace is one unit; partial copies
# break the build because members reference each other by path).
COPY crates ./crates
COPY mcp-servers ./mcp-servers
COPY tests ./tests

# Build only the CLI binary in release mode. --bin praxis matches the
# [[bin]] name in crates/cli/Cargo.toml.
RUN cargo build --release --bin praxis

# â”€â”€â”€ Runtime stage â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/praxis /usr/local/bin/praxis

# Persistent data lives here (SQLite DB, vault, projects.json).
RUN mkdir -p /data
ENV PRAXIS_DATA_DIR=/data
VOLUME ["/data"]

# API server + WebSocket
EXPOSE 8080

# Healthcheck hits the API health endpoint.
HEALTHCHECK --interval=30s --timeout=10s --retries=3 --start-period=10s \
    CMD curl -fsS http://localhost:8080/api/health || exit 1

ENTRYPOINT ["praxis"]
CMD ["server"]
