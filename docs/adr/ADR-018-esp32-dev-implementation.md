# ADR-018: ESP32 Development Implementation Path

## Status
Proposed

## Date
2026-02-28

## Context

ADR-012 established the ESP32 CSI Sensor Mesh architecture: hardware rationale, firmware file structure, `csi_feature_frame_t` C struct, aggregator design, clock-drift handling via feature-level fusion, and a $54 starter BOM. That ADR answers *what* to build and *why*.

This ADR answers *how* to build it — the concrete development sequence, the specific integration points in existing code, and how to test each layer before hardware is in hand.

### Current State

**Already implemented:**

| Component | Location | Status |
|-----------|----------|--------|
| Binary frame parser | `wifi-densepose-hardware/src/esp32_parser.rs` | Complete — `Esp32CsiParser::parse_frame()`, `parse_stream()`, 7 passing tests |
| Frame types | `wifi-densepose-hardware/src/csi_frame.rs` | Complete — `CsiFrame`, `CsiMetadata`, `SubcarrierData`, `to_amplitude_phase()` |
| Parse error types | `wifi-densepose-hardware/src/error.rs` | Complete — `ParseError` enum with 6 variants |
| Signal processing pipeline | `wifi-densepose-signal` crate | Complete — Hampel, Fresnel, BVP, Doppler, spectrogram |
| CSI extractor (Python) | `v1/src/hardware/csi_extractor.py` | Stub — `_read_raw_data()` raises `NotImplementedError` |
| Router interface (Python) | `v1/src/hardware/router_interface.py` | Stub — `_parse_csi_response()` raises `RouterConnectionError` |

**Not yet implemented:**

- ESP-IDF C firmware (`firmware/esp32-csi-node/`)
- UDP aggregator binary (`crates/wifi-densepose-hardware/src/aggregator/`)
- `CsiFrame` → `wifi_densepose_signal::CsiData` bridge
- Python `_read_raw_data()` real UDP socket implementation
- Proof capture tooling for real hardware

### Binary Frame Format (implemented in `esp32_parser.rs`)

```
Offset  Size  Field
0       4     Magic: 0xC5110001 (LE)
4       1     Node ID (0-255)
5       1     Number of antennas
6       2     Number of subcarriers (LE u16)
8       4     Frequency Hz (LE u32, e.g. 2412 for 2.4 GHz ch1)
12      4     Sequence number (LE u32)
16      1     RSSI (i8, dBm)
17      1     Noise floor (i8, dBm)
18      2     Reserved (zero)
20      N*2   I/Q pairs: (i8, i8) per subcarrier, repeated per antenna
```

Total frame size: 20 + (n_antennas × n_subcarriers × 2) bytes.

For 3 antennas, 56 subcarriers: 20 + 336 = 356 bytes per frame.

The firmware must write frames in this exact format. The parser already validates magic, bounds-checks `n_subcarriers` (≤512), and resyncs the stream on magic search for `parse_stream()`.

## Decision

We will implement the ESP32 development stack in four sequential layers, each independently testable before hardware is available.

### Layer 1 — ESP-IDF Firmware (`firmware/esp32-csi-node/`)

Implement the C firmware project per the file structure in ADR-012. Key design decisions deferred from ADR-012:

**CSI callback → frame serializer:**

```c
// main/csi_collector.c
static void csi_data_callback(void *ctx, wifi_csi_info_t *info) {
    if (!info || !info->buf) return;

    // Write binary frame header (20 bytes, little-endian)
    uint8_t frame[FRAME_MAX_BYTES];
    uint32_t magic = 0xC5110001;
    memcpy(frame + 0,  &magic,              4);
    frame[4] = g_node_id;
    frame[5] = info->rx_ctrl.ant;           // antenna index (1 for ESP32 single-antenna)
    uint16_t n_sub = info->len / 2;         // len = n_subcarriers * 2 (I + Q bytes)
    memcpy(frame + 6,  &n_sub,              2);
    uint32_t freq_mhz = g_channel_freq_mhz;
    memcpy(frame + 8,  &freq_mhz,           4);
    memcpy(frame + 12, &g_seq_num,          4);
    frame[16] = (int8_t)info->rx_ctrl.rssi;
    frame[17] = (int8_t)info->rx_ctrl.noise_floor;
    frame[18] = 0; frame[19] = 0;

    // Write I/Q payload directly from info->buf
    memcpy(frame + 20, info->buf, info->len);

    // Send over UDP to aggregator
    stream_sender_write(frame, 20 + info->len);
    g_seq_num++;
}
```

