# ADR-063: 60 GHz mmWave Sensor Fusion with WiFi CSI

**Status:** Proposed
**Date:** 2026-03-15
**Deciders:** @ruvnet
**Related:** ADR-014 (SOTA signal processing), ADR-021 (vital sign extraction), ADR-029 (RuvSense multistatic), ADR-039 (edge intelligence), ADR-042 (CHCI coherent sensing)

## Context

RuView currently senses the environment using WiFi CSI — a passive technique that analyzes how WiFi signals are disturbed by human presence and movement. While this works through walls and requires no line of sight, CSI-derived vital signs (breathing rate, heart rate) are inherently noisy because they rely on phase extraction from multipath-rich WiFi channels.

A complementary sensing modality exists: **60 GHz mmWave radar** modules (e.g., Seeed MR60BHA2) that use active FMCW radar at 60 GHz to measure breathing and heart rate with clinical-grade accuracy. These modules are inexpensive (~$15), run on ESP32-C6/C3, and output structured vital signs over UART.

**Live hardware capture (COM4, 2026-03-15)** from a Seeed MR60BHA2 on an ESP32-C6 running ESPHome:

```
[D][sensor:093]: 'Real-time respiratory rate': Sending state 22.00000
[D][sensor:093]: 'Real-time heart rate': Sending state 92.00000 bpm
[D][sensor:093]: 'Distance to detection object': Sending state 0.00000 cm
[D][sensor:093]: 'Target Number': Sending state 0.00000
[D][binary_sensor:036]: 'Person Information': Sending state OFF
[D][sensor:093]: 'Seeed MR60BHA2 Illuminance': Sending state 0.67913 lx
```

### The Opportunity

Fusing WiFi CSI with mmWave radar creates a sensor system that is greater than the sum of its parts:

| Capability | WiFi CSI Alone | mmWave Alone | Fused |
|-----------|---------------|-------------|-------|
| Through-wall sensing | Yes (5m+) | No (LoS only, ~3m) | Yes — CSI for room-scale, mmWave for precision |
| Heart rate accuracy | ±5-10 BPM | ±1-2 BPM | ±1-2 BPM (mmWave primary, CSI cross-validates) |
| Breathing accuracy | ±2-3 BPM | ±0.5 BPM | ±0.5 BPM |
| Presence detection | Good (adaptive threshold) | Excellent (range-gated) | Excellent + through-wall |
| Multi-person | Via subcarrier clustering | Via range-Doppler bins | Combined spatial + RF resolution |
| Fall detection | Phase acceleration | Range/velocity + micro-Doppler | Dual-confirm reduces false positives to near-zero |
| Pose estimation | Via trained model | Not available | CSI provides pose; mmWave provides ground-truth vitals for training |
| Coverage | Whole room (passive) | ~120° cone, 3m range | Full room + precision zone |
| Cost per node | ~$9 (ESP32-S3) | ~$15 (ESP32-C6 + MR60BHA2) | ~$24 combined |

### RuVector Integration Points

The RuVector v2.0.4 stack (already integrated per ADR-016) provides the signal processing backbone:

| RuVector Component | Role in mmWave Fusion |
|-------------------|----------------------|
| `ruvector-attention` (`bvp.rs`) | Blood Volume Pulse estimation — mmWave heart rate can calibrate the WiFi CSI BVP phase extraction |
| `ruvector-temporal-tensor` (`breathing.rs`) | Breathing rate estimation — mmWave provides ground-truth for adaptive filter tuning |
| `ruvector-solver` (`triangulation.rs`) | Multilateration — mmWave range-gated distance + CSI amplitude = 3D position |
| `ruvector-attn-mincut` (`spectrogram.rs`) | Time-frequency decomposition — mmWave Doppler complements CSI phase spectrogram |
| `ruvector-mincut` (`metrics.rs`, DynamicPersonMatcher) | Multi-person association — mmWave target IDs help disambiguate CSI subcarrier clusters |

### RuvSense Integration Points

The RuvSense multistatic sensing pipeline (ADR-029) gains new capabilities:

| RuvSense Module | mmWave Integration |
|----------------|-------------------|
| `pose_tracker.rs` (AETHER re-ID) | mmWave distance + velocity as additional re-ID features for Kalman tracker |
| `longitudinal.rs` (Welford stats) | mmWave vitals as reference signal for CSI drift detection |
| `intention.rs` (pre-movement) | mmWave micro-Doppler detects pre-movement 100-200ms earlier than CSI |
| `adversarial.rs` (consistency check) | mmWave provides independent signal to detect CSI spoofing/anomalies |
| `coherence_gate.rs` | mmWave presence as additional gate input — if mmWave says "no person", CSI coherence gate rejects |

