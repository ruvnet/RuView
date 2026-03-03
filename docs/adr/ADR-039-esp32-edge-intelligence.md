# ADR-039: ESP32-S3 Edge Intelligence — On-Device Signal Processing and RuVector Integration

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-03-03 |
| **Depends on** | ADR-018 (binary frame format), ADR-014 (SOTA signal processing), ADR-021 (vital sign extraction), ADR-029 (multistatic sensing), ADR-030 (persistent field model), ADR-031 (RuView sensing-first RF) |
| **Supersedes** | None |

## Context

The current ESP32-S3 firmware (1,018 lines, 7 files) is a "dumb sensor" — it captures raw CSI frames and streams them unprocessed over UDP at ~20 Hz. All signal processing, feature extraction, presence detection, vital sign estimation, and pose inference happen server-side in the Rust crates.

This creates several limitations:
1. **Bandwidth waste** — raw CSI frames are 128-384 bytes each at 20 Hz = ~60 KB/s per node. Most of this is noise.
2. **Latency** — round-trip to server adds 5-50ms depending on network.
3. **Server dependency** — nodes are useless without an active aggregator.
4. **Scalability ceiling** — 6-node mesh at 20 Hz = 120 frames/s = server bottleneck.
5. **No local alerting** — fall detection, breathing anomaly, or intrusion must wait for server roundtrip.

The ESP32-S3 has significant untapped compute:
- **Dual-core Xtensa LX7** at 240 MHz
- **512 KB SRAM** + optional 8 MB PSRAM (our board has 8 MB flash)
- **Vector/DSP instructions** (PIE — Processor Instruction Extensions)
- **FPU** — hardware single-precision floating point
- **~80% idle CPU** — current firmware uses <20% (WiFi + CSI callback + UDP send)

## Decision

Implement a **3-tier edge intelligence pipeline** on the ESP32-S3 firmware, progressively offloading signal processing from the server to the device. Each tier is independently toggleable via NVS configuration.

### Tier 1: Smart Filtering & Compression (Firmware C)

Lightweight processing in the CSI callback path. Zero additional latency.

| Feature | Source ADR | Algorithm | Memory | CPU |
|---------|-----------|-----------|--------|-----|
| **Phase sanitization** | ADR-014 | Linear phase unwrap + conjugate multiply | 256 B | <1% |
| **Amplitude normalization** | ADR-014 | Per-subcarrier running mean/std (Welford) | 512 B | <1% |
| **Subcarrier selection** | ADR-016 (ruvector-mincut) | Top-K variance subcarriers | 128 B | <1% |
| **Static environment suppression** | ADR-030 | Exponential moving average subtraction | 512 B | <1% |
| **Adaptive frame decimation** | New | Skip frames when CSI variance < threshold | 8 B | <1% |
| **Delta compression** | New | XOR + RLE vs. previous frame | 512 B | <2% |

**Bandwidth reduction**: 60-80% (send only changed, high-variance subcarriers).

**ADR-018 v2 frame extension** (backward-compatible):

```
Existing 20-byte header unchanged.
New optional trailer (if magic bit set):
  [N*2]    Compressed I/Q (delta-coded, only selected subcarriers)
  [2]      Subcarrier bitmap (which of 64 subcarriers included)
  [1]      Frame flags: bit0=compressed, bit1=phase-sanitized, bit2=amplitude-normed
  [1]      Motion score (0-255)
  [1]      Presence confidence (0-255)
  [1]      Reserved
```

### Tier 2: On-Device Vital Signs & Presence (Firmware C + fixed-point DSP)

Runs as a FreeRTOS task on Core 1 (CSI collection on Core 0), processing a sliding window of CSI frames.

| Feature | Source ADR | Algorithm | Memory | CPU (Core 1) |
|---------|-----------|-----------|--------|--------------|
| **Presence detection** | ADR-029 | Variance threshold on amplitude envelope | 2 KB | 5% |
| **Motion scoring** | ADR-014 | Subcarrier correlation coefficient | 1 KB | 3% |
| **Breathing rate** | ADR-021 | Bandpass 0.1-0.5 Hz + peak detection on CSI phase | 8 KB | 10% |
| **Heart rate** | ADR-021 | Bandpass 0.8-2.0 Hz + autocorrelation on CSI phase | 8 KB | 15% |
| **Fall detection** | ADR-029 | Sudden variance spike + sustained stillness | 1 KB | 2% |
| **Room occupancy count** | ADR-037 | CSI rank estimation (eigenvalue spread) | 4 KB | 8% |
| **Coherence gate** | ADR-029 (ruvsense) | Z-score coherence, accept/reject/recalibrate | 1 KB | 2% |

