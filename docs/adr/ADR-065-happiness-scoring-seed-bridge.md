# ADR-065: Hotel Guest Happiness Scoring -- WiFi CSI + Cognitum Seed Bridge

**Status:** Proposed
**Date:** 2026-03-20
**Deciders:** @ruvnet
**Related:** ADR-040 (WASM edge modules), ADR-039 (edge intelligence), ADR-042 (CHCI), ADR-064 (multimodal ambient intelligence), ADR-060 (multi-node aggregation)

## Context

Hotels lack objective, privacy-preserving methods to measure guest satisfaction in real time. Current approaches (post-stay surveys, NPS scores) are delayed, biased toward extremes, and capture less than 10% of guests. Meanwhile, ambient RF sensing can infer behavioral cues that correlate with comfort and well-being -- without cameras, wearables, or any guest interaction.

### Hardware

Two ESP32-S3 variants are deployed:

| Device | Flash | PSRAM | MAC | Port | Notes |
|--------|-------|-------|-----|------|-------|
| ESP32-S3 (QFN56 rev 0.2) | 4 MB | 2 MB | 1C:DB:D4:83:D2:40 | COM5 | Budget node, uses `sdkconfig.defaults.4mb` + `partitions_4mb.csv` |
| ESP32-S3 | 8 MB | 8 MB | -- | COM7 | Full-featured node, existing deployment |

Both run the Tier 2 DSP firmware with presence detection, vitals extraction, fall detection, and gait analysis.

### Cognitum Seed Device

A Cognitum Seed unit is deployed on the same network segment:

- **Address:** 169.254.42.1 (link-local)
- **Hardware:** Raspberry Pi Zero 2 W
- **Firmware:** 0.7.0
- **Vector store:** 398 vectors, dim=8
- **API endpoints:** 98 (REST, fully documented)
- **Sensors:** PIR, reed switch (door), vibration, ADS1115 ADC (4-ch analog), BME280 (temp/humidity/pressure)
- **Security:** Ed25519 custody chain with tamper-evident witness log

The Seed's 8-dimensional vector store and drift detection engine make it a natural aggregation point for behavioral feature vectors extracted from CSI data.

### Existing WASM Edge Modules

The following modules already run on-device and produce features relevant to happiness scoring:

| Module | Event IDs | Outputs |
|--------|-----------|---------|
| `exo_emotion_detect.rs` | 610-613 | Arousal level, stress index |
| `med_gait_analysis.rs` | 130-134 | Cadence, stride length, regularity |
| `ret_customer_flow.rs` | 410-413 | Entry/exit count, direction |
| `ret_dwell_heatmap.rs` | 420-423 | Dwell time per zone |

## Decision

### 1. New WASM Module: `exo_happiness_score.rs`

Create a new WASM edge module that fuses outputs from existing modules into an 8-dimensional happiness vector, matching the Seed's vector dimensionality (dim=8).

**Event ID registry (690-694):**

| Event ID | Name | Description |
|----------|------|-------------|
| 690 | `HAPPINESS_VECTOR` | Full 8-dim happiness vector emitted per scoring window |
| 691 | `HAPPINESS_TREND` | Windowed trend (rising/falling/stable) over last N vectors |
| 692 | `HAPPINESS_ALERT` | Score crossed a configured threshold (low satisfaction) |
| 693 | `HAPPINESS_GROUP` | Aggregate score for multi-person zone |
| 694 | `HAPPINESS_CALIBRATION` | Baseline recalibration event (new guest check-in) |

### 2. Happiness Vector Schema (8 Dimensions)

Each dimension is normalized to [0.0, 1.0] where 1.0 = maximal positive signal:

| Dim | Name | Source | Derivation |
|-----|------|--------|------------|
| 0 | `gait_speed` | `med_gait_analysis` (130) | Normalized walking velocity. Brisk = positive. |
| 1 | `stride_regularity` | `med_gait_analysis` (131) | Low stride-to-stride variance = relaxed gait. |
| 2 | `movement_fluidity` | CSI phase jerk (d3/dt3) | Low jerk = smooth, unhurried movement. |
| 3 | `breathing_calm` | Vitals BR extraction | BR 12-18 at rest = calm. Deviation penalized. |
| 4 | `posture_openness` | CSI subcarrier spread | Wide phase spread across subcarriers = open posture. |
| 5 | `dwell_comfort` | `ret_dwell_heatmap` (420) | Moderate dwell in amenity zones = engagement. |
| 6 | `direction_entropy` | `ret_customer_flow` (410) | Low entropy = purposeful movement. Wandering penalized. |
| 7 | `group_energy` | Multi-target CSI clustering | Synchronized movement of 2+ people = social engagement. |

