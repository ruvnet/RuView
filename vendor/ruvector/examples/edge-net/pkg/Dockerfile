# @ruvector/edge-net Genesis Node - Production Dockerfile
#
# Multi-stage build for minimal production image
# Supports: Docker, Kubernetes, Cloud Run, AWS ECS, Azure Container Instances
#
# Build:
#   docker build -t ruvector/edge-net-genesis:latest -f deploy/Dockerfile .
#
# Run:
#   docker run -p 8787:8787 -p 8788:8788 ruvector/edge-net-genesis:latest

# ============================================
# Stage 1: Dependencies
# ============================================
FROM node:20-alpine AS deps

WORKDIR /app

# Install build dependencies for native modules
RUN apk add --no-cache python3 make g++ linux-headers

# Copy package files
COPY package.json package-lock.json* ./

# Install production dependencies only
RUN npm ci --only=production --ignore-scripts 2>/dev/null || npm install --only=production --ignore-scripts

# ============================================
# Stage 2: Production Runtime
# ============================================
FROM node:20-alpine AS runner

# Security: Run as non-root user
RUN addgroup -g 1001 -S nodejs && \
    adduser -S edgenet -u 1001 -G nodejs

WORKDIR /app

# Install runtime dependencies
RUN apk add --no-cache \
    tini \
    dumb-init \
    curl

# Copy node_modules from deps stage
COPY --from=deps /app/node_modules ./node_modules

# Copy application files
COPY --chown=edgenet:nodejs package.json ./
COPY --chown=edgenet:nodejs *.js ./
COPY --chown=edgenet:nodejs *.d.ts ./
COPY --chown=edgenet:nodejs *.wasm ./
COPY --chown=edgenet:nodejs node/ ./node/
COPY --chown=edgenet:nodejs deploy/genesis-prod.js ./deploy/
COPY --chown=edgenet:nodejs deploy/health-check.js ./deploy/

# Create data directory with correct permissions
RUN mkdir -p /data/genesis && \
    chown -R edgenet:nodejs /data/genesis && \
    chmod 755 /data/genesis

# Set environment variables
ENV NODE_ENV=production
ENV GENESIS_PORT=8787
ENV GENESIS_HOST=0.0.0.0
ENV HEALTH_PORT=8788
ENV GENESIS_DATA=/data/genesis
ENV LOG_FORMAT=json
ENV LOG_LEVEL=info
ENV METRICS_ENABLED=true

# Expose ports
# 8787: WebSocket signaling
# 8788: Health check / metrics
EXPOSE 8787
EXPOSE 8788

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8788/health || exit 1

# Switch to non-root user
USER edgenet

# Use tini as init system for proper signal handling
ENTRYPOINT ["/sbin/tini", "--"]

# Start the genesis node
CMD ["node", "deploy/genesis-prod.js"]

# ============================================
# Labels for container registry
# ============================================
LABEL org.opencontainers.image.title="Edge-Net Genesis Node"
LABEL org.opencontainers.image.description="Bootstrap node for the RuVector Edge-Net P2P network"
LABEL org.opencontainers.image.vendor="RuVector"
LABEL org.opencontainers.image.url="https://github.com/ruvnet/ruvector"
LABEL org.opencontainers.image.source="https://github.com/ruvnet/ruvector/tree/main/examples/edge-net"
LABEL org.opencontainers.image.version="1.0.0"
LABEL org.opencontainers.image.licenses="MIT"
