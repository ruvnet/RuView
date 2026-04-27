# Multi-stage Dockerfile for nvsim-server (ADR-092 §6.2).
#
# Build (from v2/ as context):
#   docker build -f crates/nvsim-server/Dockerfile -t nvsim-server:latest v2
#
# Run:
#   docker run --rm -p 7878:7878 nvsim-server:latest

FROM rust:1.81-slim-bookworm AS builder
WORKDIR /build
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy a stub workspace Cargo.toml + Cargo.lock that only references
# nvsim and nvsim-server. This dodges the full RuView workspace's
# wifi-densepose-* deps, none of which nvsim-server uses.
COPY Cargo.lock Cargo.lock
COPY crates/nvsim crates/nvsim
COPY crates/nvsim-server crates/nvsim-server

RUN cat <<'EOF' > Cargo.toml
[workspace]
resolver = "2"
members = ["crates/nvsim", "crates/nvsim-server"]

[workspace.package]
version = "0.3.0"
edition = "2021"
authors = ["rUv <ruv@ruv.net>"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/ruvnet/RuView"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
sha2 = { version = "0.10", default-features = false }
thiserror = "1.0"
tracing = "0.1"
tokio = { version = "1.35", features = ["full"] }
axum = { version = "0.7", features = ["ws", "macros"] }
tower = { version = "0.4", features = ["full"] }
tower-http = { version = "0.5", features = ["cors", "trace", "compression-gzip"] }
criterion = "0.5"
EOF

RUN cargo build --release -p nvsim-server --bin nvsim-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r nvsim && useradd -r -g nvsim nvsim

COPY --from=builder /build/target/release/nvsim-server /usr/local/bin/nvsim-server
RUN chmod +x /usr/local/bin/nvsim-server

USER nvsim
EXPOSE 7878
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s \
    CMD curl -fsS http://localhost:7878/api/health || exit 1

ENTRYPOINT ["nvsim-server"]
CMD ["--listen", "0.0.0.0:7878"]
