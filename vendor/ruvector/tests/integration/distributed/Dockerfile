# Ruvector Distributed Node Dockerfile
FROM rust:1.87-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ ./crates/
COPY examples/ ./examples/

# Build release binaries
RUN cargo build --release -p ruvector-raft -p ruvector-cluster -p ruvector-replication

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    netcat-openbsd \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy built binaries
COPY --from=builder /app/target/release/libruvector_raft.rlib ./
COPY --from=builder /app/target/release/libruvector_cluster.rlib ./
COPY --from=builder /app/target/release/libruvector_replication.rlib ./

# Copy the node runner script
COPY tests/integration/distributed/node_runner.sh ./
RUN chmod +x node_runner.sh

# Environment variables
ENV NODE_ID=""
ENV NODE_ROLE="follower"
ENV RAFT_PORT=7000
ENV CLUSTER_PORT=8000
ENV REPLICATION_PORT=9000
ENV CLUSTER_MEMBERS=""
ENV SHARD_COUNT=64
ENV REPLICATION_FACTOR=3

EXPOSE 7000 8000 9000

CMD ["./node_runner.sh"]