**Total memory**: ~25 KB (fits in SRAM, no PSRAM needed).
**Total CPU**: ~45% of Core 1.

**Output**: Compact vital-signs UDP packet (32 bytes) at 1 Hz:

```
Offset  Size  Field
0       4     Magic: 0xC5110002 (vitals packet)
4       1     Node ID
5       1     Packet type (0x02 = vitals)
6       2     Sequence (LE u16)
8       1     Presence (0=empty, 1=present, 2=moving)
9       1     Motion score (0-255)
10      1     Occupancy estimate (0-8 persons)
11      1     Coherence gate (0=reject, 1=predict, 2=accept, 3=recalibrate)
12      2     Breathing rate (BPM * 100, LE u16) — 0 if not detected
14      2     Heart rate (BPM * 100, LE u16) — 0 if not detected
16      2     Breathing confidence (0-10000, LE u16)
18      2     Heart rate confidence (0-10000, LE u16)
20      1     Fall detected (0/1)
21      1     Anomaly flags (bitfield)
22      2     Ambient RSSI mean (LE i16)
24      4     CSI frame count since last report (LE u32)
28      4     Uptime seconds (LE u32)
```

### Tier 3: Lightweight Feature Extraction (Firmware C + optional PSRAM)

Pre-compute features that the server-side neural network needs, reducing server CPU by 60-80%.

| Feature | Source ADR | Algorithm | Memory | CPU |
|---------|-----------|-----------|--------|-----|
| **Phase difference matrix** | ADR-014 | Adjacent subcarrier phase diff | 4 KB | 5% |
| **Amplitude spectrogram** | ADR-014 | 64-bin FFT on 1s window per subcarrier | 32 KB | 15% |
| **Doppler-time map** | ADR-029 | 2D FFT across subcarriers × time | 16 KB | 10% |
| **Fresnel zone crossing** | ADR-014 | First Fresnel radius + fade count | 1 KB | 2% |
| **Cross-link correlation** | ADR-029 | Pearson correlation between TX-RX pairs | 2 KB | 5% |
| **Environment fingerprint** | ADR-027 (MERIDIAN) | PCA-compressed 16-dim CSI signature | 4 KB | 5% |
| **Gesture template match** | ADR-029 (ruvsense) | DTW on 8-dim feature vector | 8 KB | 10% |

**Total memory**: ~67 KB (SRAM) or up to 256 KB with PSRAM.
**Total CPU**: ~52% of Core 1.

**Output**: Feature vector UDP packet (variable size, ~200-500 bytes) at 4 Hz:

```
Offset  Size  Field
0       4     Magic: 0xC5110003 (feature packet)
4       1     Node ID
5       1     Packet type (0x03 = features)
6       2     Feature bitmap (which features included)
8       4     Timestamp ms (LE u32)
12      N     Feature payloads (concatenated, lengths determined by bitmap)
```

## NVS Configuration

All tiers controllable via NVS without reflashing:

| NVS Key | Type | Default | Description |
|---------|------|---------|-------------|
| `edge_tier` | u8 | 0 | 0=raw only, 1=smart filter, 2=+vitals, 3=+features |
| `decim_thresh` | u16 | 100 | Adaptive decimation variance threshold |
| `subk_count` | u8 | 32 | Top-K subcarriers to keep (Tier 1) |
| `vital_window` | u16 | 300 | Vital sign window frames (15s at 20 Hz) |
| `vital_interval` | u16 | 1000 | Vital report interval ms |
| `feature_hz` | u8 | 4 | Feature extraction rate |
| `fall_thresh` | u16 | 500 | Fall detection variance spike threshold |
| `presence_thresh` | u16 | 50 | Presence detection threshold |

Provisioning:
```bash
python firmware/esp32-csi-node/provision.py --port COM7 \
  --edge-tier 2 --vital-window 300 --presence-thresh 50
```

## Implementation Plan

### Phase 1: Infrastructure (1 week)