**No on-device FFT** (contradicting ADR-012's optional feature extraction path): The Rust aggregator will do feature extraction using the SOTA `wifi-densepose-signal` pipeline. Raw I/Q is cheaper to stream at ESP32 sampling rates (~100 Hz at 56 subcarriers = ~35 KB/s per node).

**`sdkconfig.defaults`** must enable:

```
CONFIG_ESP_WIFI_CSI_ENABLED=y
CONFIG_LWIP_SO_RCVBUF=y
CONFIG_FREERTOS_HZ=1000
```

**Build toolchain**: ESP-IDF v5.2+ (pinned). Docker image: `espressif/idf:v5.2` for reproducible CI.

### Layer 2 — UDP Aggregator (`crates/wifi-densepose-hardware/src/aggregator/`)

New module within the hardware crate. Entry point: `aggregator_main()` callable as a binary target.

```rust
// crates/wifi-densepose-hardware/src/aggregator/mod.rs

pub struct Esp32Aggregator {
    socket: UdpSocket,
    nodes: HashMap<u8, NodeState>,       // keyed by node_id from frame header
    tx: mpsc::SyncSender<CsiFrame>,      // outbound to bridge
}

struct NodeState {
    last_seq: u32,
    drop_count: u64,
    last_recv: Instant,
}

impl Esp32Aggregator {
    /// Bind UDP socket and start blocking receive loop.
    /// Each valid frame is forwarded on `tx`.
    pub fn run(&mut self) -> Result<(), AggregatorError> {
        let mut buf = vec![0u8; 4096];
        loop {
            let (n, _addr) = self.socket.recv_from(&mut buf)?;
            match Esp32CsiParser::parse_frame(&buf[..n]) {
                Ok((frame, _consumed)) => {
                    let state = self.nodes.entry(frame.metadata.node_id)
                        .or_insert_with(NodeState::default);
                    // Track drops via sequence number gaps
                    if frame.metadata.seq_num != state.last_seq + 1 {
                        state.drop_count += (frame.metadata.seq_num
                            .wrapping_sub(state.last_seq + 1)) as u64;
                    }
                    state.last_seq = frame.metadata.seq_num;
                    state.last_recv = Instant::now();
                    let _ = self.tx.try_send(frame); // drop if pipeline is full
                }
                Err(e) => {
                    // Log and continue — never crash on bad UDP packet
                    eprintln!("aggregator: parse error: {e}");
                }
            }
        }
    }
}
```

**Testable without hardware**: The test suite generates frames using `build_test_frame()` (same helper pattern as `esp32_parser.rs` tests) and sends them over a loopback UDP socket. The aggregator receives and forwards them identically to real hardware frames.

### Layer 3 — CsiFrame → CsiData Bridge

Bridge from `wifi-densepose-hardware::CsiFrame` to the signal processing type `wifi_densepose_signal::CsiData` (or a compatible intermediate type consumed by the Rust pipeline).

```rust
// crates/wifi-densepose-hardware/src/bridge.rs

use crate::{CsiFrame};

/// Intermediate type compatible with the signal processing pipeline.
/// Maps directly from CsiFrame without cloning the I/Q storage.
pub struct CsiData {
    pub timestamp_unix_ms: u64,
    pub node_id: u8,
    pub n_antennas: usize,
    pub n_subcarriers: usize,
    pub amplitude: Vec<f64>,   // length: n_antennas * n_subcarriers
    pub phase: Vec<f64>,       // length: n_antennas * n_subcarriers
    pub rssi_dbm: i8,
    pub noise_floor_dbm: i8,
    pub channel_freq_mhz: u32,
}

impl From<CsiFrame> for CsiData {
    fn from(frame: CsiFrame) -> Self {
        let n_ant = frame.metadata.n_antennas as usize;
        let n_sub = frame.metadata.n_subcarriers as usize;
        let (amplitude, phase) = frame.to_amplitude_phase();
        CsiData {
            timestamp_unix_ms: frame.metadata.timestamp_unix_ms,
            node_id: frame.metadata.node_id,
            n_antennas: n_ant,
            n_subcarriers: n_sub,
            amplitude,
            phase,
            rssi_dbm: frame.metadata.rssi_dbm,
            noise_floor_dbm: frame.metadata.noise_floor_dbm,
            channel_freq_mhz: frame.metadata.channel_freq_mhz,
        }
    }
}
```

The bridge test: parse a known binary frame, convert to `CsiData`, assert `amplitude[0]` = √(I₀² + Q₀²) to within f64 precision.

### Layer 4 — Python `_read_raw_data()` Real Implementation

Replace the `NotImplementedError` stub in `v1/src/hardware/csi_extractor.py` with a UDP socket reader. This allows the Python pipeline to receive real CSI from the aggregator while the Rust pipeline is being integrated.

```python
# v1/src/hardware/csi_extractor.py
# Replace _read_raw_data() stub:

import socket as _socket

class CSIExtractor:
    ...
    def _read_raw_data(self) -> bytes:
        """Read one raw CSI frame from the UDP aggregator.

        Expects binary frames in the ESP32 format (magic 0xC5110001 header).
        Aggregator address configured via AGGREGATOR_HOST / AGGREGATOR_PORT
        environment variables (defaults: 127.0.0.1:5005).
        """
        if not hasattr(self, '_udp_socket'):
            host = self.config.get('aggregator_host', '127.0.0.1')
            port = int(self.config.get('aggregator_port', 5005))
            sock = _socket.socket(_socket.AF_INET, _socket.SOCK_DGRAM)
            sock.bind((host, port))
            sock.settimeout(1.0)
            self._udp_socket = sock
        try:
            data, _ = self._udp_socket.recvfrom(4096)
            return data
        except _socket.timeout:
            raise CSIExtractionError(
                "No CSI data received within timeout — "
                "is the ESP32 aggregator running?"
            )
```

This is tested with a mock UDP server in the unit tests (existing `test_csi_extractor_tdd.py` pattern) and with the real aggregator in integration.

## Development Sequence

```
Phase 1 (Firmware + Aggregator — no pipeline integration needed):
  1. Write firmware/esp32-csi-node/ C project (ESP-IDF v5.2)
  2. Flash to one ESP32-S3-DevKitC board
  3. Verify binary frames arrive on laptop UDP socket using Wireshark
  4. Write aggregator crate + loopback test

Phase 2 (Bridge + Python stub):
  5. Implement CsiFrame → CsiData bridge
  6. Replace Python _read_raw_data() with UDP socket
  7. Run Python pipeline end-to-end against loopback aggregator (synthetic frames)

Phase 3 (Real hardware integration):
  8. Run Python pipeline against live ESP32 frames
  9. Capture 10-second real CSI bundle (firmware/esp32-csi-node/proof/)
  10. Verify proof bundle hash (ADR-011 pattern)
  11. Mark ADR-012 Accepted, mark this ADR Accepted
```

## Testing Without Hardware

All four layers are testable before a single ESP32 is purchased:

| Layer | Test Method |
|-------|-------------|
| Firmware binary format | Build a `build_test_frame()` helper in Rust, compare its output byte-for-byte against a hand-computed reference frame |
| Aggregator | Loopback UDP: test sends synthetic frames to 127.0.0.1:5005, aggregator receives and forwards on channel |
| Bridge | `assert_eq!(csi_data.amplitude[0], f64::sqrt((iq[0].i as f64).powi(2) + (iq[0].q as f64).powi(2)))` |
| Python UDP reader | Mock UDP server in pytest using `socket.socket` in a background thread |

The existing `esp32_parser.rs` test suite already validates parsing of correctly-formatted binary frames. The aggregator and bridge tests build on top of the same test frame construction.

## Consequences

### Positive
- **Layered testability**: Each layer can be validated independently before hardware acquisition.
- **No new external dependencies**: UDP sockets are in stdlib (both Rust and Python). Firmware uses only ESP-IDF and esp-dsp component.
- **Stub elimination**: Replaces the last two `NotImplementedError` stubs in the Python hardware layer with real code backed by real data.
- **Proof of reality**: Phase 3 produces a captured CSI bundle hashed to a known value, satisfying ADR-011 for hardware-sourced data.
- **Signal-crate reuse**: The SOTA Hampel/Fresnel/BVP/Doppler processing from ADR-014 applies unchanged to real ESP32 frames after the bridge converts them.

### Negative
- **Firmware requires ESP-IDF toolchain**: Not buildable without a 2+ GB ESP-IDF installation. CI must use the official Docker image or skip firmware compilation.
- **Raw I/Q bandwidth**: Streaming raw I/Q (not features) at 100 Hz × 3 antennas × 56 subcarriers = ~35 KB/s/node. At 6 nodes = ~210 KB/s. Fine for LAN; not suitable for WAN.
- **Single-antenna real-world**: Most ESP32-S3-DevKitC boards have one on-board antenna. Multi-antenna data requires external antenna + board with U.FL connector or purpose-built multi-radio setup.

### Deferred
- **Multi-node clock drift compensation**: ADR-012 specifies feature-level fusion. The aggregator in this ADR passes raw `CsiFrame` per-node. Drift compensation lives in a future `FeatureFuser` layer (not scoped here).
- **ESP-IDF firmware CI**: Firmware compilation in GitHub Actions requires the ESP-IDF Docker image. CI integration is deferred until Phase 3 hardware validation.

## Interaction with Other ADRs

| ADR | Interaction |
|-----|-------------|
| ADR-011 | Phase 3 produces a real CSI proof bundle satisfying mock elimination |
| ADR-012 | This ADR implements the development path for ADR-012's architecture |
| ADR-014 | SOTA signal processing applies unchanged after bridge layer |
| ADR-008 | Aggregator handles multi-node; distributed consensus is a later concern |

## References

- [Espressif ESP-CSI Repository](https://github.com/espressif/esp-csi)
- [ESP-IDF WiFi CSI API Reference](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/wifi.html#wi-fi-channel-state-information)
- `wifi-densepose-hardware/src/esp32_parser.rs` — binary frame parser implementation
- `wifi-densepose-hardware/src/csi_frame.rs` — `CsiFrame`, `to_amplitude_phase()`
- ADR-012: ESP32 CSI Sensor Mesh (architecture)
- ADR-011: Python Proof-of-Reality and Mock Elimination
- ADR-014: SOTA Signal Processing
