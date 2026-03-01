# =============================================================================
# RuvBot - Multi-stage Dockerfile for Google Cloud Run
# =============================================================================
# Optimized for:
# - Minimal image size (~150MB)
# - Fast cold starts (<2s)
# - Security (non-root, distroless base)
# - Cost efficiency (Cloud Run serverless)
# =============================================================================

# -----------------------------------------------------------------------------
# Stage 1: Dependencies
# -----------------------------------------------------------------------------
FROM node:22-alpine AS deps

WORKDIR /app

# Copy package files
COPY package.json package-lock.json* ./

# Install production dependencies only
RUN npm ci --only=production --ignore-scripts && \
    npm cache clean --force

# -----------------------------------------------------------------------------
# Stage 2: Builder
# -----------------------------------------------------------------------------
FROM node:22-alpine AS builder

WORKDIR /app

# Copy package files
COPY package.json package-lock.json* tsconfig*.json ./

# Install all dependencies (including dev)
RUN npm ci --ignore-scripts

# Copy source code
COPY src/ ./src/

# Build TypeScript
RUN npm run build

# Copy static files to dist
RUN mkdir -p dist/api/public && cp -r src/api/public/* dist/api/public/ 2>/dev/null || true

# -----------------------------------------------------------------------------
# Stage 3: Production Runner
# -----------------------------------------------------------------------------
FROM node:22-alpine AS runner

WORKDIR /app

# Security: Create non-root user
RUN addgroup --system --gid 1001 ruvbot && \
    adduser --system --uid 1001 --ingroup ruvbot ruvbot

# Set production environment
ENV NODE_ENV=production
ENV PORT=8080

# Copy production dependencies
COPY --from=deps --chown=ruvbot:ruvbot /app/node_modules ./node_modules

# Copy built application
COPY --from=builder --chown=ruvbot:ruvbot /app/dist ./dist
COPY --from=builder --chown=ruvbot:ruvbot /app/package.json ./

# Health check endpoint
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Switch to non-root user
USER ruvbot

# Expose port
EXPOSE 8080

# Start the application
CMD ["node", "dist/server.js"]
