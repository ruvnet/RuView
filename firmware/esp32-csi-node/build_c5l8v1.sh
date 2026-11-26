#!/usr/bin/env bash
# Builds esp32-csi-node for the ESP32-C5 with the native C5L8/v1 8-bit L-LTF producer.
# Needs an ESP-IDF 6.x environment already exported (idf.py on PATH): v5.5 has no
# wifi_csi_acquire_config_t.lltf_bit_mode. Output: build-c5l8v1/ (app, bootloader,
# partition table, ota_data_initial) and their sha256 in build-c5l8v1/SHA256SUMS.txt.
# Wi-Fi credentials are not set here; provision them into NVS with provision.py.
set -euo pipefail
cd "$(dirname "$0")"
B=${BUILD_DIR:-build-c5l8v1}
rm -f "$B/sdkconfig.c5l8v1"   # defaults are authoritative; never reuse a stale config
idf.py -B "$B" -DIDF_TARGET=esp32c5 -DSDKCONFIG="$B/sdkconfig.c5l8v1" \
  -DSDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.defaults.esp32c5;sdkconfig.defaults.esp32c5.c5l8v1" build
(cd "$B" && shasum -a 256 esp32-csi-node.bin bootloader/bootloader.bin \
  partition_table/partition-table.bin ota_data_initial.bin | tee SHA256SUMS.txt)
