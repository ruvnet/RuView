# Multi-stage Dockerfile for optimized Cloud Run deployment
# Combines Rust (ruvector core) and Node.js (service layer)

# Stage 1: Build Rust ruvector core
FROM rust:1.75-slim as rust-builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Copy Rust source
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates

# Build release binary with optimizations
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
RUN cargo build --release --bin ruvector

# Stage 2: Build Node.js bindings
FROM node:20-slim as node-builder

WORKDIR /build

# Install build dependencies
RUN apt-get update && apt-get install -y \
    python3 \
    make \
    g++ \
    && rm -rf /var/lib/apt/lists/*

# Copy package files
COPY package*.json ./
COPY tsconfig.json ./

# Install dependencies
RUN npm ci --include=dev

# Copy source files
COPY src ./src

# Build TypeScript
RUN npm run build

# Prune dev dependencies
RUN npm prune --production

# Stage 3: Final runtime image
FROM gcr.io/distroless/nodejs20-debian12:nonroot

WORKDIR /app

# Copy Rust binary
COPY --from=rust-builder /build/target/release/ruvector /usr/local/bin/ruvector

# Copy Node.js application
COPY --from=node-builder /build/node_modules ./node_modules
COPY --from=node-builder /build/dist ./dist
COPY --from=node-builder /build/package.json ./

# Environment variables
ENV NODE_ENV=production \
    PORT=8080 \
    HOST=0.0.0.0 \
    MAX_CONNECTIONS=100000 \
    REQUEST_TIMEOUT=30000 \
    KEEP_ALIVE_TIMEOUT=65000 \
    ENABLE_METRICS=true \
    ENABLE_TRACING=true

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=40s --retries=3 \
    CMD ["/nodejs/bin/node", "-e", "require('http').get('http://localhost:8080/health', (r) => process.exit(r.statusCode === 200 ? 0 : 1))"]

# Expose port
EXPOSE 8080

# Run as non-root user (distroless nonroot user)
USER nonroot:nonroot

# Start service
CMD ["dist/cloud-run/streaming-service.js"]
