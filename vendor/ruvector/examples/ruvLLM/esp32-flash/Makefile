# RuvLLM ESP32 - Makefile
# Cross-platform build and flash targets

.PHONY: all install deps build flash clean cluster monitor help

# Default port (override with: make flash PORT=/dev/ttyUSB1)
PORT ?= /dev/ttyUSB0
# Number of chips for cluster (override with: make cluster CHIPS=5)
CHIPS ?= 2
# Target variant
TARGET ?= xtensa-esp32-espidf

# Detect OS
UNAME_S := $(shell uname -s)
ifeq ($(UNAME_S),Darwin)
    PORT ?= /dev/cu.usbserial-0001
    OPEN_CMD = open
else ifeq ($(UNAME_S),Linux)
    PORT ?= /dev/ttyUSB0
    OPEN_CMD = xdg-open
else
    PORT ?= COM6
    OPEN_CMD = start
endif

# Default target
all: build

# Full installation
install: deps build
	@echo "✓ Installation complete!"
	@echo "Run: make flash PORT=$(PORT)"

# Install dependencies
deps:
	@echo "Installing ESP32 toolchain..."
	@command -v espup >/dev/null 2>&1 || cargo install espup
	@espup install || true
	@command -v espflash >/dev/null 2>&1 || cargo install espflash
	@command -v ldproxy >/dev/null 2>&1 || cargo install ldproxy
	@echo "✓ Dependencies installed"

# Build release binary
build:
	@echo "Building RuvLLM ESP32..."
	@. $$HOME/export-esp.sh 2>/dev/null || true
	cargo build --release
	@echo "✓ Build complete"
	@ls -lh target/$(TARGET)/release/ruvllm-esp32-flash 2>/dev/null || true

# Build with federation
build-federation:
	@echo "Building with federation support..."
	cargo build --release --features federation
	@echo "✓ Federation build complete"

# Flash single chip
flash: build
	@echo "Flashing to $(PORT)..."
	espflash flash --port $(PORT) --monitor target/$(TARGET)/release/ruvllm-esp32-flash

# Flash without monitor
flash-only: build
	espflash flash --port $(PORT) target/$(TARGET)/release/ruvllm-esp32-flash

# Monitor serial
monitor:
	espflash monitor --port $(PORT)

# Setup cluster configuration
cluster:
	@echo "Setting up $(CHIPS)-chip cluster..."
	@./install.sh cluster $(CHIPS)
	@echo "Edit cluster.toml, then run: make cluster-flash"

# Flash entire cluster
cluster-flash: build-federation
	@./cluster-flash.sh

# Monitor cluster (requires tmux or screen)
cluster-monitor:
	@./cluster-monitor.sh

# Clean build artifacts
clean:
	cargo clean
	@rm -f cluster.toml
	@echo "✓ Cleaned"

# Show binary size
size: build
	@echo "Binary size:"
	@ls -lh target/$(TARGET)/release/ruvllm-esp32-flash
	@size target/$(TARGET)/release/ruvllm-esp32-flash 2>/dev/null || true

# Run host simulation (no ESP32 needed)
sim:
	@echo "Running host simulation..."
	cd ../esp32 && cargo run --example user_demo

# Help
help:
	@echo "RuvLLM ESP32 - Makefile Targets"
	@echo ""
	@echo "Single Chip:"
	@echo "  make install          - Install deps and build"
	@echo "  make build            - Build release binary"
	@echo "  make flash            - Flash to PORT (default: $(PORT))"
	@echo "  make flash PORT=/dev/ttyUSB1  - Flash to specific port"
	@echo "  make monitor          - Serial monitor"
	@echo ""
	@echo "Cluster:"
	@echo "  make cluster CHIPS=5  - Generate 5-chip cluster config"
	@echo "  make cluster-flash    - Flash all chips in cluster"
	@echo "  make cluster-monitor  - Monitor all chips"
	@echo ""
	@echo "Other:"
	@echo "  make sim              - Run host simulation"
	@echo "  make size             - Show binary size"
	@echo "  make clean            - Clean build artifacts"
	@echo ""
	@echo "Current settings:"
	@echo "  PORT=$(PORT)"
	@echo "  CHIPS=$(CHIPS)"
	@echo "  TARGET=$(TARGET)"
