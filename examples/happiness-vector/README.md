# Happiness Vector — WiFi CSI Guest Sentiment Sensing

Contactless hotel guest happiness scoring using WiFi Channel State Information (CSI) from ESP32-S3 nodes, coordinated by a Cognitum Seed edge intelligence appliance.

No cameras. No microphones. No PII. Just radio waves.

## How It Works

```
Guest walks through lobby
        |
        v
  ESP32-S3 Node (WiFi CSI at 20 Hz)
        |
        v
  Tier 2 Edge DSP (Core 1)
  - Phase rate-of-change --> gait speed
  - Step interval variance --> stride regularity
  - Phase 2nd derivative --> movement fluidity
  - 0.15-0.5 Hz oscillation --> breathing rate
  - Amplitude spread --> posture
  - Presence duration --> dwell time
        |
        v
  8-dim Happiness Vector
  [happiness, gait, stride, fluidity, calm, posture, dwell, social]
        |
        v
  Cognitum Seed (Pi Zero 2 W)
  - kNN similarity search
  - Concept drift detection (13 detectors)
  - Ed25519 witness chain (tamper-proof audit)
  - Reflex rules (trigger actuators on patterns)
```

## The 8 Dimensions

| Dim | Name | Source | Happy | Unhappy |
|-----|------|--------|-------|---------|
| 0 | **Happiness Score** | Weighted composite of dims 1-6 | 0.7-1.0 | 0.0-0.3 |
| 1 | **Gait Speed** | Phase Doppler shift | Fast (0.8+) | Slow (0.2) |
| 2 | **Stride Regularity** | Step interval CV (inverted) | Regular (0.9) | Erratic (0.3) |
| 3 | **Movement Fluidity** | Phase acceleration (inverted) | Smooth (0.8) | Jerky (0.2) |
| 4 | **Breathing Calm** | 0.15-0.5 Hz phase oscillation | Slow/deep (0.8) | Rapid (0.2) |
| 5 | **Posture Score** | Amplitude spread across subcarriers | Upright (0.7) | Slouched (0.3) |
| 6 | **Dwell Factor** | Presence frame ratio | Lingering (0.8) | Rushing (0.2) |
| 7 | **Social Energy** | Motion + dwell + HR proxy | Animated group (0.8) | Solitary (0.2) |

Weights: gait 25%, fluidity 20%, calm 20%, stride 15%, posture 10%, dwell 10%.

## Hardware

| Component | Model | Role | Cost |
|-----------|-------|------|------|
| ESP32-S3 | QFN56 (4MB flash, 2MB PSRAM) | CSI sensing node | ~$4 |
| Cognitum Seed | Pi Zero 2 W | Swarm coordinator | ~$20 |
| WiFi Router | Any 2.4 GHz | CSI signal source | existing |

One Seed manages up to 20 ESP32 nodes. Each node covers ~10m radius through walls.

## Quick Start

### 1. Flash and Provision an ESP32 Node

```bash
# Build firmware (from repo root)
cd firmware/esp32-csi-node
idf.py build

# Flash to device
idf.py -p COM5 flash

# Provision with WiFi + Seed credentials
python provision.py \
  --port COM5 \
  --ssid "YourWiFi" \
  --password "yourpassword" \
  --node-id 1 \
  --seed-url "http://10.1.10.236" \
  --seed-token "YOUR_SEED_TOKEN" \
  --zone "lobby"
```

### 2. Pair the Seed (first time only)

```bash
# Via USB (link-local, no token needed)
curl -X POST http://169.254.42.1/api/v1/pair/window
curl -X POST http://169.254.42.1/api/v1/pair -H "Content-Type: application/json" \
  -d '{"name":"esp32-swarm"}'
# Save the token from the response
```

### 3. Run the Dashboard

```bash
# Happiness mode with Seed bridge
python examples/ruview_live.py \
  --mode happiness \
  --csi COM5 \
  --seed http://10.1.10.236 \
  --duration 300

# Output:
#    s             Happy   Gait   Calm  Social  Pres   RSSI    Seed   CSI#
#   2s  [====------] 0.43   0.00   0.64    0.00    no    -59      OK   1800
#  10s  [=======---] 0.72   0.65   0.80    0.45   YES    -55      OK   4200
```