The composite scalar happiness score is the weighted L2 norm:

```
score = sum(w[i] * v[i] for i in 0..7) / sum(w[i])
```

Default weights are uniform (all 1.0), configurable via NVS or Seed API.

### 3. ESP32 to Seed Bridge

```
ESP32-S3 (CSI)                    Cognitum Seed (169.254.42.1)
+------------------+              +----------------------------+
| Tier 2 DSP       |              |                            |
| + WASM modules   |  UDP 5555   | /api/v1/store/ingest       |
| exo_happiness    |──────────────| (POST, 8-dim vector)       |
|   _score.rs      |              |                            |
|                  |              | /api/v1/drift/check        |
|                  |◄─────────────| (drift alerts via webhook) |
|                  |              |                            |
|                  |              | /api/v1/witness/append     |
|                  |              | (Ed25519 audit trail)      |
+------------------+              +----------------------------+
```

**Data flow:**

1. ESP32 runs CSI capture at 20+ Hz and feeds subcarrier data through existing WASM modules.
2. `exo_happiness_score.rs` collects outputs from emotion, gait, flow, and dwell modules every scoring window (default: 30 seconds).
3. The 8-dim happiness vector is packed as a 32-byte payload (8x float32) and sent via UDP to port 5555 on 169.254.42.1.
4. A lightweight bridge task on the Seed receives the UDP packet and POSTs it to `/api/v1/store/ingest` with metadata (room ID, timestamp, MAC).
5. The Seed's drift detection engine monitors the happiness vector stream and flags anomalies (sudden drops, sustained low scores).
6. Every ingested vector is appended to the Seed's Ed25519 witness chain, providing a tamper-proof audit trail.

### 4. Seed Drift Detection for Happiness Trends

The Seed's built-in drift detection compares incoming vectors against a rolling baseline:

- **Check-in calibration:** When a new guest checks in, event 694 resets the baseline.
- **Drift threshold:** Configurable (default: cosine distance > 0.3 from baseline triggers alert).
- **Trend window:** Last 20 vectors (~10 minutes at 30s intervals).
- **Alert routing:** Seed webhook notifies hotel management system when happiness trend is declining.

### 5. RuView Live Dashboard Update

`ruview_live.py` gains a `--seed` flag:

```bash
python ruview_live.py --port COM5 --seed 169.254.42.1 --mode happiness
```

This mode displays:
- Real-time 8-dim radar chart of the happiness vector
- Scalar happiness score (0-100) with color coding (red/yellow/green)
- Trend sparkline over the last hour
- Seed witness chain status (last hash, chain length)
- Room-level aggregate when multiple ESP32 nodes report

### 6. Architecture

```
                    +------------------------------------------+
                    |              Hotel Room                   |
                    |                                           |
                    |  [ESP32-S3]         [Cognitum Seed]       |
                    |  COM5 or COM7       169.254.42.1          |
                    |  4MB or 8MB flash   Pi Zero 2 W           |
                    |       |                    |               |
                    |       | WiFi CSI           | PIR, reed,   |
                    |       | 20+ Hz             | BME280,      |
                    |       v                    | vibration    |
                    |  +-----------+             |               |
                    |  | Tier 2 DSP|             v               |
                    |  | presence  |      +-------------+       |
                    |  | vitals    |      | Seed API    |       |
                    |  | gait      |      | 98 endpoints|       |
                    |  | fall det  |      | 398 vectors |       |
                    |  +-----------+      | dim=8       |       |
                    |       |             +-------------+       |
                    |       v                    ^               |
                    |  +-----------+   UDP 5555  |              |
                    |  | WASM edge |─────────────┘              |
                    |  | happiness |                            |
                    |  | score     |   Drift alerts             |
                    |  | (690-694) |◄──────────────             |
                    |  +-----------+   /api/v1/drift/check      |
                    |                                           |
                    +------------------------------------------+
                              |
                              | MQTT / HTTP
                              v
                    +------------------+
                    | Hotel Management |
                    | System / RuView  |
                    | Live Dashboard   |
                    +------------------+
```

