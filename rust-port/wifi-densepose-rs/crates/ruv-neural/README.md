# rUv Neural — Brain Topology Analysis System

> Quantum sensor integration x RuVector graph memory x Dynamic mincut coherence detection

[![crates.io](https://img.shields.io/crates/v/ruv-neural-core.svg)](https://crates.io/crates/ruv-neural-core)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)]()
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)]()
[![Tests](https://img.shields.io/badge/tests-338%20passed-brightgreen.svg)]()

---

## Ethics & Responsible Use

> **This technology interfaces with human neural data. Use it responsibly.**
>
> - **Informed consent** is required before collecting neural data from any participant
> - **Never** deploy brain-computer interfaces without IRB/ethics board approval
> - **Data privacy**: Neural signals are among the most sensitive personal data categories. Encrypt at rest, anonymize before sharing, and comply with GDPR/HIPAA as applicable
> - **Clinical use** requires FDA/CE clearance and must be supervised by licensed medical professionals
> - **Do not** use this software for covert monitoring, interrogation, lie detection, or any application that violates human autonomy
> - **Dual-use awareness**: The same technology that helps paralyzed patients communicate can be misused for surveillance. Design with safeguards
> - This software is provided for **research and educational purposes**. The authors accept no liability for misuse
>
> See [IEEE Neuroethics Framework](https://standards.ieee.org/industry-connections/ec/neuroethics/) and the [Morningside Group Neurorights](https://nri.ntc.columbia.edu/content/neurorights) initiative for guidance.

---

## Overview

**rUv Neural** is a modular Rust crate ecosystem for real-time brain network topology
analysis. It transforms neural magnetic field measurements from quantum sensors (NV diamond
magnetometers, optically pumped magnetometers) into dynamic connectivity graphs, then uses
minimum cut algorithms to detect cognitive state transitions.

This is not mind reading — it measures **how cognition organizes itself** by tracking the
topology of brain networks in real time.

## Hardware Parts List

Below is a reference bill of materials for building a basic multi-channel neural sensing rig.
Prices are approximate (2026). Links are for reference only — equivalent components from any
vendor will work.

### Core: NV Diamond Magnetometer Array

| Component | Qty | Approx Price | Link | Notes |
|-----------|-----|-------------|------|-------|
| NV Diamond Sensor Chip (2x2mm, 1ppm N) | 16 | $45 ea | [AliExpress: NV Diamond Chip](https://www.aliexpress.com/w/wholesale-nv-diamond-sensor.html) | Nitrogen-vacancy center, electronic grade |
| 532nm Green Laser Diode Module (100mW) | 4 | $12 ea | [AliExpress: 532nm Laser Module](https://www.aliexpress.com/w/wholesale-532nm-laser-module-100mw.html) | Excitation source for ODMR |
| Microwave Signal Generator (2.87 GHz) | 1 | $85 | [AliExpress: RF Signal Generator 3GHz](https://www.aliexpress.com/w/wholesale-rf-signal-generator-3ghz.html) | For NV zero-field splitting resonance |
| SMA Coaxial Cable (50 Ohm, 30cm) | 4 | $3 ea | [AliExpress: SMA Cable 50 Ohm](https://www.aliexpress.com/w/wholesale-sma-cable-50-ohm.html) | Microwave delivery to diamond chips |
| Photodiode Array (Si PIN, 16-ch) | 1 | $25 | [AliExpress: Photodiode Array](https://www.aliexpress.com/w/wholesale-photodiode-array-16-channel.html) | Fluorescence detection |
| Transimpedance Amplifier Board | 1 | $18 | [AliExpress: TIA Board](https://www.aliexpress.com/w/wholesale-transimpedance-amplifier-board.html) | Converts photocurrent to voltage |

### Alternative: OPM (Optically Pumped Magnetometer)

| Component | Qty | Approx Price | Link | Notes |
|-----------|-----|-------------|------|-------|
| Rb Vapor Cell (25mm, AR coated) | 8 | $35 ea | [AliExpress: Rubidium Vapor Cell](https://www.aliexpress.com/w/wholesale-rubidium-vapor-cell.html) | SERF-mode magnetometry |
| 795nm VCSEL Laser | 8 | $8 ea | [AliExpress: 795nm VCSEL](https://www.aliexpress.com/w/wholesale-795nm-vcsel-laser.html) | D1 line pump for Rb |
| Balanced Photodetector | 8 | $15 ea | [AliExpress: Balanced Photodetector](https://www.aliexpress.com/w/wholesale-balanced-photodetector.html) | Differential detection |
| Magnetic Shielding Mu-Metal Cylinder | 1 | $120 | [AliExpress: Mu-Metal Shield](https://www.aliexpress.com/w/wholesale-mu-metal-magnetic-shield.html) | 3-layer, >60dB attenuation |

### Alternative: EEG (Electroencephalography)

| Component | Qty | Approx Price | Link | Notes |
|-----------|-----|-------------|------|-------|
| Ag/AgCl EEG Electrodes (10-20 system) | 21 | $2 ea | [AliExpress: EEG Electrode AgCl](https://www.aliexpress.com/w/wholesale-eeg-electrode-ag-agcl.html) | Reusable cup electrodes |
| EEG Cap (10-20 placement, size M) | 1 | $45 | [AliExpress: EEG Cap 10-20](https://www.aliexpress.com/w/wholesale-eeg-cap-10-20.html) | Pre-wired 21-channel |
| Conductive EEG Gel (250ml) | 1 | $8 | [AliExpress: EEG Gel](https://www.aliexpress.com/w/wholesale-eeg-conductive-gel.html) | Low impedance contact |
| ADS1299 EEG AFE Board (8-ch) | 3 | $35 ea | [AliExpress: ADS1299 Board](https://www.aliexpress.com/w/wholesale-ads1299-eeg-board.html) | 24-bit, 250 SPS, TI analog front-end |

### Data Acquisition & Processing

| Component | Qty | Approx Price | Link | Notes |
|-----------|-----|-------------|------|-------|
| ESP32-S3 DevKit (16MB Flash, 8MB PSRAM) | 4 | $8 ea | [AliExpress: ESP32-S3 DevKit](https://www.aliexpress.com/w/wholesale-esp32-s3-devkit.html) | ADC readout + TDM sync |
| ADS1256 24-bit ADC Module | 2 | $12 ea | [AliExpress: ADS1256 Module](https://www.aliexpress.com/w/wholesale-ads1256-module.html) | High-resolution for NV/OPM |
| USB-C Hub (4 port, USB 3.0) | 1 | $10 | [AliExpress: USB-C Hub](https://www.aliexpress.com/w/wholesale-usb-c-hub-4-port.html) | Connect ESP32 nodes to host |
| Shielded USB Cable (30cm, ferrite) | 4 | $3 ea | [AliExpress: Shielded USB Cable](https://www.aliexpress.com/w/wholesale-shielded-usb-cable-ferrite.html) | Reduce EMI |
| Host PC or Raspberry Pi 5 (8GB) | 1 | $80 | [AliExpress: Raspberry Pi 5](https://www.aliexpress.com/w/wholesale-raspberry-pi-5-8gb.html) | Runs the rUv Neural pipeline |

### Assembly Tools

| Component | Qty | Approx Price | Link | Notes |
|-----------|-----|-------------|------|-------|
| Soldering Station (adjustable temp) | 1 | $25 | [AliExpress: Soldering Station](https://www.aliexpress.com/w/wholesale-soldering-station-adjustable.html) | For sensor board assembly |
| Breadboard + Jumper Wire Kit | 1 | $8 | [AliExpress: Breadboard Kit](https://www.aliexpress.com/w/wholesale-breadboard-jumper-wire-kit.html) | Prototyping |
| 3D Printed Sensor Mount (STL provided) | 1 | — | Print locally | Holds diamond chips in array |

**Estimated total cost:** ~$650–$900 for a 16-channel NV diamond setup, ~$500 for OPM, ~$200 for EEG.

### Assembly Instructions

1. **Sensor Array**
   - Mount NV diamond chips (or OPM vapor cells, or EEG electrodes) in the 3D-printed helmet/mount
   - For NV: align 532nm laser to each chip, position photodiodes for fluorescence collection
   - For OPM: install Rb cells inside mu-metal shield, align 795nm VCSELs
   - For EEG: apply conductive gel, place electrodes per 10-20 system

2. **Signal Chain**
   - Connect sensor outputs to ADS1256 (NV/OPM) or ADS1299 (EEG) ADC boards
   - Wire ADC SPI bus to ESP32-S3 GPIO (MOSI=11, MISO=13, SCK=12, CS=10)
   - Flash ESP32 with `ruv-neural-esp32` firmware: `cargo flash --chip esp32s3`

3. **TDM Synchronization**
   - Connect GPIO 4 across all ESP32 nodes as a shared sync line
   - The `TdmScheduler` assigns non-overlapping time slots automatically
   - Set `sync_tolerance_us: 1000` in the aggregator config

4. **Host Software**
   - Install Rust 1.75+ and build: `cargo build --workspace --release`
   - Run the pipeline: `cargo run -p ruv-neural-cli --release -- pipeline --channels 16 --duration 60`
   - Or use individual crates as a library (see [Use as Library](#use-as-library))

5. **Verification**
   - Generate a witness bundle: `cargo run -p ruv-neural-cli -- witness --output witness.json`
   - Verify Ed25519 signature: `cargo run -p ruv-neural-cli -- witness --verify witness.json`
   - Expected output: `VERDICT: PASS` (41 capability attestations, 338 tests)

## Architecture

```
                         rUv Neural Pipeline
    ================================================================

    +------------------+     +-------------------+     +------------------+
    |                  |     |                   |     |                  |
    |  SENSOR LAYER    |---->|  SIGNAL LAYER     |---->|  GRAPH LAYER     |
    |                  |     |                   |     |                  |
    |  NV Diamond      |     |  Bandpass Filter  |     |  PLV / Coherence |
    |  OPM             |     |  Artifact Reject  |     |  Brain Regions   |
    |  EEG             |     |  Hilbert Phase    |     |  Connectivity    |
    |  Simulated       |     |  Spectral (PSD)   |     |  Matrix          |
    |                  |     |                   |     |                  |
    +------------------+     +-------------------+     +--------+---------+
                                                                |
                                                                v
    +------------------+     +-------------------+     +------------------+
    |                  |     |                   |     |                  |
    |  DECODE LAYER    |<----|  MEMORY LAYER     |<----|  MINCUT LAYER    |
    |                  |     |                   |     |                  |
    |  Cognitive State |     |  HNSW Index       |     |  Stoer-Wagner    |
    |  Classification  |     |  Pattern Store    |     |  Normalized Cut  |
    |  BCI Output      |     |  Drift Detection  |     |  Spectral Cut    |
    |  Transition Log  |     |  Temporal Window  |     |  Coherence Detect|
    |                  |     |                   |     |                  |
    +------------------+     +-------------------+     +------------------+
                                      ^
                                      |
                              +-------+--------+
                              |                |
                              |  EMBED LAYER   |
                              |                |
                              |  Spectral Pos. |
                              |  Topology Vec  |
                              |  Node2Vec      |
                              |  RVF Export     |
                              |                |
                              +----------------+

    Peripheral Crates:
    +----------+   +----------+   +----------+
    | ESP32    |   | WASM     |   | VIZ      |
    | Edge     |   | Browser  |   | ASCII    |
    | Preproc  |   | Bindings |   | Render   |
    +----------+   +----------+   +----------+
```

## Crate Map

All crates are published on [crates.io](https://crates.io/search?q=ruv-neural):

| Crate | crates.io | Description | Dependencies |
|-------|-----------|-------------|--------------|
| [`ruv-neural-core`](https://crates.io/crates/ruv-neural-core) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-core.svg)](https://crates.io/crates/ruv-neural-core) | Core types, traits, errors, RVF format | None |
| [`ruv-neural-sensor`](https://crates.io/crates/ruv-neural-sensor) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-sensor.svg)](https://crates.io/crates/ruv-neural-sensor) | NV diamond, OPM, EEG sensor interfaces | core |
| [`ruv-neural-signal`](https://crates.io/crates/ruv-neural-signal) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-signal.svg)](https://crates.io/crates/ruv-neural-signal) | DSP: filtering, spectral, connectivity | core |
| [`ruv-neural-graph`](https://crates.io/crates/ruv-neural-graph) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-graph.svg)](https://crates.io/crates/ruv-neural-graph) | Brain connectivity graph construction | core, signal |
| [`ruv-neural-mincut`](https://crates.io/crates/ruv-neural-mincut) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-mincut.svg)](https://crates.io/crates/ruv-neural-mincut) | Dynamic minimum cut topology analysis | core |
| [`ruv-neural-embed`](https://crates.io/crates/ruv-neural-embed) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-embed.svg)](https://crates.io/crates/ruv-neural-embed) | RuVector graph embeddings | core |
| [`ruv-neural-memory`](https://crates.io/crates/ruv-neural-memory) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-memory.svg)](https://crates.io/crates/ruv-neural-memory) | Persistent neural state memory + HNSW | core |
| [`ruv-neural-decoder`](https://crates.io/crates/ruv-neural-decoder) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-decoder.svg)](https://crates.io/crates/ruv-neural-decoder) | Cognitive state classification + BCI | core |
| [`ruv-neural-esp32`](https://crates.io/crates/ruv-neural-esp32) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-esp32.svg)](https://crates.io/crates/ruv-neural-esp32) | ESP32 edge sensor integration | core |
| `ruv-neural-wasm` | — | WebAssembly browser bindings | core |
| [`ruv-neural-viz`](https://crates.io/crates/ruv-neural-viz) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-viz.svg)](https://crates.io/crates/ruv-neural-viz) | Visualization and ASCII rendering | core, graph, mincut |
| [`ruv-neural-cli`](https://crates.io/crates/ruv-neural-cli) | [![crates.io](https://img.shields.io/crates/v/ruv-neural-cli.svg)](https://crates.io/crates/ruv-neural-cli) | CLI tool (`ruv-neural` binary) | all |

## Dependency Graph

```
                    ruv-neural-core
                    (types, traits, errors)
                   /    |    |    \     \
                  /     |    |     \     \
                 v      v    v      v     v
           sensor  signal  embed  esp32  (wasm)
                          |
                          v
                  graph --|------> viz
                  |
                  v
               mincut
                  |
                  v
         decoder <--- memory <--- embed
                  |
                  v
                 cli (depends on all)
```

## Quick Start

### Build

```bash
cd rust-port/wifi-densepose-rs/crates/ruv-neural
cargo build --workspace
cargo test --workspace
```

### Run CLI

```bash
cargo run -p ruv-neural-cli -- simulate --channels 64 --duration 10
cargo run -p ruv-neural-cli -- pipeline --channels 32 --duration 5 --dashboard
cargo run -p ruv-neural-cli -- mincut --input brain_graph.json
```

### Install from crates.io

```bash
# Add individual crates as needed
cargo add ruv-neural-core
cargo add ruv-neural-sensor
cargo add ruv-neural-signal
cargo add ruv-neural-mincut
cargo add ruv-neural-embed
cargo add ruv-neural-memory
cargo add ruv-neural-decoder
cargo add ruv-neural-graph
cargo add ruv-neural-viz
cargo add ruv-neural-esp32
cargo add ruv-neural-cli
```

### Use as Library

```rust
use ruv_neural_core::*;
use ruv_neural_sensor::simulator::SimulatedSensorArray;
use ruv_neural_signal::PreprocessingPipeline;
use ruv_neural_mincut::DynamicMincutTracker;
use ruv_neural_embed::NeuralEmbedding;

// Create simulated sensor array (64 channels, 1000 Hz)
let mut sensor = SimulatedSensorArray::new(64, 1000.0);
let data = sensor.acquire(1000)?;

// Preprocess: bandpass filter + artifact rejection
let pipeline = PreprocessingPipeline::default();
let clean = pipeline.process(&data)?;

// Compute connectivity and build graph
let connectivity = ruv_neural_signal::compute_all_pairs(
    &clean,
    ruv_neural_signal::ConnectivityMetric::PhaseLockingValue,
);

// Track topology changes via dynamic mincut
let mut tracker = DynamicMincutTracker::new();
let result = tracker.update(&graph)?;
println!(
    "Mincut: {:.3}, Partitions: {} | {}",
    result.cut_value,
    result.partition_a.len(),
    result.partition_b.len()
);

// Generate embedding for downstream classification
let embedding = NeuralEmbedding::new(
    result.to_feature_vector(),
    data.timestamp,
    "spectral",
)?;
println!("Embedding dim: {}", embedding.dimension);
```

## Mix and Match

Each crate is independently usable. Common combinations:

- **Sensor + Signal** -- Data acquisition and preprocessing only
- **Graph + Mincut** -- Graph analysis without sensor dependency
- **Embed + Memory** -- Embedding storage without real-time pipeline
- **Core + WASM** -- Browser-based graph visualization
- **ESP32 alone** -- Edge preprocessing on embedded hardware
- **Signal + Embed** -- Feature extraction pipeline without graph construction
- **Mincut + Viz** -- Topology analysis with ASCII dashboard output

## Platform Support

| Platform | Status | Crates Available |
|----------|--------|-----------------|
| Linux x86_64 | Full | All 12 |
| macOS ARM64 | Full | All 12 |
| Windows x86_64 | Full | All 12 |
| WASM (browser) | Partial | core, wasm, viz |
| ESP32 (no_std) | Partial | core, esp32 |

**Note:** The `ruv-neural-wasm` crate is excluded from the default workspace members.
Build it separately with:

```bash
cargo build -p ruv-neural-wasm --target wasm32-unknown-unknown --release
```

## Key Algorithms

### Signal Processing (`ruv-neural-signal`)

- **Butterworth IIR filters** in second-order sections (SOS) form
- **Welch PSD** estimation with configurable window and overlap
- **Hilbert transform** for instantaneous phase extraction
- **Artifact detection** -- eye blink, muscle, cardiac artifact rejection
- **Connectivity metrics** -- PLV, coherence, imaginary coherence, AEC

### Minimum Cut Analysis (`ruv-neural-mincut`)

- **Stoer-Wagner** -- Global minimum cut in O(V^3)
- **Normalized cut** (Shi-Malik) -- Spectral bisection via the Fiedler vector
- **Multiway cut** -- Recursive normalized cut for k-module detection
- **Spectral cut** -- Cheeger constant and spectral bisection bounds
- **Dynamic tracking** -- Temporal topology transition detection
- **Coherence events** -- Network formation, dissolution, merger, split

### Embeddings (`ruv-neural-embed`)

- **Spectral** -- Laplacian eigenvector positional encoding
- **Topology** -- Hand-crafted topological feature vectors
- **Node2Vec** -- Random-walk co-occurrence embeddings
- **Combined** -- Weighted concatenation of multiple methods
- **Temporal** -- Sliding-window context-enriched embeddings
- **RVF export** -- Serialization to RuVector `.rvf` format

## RVF Format

RuVector File (RVF) is a binary format for neural data interchange:

```
+--------+--------+---------+----------+----------+
| Magic  | Version| Type    | Payload  | Checksum |
| RVF\x01| u8     | u8      | [u8; N]  | u32      |
+--------+--------+---------+----------+----------+
```

- **Magic bytes**: `RVF\x01`
- **Supported types**: brain graphs, embeddings, topology metrics, time series
- **Binary format** for efficient storage and streaming
- **Compatible** with the broader RuVector ecosystem

## Cryptographic Witness Verification

rUv Neural includes an Ed25519-signed capability attestation system. Every build can
generate a witness bundle that cryptographically proves which capabilities are present
and that all tests passed.

```bash
# Generate a signed witness bundle
cargo run -p ruv-neural-cli -- witness --output witness-bundle.json

# Verify (any third party can do this)
cargo run -p ruv-neural-cli -- witness --verify witness-bundle.json
```

The bundle contains:
- **41 capability attestations** covering all 12 crates
- **SHA-256 digest** of the capability matrix
- **Ed25519 signature** (unique per generation)
- **Public key** for independent verification
- Test count and pass/fail status

Tampered bundles are detected — modifying any attestation invalidates the digest and
signature verification returns `FAIL`.

## Testing

```bash
# Run all workspace tests
cargo test --workspace

# Run a specific crate's tests
cargo test -p ruv-neural-mincut

# Run with logging enabled
RUST_LOG=debug cargo test --workspace -- --nocapture

# Run benchmarks (requires nightly or criterion)
cargo bench -p ruv-neural-mincut
```

## Crate Publishing Order

Crates must be published in dependency order:

1. `ruv-neural-core` (no internal deps)
2. `ruv-neural-sensor` (depends on core)
3. `ruv-neural-signal` (depends on core)
4. `ruv-neural-esp32` (depends on core)
5. `ruv-neural-graph` (depends on core, signal)
6. `ruv-neural-embed` (depends on core)
7. `ruv-neural-mincut` (depends on core)
8. `ruv-neural-viz` (depends on core, graph)
9. `ruv-neural-memory` (depends on core, embed)
10. `ruv-neural-decoder` (depends on core, embed)
11. `ruv-neural-wasm` (depends on core)
12. `ruv-neural-cli` (depends on all)

## License

MIT OR Apache-2.0
