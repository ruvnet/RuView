# ---- Build stage ----
FROM rust:1.85-slim AS builder

RUN apt-get update -qq && apt-get install -y \
    libglib2.0-dev \
    libgtk-3-dev \
    libsoup-3.0-dev \
    libjavascriptcoregtk-4.1-dev \
    libwebkit2gtk-4.1-dev \
    libudev-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY rust-port/wifi-densepose-rs ./rust-port/wifi-densepose-rs

WORKDIR /app/rust-port/wifi-densepose-rs

# Validar que compila sin generar binarios (más rápido en CI)
RUN cargo check --workspace

# ---- Runtime stage ----
FROM debian:bookworm-slim AS runtime

RUN apt-get update -qq && apt-get install -y \
    libglib2.0-0 \
    libgtk-3-0 \
    libudev1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Placeholder hasta tener binario compilado
COPY --from=builder /app/rust-port/wifi-densepose-rs/Cargo.toml /app/

EXPOSE 3000

CMD ["echo", "RuView container ready - deploy binary to activate"]