# ADR-012: ESP32 CSI Sensor Mesh for Distributed Sensing

## Status
Proposed

## Date
2026-02-28

## Context

### The Hardware Reality Gap

WiFi-DensePose's Rust and Python pipelines implement real signal processing (FFT, phase unwrapping, Doppler extraction, correlation features), but the system currently has no defined path from **physical WiFi hardware → CSI bytes → pipeline input**. The `csi_extractor.py` and `router_interface.py` modules contain placeholder parsers that return `np.random.rand()` instead of real parsed data (see ADR-011).

To close this gap, we need a concrete, affordable, reproducible hardware platform that produces real CSI data and streams it into the existing pipeline.

### Why ESP32

| Factor | ESP32/ESP32-S3 | Intel 5300 (iwl5300) | Atheros AR9580 |
|--------|---------------|---------------------|----------------|
| Cost | ~$5-15/node | ~$50-100 (used NIC) | ~$30-60 (used NIC) |
| Availability | Mass produced, in stock | Discontinued, eBay only | Discontinued, eBay only |
| CSI Support | Official ESP-IDF API | Linux CSI Tool (kernel mod) | Atheros CSI Tool |
| Form Factor | Standalone MCU | Requires PCIe/Mini-PCIe host | Requires PCIe host |
| Deployment | Battery/USB, wireless | Desktop/laptop only | Desktop/laptop only |
| Antenna Config | 1-2 TX, 1-2 RX | 3 TX, 3 RX (MIMO) | 3 TX, 3 RX (MIMO) |
| Subcarriers | 52-56 (802.11n) | 30 (compressed) | 56 (full) |
| Fidelity | Lower (consumer SoC) | Higher (dedicated NIC) | Higher (dedicated NIC) |

**ESP32 wins on deployability**: It's the only option where a stranger can buy nodes on Amazon, flash firmware, and have a working CSI mesh in an afternoon. Intel 5300 and Atheros cards require specific hardware, kernel modifications, and legacy OS versions.

### ESP-IDF CSI API

Espressif provides official CSI support through three key functions:

```c
// 1. Configure what CSI data to capture
wifi_csi_config_t csi_config = {
    .lltf_en = true,         // Long Training Field (best for CSI)
    .htltf_en = true,        // HT-LTF
    .stbc_htltf2_en = true,  // STBC HT-LTF2
    .ltf_merge_en = true,    // Merge LTFs
    .channel_filter_en = false,
    .manu_scale = false,
};
esp_wifi_set_csi_config(&csi_config);

// 2. Register callback for received CSI data
esp_wifi_set_csi_rx_cb(csi_data_callback, NULL);

// 3. Enable CSI collection
esp_wifi_set_csi(true);

// Callback receives:
void csi_data_callback(void *ctx, wifi_csi_info_t *info) {
    // info->rx_ctrl: RSSI, noise_floor, channel, secondary_channel, etc.
    // info->buf: Raw CSI data (I/Q pairs per subcarrier)
    // info->len: Length of CSI data buffer
    // Typical: 112 bytes = 56 subcarriers × 2 (I,Q) × 1 byte each
}
```

## Decision

We will build an ESP32 CSI Sensor Mesh as the primary hardware integration path, with a full stack from firmware to aggregator to Rust pipeline to visualization.

