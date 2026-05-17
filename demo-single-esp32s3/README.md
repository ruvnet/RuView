# RuView Single ESP32-S3 Demo

A simplified setup for running RuView with just ONE ESP32-S3 board for basic presence detection.

> **Note:** Single-node detection has limited spatial resolution. For full pose estimation (17 keypoints), use 2+ nodes. This demo focuses on **presence detection** - detecting if someone is in the room.

## What You Need

| Item | Cost | Notes |
|------|------|-------|
| ESP32-S3 DevKit (8MB flash) | ~$9 | Any ESP32-S3 with 8MB flash works |
| Micro USB cable | ~$5 | For power + programming |
| WiFi network | - | 2.4GHz WiFi required |

## Quick Start

### Step 1: Get Pre-built Firmware

Download the latest firmware from releases:
- Go to: https://github.com/Rudraa-25/RuView/releases
- Download `esp32-csi-node.bin` (or build from source - see below)

### Step 2: Flash the Firmware

Connect your ESP32-S3 to your PC via USB. Note the COM port (e.g., COM7 on Windows).

```bash
# Install esptool if you don't have it
pip install esptool

# Flash the firmware (replace COM7 with your port)
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write_flash --flash_mode dio --flash_size 8MB \
  0x0 release/bootloader.bin \
  0x8000 release/partition-table.bin \
  0x10000 release/esp32-csi-node.bin
```

### Step 3: Provision WiFi

After flashing, provision your WiFi credentials (no reflashing needed):

```bash
python firmware/esp32-csi-node/provision.py --port COM7 \
  --ssid "YourWiFiName" --password "YourWiFiPassword" --target-ip 192.168.1.100
```

### Step 4: Run the Sensing Server

```bash
# Install dependencies
pip install -r requirements.txt

# Start the server (single-node mode)
cargo run -p wifi-densepose-sensing-server -- --http-port 3000 --source single
```

### Step 5: Open the Dashboard

Navigate to http://localhost:3000 in your browser.

You should see presence detection data flowing in when people are in range of your WiFi router and the ESP32-S3.

## Building from Source (Optional)

If you want to build the firmware yourself:

```bash
# Using Docker (recommended for Windows)
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd)/firmware/esp32-csi-node:/project" -w /project \
  espressif/idf:v5.2 bash -c \
  "rm -rf build sdkconfig && idf.py set-target esp32s3 && idf.py build"

# Flash the built firmware
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write_flash --flash_mode dio --flash_size 8MB \
  0x0 firmware/esp32-csi-node/build/bootloader/bootloader.bin \
  0x8000 firmware/esp32-csi-node/build/partition_table/partition-table.bin \
  0x10000 firmware/esp32-csi-node/build/esp32-csi-node.bin
```

## What's Detected

With a single ESP32-S3, you can detect:

| Feature | Description |
|---------|-------------|
| **Presence** | Is someone in the room? (Yes/No) |
| **Motion** | Is someone moving? |
| **Distance** | Approximate distance from the ESP32 |

## What's NOT Available (Needs 2+ Nodes)

- Full body pose (17 keypoints)
- Breathing rate
- Heart rate
- 3D position tracking

## Troubleshooting

**No data appearing?**
- Check ESP32 is connected to WiFi: `python -m serial.tools.miniterm COM7 115200`
- Verify the IP address is reachable from your PC
- Make sure your WiFi router is on the same network

**Getting errors?**
- Check the logs in `logs/` folder
- Verify COM port is correct (check Device Manager on Windows)

## Files in This Demo

```
demo-single-esp32s3/
├── README.md           # This file
├── setup.sh            # Quick setup script
└── config.env.example  # Example configuration
```

## License

MIT - Same as main RuView project