### Cross-Viewpoint Fusion Integration

The viewpoint fusion pipeline (`ruvector/src/viewpoint/`) extends naturally:

| Viewpoint Module | mmWave Extension |
|-----------------|-----------------|
| `attention.rs` (CrossViewpointAttention) | mmWave range becomes a new "viewpoint" in the attention mechanism |
| `geometry.rs` (GeometricDiversityIndex) | mmWave cone geometry contributes to Fisher Information / Cramer-Rao bounds |
| `coherence.rs` (phase phasor) | mmWave phase coherence as validation for WiFi phasor coherence |
| `fusion.rs` (MultistaticArray) | mmWave node becomes a member of the multistatic array with its own domain events |

## Decision

Add 60 GHz mmWave radar sensor support to the RuView firmware and sensing pipeline with auto-detection and device-specific capabilities.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Sensing Node                          │
│                                                          │
│  ┌──────────────┐    ┌──────────────┐    ┌────────────┐ │
│  │ ESP32-S3     │    │ ESP32-C6     │    │ Combined   │ │
│  │ WiFi CSI     │    │ + MR60BHA2   │    │ S3 + UART  │ │
│  │ (COM7)       │    │ 60GHz mmWave │    │ mmWave     │ │
│  │              │    │ (COM4)       │    │            │ │
│  │ Passive      │    │ Active radar │    │ Both modes │ │
│  │ Through-wall │    │ LoS, precise │    │            │ │
│  └──────┬───────┘    └──────┬───────┘    └─────┬──────┘ │
│         │                    │                   │       │
│         └────────┬───────────┘                   │       │
│                  ▼                               │       │
│         ┌────────────────┐                       │       │
│         │ Fusion Engine  │◄──────────────────────┘       │
│         │                │                               │
│         │ • Kalman fuse  │  Vitals packet (extended):    │
│         │ • Cross-validate│  magic 0xC5110004             │
│         │ • Ground-truth │  + mmwave_hr, mmwave_br       │
│         │   calibration  │  + mmwave_distance             │
│         │ • Fall confirm │  + mmwave_target_count         │
│         └────────────────┘  + confidence scores           │
└─────────────────────────────────────────────────────────┘
```

### Three Deployment Modes

**Mode 1: Standalone CSI (existing)** — ESP32-S3 only, WiFi CSI sensing.

**Mode 2: Standalone mmWave** — ESP32-C6 + MR60BHA2, precise vitals in a single room.

**Mode 3: Fused (recommended)** — ESP32-S3 + mmWave module on UART, or two separate nodes with server-side fusion.

### Auto-Detection Protocol

The firmware will auto-detect connected mmWave modules at boot:

1. **UART probe** — On configured UART pins, send the MR60BHA2 identification command (`0x01 0x01 0x00 0x01 ...`) and check for valid response header
2. **Protocol detection** — Identify the sensor family:
   - Seeed MR60BHA2 (breathing + heart rate)
   - Seeed MR60FDA1 (fall detection)
   - Seeed MR24HPC1 (presence + light sleep/deep sleep)
   - HLK-LD2410 (presence + distance)
   - HLK-LD2450 (multi-target tracking)
3. **Capability registration** — Register detected sensor capabilities in the edge config:

```c
typedef struct {
    uint8_t  mmwave_detected;      /** 1 if mmWave module found on UART */
    uint8_t  mmwave_type;          /** Sensor family (MR60BHA2, MR60FDA1, etc.) */
    uint8_t  mmwave_has_hr;        /** Heart rate capability */
    uint8_t  mmwave_has_br;        /** Breathing rate capability */
    uint8_t  mmwave_has_fall;      /** Fall detection capability */
    uint8_t  mmwave_has_presence;  /** Presence detection capability */
    uint8_t  mmwave_has_distance;  /** Range measurement capability */
    uint8_t  mmwave_has_tracking;  /** Multi-target tracking capability */
    float    mmwave_hr_bpm;        /** Latest heart rate from mmWave */
    float    mmwave_br_bpm;        /** Latest breathing rate from mmWave */
    float    mmwave_distance_cm;   /** Distance to nearest target */
    uint8_t  mmwave_target_count;  /** Number of detected targets */
    bool     mmwave_person_present;/** mmWave presence state */
} mmwave_state_t;
```

### Supported Sensors

| Sensor | Frequency | Capabilities | UART Protocol | Cost |
|--------|-----------|-------------|---------------|------|
| **Seeed MR60BHA2** | 60 GHz | HR, BR, presence, illuminance | Seeed proprietary frames | ~$15 |
| **Seeed MR60FDA1** | 60 GHz | Fall detection, presence | Seeed proprietary frames | ~$15 |
| **Seeed MR24HPC1** | 24 GHz | Presence, sleep stage, distance | Seeed proprietary frames | ~$10 |
| **HLK-LD2410** | 24 GHz | Presence, distance (motion + static) | HLK binary protocol | ~$3 |
| **HLK-LD2450** | 24 GHz | Multi-target tracking (x,y,speed) | HLK binary protocol | ~$5 |

### Fusion Algorithms

**1. Vital Sign Fusion (Kalman filter)**
```
mmWave HR (high confidence, 1 Hz) ─┐
                                    ├─► Kalman fuse → fused HR ± confidence
