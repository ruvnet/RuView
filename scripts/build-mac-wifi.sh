#!/usr/bin/env bash
set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "build-mac-wifi.sh must be run on macOS" >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SOURCE_PATH="${REPO_ROOT}/tools/macos-wifi-scan/main.swift"
OUTPUT_DIR="${REPO_ROOT}/rust-port/wifi-densepose-rs/target/tools/macos-wifi-scan"
OUTPUT_PATH="${OUTPUT_DIR}/macos-wifi-scan"

mkdir -p "${OUTPUT_DIR}"

swiftc \
  -O \
  -framework Foundation \
  -framework CoreWLAN \
  "${SOURCE_PATH}" \
  -o "${OUTPUT_PATH}"

echo "Built macOS Wi-Fi helper: ${OUTPUT_PATH}"
