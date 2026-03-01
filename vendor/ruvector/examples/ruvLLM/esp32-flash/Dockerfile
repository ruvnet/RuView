# RuvLLM ESP32 - Docker Build Environment
# Provides complete ESP32 toolchain without local installation
#
# Usage:
#   docker build -t ruvllm-esp32-builder .
#   docker run -v $(pwd):/app -v /dev:/dev --privileged ruvllm-esp32-builder build
#   docker run -v $(pwd):/app -v /dev:/dev --privileged ruvllm-esp32-builder flash /dev/ttyUSB0

FROM rust:1.75-bookworm

# Install system dependencies
RUN apt-get update && apt-get install -y \
    git \
    wget \
    flex \
    bison \
    gperf \
    python3 \
    python3-pip \
    python3-venv \
    cmake \
    ninja-build \
    ccache \
    libffi-dev \
    libssl-dev \
    dfu-util \
    libusb-1.0-0 \
    libudev-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Install ESP-IDF prerequisites
RUN pip3 install --break-system-packages pyserial

# Install Rust ESP32 toolchain
RUN cargo install espup && \
    espup install && \
    cargo install espflash ldproxy

# Set up environment
ENV PATH="/root/.cargo/bin:${PATH}"
RUN echo 'source /root/export-esp.sh 2>/dev/null || true' >> /root/.bashrc

WORKDIR /app

# Entry point script
COPY <<'EOF' /entrypoint.sh
#!/bin/bash
source /root/export-esp.sh 2>/dev/null || true

case "$1" in
    build)
        echo "Building RuvLLM ESP32..."
        cargo build --release
        ;;
    flash)
        PORT="${2:-/dev/ttyUSB0}"
        echo "Flashing to $PORT..."
        cargo build --release
        espflash flash --port "$PORT" target/xtensa-esp32-espidf/release/ruvllm-esp32-flash
        ;;
    monitor)
        PORT="${2:-/dev/ttyUSB0}"
        espflash monitor --port "$PORT"
        ;;
    shell)
        exec /bin/bash
        ;;
    *)
        echo "Usage: docker run ... [build|flash|monitor|shell] [port]"
        ;;
esac
EOF
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
CMD ["build"]
