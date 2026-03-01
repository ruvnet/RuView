# Benchmark Dockerfile for RuVector-Postgres
# Runs performance benchmarks and generates reports
#
# Usage:
#   docker build -f docker/benchmark/Dockerfile -t ruvector-benchmark .
#   docker run --rm -v ./results:/benchmark-results ruvector-benchmark

ARG PG_VERSION=17
ARG RUST_VERSION=1.83

# ============================================================================
# Stage 1: Benchmark Runner
# ============================================================================
FROM rust:${RUST_VERSION}-bookworm AS benchmark-runner

ARG PG_VERSION

# Add PostgreSQL APT repository
RUN sh -c 'echo "deb http://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" > /etc/apt/sources.list.d/pgdg.list' && \
    wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -

# Install dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    postgresql-${PG_VERSION} \
    postgresql-server-dev-${PG_VERSION} \
    postgresql-client-${PG_VERSION} \
    libclang-dev \
    clang \
    pkg-config \
    libssl-dev \
    cmake \
    git \
    jq \
    gnuplot \
    && rm -rf /var/lib/apt/lists/*

# Install pgrx and benchmarking tools
RUN cargo install cargo-pgrx --version 0.12.6 --locked && \
    cargo install cargo-criterion --locked && \
    cargo install hyperfine --locked

# Initialize pgrx for the specified PostgreSQL version
RUN cargo pgrx init --pg${PG_VERSION} /usr/lib/postgresql/${PG_VERSION}/bin/pg_config

# Set environment variables
ENV PGRX_PG_CONFIG_PATH=/usr/lib/postgresql/${PG_VERSION}/bin/pg_config
ENV PGRX_HOME=/root/.pgrx
ENV PG_VERSION=${PG_VERSION}
ENV RUST_LOG=info

WORKDIR /app

# Create directories for benchmark results
RUN mkdir -p /benchmark-results /baseline

# Copy benchmark runner script
COPY --chmod=755 crates/ruvector-postgres/docker/benchmark/run-benchmarks.sh /usr/local/bin/run-benchmarks.sh

# Default command runs benchmarks
CMD ["/usr/local/bin/run-benchmarks.sh"]