CSI-derived HR (lower confidence)  ─┘
```

**2. Fall Detection (dual-confirm)**
```
CSI phase accel > thresh ──────┐
                               ├─► AND gate → confirmed fall (near-zero false positives)
mmWave range-velocity pattern ─┘
```

**3. Presence Validation**
```
CSI adaptive threshold ────┐
                           ├─► Weighted vote → robust presence
mmWave target count > 0 ──┘
```

**4. Training Calibration**
```
mmWave ground-truth vitals → train CSI BVP extraction model
mmWave distance → calibrate CSI triangulation
mmWave micro-Doppler → label CSI activity patterns
```

### Vitals Packet Extension

Extend the existing 32-byte vitals packet (magic `0xC5110002`) with a new 48-byte fused packet:

```c
typedef struct __attribute__((packed)) {
    /* Existing 32-byte vitals fields */
    uint32_t magic;            /* 0xC5110004 (fused vitals) */
    uint8_t  node_id;
    uint8_t  flags;            /* Bit0=presence, Bit1=fall, Bit2=motion, Bit3=mmwave_present */
    uint16_t breathing_rate;   /* Fused BPM * 100 */
    uint32_t heartrate;        /* Fused BPM * 10000 */
    int8_t   rssi;
    uint8_t  n_persons;
    uint8_t  mmwave_type;      /* Sensor type enum */
    uint8_t  fusion_confidence;/* 0-100 fusion quality score */
    float    motion_energy;
    float    presence_score;
    uint32_t timestamp_ms;
    /* New mmWave fields (16 bytes) */
    float    mmwave_hr_bpm;    /* Raw mmWave heart rate */
    float    mmwave_br_bpm;    /* Raw mmWave breathing rate */
    float    mmwave_distance;  /* Distance to nearest target (cm) */
    uint8_t  mmwave_targets;   /* Target count */
    uint8_t  mmwave_confidence;/* mmWave signal quality 0-100 */
    uint16_t reserved;
} edge_fused_vitals_pkt_t;

_Static_assert(sizeof(edge_fused_vitals_pkt_t) == 48, "fused vitals must be 48 bytes");
```

### NVS Configuration

New provisioning parameters:

```bash
python provision.py --port COM7 \
  --mmwave-uart-tx 17 --mmwave-uart-rx 18 \  # UART pins for mmWave module
  --mmwave-type auto \                         # auto-detect, or: mr60bha2, ld2410, etc.
  --fusion-mode kalman \                       # kalman, vote, mmwave-primary, csi-primary
  --fall-dual-confirm true                     # require both CSI + mmWave for fall alert
```

### Implementation Phases

| Phase | Scope | Effort |
|-------|-------|--------|
| **Phase 1** | UART driver + MR60BHA2 parser + auto-detection | 2 weeks |
| **Phase 2** | Fused vitals packet + Kalman vital sign fusion | 1 week |
| **Phase 3** | Dual-confirm fall detection + presence voting | 1 week |
| **Phase 4** | HLK-LD2410/LD2450 support + multi-target fusion | 2 weeks |
| **Phase 5** | RuVector calibration pipeline (mmWave as ground truth) | 3 weeks |
| **Phase 6** | Server-side fusion for separate CSI + mmWave nodes | 2 weeks |

## Consequences

### Positive
- Near-zero false positive fall detection (dual-confirm)
- Clinical-grade vital signs when mmWave is present, with CSI as fallback
- Self-calibrating CSI pipeline using mmWave ground truth
- Backward compatible — existing CSI-only nodes work unchanged
- Low incremental cost (~$3-15 per mmWave module)
- Auto-detection means zero configuration for supported sensors
- RuVector attention/solver/temporal-tensor modules gain a high-quality reference signal

### Negative
- Added firmware complexity (~2-3 KB RAM for mmWave state + UART buffer)
- mmWave modules require line-of-sight (complementary to CSI, not replacement)
- Multiple UART protocols to maintain (Seeed, HLK families)
- 48-byte fused packet requires server parser update

### Neutral
- ESP32-C6 cannot run the full CSI pipeline (single-core RISC-V) but can serve as a dedicated mmWave bridge node
- mmWave modules add ~15 mA power draw per node
