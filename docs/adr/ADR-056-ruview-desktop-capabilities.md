# ADR-056: RuView Desktop Complete Capabilities Reference

## Status
Accepted

## Context
RuView Desktop is a comprehensive WiFi-based sensing platform that combines hardware management, real-time signal processing, neural network inference, and intelligent monitoring. This ADR documents all integrated capabilities across the desktop application and underlying crates.

## Decision
The RuView Desktop application consolidates all WiFi-DensePose functionality into a single, unified interface with the following capabilities.

---

## 1. Hardware Management

### 1.1 Node Discovery
- **mDNS discovery**: Automatic detection of ESP32 nodes via Bonjour/Avahi
- **UDP probe**: Direct UDP broadcast discovery on port 5005
- **HTTP sweep**: Sequential IP scanning with health checks
- **Manual registration**: User-defined node configuration

### 1.2 Firmware Flashing
- **Serial flashing**: Direct USB flash via espflash integration
- **Chip detection**: Automatic ESP32/S2/S3/C3/C6 identification
- **Progress monitoring**: Real-time progress with speed metrics
- **Verification**: Post-flash integrity verification

### 1.3 OTA Updates
- **Single-node OTA**: HTTP-based firmware push to individual nodes
- **Batch OTA**: Coordinated multi-node updates with strategies:
  - `sequential`: One node at a time
  - `tdm_safe`: Respects TDM slot timing
  - `parallel`: Concurrent updates with throttling
- **Rollback support**: Automatic rollback on verification failure
- **Version tracking**: Pre/post version comparison

### 1.4 Node Configuration
- **NVS provisioning**: WiFi credentials, node ID, TDM slot assignment
- **Mesh configuration**: Coordinator/node/aggregator role assignment
- **TDM scheduling**: Time-division multiplexing slot allocation

---

## 2. Sensing Server

### 2.1 Data Sources
- **ESP32 CSI**: Real UDP frames from ESP32 hardware (port 5005)
- **Windows WiFi**: Native Windows RSSI monitoring via netsh
- **Simulation**: Synthetic data generation for demo/testing
- **Auto**: Automatic source detection based on available hardware

### 2.2 Real-Time Processing
- **CSI pipeline**: 56-subcarrier amplitude/phase extraction
- **FFT analysis**: Spectral decomposition for motion detection
- **Vital signs**: Breathing rate (0.1-0.5 Hz), heart rate (0.8-2.0 Hz)
- **Motion classification**: still/walking/running/exercising
- **Presence detection**: Binary presence with confidence score

### 2.3 WebSocket Streaming
- **Sensing endpoint**: `ws://localhost:8765/ws/sensing`
- **Pose endpoint**: `ws://localhost:8765/ws/pose`
- **Real-time broadcast**: 10-100 Hz update rate
- **Multi-client support**: Concurrent WebSocket connections

### 2.4 REST API
- **Health check**: `GET /health`
- **Status**: `GET /api/status`
- **Recording control**: `POST /api/recording/start|stop`
- **Model management**: `GET/POST /api/models`

---

## 3. Neural Network Inference

### 3.1 Model Formats
- **RVF (RuVector Format)**: Proprietary binary container with:
  - Model weights (quantized f32/f16/i8)
  - Vital sign configuration
  - SONA environment profiles
  - Training provenance
  - Cryptographic attestation

### 3.2 Inference Capabilities
- **Pose estimation**: 17 COCO keypoints from WiFi CSI
- **Activity recognition**: Multi-class classification
- **Vital signs**: Breathing and heart rate extraction
- **Multi-person detection**: Up to 3 simultaneous subjects

### 3.3 Self-Learning (SONA)
- **Environment adaptation**: LoRA-based fine-tuning to room geometry
- **Profile switching**: Multiple learned environment profiles
- **Online learning**: Continuous adaptation during runtime
- **Transfer learning**: Profile export/import between deployments

---

## 4. WASM Edge Modules

### 4.1 Module Management
- **Upload**: Deploy WASM modules to ESP32 nodes
- **Start/Stop**: Runtime control of edge processing
- **Status monitoring**: CPU, memory, execution count
- **Hot reload**: Update modules without node reboot

### 4.2 Supported Operations
- **Local filtering**: On-device noise reduction
- **Feature extraction**: Pre-compute features at edge
- **Compression**: Reduce data before transmission
- **Custom logic**: User-defined processing pipelines

---

## 5. Mesh Visualization

### 5.1 Network Topology
- **Live mesh view**: Real-time node connectivity graph
- **Signal quality**: RSSI/SNR visualization per link
- **Latency monitoring**: Round-trip time measurement
- **Packet loss**: Delivery success rate tracking