### System Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                   ESP32 CSI Sensor Mesh                              │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                          │
│  │ ESP32    │  │ ESP32    │  │ ESP32    │  ... (3-6 nodes)         │
│  │ Node 1  │  │ Node 2  │  │ Node 3  │                          │
│  │          │  │          │  │          │                          │
│  │ CSI Rx   │  │ CSI Rx   │  │ CSI Rx   │  ← WiFi frames from    │
│  │ FFT      │  │ FFT      │  │ FFT      │     consumer router     │
│  │ Features │  │ Features │  │ Features │                          │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                          │
│       │              │              │                                │
│       │    UDP/TCP stream (WiFi or secondary channel)               │
│       │              │              │                                │
│       ▼              ▼              ▼                                │
│  ┌─────────────────────────────────────────┐                        │
│  │           Aggregator                     │                        │
│  │  (Laptop / Raspberry Pi / Seed device)  │                        │
│  │                                          │                        │
│  │  1. Receive CSI streams from all nodes  │                        │
│  │  2. Timestamp alignment (per-node)       │                        │
│  │  3. Feature-level fusion                │                        │
│  │  4. Feed into Rust/Python pipeline      │                        │
│  │  5. Serve WebSocket to visualization    │                        │
│  └──────────────────┬──────────────────────┘                        │
│                      │                                               │
│                      ▼                                               │
│  ┌─────────────────────────────────────────┐                        │
│  │        WiFi-DensePose Pipeline           │                        │
│  │                                          │                        │
│  │  CsiProcessor → FeatureExtractor →      │                        │
│  │  MotionDetector → PoseEstimator →       │                        │
│  │  Three.js Visualization                 │                        │
│  └─────────────────────────────────────────┘                        │
└─────────────────────────────────────────────────────────────────────┘
```

### Node Firmware Specification

**ESP-IDF project**: `firmware/esp32-csi-node/`

```
firmware/esp32-csi-node/
├── CMakeLists.txt
├── sdkconfig.defaults      # Menuconfig defaults with CSI enabled
├── main/
│   ├── CMakeLists.txt
│   ├── main.c              # Entry point, WiFi init, CSI callback
│   ├── csi_collector.c     # CSI data collection and buffering
│   ├── csi_collector.h
│   ├── feature_extract.c   # On-device FFT and feature extraction
│   ├── feature_extract.h
│   ├── stream_sender.c     # UDP stream to aggregator
│   ├── stream_sender.h
│   ├── config.h             # Node configuration (SSID, aggregator IP)
│   └── Kconfig.projbuild   # Menuconfig options
├── components/
│   └── esp_dsp/            # Espressif DSP library for FFT
└── README.md               # Flash instructions
```

**On-device processing** (reduces bandwidth, node does pre-processing):

```c
// feature_extract.c
typedef struct {
    uint32_t timestamp_ms;      // Local monotonic timestamp
    uint8_t  node_id;           // This node's ID
    int8_t   rssi;              // Received signal strength
    int8_t   noise_floor;       // Noise floor estimate
    uint8_t  channel;           // WiFi channel
    float    amplitude[56];     // |CSI| per subcarrier (from I/Q)
    float    phase[56];         // arg(CSI) per subcarrier
    float    doppler_energy;    // Motion energy from temporal FFT
    float    breathing_band;    // 0.1-0.5 Hz band power
    float    motion_band;       // 0.5-3 Hz band power
} csi_feature_frame_t;
// Size: ~470 bytes per frame
// At 100 Hz: ~47 KB/s per node, ~280 KB/s for 6 nodes
```

**Key firmware design decisions**:

1. **Feature extraction on-device**: Raw CSI I/Q → amplitude + phase + spectral bands. This cuts bandwidth from raw ~11 KB/frame to ~470 bytes/frame.

2. **Monotonic timestamps**: Each node uses its own monotonic clock. No NTP synchronization attempted between nodes - clock drift is handled at the aggregator by fusing features, not raw phases (see "Clock Drift" section below).

3. **UDP streaming**: Low-latency, loss-tolerant. Missing frames are acceptable; ordering is maintained via sequence numbers.

4. **Configurable sampling rate**: 10-100 Hz via menuconfig. 100 Hz for motion detection, 10 Hz sufficient for occupancy.

### Aggregator Specification

The aggregator runs on any machine with WiFi/Ethernet to the nodes:

```rust
// In wifi-densepose-rs, new module: crates/wifi-densepose-hardware/src/esp32/
pub struct Esp32Aggregator {
    /// UDP socket listening for node streams
    socket: UdpSocket,

    /// Per-node state (last timestamp, feature buffer, drift estimate)
    nodes: HashMap<u8, NodeState>,

    /// Ring buffer of fused feature frames
    fused_buffer: VecDeque<FusedFrame>,

    /// Channel to pipeline
    pipeline_tx: mpsc::Sender<CsiData>,
}

/// Fused frame from all nodes for one time window
pub struct FusedFrame {
    /// Timestamp (aggregator local, monotonic)
    timestamp: Instant,

    /// Per-node features (may have gaps if node dropped)
    node_features: Vec<Option<CsiFeatureFrame>>,

    /// Cross-node correlation (computed by aggregator)
    cross_node_correlation: Array2<f64>,

    /// Fused motion energy (max across nodes)
    fused_motion_energy: f64,

    /// Fused breathing band (coherent sum where phase aligns)
    fused_breathing_band: f64,
}
```

### Clock Drift Handling

ESP32 crystal oscillators drift ~20-50 ppm. Over 1 hour, two nodes may diverge by 72-180ms. This makes raw phase alignment across nodes impossible.

**Solution**: Feature-level fusion, not signal-level fusion.

```
Signal-level (WRONG for ESP32):
  Align raw I/Q samples across nodes → requires <1µs sync → impractical

Feature-level (CORRECT for ESP32):
  Each node: raw CSI → amplitude + phase + spectral features (local)
  Aggregator: collect features → correlate → fuse decisions
  No cross-node phase alignment needed