### 7. 4MB Flash Support

The 4MB ESP32-S3 variant (COM5) is officially supported for happiness scoring. The existing `partitions_4mb.csv` and `sdkconfig.defaults.4mb` from ADR-265 provide dual OTA slots (1.856 MB each), sufficient for the full Tier 2 DSP firmware plus `exo_happiness_score.wasm` (estimated < 40 KB).

Build for 4MB variant:

```bash
cp sdkconfig.defaults.4mb sdkconfig.defaults
idf.py build
```

The WASM module loader selects which modules to instantiate based on available heap. On the 4MB/2MB PSRAM variant, happiness scoring runs with a reduced scoring window (60s instead of 30s) to conserve memory.

### 8. Privacy Considerations

- **No cameras.** All sensing is RF-based (WiFi subcarrier amplitude/phase).
- **No facial recognition.** Happiness is inferred from movement patterns, not expressions.
- **No audio capture.** Breathing rate is extracted from chest wall displacement via RF, not microphone.
- **No PII stored on device.** Vectors are anonymous; room-to-guest mapping lives only in the hotel PMS.
- **Seed witness chain** provides auditable proof of what data was collected and when, satisfying GDPR Article 30 record-keeping requirements.
- **Guest opt-out:** A physical switch on the ESP32 node (GPIO connected to a toggle) disables CSI capture entirely. The Seed's reed switch can also serve as a "privacy mode" trigger (door-mounted magnet removed = sensing paused).
- **Data retention:** Vectors are retained on the Seed for the duration of the stay plus 24 hours, then purged. The witness chain retains hashes (not vectors) indefinitely for audit.

### 9. API Integration

Key Cognitum Seed endpoints used:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/v1/store/ingest` | POST | Ingest 8-dim happiness vector |
| `/api/v1/store/query` | POST | Retrieve vectors by room/time range |
| `/api/v1/drift/check` | GET | Check if current vector drifts from baseline |
| `/api/v1/drift/configure` | PUT | Set drift threshold and window size |
| `/api/v1/witness/append` | POST | Append event to Ed25519 custody chain |
| `/api/v1/witness/verify` | GET | Verify chain integrity |
| `/api/v1/sensors/bme280` | GET | Room temperature/humidity (comfort correlation) |
| `/api/v1/sensors/pir` | GET | PIR presence (cross-validate with CSI) |

## Consequences

### Positive

- Provides real-time, objective guest satisfaction measurement without surveys or wearables.
- Reuses four existing WASM modules -- the happiness module is a fusion layer, not a rewrite.
- The Seed's 8-dim vector store is a natural fit; no schema changes needed.
- Ed25519 witness chain satisfies hospitality industry audit requirements and GDPR record-keeping.
- Both 4MB and 8MB ESP32-S3 variants are supported, enabling low-cost deployment at scale (~$8 per room for the 4MB node).
- Seed's environmental sensors (BME280, PIR) provide complementary context (room temperature, humidity) that can be correlated with happiness scores.
- No cloud dependency -- all processing is local (ESP32 edge + Seed link-local network).

### Negative

- Happiness inference from movement patterns is a proxy, not a direct measurement. Correlation with actual guest satisfaction must be validated empirically.
- The 4MB variant has reduced scoring frequency (60s vs 30s) due to memory constraints.
- UDP transport between ESP32 and Seed is unreliable; packets may be lost. Mitigation: sequence numbers and a small retry buffer on the ESP32 side.
- Link-local addressing (169.254.x.x) limits the Seed to the same network segment as the ESP32. Multi-room deployments need one Seed per subnet or a routed bridge.
- Drift detection thresholds require per-property tuning; a luxury resort has different movement patterns than a budget hotel.
- The system cannot distinguish between guests in a multi-occupancy room without additional multi-target CSI clustering, which is experimental (ADR-064, Tier 3).