### 5.2 CSI Visualization
- **Amplitude heatmap**: Per-subcarrier amplitude display
- **Phase unwrapping**: Continuous phase visualization
- **Spectrogram**: Time-frequency representation
- **Signal field**: 3D voxel grid of RF perturbations

---

## 6. Training & Export

### 6.1 Dataset Management
- **Recording**: Capture CSI frames with annotations
- **Labeling**: Activity and pose ground truth
- **Augmentation**: Synthetic data generation
- **Export**: Standard formats (JSON, CSV, NumPy)

### 6.2 Training Pipeline (ADR-023)
- **Contrastive pretraining**: Self-supervised feature learning
- **Supervised fine-tuning**: Labeled pose estimation
- **SONA adaptation**: Environment-specific tuning
- **Validation**: Cross-environment testing

### 6.3 Export Formats
- **RVF container**: Production deployment format
- **ONNX**: Interoperability with external tools
- **PyTorch**: Research and experimentation
- **Candle**: Rust-native inference

---

## 7. Security Features

### 7.1 Network Security
- **OTA PSK**: Pre-shared key for firmware updates
- **Node authentication**: MAC-based node verification
- **Encrypted transport**: Optional TLS for API endpoints

### 7.2 Code Signing
- **Firmware verification**: Hash-based integrity checks
- **WASM attestation**: Module signature validation
- **Model provenance**: Training lineage tracking

---

## 8. Configuration & Settings

### 8.1 Server Configuration
- **Ports**: HTTP (8080), WebSocket (8765), UDP (5005)
- **Bind address**: Localhost or network-wide
- **Data source**: auto/wifi/esp32/simulate
- **Log level**: debug/info/warn/error

### 8.2 Application Settings
- **Theme**: Dark/light mode
- **Auto-discovery**: Periodic node scanning
- **Discovery interval**: Configurable scan frequency
- **UI customization**: Responsive layout options

---

## 9. Crate Architecture

| Crate | Capabilities |
|-------|-------------|
| `wifi-densepose-core` | CSI frame primitives, traits, error types |
| `wifi-densepose-signal` | FFT, phase unwrapping, vital signs, RuvSense |
| `wifi-densepose-nn` | ONNX/PyTorch/Candle inference backends |
| `wifi-densepose-train` | Training pipeline, dataset, metrics |
| `wifi-densepose-mat` | Mass casualty assessment tool |
| `wifi-densepose-hardware` | ESP32 protocol, TDM, channel hopping |
| `wifi-densepose-ruvector` | Cross-viewpoint fusion, attention |
| `wifi-densepose-api` | REST API (Axum) |
| `wifi-densepose-db` | Postgres/SQLite/Redis persistence |
| `wifi-densepose-config` | Configuration management |
| `wifi-densepose-wasm` | Browser WASM bindings |
| `wifi-densepose-cli` | Command-line interface |
| `wifi-densepose-sensing-server` | Real-time sensing server |
| `wifi-densepose-wifiscan` | Multi-BSSID scanning |
| `wifi-densepose-vitals` | Vital sign extraction |
| `wifi-densepose-desktop` | Tauri desktop application |

---

## 10. UI Design System (ADR-053)

### 10.1 Pages
- **Dashboard**: Overview, node status, quick actions
- **Discovery**: Network scanning interface
- **Nodes**: Node management and configuration
- **Flash**: Serial firmware flashing
- **OTA**: Over-the-air update management
- **Edge Modules**: WASM deployment
- **Sensing**: Real-time monitoring with server control
- **Mesh View**: Network topology visualization
- **Settings**: Application configuration

### 10.2 Components
- **StatusBadge**: Health indicator
- **NodeCard**: Node information display
- **LogViewer**: Real-time log streaming
- **ActivityFeed**: Sensing data visualization
- **ProgressBar**: Operation progress
- **ConfigForm**: Settings input

---

## Consequences

### Positive
- **Unified interface**: All capabilities in one application
- **Bundled deployment**: Single package with server included
- **Real-time feedback**: WebSocket-based live updates
- **Cross-platform**: macOS, Windows, Linux support
- **Extensible**: WASM modules, custom models, API access

### Negative
- **Larger bundle**: ~6MB app + ~2.6MB server
- **Complexity**: Many features require learning curve
- **Hardware dependency**: Full functionality requires ESP32 nodes

### Neutral
- Documentation required for all features
- Training materials needed for advanced capabilities
- Community contributions welcome

## Related ADRs
- ADR-053: UI Design System
- ADR-054: Desktop Full Implementation
- ADR-055: Integrated Sensing Server
- ADR-023: 8-Phase Training Pipeline
- ADR-016: RuVector Integration