1. **Dual-core task architecture**
   - Core 0: WiFi + CSI callback (existing)
   - Core 1: Edge processing task (new FreeRTOS task)
   - Lock-free ring buffer between cores (producer-consumer)

2. **Ring buffer design**
   ```c
   #define RING_BUF_FRAMES 64  // ~3.2s at 20 Hz
   typedef struct {
       wifi_csi_info_t info;
       int8_t iq_data[384];    // Max I/Q payload
       uint32_t timestamp_ms;
       uint8_t tx_mac[6];
   } csi_ring_entry_t;
   ```

3. **NVS config extension** — add `edge_tier` and tier-specific params
4. **ADR-018 v2 header** — backward-compatible extension bit

### Phase 2: Tier 1 — Smart Filtering (1 week)

1. **Phase unwrap** — O(N) linear scan, in-place
2. **Welford running stats** — per-subcarrier mean/variance, O(1) update
3. **Top-K subcarrier selection** — partial sort, O(N) with selection algorithm
4. **Delta compression** — XOR vs previous frame, RLE encode
5. **Adaptive decimation** — skip frame if total variance < threshold

### Phase 3: Tier 2 — Vital Signs (2 weeks)

1. **Presence detector** — amplitude variance over 1s window
2. **Motion scorer** — correlation coefficient between consecutive frames
3. **Breathing extractor** — port from `wifi-densepose-vitals::BreathingExtractor::esp32_default()`
   - Bandpass via biquad IIR filter (0.1-0.5 Hz)
   - Peak detection with parabolic interpolation
   - Fixed-point arithmetic (Q15.16) for efficiency
4. **Heart rate extractor** — port from `wifi-densepose-vitals::HeartRateExtractor::esp32_default()`
   - Bandpass via biquad IIR (0.8-2.0 Hz)
   - Autocorrelation peak search
5. **Fall detection** — variance spike (>5σ) followed by sustained stillness (>3s)
6. **Coherence gate** — port from `ruvsense::coherence_gate` (Z-score threshold)

### Phase 4: Tier 3 — Feature Extraction (2 weeks)

1. **FFT engine** — fixed-point 64-point FFT (radix-2 DIT, no library needed)
2. **Amplitude spectrogram** — 1s sliding window FFT per subcarrier
3. **Doppler-time map** — 2D FFT across subcarrier × time dimensions
4. **Phase difference matrix** — adjacent subcarrier Δφ
5. **Environment fingerprint** — online PCA (incremental SVD, 16 components)
6. **Gesture DTW** — 8 stored templates, dynamic time warping on 8-dim feature

### Phase 5: CI/CD + Testing (1 week)

1. **GitHub Actions firmware build** — Docker `espressif/idf:v5.2` on every PR
2. **Host-side unit tests** — compile edge processing functions on x86 with mock CSI data
3. **Credential leak check** — binary string scan in CI
4. **Binary size tracking** — fail CI if firmware exceeds 90% of partition
5. **QEMU smoke test** — boot verification, NVS load, task creation

## ESP32-S3 Resource Budget

| Resource | Available | Tier 1 | Tier 2 | Tier 3 | Remaining |
|----------|-----------|--------|--------|--------|-----------|
| **SRAM** | 512 KB | 2 KB | 25 KB | 67 KB | 418 KB |
| **Core 0 CPU** | 100% | 5% | 0% | 0% | 75% (WiFi uses ~20%) |
| **Core 1 CPU** | 100% | 0% | 45% | 52% | 3% (Tier 2+3 exclusive) |
| **Flash** | 1 MB partition | 4 KB code | 12 KB code | 20 KB code | 964 KB |

Note: Tier 2 and Tier 3 run on Core 1 but are time-multiplexed — vitals at 1 Hz, features at 4 Hz. Combined peak load is ~60% of Core 1.

## Mapping to Existing ADRs

