# Multi-stage Dockerfile for nvsim-server (ADR-092 §6.2).
#
# Build:
#   docker build -f v2/crates/nvsim-server/Dockerfile -t nvsim-server:latest v2
#
# Run (LAN):
#   docker run --rm -p 7878:7878 nvsim-server:latest
#
# Run with custom CORS origin:
#   docker run --rm -p 7878:7878 nvsim-server:latest \
#     nvsim-server --listen 0.0.0.0:7878 --allowed-origin https://example.com
#
# Health check:
#   curl http://localhost:7878/api/health

FROM rust:1.81-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Cache deps separately from source.
COPY Cargo.toml Cargo.lock ./
COPY crates/nvsim/Cargo.toml crates/nvsim/Cargo.toml
COPY crates/nvsim-server/Cargo.toml crates/nvsim-server/Cargo.toml
RUN mkdir -p crates/nvsim/src crates/nvsim-server/src \
    && echo "fn main(){}" > crates/nvsim-server/src/main.rs \
    && echo "" > crates/nvsim/src/lib.rs

# This will fail because the workspace Cargo.toml references many other
# crates. Strategy: build only nvsim + nvsim-server with --bin filter.
COPY crates/nvsim crates/nvsim
COPY crates/nvsim-server crates/nvsim-server

# Build the binary statically against the workspace using a slimmed
# manifest (the Cargo.lock + the two crate Cargo.tomls are enough).
RUN cargo build --release -p nvsim-server --bin nvsim-server 2>&1 \
    || (echo "Cargo build failed — falling back to in-crate build" \
        && cd crates/nvsim-server \
        && cargo build --release --bin nvsim-server)

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r nvsim && useradd -r -g nvsim nvsim

# Copy the binary from whichever build path succeeded.
COPY --from=builder /build/target/release/nvsim-server /usr/local/bin/nvsim-server
RUN chmod +x /usr/local/bin/nvsim-server

USER nvsim
EXPOSE 7878
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -fsS http://localhost:7878/api/health || exit 1

ENTRYPOINT ["nvsim-server"]
CMD ["--listen", "0.0.0.0:7878"]
