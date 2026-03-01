# Test Runner Dockerfile for RuVector-Postgres
# Multi-stage build for efficient test execution with JUnit XML output
#
# Usage:
#   docker build -f docker/test-runner/Dockerfile -t ruvector-test-runner .
#   docker run --rm ruvector-test-runner

ARG PG_VERSION=17
ARG RUST_VERSION=1.83

# ============================================================================
# Stage 1: Test Runner Base
# ============================================================================
FROM rust:${RUST_VERSION}-bookworm AS test-runner

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
    && rm -rf /var/lib/apt/lists/*

# Install pgrx and testing tools
RUN cargo install cargo-pgrx --version 0.12.6 --locked && \
    cargo install cargo-nextest --locked && \
    cargo install cargo2junit --locked

# Initialize pgrx for the specified PostgreSQL version
RUN cargo pgrx init --pg${PG_VERSION} /usr/lib/postgresql/${PG_VERSION}/bin/pg_config

# Set environment variables
ENV PGRX_PG_CONFIG_PATH=/usr/lib/postgresql/${PG_VERSION}/bin/pg_config
ENV PGRX_HOME=/root/.pgrx
ENV PG_VERSION=${PG_VERSION}
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

WORKDIR /app

# Create directories for test results
RUN mkdir -p /test-results /coverage

# Copy test runner script
COPY --chmod=755 crates/ruvector-postgres/docker/test-runner/run-tests.sh /usr/local/bin/run-tests.sh

# Default command runs pgrx tests and outputs JUnit XML
CMD ["/usr/local/bin/run-tests.sh"]
