# RuVector Benchmark Container
# Containerized benchmarking environment with k6 and all dependencies

FROM loadimpact/k6:0.48.0 as k6

FROM node:20-alpine

# Install dependencies
RUN apk add --no-cache \
    bash \
    curl \
    git \
    python3 \
    py3-pip

# Copy k6 binary from k6 image
COPY --from=k6 /usr/bin/k6 /usr/bin/k6

# Set working directory
WORKDIR /benchmarks

# Copy package files
COPY package*.json ./

# Install Node.js dependencies
RUN npm install -g typescript ts-node && \
    npm install --production

# Copy benchmark files
COPY *.ts ./
COPY *.html ./
COPY *.md ./
COPY setup.sh ./

# Make scripts executable
RUN chmod +x setup.sh

# Create results directory
RUN mkdir -p results

# Set environment variables
ENV BASE_URL=http://localhost:8080
ENV PARALLEL=1
ENV ENABLE_HOOKS=false
ENV LOG_LEVEL=info
ENV NODE_OPTIONS=--max-old-space-size=4096

# Volume for results
VOLUME ["/benchmarks/results"]

# Default command
CMD ["ts-node", "benchmark-runner.ts", "list"]

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD k6 version || exit 1

# Labels
LABEL org.opencontainers.image.title="RuVector Benchmarks"
LABEL org.opencontainers.image.description="Enterprise-grade benchmarking suite for RuVector"
LABEL org.opencontainers.image.version="1.0.0"
LABEL org.opencontainers.image.vendor="RuVector Team"
LABEL org.opencontainers.image.source="https://github.com/ruvnet/ruvector"
