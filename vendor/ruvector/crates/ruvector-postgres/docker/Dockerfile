# RuVector-Postgres Development & Testing Dockerfile
# Multi-stage build with PostgreSQL version support (14-17)
# Default: PostgreSQL 17 (latest with pgrx 0.12 support)
# Note: PostgreSQL 18 requires pgrx 0.15.0+ (planned for future release)

ARG PG_VERSION=17
ARG RUST_VERSION=1.85

# ============================================================================
# Stage 1: Base Builder with Rust and PostgreSQL dev dependencies
# ============================================================================
FROM rust:${RUST_VERSION}-bookworm AS base-builder

ARG PG_VERSION

# Add PostgreSQL APT repository
RUN sh -c 'echo "deb http://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" > /etc/apt/sources.list.d/pgdg.list' && \
    wget --quiet -O - https://www.postgresql.org/media/keys/ACCC4CF8.asc | apt-key add -

# Install PostgreSQL development dependencies for specified version
RUN apt-get update && apt-get install -y --no-install-recommends \
    postgresql-${PG_VERSION} \
    postgresql-server-dev-${PG_VERSION} \
    libclang-dev \
    clang \
    pkg-config \
    libssl-dev \
    cmake \
    wget \
    git \
    && rm -rf /var/lib/apt/lists/*

# Install pgrx (compatible with pgrx = "0.12" in Cargo.toml)
RUN cargo install cargo-pgrx --version 0.12.6 --locked

# Initialize pgrx for the specified PostgreSQL version
RUN cargo pgrx init --pg${PG_VERSION} /usr/lib/postgresql/${PG_VERSION}/bin/pg_config

# Set PGRX environment for consistent builds
ENV PGRX_PG_CONFIG_PATH=/usr/lib/postgresql/${PG_VERSION}/bin/pg_config
ENV PGRX_HOME=/root/.pgrx
ENV PG_VERSION=${PG_VERSION}

# ============================================================================
# Stage 2: Dependency Cache Builder
# ============================================================================
FROM base-builder AS deps-builder

ARG PG_VERSION

# Use workspace layout: /build is the workspace root
WORKDIR /build/crates/ruvector-postgres

# Copy only dependency files first for better caching
COPY crates/ruvector-postgres/Cargo.toml ./
COPY crates/ruvector-postgres/build.rs ./

# Create dummy src to build dependencies
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    echo "#[no_mangle] pub extern \"C\" fn pg_finfo_dummy() {}" > src/lib.rs

# Build dependencies only (this layer is cached)
RUN cargo build --release --features pg${PG_VERSION} || true
RUN rm -rf src

# ============================================================================
# Stage 3: Extension Builder
# ============================================================================
FROM deps-builder AS extension-builder

ARG PG_VERSION

# Create a minimal workspace Cargo.toml so dependency crates can resolve
# workspace inheritance (edition.workspace, version.workspace, etc.)
RUN cat > /build/Cargo.toml << 'WORKSPACE_EOF'
[workspace]
members = [
    "crates/ruvector-postgres",
    "crates/ruvector-solver",
    "crates/ruvector-math",
    "crates/ruvector-attention",
    "crates/sona",
    "crates/ruvector-domain-expansion",
    "crates/ruvector-mincut-gated-transformer",
]
resolver = "2"

[workspace.package]
version = "2.0.4"
edition = "2021"
rust-version = "1.77"
license = "MIT"
authors = ["Ruvector Team"]
repository = "https://github.com/ruvnet/ruvector"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
rand = "0.8"
rand_distr = "0.4"
tracing = "0.1"
rayon = "1.10"
crossbeam = "0.8"
dashmap = "6.1"
parking_lot = "0.12"
once_cell = "1.20"
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1.5"
nalgebra = { version = "0.33", default-features = false, features = ["std"] }
ndarray = "0.16"
chrono = "0.4"
anyhow = "1.0"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
panic = "unwind"
WORKSPACE_EOF

# Copy the ruvector-mincut-gated-transformer dependency (required for gated-transformer feature)
COPY crates/ruvector-mincut-gated-transformer /build/crates/ruvector-mincut-gated-transformer/

# Copy v0.3 dependencies (workspace layout preserves inheritance resolution)
COPY crates/ruvector-solver /build/crates/ruvector-solver/
COPY crates/ruvector-math /build/crates/ruvector-math/
COPY crates/ruvector-attention /build/crates/ruvector-attention/
COPY crates/sona /build/crates/sona/
COPY crates/ruvector-domain-expansion /build/crates/ruvector-domain-expansion/

# Copy rvf crates (optional path deps of ruvector-domain-expansion, Cargo validates they exist)
COPY crates/rvf/rvf-types /build/crates/rvf/rvf-types/
COPY crates/rvf/rvf-wire /build/crates/rvf/rvf-wire/
COPY crates/rvf/rvf-crypto /build/crates/rvf/rvf-crypto/

# Copy actual source code
COPY crates/ruvector-postgres/Cargo.toml ./
COPY crates/ruvector-postgres/build.rs ./
COPY crates/ruvector-postgres/ruvector.control ./
COPY crates/ruvector-postgres/src ./src/
COPY crates/ruvector-postgres/sql ./sql/
COPY crates/ruvector-postgres/benches ./benches/

# Build the extension with all features including v0.3 modules
RUN cargo pgrx package \
    --pg-config /usr/lib/postgresql/${PG_VERSION}/bin/pg_config \
    --features pg${PG_VERSION},graph-complete,gated-transformer,analytics-complete,attention-extended,sona-learning,domain-expansion

# pgrx generates .control and .so but not SQL - copy our hand-written SQL files
# In a workspace, target/ is at the workspace root /build/target/, not per-crate
RUN cp sql/ruvector--0.3.0.sql /build/target/release/ruvector-pg${PG_VERSION}/usr/share/postgresql/${PG_VERSION}/extension/ 2>/dev/null || true && \
    cp sql/ruvector--2.0.0.sql /build/target/release/ruvector-pg${PG_VERSION}/usr/share/postgresql/${PG_VERSION}/extension/ 2>/dev/null || true && \
    cp sql/ruvector--2.0.0--0.3.0.sql /build/target/release/ruvector-pg${PG_VERSION}/usr/share/postgresql/${PG_VERSION}/extension/ 2>/dev/null || true

# ============================================================================
# Stage 4: Runtime (Production)
# ============================================================================
FROM postgres:${PG_VERSION}-bookworm AS runtime

ARG PG_VERSION

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy built extension from builder (workspace target is at /build/target/)
COPY --from=extension-builder /build/target/release/ruvector-pg${PG_VERSION}/usr/share/postgresql/${PG_VERSION}/extension/* /usr/share/postgresql/${PG_VERSION}/extension/
COPY --from=extension-builder /build/target/release/ruvector-pg${PG_VERSION}/usr/lib/postgresql/${PG_VERSION}/lib/* /usr/lib/postgresql/${PG_VERSION}/lib/

# Copy initialization script with proper permissions
COPY --chmod=644 crates/ruvector-postgres/docker/init.sql /docker-entrypoint-initdb.d/

# Set environment variables
ENV POSTGRES_USER=ruvector
ENV POSTGRES_PASSWORD=ruvector
ENV POSTGRES_DB=ruvector_test
ENV PG_VERSION=${PG_VERSION}

# PostgreSQL performance tuning
ENV POSTGRES_INITDB_ARGS="--data-checksums"

# Labels for version tracking
LABEL org.opencontainers.image.title="RuVector PostgreSQL Extension v0.3"
LABEL org.opencontainers.image.description="High-performance vector database extension for PostgreSQL with 143 SQL functions, Solver, Math, TDA, Extended Attention, Sona, and Domain Expansion"
LABEL org.opencontainers.image.version="0.3.0"
LABEL org.opencontainers.image.vendor="ruv.io"
LABEL org.opencontainers.image.source="https://github.com/ruvnet/ruvector"
LABEL ruvector.pg.version="${PG_VERSION}"
LABEL ruvector.features="attention,gnn,hybrid,tenancy,healing,learning,hyperbolic,graph,solver,math,tda,sona,domain-expansion"

# Health check
HEALTHCHECK --interval=5s --timeout=5s --start-period=10s --retries=5 \
    CMD pg_isready -U $POSTGRES_USER -d $POSTGRES_DB || exit 1

EXPOSE 5432