### 4. Query the Seed

```bash
# Status
python examples/happiness-vector/seed_query.py \
  --seed http://10.1.10.236 --token YOUR_TOKEN status

# Live monitor vectors flowing in
python examples/happiness-vector/seed_query.py \
  --seed http://10.1.10.236 --token YOUR_TOKEN monitor

# Happiness report
python examples/happiness-vector/seed_query.py \
  --seed http://10.1.10.236 --token YOUR_TOKEN report

# Witness chain audit
python examples/happiness-vector/seed_query.py \
  --seed http://10.1.10.236 --token YOUR_TOKEN witness
```

## Multi-Node Swarm

Deploy multiple ESP32 nodes across zones. The Seed aggregates all vectors and detects cross-zone patterns.

```bash
# Provision all nodes at once
bash examples/happiness-vector/provision_swarm.sh

# Or manually per node
python provision.py --port COM5  --node-id 1 --zone lobby      ...
python provision.py --port COM6  --node-id 2 --zone hallway    ...
python provision.py --port COM8  --node-id 3 --zone restaurant ...
```

Each node independently:
- Collects CSI at ~100 fps
- Runs Tier 2 DSP on Core 1 (presence, vitals, fall detection)
- Pushes happiness vectors to Seed every 5 seconds (when presence detected)
- Sends heartbeats every 30 seconds

The Seed provides:
- **kNN search** across all zones ("which room is happiest right now?")
- **Drift detection** (13 detectors monitoring mood trends over time)
- **Witness chain** (Ed25519-signed, tamper-proof audit trail)
- **Reflex rules** (trigger alarms, lights, or alerts on swarm-wide patterns)

## WASM Edge Modules

The happiness scoring algorithm also exists as a WASM module for on-device execution:

```bash
# Build the happiness scorer WASM
cd rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm-edge
cargo build --bin ghost_hunter --target wasm32-unknown-unknown --release --no-default-features

# Output: target/wasm32-unknown-unknown/release/ghost_hunter.wasm (5.7 KB)
```

Event IDs emitted by the WASM module:

| ID | Event | Rate |
|----|-------|------|
| 690 | `HAPPINESS_SCORE` | Every frame (20 Hz) |
| 691 | `GAIT_ENERGY` | Every 4th frame (5 Hz) |
| 692 | `AFFECT_VALENCE` | Every 4th frame |
| 693 | `SOCIAL_ENERGY` | Every 4th frame |
| 694 | `TRANSIT_DIRECTION` | Every 4th frame |

## Privacy

This system is designed to be privacy-preserving by construction:

- **No images** — WiFi CSI captures RF signal patterns, not visual data
- **No audio** — radio waves only
- **No facial recognition** — physically impossible with CSI
- **No individual identity** — cannot distinguish Bob from Alice
- **Aggregate only** — 8 floating-point numbers per observation
- **Works in the dark** — RF sensing needs no lighting
- **Through-wall** — single sensor covers adjacent rooms without line-of-sight
- **GDPR-friendly** — no personal data collected; happiness scores are anonymous statistical aggregates

## Files

| File | Description |
|------|-------------|
| `seed_query.py` | CLI tool: status, search, witness, monitor, report |
| `provision_swarm.sh` | Batch provisioning for multi-node deployment |
| `happiness_vector_schema.json` | JSON Schema for the 8-dim vector format |
| `README.md` | This file |

## Related

- [ADR-065](../../docs/adr/ADR-065-happiness-scoring-seed-bridge.md) — Happiness scoring pipeline architecture
- [ADR-066](../../docs/adr/ADR-066-esp32-swarm-seed-coordinator.md) — ESP32 swarm with Seed coordinator
- [exo_happiness_score.rs](../../rust-port/wifi-densepose-rs/crates/wifi-densepose-wasm-edge/src/exo_happiness_score.rs) — WASM edge module (Rust)
- [swarm_bridge.c](../../firmware/esp32-csi-node/main/swarm_bridge.c) — ESP32 firmware swarm bridge
- [ruview_live.py](../ruview_live.py) — RuView Live dashboard with `--mode happiness`
