# ESP32-S3 CSI Node Firmware (ADR-018)

Firmware for ESP32-S3 that collects WiFi Channel State Information (CSI)
and streams it as ADR-018 binary frames over UDP to the aggregator.

Verified working with ESP32-S3-DevKitC-1 (CP2102, MAC 3C:0F:02:EC:C2:28)
streaming ~20 Hz CSI to the Rust aggregator binary.

## Prerequisites

| Component | Version | Purpose |
|-----------|---------|---------|
| Docker Desktop | 28.x+ | Cross-compile ESP-IDF firmware |
| esptool | 5.x+ | Flash firmware to ESP32 |
| ESP32-S3 board | - | Hardware (DevKitC-1 or similar) |
| USB-UART driver | CP210x | Silicon Labs driver for serial |

## Quick Start

### Step 1: Configure WiFi credentials

Create `sdkconfig.defaults` in this directory (it is gitignored):

```
CONFIG_IDF_TARGET="esp32s3"
CONFIG_ESP_WIFI_CSI_ENABLED=y
CONFIG_CSI_NODE_ID=1
CONFIG_CSI_WIFI_SSID="YOUR_WIFI_SSID"
CONFIG_CSI_WIFI_PASSWORD="YOUR_WIFI_PASSWORD"
CONFIG_CSI_TARGET_IP="192.168.1.20"
CONFIG_CSI_TARGET_PORT=5005
CONFIG_ESPTOOLPY_FLASHSIZE_4MB=y
```

Replace `YOUR_WIFI_SSID`, `YOUR_WIFI_PASSWORD`, and `CONFIG_CSI_TARGET_IP`
with your actual values. The target IP is the machine running the aggregator.

### Step 2: Build with Docker

```bash
cd firmware/esp32-csi-node

# On Linux/macOS:
docker run --rm -v "$(pwd):/project" -w /project \
  espressif/idf:v5.2 bash -c "idf.py set-target esp32s3 && idf.py build"

# On Windows (Git Bash — MSYS path fix required):
MSYS_NO_PATHCONV=1 docker run --rm -v "$(pwd -W)://project" -w //project \
  espressif/idf:v5.2 bash -c "idf.py set-target esp32s3 && idf.py build"
```

Build output: `build/bootloader.bin`, `build/partition_table/partition-table.bin`,
`build/esp32-csi-node.bin`.

### Step 3: Flash to ESP32-S3

Find your serial port (`COM7` on Windows, `/dev/ttyUSB0` on Linux):

```bash
cd firmware/esp32-csi-node/build

python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  --before default-reset --after hard-reset \
  write-flash --flash-mode dio --flash-freq 80m --flash-size 4MB \
  0x0 bootloader/bootloader.bin \
  0x8000 partition_table/partition-table.bin \
  0x10000 esp32-csi-node.bin
```

### Step 4: Run the aggregator

```bash
cargo run -p wifi-densepose-hardware --bin aggregator -- --bind 0.0.0.0:5005 --verbose
```

Expected output:
```
Listening on 0.0.0.0:5005...
  [148 bytes from 192.168.1.71:60764]
[node:1 seq:0] sc=64 rssi=-49 amp=9.5
  [276 bytes from 192.168.1.71:60764]
[node:1 seq:1] sc=128 rssi=-64 amp=16.0
```

### Step 5: Verify presence detection

If you see frames streaming (~20/sec), the system is working. Walk near the
ESP32 and observe amplitude variance changes in the CSI data.

## Configuration Reference

Edit via `idf.py menuconfig` or `sdkconfig.defaults`:

| Setting | Default | Description |
|---------|---------|-------------|
| `CSI_NODE_ID` | 1 | Unique node identifier (0-255) |
| `CSI_TARGET_IP` | 192.168.1.100 | Aggregator host IP |
| `CSI_TARGET_PORT` | 5005 | Aggregator UDP port |
| `CSI_WIFI_SSID` | wifi-densepose | WiFi network SSID |
| `CSI_WIFI_PASSWORD` | (empty) | WiFi password |
| `CSI_WIFI_CHANNEL` | 6 | WiFi channel to monitor |

## Firewall Note

On Windows, you may need to allow inbound UDP on port 5005:

```
netsh advfirewall firewall add rule name="ESP32 CSI" dir=in action=allow protocol=UDP localport=5005
```

## Architecture

```
ESP32-S3                              Host Machine
+-------------------+                +-------------------+
| WiFi CSI callback |  UDP/5005      | aggregator binary |
| (promiscuous mode)|  ──────────>   | (Rust, clap CLI)  |
| ADR-018 serialize |  ADR-018       | Esp32CsiParser    |
| stream_sender.c   |  binary frames | CsiFrame output   |
+-------------------+                +-------------------+
```

## Binary Frame Format (ADR-018)

```
Offset  Size  Field
0       4     Magic: 0xC5110001
4       1     Node ID
5       1     Number of antennas
6       2     Number of subcarriers (LE u16)
8       4     Frequency MHz (LE u32)
12      4     Sequence number (LE u32)
16      1     RSSI (i8)
17      1     Noise floor (i8)
18      2     Reserved
20      N*2   I/Q pairs (n_antennas * n_subcarriers * 2 bytes)
```

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| No serial output | Wrong baud rate | Use 115200 |
| WiFi won't connect | Wrong SSID/password | Check sdkconfig.defaults |
| No UDP frames | Firewall blocking | Add UDP 5005 inbound rule |
| CSI callback not firing | Promiscuous mode off | Verify `esp_wifi_set_promiscuous(true)` in csi_collector.c |
| Parse errors in aggregator | Firmware/parser mismatch | Rebuild both from same source |
