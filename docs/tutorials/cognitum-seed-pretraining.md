# ESP32 CSI to Cognitum Seed Pretraining Pipeline

A beginner-friendly tutorial for collecting WiFi CSI data with ESP32 nodes
and building a pre-trained model using the Cognitum Seed edge intelligence appliance.

**Estimated time:** 1 hour (setup 20 min, data collection 30 min, verification 10 min)

**What you will build:** A self-supervised pretraining dataset stored on a
Cognitum Seed, containing 8-dimensional feature vectors extracted from live
WiFi Channel State Information. The Seed's RVF vector store, kNN search, and
witness chain turn raw radio signals into a searchable, cryptographically
attested knowledge base -- no cameras or manual labeling required.

**Who this is for:** Makers, embedded engineers, and ML practitioners who want
to experiment with WiFi-based human sensing. No Rust knowledge is needed; the
entire workflow uses Python and pre-built firmware binaries.

---

## Table of Contents

1. [Prerequisites](#1-prerequisites)
2. [Hardware Setup](#2-hardware-setup)
3. [Running the Bridge](#3-running-the-bridge)
4. [Data Collection Protocol](#4-data-collection-protocol)
5. [Monitoring Progress](#5-monitoring-progress)
6. [Understanding the Feature Vectors](#6-understanding-the-feature-vectors)
7. [Using the Pre-trained Data](#7-using-the-pre-trained-data)
8. [Troubleshooting](#8-troubleshooting)
9. [Next Steps](#9-next-steps)

---

## 1. Prerequisites

### Hardware

| Item | Quantity | Approx. Cost | Notes |
|------|----------|-------------|-------|
| ESP32-S3 (8MB flash) | 2 | ~$9 each | Must be S3 variant -- original ESP32 and C3 are not supported (single-core, cannot run CSI DSP) |
| Cognitum Seed (Pi Zero 2 W) | 1 | ~$15 | Available at [cognitum.one](https://cognitum.one) |
| USB-C data cables | 3 | ~$3 each | Must be **data** cables, not charge-only |

**Total cost: ~$36**

### Software

Install these on your host laptop/desktop (Windows, macOS, or Linux):

```bash
# Python 3.10 or later
python --version
# Expected: Python 3.10.x or later

# esptool for flashing firmware
pip install esptool

# pyserial for serial monitoring (optional but useful)
pip install pyserial
```

> **Tip:** You do not need the Rust toolchain for this tutorial. The ESP32
> firmware is distributed as pre-built binaries, and the bridge script is
> pure Python.

### Firmware

Download the v0.5.4 firmware binaries from the GitHub releases page:

```
esp32-csi-node.bin          -- Main firmware (8MB flash)
bootloader.bin              -- Bootloader
partition-table.bin         -- Partition table
ota_data_initial.bin        -- OTA data
```

### Network

All devices must be on the same WiFi network. You will need:

- Your WiFi SSID and password
- Your host laptop's local IP address (e.g., `192.168.1.20`)

Find your host IP:

```bash
# Windows
ipconfig | findstr "IPv4"

# macOS / Linux
ip addr show | grep "inet " | grep -v 127.0.0.1
```

---

## 2. Hardware Setup

### Physical Layout

```
  ┌─────────────────────────────────────────────────┐
  │                    Room                          │
  │                                                  │
  │  [ESP32 #1]                       [ESP32 #2]    │
  │   node_id=1                        node_id=2    │
  │   on shelf                         on desk      │
  │   ~1.5m high                       ~0.8m high   │
  │                                                  │
  │           3-5 meters apart                       │
  │                                                  │
  │            [Cognitum Seed]                       │
  │             on table, USB to laptop              │
  │                                                  │
  │            [Host Laptop]                         │
  │             running bridge script                │
  └─────────────────────────────────────────────────┘
```

> **Tip:** Place the two ESP32 nodes 3-5 meters apart at different heights.
> This gives the multi-node pipeline spatial diversity, which improves the
> quality of cross-viewpoint features.

### Step 2.1: Connect and Verify the Cognitum Seed

Plug the Cognitum Seed into your laptop using a USB **data** cable.

Wait 30-60 seconds for it to boot. Then verify connectivity:

```bash
curl -sk https://169.254.42.1:8443/api/v1/status
```

Expected output (abbreviated):

```json
{
  "device_id": "ecaf97dd-fc90-4b0e-b0e7-e9f896b9fbb6",
  "total_vectors": 0,
  "epoch": 1,
  "dimension": 8,
  "uptime_secs": 45
}
```

> **Note:** The `-sk` flags tell curl to use HTTPS (`-s` silent, `-k` skip
> TLS certificate verification). The Seed uses a self-signed certificate.

You can also open `https://169.254.42.1:8443/guide` in a browser (accept
the self-signed certificate warning) to see the Seed's setup guide.

### Step 2.2: Pair the Seed

Pairing generates a bearer token that authorizes write access. Pairing can
only be initiated from the USB interface (169.254.42.1), not from WiFi -- this
is a security feature.

```bash
curl -sk -X POST https://169.254.42.1:8443/api/v1/pair \
  -H "Content-Type: application/json" \
  -d '{"client_name": "wifi-densepose-tutorial"}'
```

Expected output:

```json
{
  "token": "seed_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
  "expires": null,
  "permissions": ["read", "write", "admin"]
}
```

Save this token -- you will need it for every bridge command:

```bash
export SEED_TOKEN="seed_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
```

> **Warning:** Treat the token like a password. Do not commit it to git or
> share it publicly.

### Step 2.3: Flash ESP32 #1

Connect the first ESP32-S3 to your laptop via USB. Identify its serial port:

```bash
# Windows -- look for "Silicon Labs" or "CP210x" in Device Manager
# or run:
python -m serial.tools.list_ports

# macOS
ls /dev/tty.usb*

# Linux
ls /dev/ttyUSB* /dev/ttyACM*
```

Flash the firmware (replace `COM9` with your port):

```bash
esptool.py --chip esp32s3 --port COM9 --baud 460800 \
  write_flash \
  0x0     bootloader.bin \
  0x8000  partition-table.bin \
  0xd000  ota_data_initial.bin \
  0x10000 esp32-csi-node.bin
```

Expected output (last lines):

```
Writing at 0x000f4000... (100 %)
Wrote 978432 bytes (...)
Hash of data verified.
Leaving...
Hard resetting via RTS pin...
```

### Step 2.4: Provision ESP32 #1

Tell the ESP32 which WiFi network to join and where to send data:

```bash
python firmware/esp32-csi-node/provision.py \
  --port COM9 \
  --ssid "YourWiFi" \
  --password "YourPassword" \
  --target-ip 192.168.1.20 \
  --target-port 5006 \
  --node-id 1
```

Replace:
- `COM9` with your actual serial port
- `YourWiFi` / `YourPassword` with your WiFi credentials
- `192.168.1.20` with your host laptop's IP address

Expected output:

```
Writing NVS partition (24576 bytes) at offset 0x9000...
Provisioning complete. Reset the device to apply.
```

> **Important:** The `--target-ip` is your **host laptop**, not the Seed.
> The bridge script runs on your laptop and forwards vectors to the Seed
> via HTTPS.

### Step 2.5: Verify ESP32 #1 Is Streaming

After provisioning, the ESP32 resets and begins streaming. Verify with a
quick UDP listener:

```bash
python -c "
import socket, struct
sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(('0.0.0.0', 5006))
sock.settimeout(10)
print('Listening on UDP 5006 for 10 seconds...')
count = 0
try:
    while True:
        data, addr = sock.recvfrom(2048)
        magic = struct.unpack_from('<I', data)[0]
        names = {0xC5110001: 'CSI_RAW', 0xC5110002: 'VITALS', 0xC5110003: 'FEATURES'}
        name = names.get(magic, f'UNKNOWN(0x{magic:08X})')
        count += 1
        if count <= 5:
            print(f'  Packet {count}: {name} from {addr[0]} ({len(data)} bytes)')
except socket.timeout:
    pass
sock.close()
print(f'Received {count} packets total')
"
```

Expected output:

```
Listening on UDP 5006 for 10 seconds...
  Packet 1: VITALS from 192.168.1.105 (32 bytes)
  Packet 2: FEATURES from 192.168.1.105 (48 bytes)
  Packet 3: VITALS from 192.168.1.105 (32 bytes)
  Packet 4: FEATURES from 192.168.1.105 (48 bytes)
  Packet 5: VITALS from 192.168.1.105 (32 bytes)
Received 20 packets total
```

If you see 0 packets, check the [Troubleshooting](#8-troubleshooting) section.

### Step 2.6: Flash and Provision ESP32 #2

Repeat steps 2.3-2.5 for the second ESP32, using `--node-id 2`:

```bash
# Flash (replace COM8 with your port)
esptool.py --chip esp32s3 --port COM8 --baud 460800 \
  write_flash \
  0x0     bootloader.bin \
  0x8000  partition-table.bin \
  0xd000  ota_data_initial.bin \
  0x10000 esp32-csi-node.bin

# Provision
python firmware/esp32-csi-node/provision.py \
  --port COM8 \
  --ssid "YourWiFi" \
  --password "YourPassword" \
  --target-ip 192.168.1.20 \
  --target-port 5006 \
  --node-id 2
```

### Step 2.7: Verify Both Nodes

Run the UDP listener again. You should see packets from two different IPs:

```
  Packet 1: FEATURES from 192.168.1.105 (48 bytes)   <-- node 1
  Packet 2: FEATURES from 192.168.1.104 (48 bytes)   <-- node 2
  Packet 3: VITALS from 192.168.1.105 (32 bytes)
  Packet 4: VITALS from 192.168.1.104 (32 bytes)
```

---

## 3. Running the Bridge

The bridge script (`scripts/seed_csi_bridge.py`) listens for UDP packets
from the ESP32 nodes, batches them, and ingests them into the Seed's RVF
vector store via HTTPS.

### Basic Start

```bash
python scripts/seed_csi_bridge.py \
  --seed-url https://169.254.42.1:8443 \
  --token "$SEED_TOKEN" \
  --udp-port 5006 \
  --batch-size 10
```

Expected output:

```
12:00:01 [INFO] Connected to Seed ecaf97dd — 0 vectors, epoch 1, dim 8
12:00:01 [INFO] Listening on UDP port 5006 (batch size: 10, flush interval: 10s)
12:00:11 [INFO] Ingested 10 vectors (epoch=2, witness=a3b7c9d2e4f6...)
12:00:21 [INFO] Ingested 10 vectors (epoch=3, witness=f1e2d3c4b5a6...)
```

### Bridge Flags Explained

| Flag | Default | Description |
|------|---------|-------------|
| `--seed-url` | `https://169.254.42.1:8443` | Seed HTTPS endpoint (USB link-local) |
| `--token` | `$SEED_TOKEN` env var | Bearer token from pairing step |
| `--udp-port` | `5006` | UDP port to listen for ESP32 packets |
| `--batch-size` | `10` | Number of vectors per ingest call |
| `--flush-interval` | `10` | Maximum seconds between flushes (time-based batching) |
| `--validate` | off | After each batch, run kNN query + PIR comparison |
| `--stats` | off | Print Seed stats and exit (no bridge loop) |
| `--compact` | off | Trigger store compaction and exit |
| `--allowed-sources` | none | Comma-separated IPs to accept (anti-spoofing) |
| `-v` / `--verbose` | off | Log every received packet |

### Recommended: Validation Mode

For your first data collection, enable `--validate` so the bridge verifies
each batch against the Seed's kNN index:

```bash
python scripts/seed_csi_bridge.py \
  --seed-url https://169.254.42.1:8443 \
  --token "$SEED_TOKEN" \
  --udp-port 5006 \
  --batch-size 10 \
  --validate
```

With validation enabled, you will see additional output after each batch:

```
12:00:11 [INFO] Ingested 10 vectors (epoch=2, witness=a3b7c9d2...)
12:00:11 [INFO] Validation: kNN distance=0.000000 (exact match)
12:00:11 [INFO] PIR=LOW CSI_presence=0.14 (absent) -- agreement 100.0% (1/1)
```

### Recommended: Source IP Filtering

If you are on a shared network, restrict the bridge to only accept packets
from your ESP32 nodes:

```bash
python scripts/seed_csi_bridge.py \
  --token "$SEED_TOKEN" \
  --udp-port 5006 \
  --batch-size 10 \
  --allowed-sources "192.168.1.104,192.168.1.105"
```

---

## 4. Data Collection Protocol

Collect 6 scenarios, 5 minutes each, for a total of 30 minutes of data.
With 2 nodes at 1 Hz each, each scenario produces ~600 feature vectors.

> **Before you begin:** Make sure the bridge is running (Section 3). Leave
> the terminal open and start a new terminal for the commands below.

### Scenario 1: Empty Room (5 min)

This establishes the baseline -- what the room looks like with no one in it.

```bash
echo "=== SCENARIO 1: EMPTY ROOM ==="
echo "Leave the room now. Data collection starts in 10 seconds."
sleep 10
echo "Recording for 5 minutes... ($(date))"
sleep 300
echo "Done. You may re-enter the room."
```

**What to do:** Leave the room. Close the door if possible. Stay out for
the full 5 minutes.

### Scenario 2: One Person Stationary (5 min)

```bash
echo "=== SCENARIO 2: 1 PERSON STATIONARY ==="
echo "Sit at a desk or chair. Stay still. Breathe normally."
sleep 300
echo "Done."
```

**What to do:** Sit at a desk roughly between the two ESP32 nodes. Stay
still. Breathe normally. Do not use your phone (arm movement adds noise).

### Scenario 3: One Person Walking (5 min)

```bash
echo "=== SCENARIO 3: 1 PERSON WALKING ==="
echo "Walk around the room at a normal pace."
sleep 300
echo "Done."
```

**What to do:** Walk around the room in varied paths. Go near each ESP32
node at least once. Walk at a normal pace -- not too fast, not too slow.

### Scenario 4: One Person Varied Activity (5 min)

```bash
echo "=== SCENARIO 4: 1 PERSON VARIED ==="
echo "Move around: stand, sit, wave arms, turn in place."
sleep 300
echo "Done."
```

**What to do:** Mix activities. Stand up, sit down, wave your arms, turn
around, reach for a shelf, crouch down. The goal is to capture a variety of
body positions and motions.

### Scenario 5: Two People (5 min)

```bash
echo "=== SCENARIO 5: TWO PEOPLE ==="
echo "Two people in the room, both moving around."
sleep 300
echo "Done."
```

**What to do:** Have a second person enter the room. Both people should
move around naturally -- walking, sitting, standing at different positions.

### Scenario 6: Transitions (5 min)

```bash
echo "=== SCENARIO 6: TRANSITIONS ==="
echo "Enter and exit the room repeatedly."
sleep 300
echo "Done."
```

**What to do:** Walk in and out of the room several times. Pause for
30-60 seconds inside, then leave for 30-60 seconds. This teaches the model
what state transitions look like.

### Expected Data Volume

After all 6 scenarios:

| Metric | Expected |
|--------|----------|
| Total time | 30 minutes |
| Vectors per node | ~1,800 |
| Total vectors (2 nodes) | ~3,600 |
| RVF store size | ~150 KB |
| Witness chain entries | ~360+ |

---

## 5. Monitoring Progress

### Check Seed Stats

At any time, open a new terminal and run:

```bash
python scripts/seed_csi_bridge.py --token "$SEED_TOKEN" --stats
```

Expected output (after completing all 6 scenarios):

```
=== Seed Status ===
  Device ID:      ecaf97dd-fc90-4b0e-b0e7-e9f896b9fbb6
  Total vectors:  3612
  Epoch:          362
  Dimension:      8
  Uptime:         3845s

=== Witness Chain ===
  Valid:          True
  Chain length:   1747
  Head:           a3b7c9d2e4f6g8h1i2j3k4l5m6n7...

=== Boundary Analysis ===
  Fragility score: 0.42
  Boundary count:  6

=== Coherence Profile ===
  phase_count: 6
  current_phase: 5
  coherence: 0.87

=== kNN Graph Stats ===
  nodes: 3612
  edges: 18060
  avg_degree: 5.0
```

> **What to look for:**
> - `Total vectors` should grow by ~2 per second (1 per node per second)
> - `Valid: True` on the witness chain means no data tampering
> - `Fragility score` rises during transitions and drops during stable
>   scenarios -- this is normal and expected
> - `phase_count` should roughly correspond to the number of distinct
>   scenarios the Seed has observed

### Verify kNN Quality

Query the Seed for the 5 nearest neighbors to a "someone present" vector:

```bash
curl -sk -X POST https://169.254.42.1:8443/api/v1/store/query \
  -H "Authorization: Bearer $SEED_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"vector": [0.8, 0.5, 0.5, 0.6, 0.5, 0.25, 0.0, 0.6], "k": 5}'
```

Expected output:

```json
{
  "results": [
    {"id": 2847193655, "distance": 0.023},
    {"id": 1038476291, "distance": 0.031},
    {"id": 3719284651, "distance": 0.045},
    {"id": 928374651, "distance": 0.052},
    {"id": 1847293746, "distance": 0.068}
  ]
}
```

Low distances (< 0.1) indicate the query vector is similar to stored
vectors -- the store contains meaningful data.

### Verify Witness Chain

The witness chain is a SHA-256 hash chain that proves no vectors were
tampered with after ingestion:

```bash
curl -sk -X POST https://169.254.42.1:8443/api/v1/witness/verify \
  -H "Authorization: Bearer $SEED_TOKEN"
```

Expected output:

```json
{
  "valid": true,
  "chain_length": 1747,
  "head": "a3b7c9d2e4f6..."
}
```

> **Warning:** If `valid` is `false`, the witness chain has been broken.
> This means data was modified outside the normal ingest path. Discard
> the dataset and re-collect.

---

## 6. Understanding the Feature Vectors

Each ESP32 node extracts an 8-dimensional feature vector once per second
from the 100 Hz CSI processing pipeline. Every dimension is normalized to
the range 0.0 to 1.0.

### Feature Dimension Table

| Dim | Name | Raw Source | Normalization | Range | Example Values |
|-----|------|-----------|---------------|-------|----------------|
| 0 | Presence score | `presence_score` | `/ 15.0`, clamped | 0.0 -- 1.0 | Empty: 0.01-0.05, Occupied: 0.19-1.0 |
| 1 | Motion energy | `motion_energy` | `/ 10.0`, clamped | 0.0 -- 1.0 | Still: 0.05-0.15, Walking: 0.3-0.8 |
| 2 | Breathing rate | `breathing_bpm` | `/ 30.0`, clamped | 0.0 -- 1.0 | Normal: 0.5-0.8 (15-24 BPM), At rest: 0.67-1.0 (20-34 BPM observed) |
| 3 | Heart rate | `heartrate_bpm` | `/ 120.0`, clamped | 0.0 -- 1.0 | Resting: 0.50-0.67 (60-80 BPM), Active: 0.63-0.83 (75-99 BPM observed) |
| 4 | Phase variance | Welford variance | Mean of top-K subcarriers | 0.0 -- 1.0 | Stable: 0.1-0.3, Disturbed: 0.5-0.9 |
| 5 | Person count | `n_persons / 4.0` | Clamped to [0, 1] | 0.0 -- 1.0 | 0 people: 0.0, 1 person: 0.25, 2 people: 0.5 |
| 6 | Fall detected | Binary flag | 1.0 if fall, else 0.0 | 0.0 or 1.0 | Normal: 0.0, Fall event: 1.0 |
| 7 | RSSI | `(rssi + 100) / 100` | Clamped to [0, 1] | 0.0 -- 1.0 | Close: 0.57-0.66 (-43 to -34 dBm), Far: 0.28-0.40 (-72 to -60 dBm) |

### How to Read a Feature Vector

Example vector from live validation:

```
[0.99, 0.47, 0.67, 0.63, 0.50, 0.25, 0.00, 0.57]
```

Reading this:

- **0.99** (dim 0, presence) -- Strong presence detected
- **0.47** (dim 1, motion) -- Moderate motion (slow walking or fidgeting)
- **0.67** (dim 2, breathing) -- 20.1 BPM (0.67 x 30), normal at-rest breathing
- **0.63** (dim 3, heart rate) -- 75.6 BPM (0.63 x 120), normal resting heart rate
- **0.50** (dim 4, phase variance) -- Placeholder (future use)
- **0.25** (dim 5, person count) -- 1 person (0.25 x 4 = 1)
- **0.00** (dim 6, fall) -- No fall detected
- **0.57** (dim 7, RSSI) -- RSSI of -43 dBm ((0.57 x 100) - 100), strong signal

### Packet Format

The feature vector is transmitted as a 48-byte binary packet with magic
number `0xC5110003`:

```
Offset  Size  Type     Field
------  ----  -------  ----------------
0       4     uint32   magic (0xC5110003)
4       1     uint8    node_id
5       1     uint8    reserved
6       2     uint16   sequence number
8       8     int64    timestamp (microseconds since boot)
16      32    float[8] feature vector (8 x 4 bytes)
------  ----
Total: 48 bytes
```

---

## 7. Using the Pre-trained Data

After collecting 30 minutes of data, the Seed holds ~3,600 feature vectors
organized as a kNN graph with witness chain attestation.

### Query for Similar States

Find vectors similar to "one person sitting quietly":

```bash
curl -sk -X POST https://169.254.42.1:8443/api/v1/store/query \
  -H "Authorization: Bearer $SEED_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"vector": [0.8, 0.1, 0.6, 0.6, 0.5, 0.25, 0.0, 0.5], "k": 10}'
```

Find vectors similar to "empty room":

```bash
curl -sk -X POST https://169.254.42.1:8443/api/v1/store/query \
  -H "Authorization: Bearer $SEED_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"vector": [0.05, 0.02, 0.0, 0.0, 0.3, 0.0, 0.0, 0.5], "k": 10}'
```

### Environment Fingerprinting

The Seed's boundary analysis detects regime changes in the vector space.
When someone enters or leaves the room, the fragility score spikes:

```bash
curl -sk https://169.254.42.1:8443/api/v1/boundary
```

```json
{
  "fragility_score": 0.42,
  "boundary_count": 6
}
```

A `fragility_score` above 0.3 indicates the environment is in or near a
transition state. The `boundary_count` roughly corresponds to the number
of distinct "states" (scenarios) the Seed has observed.

### Export Vectors

To export all vectors for offline analysis or training:

```bash
curl -sk https://169.254.42.1:8443/api/v1/store/export \
  -H "Authorization: Bearer $SEED_TOKEN" \
  -o pretrain-vectors.rvf
```

The exported `.rvf` file contains the raw vector data and can be loaded
by the Rust training pipeline (`wifi-densepose-train` crate) or converted
to NumPy arrays for Python-based training.

### Compact the Store

For long-running deployments, run compaction daily to keep the store
within the Seed's memory budget:

```bash
python scripts/seed_csi_bridge.py --token "$SEED_TOKEN" --compact
```

```
Triggering store compaction...
Compaction result: {
  "vectors_before": 3612,
  "vectors_after": 3200,
  "bytes_freed": 16544
}
```

### Use with the Sensing Server

Start a recording session to capture the raw CSI frames alongside the
feature vectors (the sensing-server provides the recording API):

```bash
# Start the recording (5 minutes)
curl -X POST http://localhost:3000/api/v1/recording/start \
  -H "Content-Type: application/json" \
  -d '{"session_name":"pretrain-1p-still","label":"1p-still","duration_secs":300}'
```

The recording saves `.csi.jsonl` files that the `wifi-densepose-train`
crate can load for full contrastive pretraining (see ADR-070).

---

## 8. Troubleshooting

### ESP32 Won't Connect to WiFi

**Symptoms:** No packets received, ESP32 serial output shows repeated
"WiFi: Connecting..." messages.

**Fixes:**
1. Verify SSID and password are correct (re-provision if needed)
2. Make sure you are on a 2.4 GHz network (ESP32 does not support 5 GHz)
3. Move the ESP32 closer to the access point
4. Check the serial output for the exact error:

```bash
python -m serial.tools.miniterm COM9 115200
```

Look for lines like `wifi:connected` or `wifi:reason 201` (wrong password).

### Bridge Shows 0 Packets

**Symptoms:** Bridge starts but never logs "Ingested" messages.

**Fixes:**
1. Make sure the ESP32's `--target-ip` matches your laptop's IP
2. Check that `--target-port` matches `--udp-port` on the bridge (default: 5006)
3. Check your firewall -- UDP port 5006 must be open for inbound traffic
4. Run the UDP listener test from Section 2.5 to confirm raw packets arrive
5. If using `--allowed-sources`, make sure the ESP32 IP addresses are listed

### Seed Returns 401 Unauthorized

**Symptoms:** Bridge logs `HTTP Error 401` on ingest.

**Fixes:**
1. Make sure `$SEED_TOKEN` is set correctly: `echo $SEED_TOKEN`
2. Re-pair the Seed if the token was lost (Section 2.2)
3. Verify the token works with a status query:

```bash
curl -sk -H "Authorization: Bearer $SEED_TOKEN" \
  https://169.254.42.1:8443/api/v1/store/graph/stats
```

### NaN Values in Features

**Symptoms:** Bridge logs `Dropping feature packet: features[X]=nan (NaN/inf)`.

**Fixes:**
- This is expected during the first few seconds after ESP32 boot while the
  DSP pipeline initializes. The bridge automatically drops NaN/inf packets.
- If NaN persists beyond 10 seconds, reflash the firmware -- the DSP state
  may be corrupted.

### ENOMEM on ESP32 Boot

**Symptoms:** Serial output shows `E (xxx) heap: alloc failed` or
`ENOMEM` errors.

**Fixes:**
1. If using a 4MB flash ESP32-S3, use the 4MB partition table and
   sdkconfig (see `sdkconfig.defaults.4mb`)
2. Reduce buffer sizes by setting edge tier to 1 during provisioning:

```bash
python firmware/esp32-csi-node/provision.py \
  --port COM9 --edge-tier 1 \
  --ssid "YourWiFi" --password "YourPassword" \
  --target-ip 192.168.1.20 --node-id 1
```

### Seed Not Reachable at 169.254.42.1

**Symptoms:** `curl` to `169.254.42.1:8443` times out.

**Fixes:**
1. Ensure you are using a **data** USB cable (charge-only cables lack data pins)
2. Wait 60 seconds after plugging in for the Seed to fully boot
3. Check the USB network interface appeared on your host:

```bash
# Windows
ipconfig | findstr "169.254"

# macOS / Linux
ip addr show | grep "169.254"
```

4. If the Seed is on WiFi instead, use its WiFi IP (e.g., `192.168.1.109`):

```bash
python scripts/seed_csi_bridge.py \
  --seed-url https://192.168.1.109:8443 \
  --token "$SEED_TOKEN"
```

### Bridge Ingest Failures (Connection Reset)

**Symptoms:** Periodic `Ingest failed` messages, then recovery.

**Fixes:**
- The bridge retries once automatically (2-second delay). Occasional failures
  are normal when the Seed is rebuilding its kNN graph.
- If failures are frequent (>10% of batches), increase `--batch-size` to
  reduce the number of HTTPS calls:

```bash
python scripts/seed_csi_bridge.py --token "$SEED_TOKEN" --batch-size 20
```

---

## 9. Next Steps

### Full Contrastive Pretraining (ADR-070)

This tutorial covers Phase 1 (data collection) of the pretraining pipeline
defined in [ADR-070](../adr/ADR-070-self-supervised-pretraining.md). The
remaining phases are:

- **Phase 2: Contrastive pretraining** -- Train a TCN encoder using temporal
  coherence and multi-node consistency as self-supervised signals
- **Phase 3: Downstream heads** -- Attach task-specific heads (presence,
  person count, activity, vital signs) using weak labels from the Seed's
  PIR sensor and scenario boundaries
- **Phase 4: Package and distribute** -- Export as ONNX model weights for
  distribution in GitHub releases

### Architecture Documentation

- [ADR-069: ESP32 CSI to Cognitum Seed Pipeline](../adr/ADR-069-cognitum-seed-csi-pipeline.md) --
  Full architecture of the bridge pipeline
- [ADR-070: Self-Supervised Pretraining](../adr/ADR-070-self-supervised-pretraining.md) --
  Complete pretraining pipeline design

### Multi-Node Mesh

Scale to 3-4 ESP32 nodes for better spatial coverage. Each node gets a
unique `--node-id` and all target the same host laptop. The Seed's kNN
graph naturally clusters vectors by node and sensing state.

### Cognitum Seed Resources

- [cognitum.one](https://cognitum.one) -- Hardware and firmware information
- Seed API: 98 HTTPS endpoints with bearer token authentication
- MCP proxy: 114 tools accessible via JSON-RPC 2.0 for AI assistant integration

### Rust Training Pipeline

For users with the Rust toolchain, the `wifi-densepose-train` crate
provides the full training pipeline with RuVector integration:

```bash
cd v2
cargo run -p wifi-densepose-train -- \
  --data pretrain-vectors.rvf \
  --epochs 50 \
  --output pretrained-encoder.onnx
```