```

Specifically:
- **Motion energy**: Take max across nodes (any node seeing motion = motion)
- **Breathing band**: Use node with highest SNR as primary, others as corroboration
- **Location**: Cross-node amplitude ratios estimate position (no phase needed)

### Sensing Capabilities by Deployment

| Capability | 1 Node | 3 Nodes | 6 Nodes | Evidence |
|-----------|--------|---------|---------|----------|
| Presence detection | Good | Excellent | Excellent | Single-node RSSI variance |
| Coarse motion | Good | Excellent | Excellent | Doppler energy |
| Room-level location | None | Good | Excellent | Amplitude ratios |
| Respiration | Marginal | Good | Good | 0.1-0.5 Hz band, placement-sensitive |
| Heartbeat | Poor | Poor-Marginal | Marginal | Requires ideal placement, low noise |
| Multi-person count | None | Marginal | Good | Spatial diversity |
| Pose estimation | None | Poor | Marginal | Requires model + sufficient diversity |

**Honest assessment**: ESP32 CSI is lower fidelity than Intel 5300 or Atheros. Heartbeat detection is placement-sensitive and unreliable. Respiration works with good placement. Motion and presence are solid.

### Failure Modes and Mitigations

| Failure Mode | Severity | Mitigation |
|-------------|----------|------------|
| Multipath dominates in cluttered rooms | High | Mesh diversity: 3+ nodes from different angles |
| Person occludes path between node and router | Medium | Mesh: other nodes still have clear paths |
| Clock drift ruins cross-node fusion | Medium | Feature-level fusion only; no cross-node phase alignment |
| UDP packet loss during high traffic | Low | Sequence numbers, interpolation for gaps <100ms |
| ESP32 WiFi driver bugs with CSI | Medium | Pin ESP-IDF version, test on known-good boards |
| Node power failure | Low | Aggregator handles missing nodes gracefully |

### Bill of Materials (Starter Kit)

| Item | Quantity | Unit Cost | Total |
|------|----------|-----------|-------|
| ESP32-S3-DevKitC-1 | 3 | $10 | $30 |
| USB-A to USB-C cables | 3 | $3 | $9 |
| USB power adapter (multi-port) | 1 | $15 | $15 |
| Consumer WiFi router (any) | 1 | $0 (existing) | $0 |
| Aggregator (laptop or Pi 4) | 1 | $0 (existing) | $0 |
| **Total** | | | **$54** |

### Minimal Build Spec (Clone-Flash-Run)

```
# Step 1: Flash one node (requires ESP-IDF installed)
cd firmware/esp32-csi-node
idf.py set-target esp32s3
idf.py menuconfig  # Set WiFi SSID/password, aggregator IP
idf.py build flash monitor

# Step 2: Run aggregator (Docker)
docker compose -f docker-compose.esp32.yml up

# Step 3: Verify with proof bundle
# Aggregator captures 10 seconds, produces feature JSON, verifies hash
docker exec aggregator python verify_esp32.py

# Step 4: Open visualization
open http://localhost:3000  # Three.js dashboard
```

### Proof of Reality for ESP32

```
firmware/esp32-csi-node/proof/
├── captured_csi_10sec.bin          # Real 10-second CSI capture from ESP32
├── captured_csi_meta.json          # Board: ESP32-S3-DevKitC, ESP-IDF: 5.2, Router: TP-Link AX1800
├── expected_features.json          # Feature extraction output
├── expected_features.sha256        # Hash verification
└── capture_photo.jpg               # Photo of actual hardware setup
```

## Consequences

### Positive
- **$54 starter kit**: Lowest possible barrier to real CSI data
- **Mass available hardware**: ESP32 boards are in stock globally
- **Real data path**: Eliminates every `np.random.rand()` placeholder with actual hardware input
- **Proof artifact**: Captured CSI + expected hash proves the pipeline processes real data
- **Scalable mesh**: Add nodes for more coverage without changing software
- **Feature-level fusion**: Avoids the impossible problem of cross-node phase synchronization

### Negative
- **Lower fidelity than research NICs**: ESP32 CSI is noisier than Intel 5300
- **Heartbeat detection unreliable**: Micro-Doppler resolution insufficient for consistent heartbeat
- **ESP-IDF learning curve**: Firmware development requires embedded C knowledge
- **WiFi interference**: Nodes sharing the same channel as data traffic adds noise
- **Placement sensitivity**: Respiration detection requires careful node positioning

### Interaction with Other ADRs
- **ADR-011** (Proof of Reality): ESP32 provides the real CSI capture for the proof bundle
- **ADR-008** (Distributed Consensus): Mesh nodes can use simplified Raft for configuration distribution
- **ADR-003** (RVF Containers): Aggregator stores CSI features in RVF format
- **ADR-004** (HNSW): Environment fingerprints from ESP32 mesh feed HNSW index

## References

- [Espressif ESP-CSI Repository](https://github.com/espressif/esp-csi)
- [ESP-IDF WiFi CSI API](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi.html#wi-fi-channel-state-information)
- [ESP32 CSI Research Papers](https://ieeexplore.ieee.org/document/9439871)
- [Wi-Fi Sensing with ESP32: A Tutorial](https://arxiv.org/abs/2207.07859)
- ADR-011: Python Proof-of-Reality and Mock Elimination
