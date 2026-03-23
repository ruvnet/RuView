#!/bin/bash
# ESP32 Swarm Provisioning — ADR-065/066
#
# Provisions multiple ESP32-S3 nodes for a hotel happiness sensing deployment.
# Each node gets WiFi credentials, a unique node_id, zone name, and Seed token.
#
# Prerequisites:
#   - ESP-IDF Python venv with esptool and nvs_partition_gen
#   - Firmware already flashed to each ESP32
#   - Seed paired (obtain token via: curl -X POST http://169.254.42.1/api/v1/pair)
#
# Usage:
#   bash provision_swarm.sh

set -euo pipefail

# ---- Configuration ----
SSID="RedCloverWifi"
PASSWORD="redclover2.4"
SEED_URL="http://10.1.10.236"
SEED_TOKEN="hyHVY4Ux6uBAh8FaQzF_9OwWCWMFB-YuM2OJ3Dcwdm8"  # Replace with your token

PROVISION="../../firmware/esp32-csi-node/provision.py"

# ---- Node definitions: PORT NODE_ID ZONE ----
NODES=(
    "COM5  1  lobby"
    "COM6  2  hallway"
    "COM8  3  restaurant"
    "COM9  4  pool"
    "COM10 5  conference"
)

echo "========================================"
echo "  ESP32 Swarm Provisioning"
echo "  Seed: $SEED_URL"
echo "  WiFi: $SSID"
echo "  Nodes: ${#NODES[@]}"
echo "========================================"
echo

for entry in "${NODES[@]}"; do
    read -r port node_id zone <<< "$entry"
    echo "--- Node $node_id: $zone ($port) ---"
    python "$PROVISION" \
        --port "$port" \
        --ssid "$SSID" \
        --password "$PASSWORD" \
        --node-id "$node_id" \
        --seed-url "$SEED_URL" \
        --seed-token "$SEED_TOKEN" \
        --zone "$zone" \
    && echo "  OK" || echo "  FAILED (device not connected?)"
    echo
done

echo "========================================"
echo "  Provisioning complete."
echo "  Monitor with: python seed_query.py monitor --seed $SEED_URL --token $SEED_TOKEN"
echo "========================================"
