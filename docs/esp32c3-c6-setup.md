# ESP32-C3 / ESP32-C6 CSI Node Setup Guide

This guide covers building, flashing, and deploying the WiFi-DensePose firmware onto **ESP32-C3** (e.g., Super Mini) and **ESP32-C6** (e.g., M5NanoC6) devices, as well as configuring your **OpenWrt** router and **x86 Linux** host.

---

## 1. Prerequisites

### Hardware
- **ESP32-C3 Super Mini** or **M5NanoC6**
- **OpenWrt Router** (to serve as the WiFi Access Point)
- **x86 Linux Host** (for the sensing server/aggregator)

### Software (on Linux Host)
- **Docker** and **Docker Compose**
- **Python 3.10+**
- **esptool**: `pip install esptool`

---

## 2. Building the Firmware

Use Docker to build the firmware for your specific target. This ensures a consistent build environment without installing the full ESP-IDF toolchain.

### For ESP32-S3
```bash
# From the repository root
cd firmware/esp32-csi-node
cp sdkconfig.defaults.esp32s3 sdkconfig.defaults

MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd):/project" -w /project \
  espressif/idf:v5.2 bash -c \
  "rm -rf build sdkconfig && idf.py set-target esp32s3 && idf.py build"
```

### For ESP32-C3
```bash
# From the repository root
cd firmware/esp32-csi-node
cp sdkconfig.defaults.esp32c3 sdkconfig.defaults

MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd):/project" -w /project \
  espressif/idf:v5.2 bash -c \
  "rm -rf build sdkconfig && idf.py set-target esp32c3 && idf.py build"
```

### For ESP32-C6
```bash
# From the repository root
cd firmware/esp32-csi-node
cp sdkconfig.defaults.esp32c6 sdkconfig.defaults

MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd):/project" -w /project \
  espressif/idf:v5.2 bash -c \
  "rm -rf build sdkconfig && idf.py set-target esp32c6 && idf.py build"
```

---

## 3. Flashing the Devices

Connect your ESP32 via USB and find the serial port (e.g., `/dev/ttyACM0`).

### Flash ESP32-C3
```bash
python -m esptool --chip esp32c3 --port /dev/ttyACM0 --baud 460800 \
  write_flash --flash_mode dio --flash_size 4MB \
  0x0 build/bootloader/bootloader.bin \
  0x8000 build/partition_table/partition-table.bin \
  0x10000 build/esp32-csi-node.bin
```

### Flash ESP32-C6
```bash
python -m esptool --chip esp32c6 --port /dev/ttyACM0 --baud 460800 \
  write_flash --flash_mode dio --flash_size 4MB \
  0x0 build/bootloader/bootloader.bin \
  0x8000 build/partition_table/partition-table.bin \
  0x10000 build/esp32-csi-node.bin
```

---

## 4. Provisioning (WiFi & Target IP)

Configure the devices to connect to your OpenWrt router and send data to your Linux host. No re-flashing required.

```bash
# Use the provision.py script in the firmware directory
python provision.py --port /dev/ttyACM0 \
  --ssid "YourOpenWrtSSID" --password "YourWiFiPassword" \
  --target-ip 192.168.1.XX --node-id 1
```
*Note: Replace `192.168.1.XX` with the actual IP address of your Linux host.*

---

## 5. OpenWrt Configuration

For stable CSI capture, your OpenWrt router should be configured with a fixed channel and minimal interference-inducing features.

### SSH into your Router
```bash
ssh root@192.168.1.1
```

### Apply Optimal Sensing Settings
Run these commands to set a fixed channel (e.g., 6 on 2.4GHz) and disable features that can cause phase noise.

```bash
# Set 2.4GHz radio to Channel 6, HT20 (20MHz)
# Adjust 'radio0' or 'radio1' depending on your hardware
uci set wireless.radio0.channel='6'
uci set wireless.radio0.htmode='HT20'
uci set wireless.radio0.noscan='1'

# Optional: Disable MU-MIMO and Beamforming for more deterministic signal
uci set wireless.radio0.mu_beamformer='0'
uci set wireless.radio0.beamformer='0'

# Commit changes and restart WiFi
uci commit wireless
wifi
```

---

## 6. x86 Linux Host Setup (Docker)

Run the sensing server on your Linux host using Docker.

### Start the Aggregator & UI
```bash
# From the repository root
docker pull ruvnet/wifi-densepose:latest
docker run -d \
  --name wifi-densepose \
  -e CSI_SOURCE=esp32 \
  -p 3000:3000 \
  -p 3001:3001 \
  -p 5005:5005/udp \
  ruvnet/wifi-densepose:latest
```

### Verify Data Flow
Check the logs to see if CSI frames are arriving from your nodes:
```bash
docker logs -f wifi-densepose
```

Open your browser to `http://localhost:3000` to view the live dashboard.

---

## 7. Memory & Performance Notes (C3/C6)

- **Tier 2 Active:** These devices run the Full Pipeline (Tier 2) including breathing and heart rate detection.
- **WASM Disabled:** Tier 3 WASM is disabled by default on C3/C6 to fit within the 400KB internal SRAM.
- **Single Core:** The firmware automatically detects the single-core architecture and adjusts task scheduling accordingly.
