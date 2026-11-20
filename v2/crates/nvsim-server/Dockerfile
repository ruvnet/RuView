# Multi-stage Dockerfile for nvsim-server (ADR-092 §6.2).
#
# Build (from v2/ as context):
#   docker build -f crates/nvsim-server/Dockerfile -t nvsim-server:latest v2
#
# Run:
#   docker run --rm -p 7878:7878 nvsim-server:latest

FROM rust:1.89-slim-bookworm@sha256:d7fc7de78bb8c1469933aeecbf801314d30d7d6e9f0578bba4cfa285bfa37fe6 AS builder
WORKDIR /build
# Debian security revisions intentionally float within the immutable base
# snapshot so rebuilds receive patched packages without brittle version pins.
# kics-scan ignore-line
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy a stub workspace Cargo.toml + Cargo.lock that only references
# nvsim and nvsim-server. This dodges the full RuView workspace's
# wifi-densepose-* deps, none of which nvsim-server uses.
#
# The real manifest is kept under a side name so the stub below can adopt its
# [workspace.package] version. Hard-coding that version here drifts as soon as
# the workspace is bumped, and the drift breaks nvsim-server's path+version
# requirement on nvsim (nvsim = { path = "../nvsim", version = "..." }).
COPY Cargo.lock Cargo.lock
# Builder image pinned to the same release as [toolchain] channel in
# rust-toolchain.toml. The declared channel travels with the build context
# and is checked below, so bumping one without the other fails the build
# loudly instead of silently compiling on a stale rustc.
COPY rust-toolchain.toml toolchain.expected.toml
COPY Cargo.toml workspace.Cargo.toml.real
COPY crates/nvsim crates/nvsim
COPY crates/nvsim-server crates/nvsim-server

RUN cat <<'EOF' > Cargo.toml
[workspace]
resolver = "2"
members = ["crates/nvsim", "crates/nvsim-server"]

[workspace.package]
version = "@WS_VERSION@"
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

# Adopt the real workspace version so the stub cannot drift from it.
RUN WS_VERSION="$(sed -n '/^\[workspace\.package\]/,/^\[/{s/^version *= *"\([^"]*\)".*/\1/p}' workspace.Cargo.toml.real | head -n1)" \
 && test -n "$WS_VERSION" \
 && echo "stub workspace version: $WS_VERSION" \
 && sed -i "s|@WS_VERSION@|$WS_VERSION|" Cargo.toml \
 && rm workspace.Cargo.toml.real

RUN set -eu; \
    want="$(sed -n 's/^channel *= *"\(.*\)"/\1/p' toolchain.expected.toml | head -n1)"; \
    have="$(rustc --version | cut -d' ' -f2)"; \
    echo "rust-toolchain.toml channel=$want builder rustc=$have"; \
    case "$have" in "$want"|"$want".*) ;; \
      *) echo "builder rustc $have does not satisfy rust-toolchain.toml channel $want; bump the FROM tag in this Dockerfile to match" >&2; exit 1 ;; \
    esac
RUN cargo build --release -p nvsim-server --bin nvsim-server

FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818
# Debian security revisions intentionally float within the immutable base
# snapshot so rebuilds receive patched packages without brittle version pins.
# kics-scan ignore-line
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
