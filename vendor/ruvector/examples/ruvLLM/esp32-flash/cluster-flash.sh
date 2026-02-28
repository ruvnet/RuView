#!/bin/bash
# RuvLLM ESP32 - Cluster Flash Script
# Flashes multiple ESP32s with configured roles

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

CONFIG_FILE="${1:-cluster.toml}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}"
echo "╔══════════════════════════════════════════════════════════╗"
echo "║          RuvLLM ESP32 - Cluster Flash Tool               ║"
echo "╚══════════════════════════════════════════════════════════╝"
echo -e "${NC}"

if [ ! -f "$CONFIG_FILE" ]; then
    echo -e "${RED}Error: $CONFIG_FILE not found${NC}"
    echo "Run: ./install.sh cluster <num_chips>"
    exit 1
fi

# Parse cluster config (simple grep-based for portability)
CLUSTER_NAME=$(grep 'name = ' "$CONFIG_FILE" | head -1 | cut -d'"' -f2)
NUM_CHIPS=$(grep 'chips = ' "$CONFIG_FILE" | head -1 | awk '{print $3}')
TOPOLOGY=$(grep 'topology = ' "$CONFIG_FILE" | head -1 | cut -d'"' -f2)

echo -e "${GREEN}Cluster: $CLUSTER_NAME${NC}"
echo -e "Chips: $NUM_CHIPS"
echo -e "Topology: $TOPOLOGY"
echo ""

# Build with federation support
echo -e "${YELLOW}Building with federation support...${NC}"
cargo build --release --features federation

# Extract ports from config
PORTS=$(grep 'port = ' "$CONFIG_FILE" | cut -d'"' -f2)

# Flash each chip
CHIP_ID=1
for PORT in $PORTS; do
    echo ""
    echo -e "${YELLOW}═══════════════════════════════════════════${NC}"
    echo -e "${YELLOW}Flashing Chip $CHIP_ID to $PORT${NC}"
    echo -e "${YELLOW}═══════════════════════════════════════════${NC}"

    if [ ! -e "$PORT" ]; then
        echo -e "${RED}Warning: $PORT not found, skipping...${NC}"
        CHIP_ID=$((CHIP_ID + 1))
        continue
    fi

    # Set chip ID via environment (embedded in binary)
    RUVLLM_CHIP_ID=$CHIP_ID RUVLLM_TOTAL_CHIPS=$NUM_CHIPS \
        espflash flash --port "$PORT" target/xtensa-esp32-espidf/release/ruvllm-esp32-flash

    echo -e "${GREEN}✓ Chip $CHIP_ID flashed successfully${NC}"

    CHIP_ID=$((CHIP_ID + 1))

    # Wait between flashes
    sleep 2
done

echo ""
echo -e "${GREEN}═══════════════════════════════════════════${NC}"
echo -e "${GREEN}Cluster flash complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════${NC}"
echo ""
echo "To monitor all chips:"
echo "  ./cluster-monitor.sh"