| Existing ADR | Capability | Edge Tier | Implementation |
|-------------|------------|-----------|----------------|
| **ADR-014** (SOTA signal) | Phase sanitization | 1 | Linear unwrap in CSI callback |
| **ADR-014** | Amplitude normalization | 1 | Welford running stats |
| **ADR-014** | Feature extraction | 3 | FFT spectrogram + phase diff matrix |
| **ADR-014** | Fresnel zone detection | 3 | Fade counting + first Fresnel radius |
| **ADR-016** (RuVector) | Subcarrier selection | 1 | Top-K variance (simplified mincut) |
| **ADR-021** (Vitals) | Breathing rate | 2 | Biquad IIR + peak detect |
| **ADR-021** | Heart rate | 2 | Biquad IIR + autocorrelation |
| **ADR-021** | Anomaly detection | 2 | Z-score on vital readings |
| **ADR-027** (MERIDIAN) | Environment fingerprint | 3 | Online PCA, 16-dim signature |
| **ADR-029** (RuvSense) | Coherence gate | 2 | Z-score coherence scoring |
| **ADR-029** | Multistatic correlation | 3 | Pearson cross-link correlation |
| **ADR-029** | Gesture recognition | 3 | DTW template matching |
| **ADR-030** (Field model) | Static suppression | 1 | EMA background subtraction |
| **ADR-031** (RuView) | Sensing-first NDP | Existing | Already in firmware (stub) |
| **ADR-037** (Multi-person) | Occupancy counting | 2 | CSI rank estimation |

## Server-Side Changes

The Rust aggregator (`wifi-densepose-hardware`) needs to handle the new packet types:

```rust
match magic {
    0xC5110001 => parse_raw_csi_frame(buf),     // Existing
    0xC5110002 => parse_vitals_packet(buf),      // New: Tier 2
    0xC5110003 => parse_feature_packet(buf),     // New: Tier 3
    _ => Err(ParseError::UnknownMagic(magic)),
}
```

When edge tier ≥ 1, the server can skip its own phase sanitization and amplitude normalization. When edge tier = 3, the server skips feature extraction entirely and feeds pre-computed features directly to the neural network.

## Testing Strategy

| Test Type | Tool | What |
|-----------|------|------|
| **Host unit tests** | gcc + Unity + mock CSI data | Phase unwrap, Welford stats, IIR filter, peak detect, DTW |
| **QEMU smoke test** | Docker QEMU | Boot, NVS load, task creation, ring buffer |
| **Hardware regression** | ESP32-S3 + serial log | Full pipeline: CSI → edge processing → UDP → server |
| **Accuracy validation** | Python reference impl | Compare edge vitals vs. server vitals on same CSI data |
| **Stress test** | 6-node mesh | Tier 3 at 20 Hz sustained, no frame drops |

## Alternatives Considered

1. **Rust on ESP32 (esp-rs)** — More type-safe, could share code with server crates. Rejected: larger binary, longer compile times, less mature ESP-IDF support for CSI APIs.

2. **MicroPython on ESP32** — Easier prototyping. Rejected: too slow for 20 Hz real-time processing, no fixed-point DSP.

3. **External co-processor (FPGA/DSP)** — Maximum throughput. Rejected: cost ($50+ per node), defeats the $8 ESP32 value proposition.

4. **Server-only processing** — Keep firmware dumb. Rejected: doesn't solve bandwidth, latency, or standalone operation requirements.

## Risks

| Risk | Mitigation |
|------|------------|
| Core 1 processing exceeds real-time budget | Adaptive quality: reduce feature_hz or fall back to lower tier |
| Fixed-point arithmetic introduces accuracy drift | Validate against Rust f64 reference on same CSI data; track error bounds |
| NVS config complexity overwhelms users | Sensible defaults; provision.py presets: `--preset home`, `--preset medical`, `--preset security` |
| ADR-018 v2 header breaks old aggregators | Backward-compatible: old magic = old format. New bit in flags field signals extension |
| Memory fragmentation from ring buffer | Static allocation only; no malloc in edge processing path |

## Success Criteria

- [ ] Tier 1 reduces bandwidth by ≥60% with <1 dB SNR loss
- [ ] Tier 2 breathing rate within ±1 BPM of server-side estimate
- [ ] Tier 2 heart rate within ±3 BPM of server-side estimate
- [ ] Tier 2 fall detection latency <500ms (vs. ~2s server roundtrip)
- [ ] Tier 2 presence detection accuracy ≥95%
- [ ] Tier 3 feature extraction matches server output within 5% RMSE
- [ ] All tiers: zero frame drops at 20 Hz sustained on single node
- [ ] Firmware binary stays under 90% of 1 MB app partition
- [ ] SRAM usage stays under 400 KB (leave headroom for WiFi stack)
- [ ] CI pipeline: build + host unit tests + binary size check on every PR
