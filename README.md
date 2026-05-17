# π RuView

<p align="center">
  <a href="https://x.com/rUv/status/2037556932802761004">
    <img src="assets/ruview-small-gemini.jpg" alt="RuView - WiFi DensePose" width="100%">
  </a>
</p>

> **Beta Software** — Under active development. APIs and firmware may change. Known limitations:
> - ESP32-C3 and original ESP32 are not supported (single-core, insufficient for CSI DSP)
> - Single ESP32 deployments have limited spatial resolution — use 2+ nodes or add a [Cognitum Seed](https://cognitum.one) for best results
> - Camera-free pose accuracy is limited (PCK@20 ≈ 2.5% with proxy labels) — [camera ground-truth training](docs/adr/ADR-079-camera-ground-truth-training.md) targets **35%+ PCK@20**; the pipeline is implemented, but the data-collection and evaluation phases (ADR-079 P7–P9) are still pending, so no measured camera-supervised PCK@20 has been published yet
>
> Contributions and bug reports welcome at [Issues](https://github.com/ruvnet/RuView/issues).

## **See through walls with WiFi** ##

**Turn ordinary WiFi into a spatial intelligence / sensing system.** Detect people, measure breathing and heart rate, track movement, and monitor rooms — through walls, in the dark, with no cameras or wearables. Just physics.

### π RuView is a WiFi sensing platform that turns radio signals into spatial intelligence.

Every WiFi router already fills your space with radio waves. When people move, breathe, or even sit still, they disturb those waves in measurable ways. RuView captures these disturbances using Channel State Information (CSI) from low-cost ESP32 sensors and turns them into actionable data: who's there, what they're doing, and whether they're okay.

**What it senses:**
- **Presence and occupancy** — detect people through walls, count them, track entries and exits
- **Vital signs** — breathing rate and heart rate, contactless, while sleeping or sitting
- **Activity recognition** — walking, sitting, gestures, falls — from temporal CSI patterns
- **Environment mapping** — RF fingerprinting identifies rooms, detects moved furniture, spots new objects
- **Sleep quality** — overnight monitoring with sleep stage classification and apnea screening

Built on [RuVector](https://github.com/ruvnet/ruvector/) and [Cognitum Seed](https://cognitum.one), RuView runs entirely on edge hardware — an ESP32 mesh (as low as $9 per node) paired with a Cognitum Seed for persistent memory, cryptographic attestation, and AI integration. No cloud, no cameras, no internet required.

The system learns each environment locally using spiking neural networks that adapt in under 30 seconds, with multi-frequency mesh scanning across 6 WiFi channels that uses your neighbors' routers as free radar illuminators. Every measurement is cryptographically attested via an Ed25519 witness chain.

RuView also supports pose estimation (17 COCO keypoints via the WiFlow architecture), trained entirely without cameras using 10 sensor signals — a technique pioneered from the original *DensePose From WiFi* research at Carnegie Mellon University.

### Built for low-power edge applications

[Edge modules](#edge-intelligence-adr-041) are small programs that run directly on the ESP32 sensor — no internet needed, no cloud fees, instant response.

[![Rust 1.85+](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests: 1463](https://img.shields.io/badge/tests-1463%20passed-brightgreen.svg)](https://github.com/ruvnet/RuView)
[![Docker: multi-arch](https://img.shields.io/badge/docker-amd64%20%2B%20arm64-blue.svg)](https://hub.docker.com/r/ruvnet/wifi-densepose)
[![Vital Signs](https://img.shields.io/badge/vital%20signs-breathing%20%2B%20heartbeat-red.svg)](#vital-sign-detection)
[![ESP32 Ready](https://img.shields.io/badge/ESP32--S3-CSI%20streaming-purple.svg)](#esp32-s3-hardware-pipeline)
[![crates.io](https://img.shields.io/crates/v/wifi-densepose-ruvector.svg)](https://crates.io/crates/wifi-densepose-ruvector)

 
> | What | How | Speed |
> |------|-----|-------|
> | 🦴 **Pose estimation** | CSI subcarrier amplitude/phase → 17 COCO keypoints | 171K emb/s (M4 Pro) |
> | 🫁 **Breathing detection** | Bandpass 0.1-0.5 Hz → zero-crossing BPM | 6-30 BPM |
> | 💓 **Heart rate** | Bandpass 0.8-2.0 Hz → zero-crossing BPM | 40-120 BPM |
> | 👤 **Presence sensing** | Trained model + PIR fusion — 100% accuracy | 0.012 ms latency |
> | 🧱 **Through-wall** | Fresnel zone geometry + multipath modeling | Up to 5m depth |
> | 🧠 **Edge intelligence** | 8-dim feature vectors + RVF store on Cognitum Seed | $140 total BOM |
> | 🎯 **Camera-free training** | 10 sensor signals, no labels needed | 84s on M4 Pro |
> | 📷 **Camera-supervised training** | MediaPipe + ESP32 CSI → **35%+ PCK@20 target** (ADR-079; eval phases pending) | ~19 min on laptop (pipeline) |
> | 📡 **Multi-frequency mesh** | Channel hopping across 6 bands, neighbor APs as illuminators | 3x sensing bandwidth |
> | 🌐 **3D point cloud** *(optional fusion)* | Camera depth (MiDaS) + WiFi CSI + mmWave radar → unified spatial model | 22 ms pipeline · 19K+ points/frame |

```bash
# Option 1: Docker (simulated data, no hardware needed)
docker pull ruvnet/wifi-densepose:latest
docker run -p 3000:3000 ruvnet/wifi-densepose:latest
# Open http://localhost:3000

# Option 2: Live sensing with ESP32-S3 hardware ($9)
# Flash firmware, provision WiFi, and start sensing:
python -m esptool --chip esp32s3 --port COM9 --baud 460800 \
  write_flash 0x0 bootloader.bin 0x8000 partition-table.bin \
  0xf000 ota_data_initial.bin 0x20000 esp32-csi-node.bin
python firmware/esp32-csi-node/provision.py --port COM9 \
  --ssid "YourWiFi" --password "secret" --target-ip 192.168.1.20

# Option 3: Full system with Cognitum Seed ($140)
# ESP32 streams CSI → bridge forwards to Seed for persistent storage + kNN + witness chain
node scripts/rf-scan.js --port 5006           # Live RF room scan
node scripts/snn-csi-processor.js --port 5006  # SNN real-time learning
node scripts/mincut-person-counter.js --port 5006  # Correct person counting
```

> [!NOTE]
> **CSI-capable hardware recommended.** Presence, vital signs, through-wall sensing, and all advanced capabilities require Channel State Information (CSI) from an ESP32-S3 ($9) or research NIC. The Docker image runs with simulated data for evaluation. Consumer WiFi laptops provide RSSI-only presence detection.

> **Hardware options** for live CSI capture:
>
> | Option | Hardware | Cost | Full CSI | Capabilities |
> |--------|----------|------|----------|-------------|
> | **ESP32 + Cognitum Seed** (recommended) | ESP32-S3 + [Cognitum Seed](https://cognitum.one) | ~$140 | Yes | Pose, breathing, heartbeat, motion, presence + persistent vector store, kNN search, witness chain, MCP proxy |
> | **ESP32 Mesh** | 3-6x ESP32-S3 + WiFi router | ~$54 | Yes | Pose, breathing, heartbeat, motion, presence |
> | **Research NIC** | Intel 5300 / Atheros AR9580 | ~$50-100 | Yes | Full CSI with 3x3 MIMO |
> | **Any WiFi** | Windows, macOS, or Linux laptop | $0 | No | RSSI-only: coarse presence and motion |
>
> No hardware? Verify the signal processing pipeline with the deterministic reference signal: `python archive/v1/data/proof/verify.py`
>
---


  <a href="https://ruvnet.github.io/RuView/">
    <img src="assets/v2-screen.png" alt="WiFi DensePose — Live pose detection with setup guide" width="800">
  </a>
  <br>
  <em>Real-time pose skeleton from WiFi CSI signals — no cameras, no wearables</em>
  <br><br>
  <a href="https://ruvnet.github.io/RuView/"><strong>▶ Live Observatory Demo</strong></a>
  &nbsp;|&nbsp;
  <a href="https://ruvnet.github.io/RuView/pose-fusion.html"><strong>▶ Dual-Modal Pose Fusion Demo</strong></a>
  &nbsp;|&nbsp;
  <a href="https://ruvnet.github.io/RuView/pointcloud/"><strong>▶ Live 3D Point Cloud</strong></a>

> The [server](#-quick-start) is optional for visualization and aggregation — the ESP32 [runs independently](#esp32-s3-hardware-pipeline) for presence detection, vital signs, and fall alerts.
>
> **Live ESP32 pipeline**: Connect an ESP32-S3 node → run the [sensing server](#sensing-server) → open the [pose fusion demo](https://ruvnet.github.io/RuView/pose-fusion.html) for real-time dual-modal pose estimation (webcam + WiFi CSI). See [ADR-059](docs/adr/ADR-059-live-esp32-csi-pipeline.md).


## 🔬 How It Works

WiFi routers flood every room with radio waves. When a person moves — or even breathes — those waves scatter differently. WiFi DensePose reads that scattering pattern and reconstructs what happened:

```
WiFi Router → radio waves pass through room → hit human body → scatter
    ↓
ESP32 mesh (4-6 nodes) captures CSI on channels 1/6/11 via TDM protocol
    ↓
Multi-Band Fusion: 3 channels × 56 subcarriers = 168 virtual subcarriers per link
    ↓
Multistatic Fusion: N×(N-1) links → attention-weighted cross-viewpoint embedding
    ↓
Coherence Gate: accept/reject measurements → stable for days without tuning
    ↓
Signal Processing: Hampel, SpotFi, Fresnel, BVP, spectrogram → clean features
    ↓
AI Backbone (RuVector): attention, graph algorithms, compression, field model
    ↓
Signal-Line Protocol (CRV): 6-stage gestalt → sensory → topology → coherence → search → model
    ↓
Neural Network: processed signals → 17 body keypoints + vital signs + room model
    ↓
Output: real-time pose, breathing, heart rate, room fingerprint, drift alerts
```

No training cameras required — the [Self-Learning system (ADR-024)](docs/adr/ADR-024-contrastive-csi-embedding-model.md) bootstraps from raw WiFi data alone. [MERIDIAN (ADR-027)](docs/adr/ADR-027-cross-environment-domain-generalization.md) ensures the model works in any room, not just the one it trained in.

---

## 🏢 Use Cases & Applications

WiFi sensing works anywhere WiFi exists. No new hardware in most cases — just software on existing access points or a $8 ESP32 add-on. Because there are no cameras, deployments avoid privacy regulations (GDPR video, HIPAA imaging) by design.

**Scaling:** Each AP distinguishes ~3-5 people (56 subcarriers). Multi-AP multiplies linearly — a 4-AP retail mesh covers ~15-20 occupants. No hard software limit; the practical ceiling is signal physics.

| | Why WiFi sensing wins | Traditional alternative |
|---|----------------------|----------------------|
| 🔒 | **No video, no GDPR/HIPAA imaging rules** | Cameras require consent, signage, data retention policies |
| 🧱 | **Works through walls, shelving, debris** | Cameras need line-of-sight per room |
| 🌙 | **Works in total darkness** | Cameras need IR or visible light |
| 💰 | **$0-$8 per zone** (existing WiFi or ESP32) | Camera systems: $200-$2,000 per zone |
| 🔌 | **WiFi already deployed everywhere** | PIR/radar sensors require new wiring per room |

<details>
<summary><strong>🏥 Everyday</strong> — Healthcare, retail, office, hospitality (commodity WiFi)</summary>

| Use Case | What It Does | Hardware | Key Metric | Edge Module |
|----------|-------------|----------|------------|-------------|
| **Elderly care / assisted living** | Fall detection, nighttime activity monitoring, breathing rate during sleep — no wearable compliance needed | 1 ESP32-S3 per room ($8) | Fall alert <2s | [Sleep Apnea](docs/edge-modules/medical.md), [Gait Analysis](docs/edge-modules/medical.md) |
| **Hospital patient monitoring** | Continuous breathing + heart rate for non-critical beds without wired sensors; nurse alert on anomaly | 1-2 APs per ward | Breathing: 6-30 BPM | [Respiratory Distress](docs/edge-modules/medical.md), [Cardiac Arrhythmia](docs/edge-modules/medical.md) |
| **Emergency room triage** | Automated occupancy count + wait-time estimation; detect patient distress (abnormal breathing) in waiting areas | Existing hospital WiFi | Occupancy accuracy >95% | [Queue Length](docs/edge-modules/retail.md), [Panic Motion](docs/edge-modules/security.md) |
| **Retail occupancy & flow** | Real-time foot traffic, dwell time by zone, queue length — no cameras, no opt-in, GDPR-friendly | Existing store WiFi + 1 ESP32 | Dwell resolution ~1m | [Customer Flow](docs/edge-modules/retail.md), [Dwell Heatmap](docs/edge-modules/retail.md) |
| **Office space utilization** | Which desks/rooms are actually occupied, meeting room no-shows, HVAC optimization based on real presence | Existing enterprise WiFi | Presence latency <1s | [Meeting Room](docs/edge-modules/building.md), [HVAC Presence](docs/edge-modules/building.md) |
| **Hotel & hospitality** | Room occupancy without door sensors, minibar/bathroom usage patterns, energy savings on empty rooms | Existing hotel WiFi | 15-30% HVAC savings | [Energy Audit](docs/edge-modules/building.md), [Lighting Zones](docs/edge-modules/building.md) |
| **Restaurants & food service** | Table turnover tracking, kitchen staff presence, restroom occupancy displays — no cameras in dining areas | Existing WiFi | Queue wait ±30s | [Table Turnover](docs/edge-modules/retail.md), [Queue Length](docs/edge-modules/retail.md) |
| **Parking garages** | Pedestrian presence in stairwells and elevators where cameras have blind spots; security alert if someone lingers | Existing WiFi | Through-concrete walls | [Loitering](docs/edge-modules/security.md), [Elevator Count](docs/edge-modules/building.md) |

</details>

<details>
<summary><strong>🏟️ Specialized</strong> — Events, fitness, education, civic (CSI-capable hardware)</summary>

| Use Case | What It Does | Hardware | Key Metric | Edge Module |
|----------|-------------|----------|------------|-------------|
| **Smart home automation** | Room-level presence triggers (lights, HVAC, music) that work through walls — no dead zones, no motion-sensor timeouts | 2-3 ESP32-S3 nodes ($24) | Through-wall range ~5m | [HVAC Presence](docs/edge-modules/building.md), [Lighting Zones](docs/edge-modules/building.md) |
| **Fitness & sports** | Rep counting, posture correction, breathing cadence during exercise — no wearable, no camera in locker rooms | 3+ ESP32-S3 mesh | Pose: 17 keypoints | [Breathing Sync](docs/edge-modules/exotic.md), [Gait Analysis](docs/edge-modules/medical.md) |
| **Childcare & schools** | Naptime breathing monitoring, playground headcount, restricted-area alerts — privacy-safe for minors | 2-4 ESP32-S3 per zone | Breathing: ±1 BPM | [Sleep Apnea](docs/edge-modules/medical.md), [Perimeter Breach](docs/edge-modules/security.md) |
| **Event venues & concerts** | Crowd density mapping, crush-risk detection via breathing compression, emergency evacuation flow tracking | Multi-AP mesh (4-8 APs) | Density per m² | [Customer Flow](docs/edge-modules/retail.md), [Panic Motion](docs/edge-modules/security.md) |
| **Stadiums & arenas** | Section-level occupancy for dynamic pricing, concession staffing, emergency egress flow modeling | Enterprise AP grid | 15-20 per AP mesh | [Dwell Heatmap](docs/edge-modules/retail.md), [Queue Length](docs/edge-modules/retail.md) |
| **Houses of worship** | Attendance counting without facial recognition — privacy-sensitive congregations, multi-room campus tracking | Existing WiFi | Zone-level accuracy | [Elevator Count](docs/edge-modules/building.md), [Energy Audit](docs/edge-modules/building.md) |
| **Warehouse & logistics** | Worker safety zones, forklift proximity alerts, occupancy in hazardous areas — works through shelving and pallets | Industrial AP mesh | Alert latency <500ms | [Forklift Proximity](docs/edge-modules/industrial.md), [Confined Space](docs/edge-modules/industrial.md) |
| **Civic infrastructure** | Public restroom occupancy (no cameras possible), subway platform crowding, shelter headcount during emergencies | Municipal WiFi + ESP32 | Real-time headcount | [Customer Flow](docs/edge-modules/retail.md), [Loitering](docs/edge-modules/security.md) |
| **Museums & galleries** | Visitor flow heatmaps, exhibit dwell time, crowd bottleneck alerts — no cameras near artwork (flash/theft risk) | Existing WiFi | Zone dwell ±5s | [Dwell Heatmap](docs/edge-modules/retail.md), [Shelf Engagement](docs/edge-modules/retail.md) |

</details>

<details>
<summary><strong>🤖 Robotics & Industrial</strong> — Autonomous systems, manufacturing, android spatial awareness</summary>

WiFi sensing gives robots and autonomous systems a spatial awareness layer that works where LIDAR and cameras fail — through dust, smoke, fog, and around corners. The CSI signal field acts as a "sixth sense" for detecting humans in the environment without requiring line-of-sight.

| Use Case | What It Does | Hardware | Key Metric | Edge Module |
|----------|-------------|----------|------------|-------------|
| **Cobot safety zones** | Detect human presence near collaborative robots — auto-slow or stop before contact, even behind obstructions | 2-3 ESP32-S3 per cell | Presence latency <100ms | [Forklift Proximity](docs/edge-modules/industrial.md), [Perimeter Breach](docs/edge-modules/security.md) |
| **Warehouse AMR navigation** | Autonomous mobile robots sense humans around blind corners, through shelving racks — no LIDAR occlusion | ESP32 mesh along aisles | Through-shelf detection | [Forklift Proximity](docs/edge-modules/industrial.md), [Loitering](docs/edge-modules/security.md) |
| **Android / humanoid spatial awareness** | Ambient human pose sensing for social robots — detect gestures, approach direction, and personal space without cameras always on | Onboard ESP32-S3 module | 17-keypoint pose | [Gesture Language](docs/edge-modules/exotic.md), [Emotion Detection](docs/edge-modules/exotic.md) |
| **Manufacturing line monitoring** | Worker presence at each station, ergonomic posture alerts, headcount for shift compliance — works through equipment | Industrial AP per zone | Pose + breathing | [Confined Space](docs/edge-modules/industrial.md), [Gait Analysis](docs/edge-modules/medical.md) |
| **Construction site safety** | Exclusion zone enforcement around heavy machinery, fall detection from scaffolding, personnel headcount | Ruggedized ESP32 mesh | Alert <2s, through-dust | [Panic Motion](docs/edge-modules/security.md), [Structural Vibration](docs/edge-modules/industrial.md) |
| **Agricultural robotics** | Detect farm workers near autonomous harvesters in dusty/foggy field conditions where cameras are unreliable | Weatherproof ESP32 nodes | Range ~10m open field | [Forklift Proximity](docs/edge-modules/industrial.md), [Rain Detection](docs/edge-modules/exotic.md) |
| **Drone landing zones** | Verify landing area is clear of humans — WiFi sensing works in rain, dust, and low light where downward cameras fail | Ground ESP32 nodes | Presence: >95% accuracy | [Perimeter Breach](docs/edge-modules/security.md), [Tailgating](docs/edge-modules/security.md) |
| **Clean room monitoring** | Personnel tracking without cameras (particle contamination risk from camera fans) — gown compliance via pose | Existing cleanroom WiFi | No particulate emission | [Clean Room](docs/edge-modules/industrial.md), [Livestock Monitor](docs/edge-modules/industrial.md) |

</details>

<details>
<summary><strong>🔥 Extreme</strong> — Through-wall, disaster, defense, underground</summary>

These scenarios exploit WiFi's ability to penetrate solid materials — concrete, rubble, earth — where no optical or infrared sensor can reach. The WiFi-Mat disaster module (ADR-001) is specifically designed for this tier.

| Use Case | What It Does | Hardware | Key Metric | Edge Module |
|----------|-------------|----------|------------|-------------|
| **Search & rescue (WiFi-Mat)** | Detect survivors through rubble/debris via breathing signature, START triage color classification, 3D localization | Portable ESP32 mesh + laptop | Through 30cm concrete | [Respiratory Distress](docs/edge-modules/medical.md), [Seizure Detection](docs/edge-modules/medical.md) |
| **Firefighting** | Locate occupants through smoke and walls before entry; breathing detection confirms life signs remotely | Portable mesh on truck | Works in zero visibility | [Sleep Apnea](docs/edge-modules/medical.md), [Panic Motion](docs/edge-modules/security.md) |
| **Prison & secure facilities** | Cell occupancy verification, distress detection (abnormal vitals), perimeter sensing — no camera blind spots | Dedicated AP infrastructure | 24/7 vital signs | [Cardiac Arrhythmia](docs/edge-modules/medical.md), [Loitering](docs/edge-modules/security.md) |
| **Military / tactical** | Through-wall personnel detection, room clearing confirmation, hostage vital signs at standoff distance | Directional WiFi + custom FW | Range: 5m through wall | [Perimeter Breach](docs/edge-modules/security.md), [Weapon Detection](docs/edge-modules/security.md) |
| **Border & perimeter security** | Detect human presence in tunnels, behind fences, in vehicles — passive sensing, no active illumination to reveal position | Concealed ESP32 mesh | Passive / covert | [Perimeter Breach](docs/edge-modules/security.md), [Tailgating](docs/edge-modules/security.md) |
| **Mining & underground** | Worker presence in tunnels where GPS/cameras fail, breathing detection after collapse, headcount at safety points | Ruggedized ESP32 mesh | Through rock/earth | [Confined Space](docs/edge-modules/industrial.md), [Respiratory Distress](docs/edge-modules/medical.md) |
| **Maritime & naval** | Below-deck personnel tracking through steel bulkheads (limited range, requires tuning), man-overboard detection | Ship WiFi + ESP32 | Through 1-2 bulkheads | [Structural Vibration](docs/edge-modules/industrial.md), [Panic Motion](docs/edge-modules/security.md) |
| **Wildlife research** | Non-invasive animal activity monitoring in enclosures or dens — no light pollution, no visual disturbance | Weatherproof ESP32 nodes | Zero light emission | [Livestock Monitor](docs/edge-modules/industrial.md), [Dream Stage](docs/edge-modules/exotic.md) |

</details>

<details>
<summary><strong>🧩 Edge Intelligence (<a href="docs/adr/ADR-041-wasm-module-collection.md">ADR-041</a>)</strong> — 60 WASM modules across 13 categories, all implemented (609 tests)</summary>

Small programs that run directly on the ESP32 sensor — no internet needed, no cloud fees, instant response. Each module is a tiny WASM file (5-30 KB) that you upload to the device over-the-air. It reads WiFi signal data and makes decisions locally in under 10 ms. [ADR-041](docs/adr/ADR-041-wasm-module-collection.md) defines 60 modules across 13 categories — all 60 are implemented with 609 tests passing.

| | Category | Examples |
|---|----------|---------|
| 🏥 | [**Medical & Health**](docs/edge-modules/medical.md) | Sleep apnea detection, cardiac arrhythmia, gait analysis, seizure detection |
| 🔐 | [**Security & Safety**](docs/edge-modules/security.md) | Intrusion detection, perimeter breach, loitering, panic motion |
| 🏢 | [**Smart Building**](docs/edge-modules/building.md) | Zone occupancy, HVAC control, elevator counting, meeting room tracking |
| 🛒 | [**Retail & Hospitality**](docs/edge-modules/retail.md) | Queue length, dwell heatmaps, customer flow, table turnover |
| 🏭 | [**Industrial**](docs/edge-modules/industrial.md) | Forklift proximity, confined space monitoring, structural vibration |
| 🔮 | [**Exotic & Research**](docs/edge-modules/exotic.md) | Sleep staging, emotion detection, sign language, breathing sync |
| 📡 | [**Signal Intelligence**](docs/edge-modules/signal-intelligence.md) | Cleans and sharpens raw WiFi signals — focuses on important regions, filters noise, fills in missing data, and tracks which person is which |
| 🧠 | [**Adaptive Learning**](docs/edge-modules/adaptive-learning.md) | The sensor learns new gestures and patterns on its own over time — no cloud needed, remembers what it learned even after updates |
| 🗺️ | [**Spatial Reasoning**](docs/edge-modules/spatial-temporal.md) | Figures out where people are in a room, which zones matter most, and tracks movement across areas using graph-based spatial logic |
| ⏱️ | [**Temporal Analysis**](docs/edge-modules/spatial-temporal.md) | Learns daily routines, detects when patterns break (someone didn't get up), and verifies safety rules are being followed over time |
| 🛡️ | [**AI Security**](docs/edge-modules/ai-security.md) | Detects signal replay attacks, WiFi jamming, injection attempts, and flags abnormal behavior that could indicate tampering |
| ⚛️ | [**Quantum-Inspired**](docs/edge-modules/autonomous.md) | Uses quantum-inspired math to map room-wide signal coherence and search for optimal sensor configurations |
| 🤖 | [**Autonomous & Exotic**](docs/edge-modules/autonomous.md) | Self-managing sensor mesh — auto-heals dropped nodes, plans its own actions, and explores experimental signal representations |

All implemented modules are `no_std` Rust, share a [common utility library](v2/crates/wifi-densepose-wasm-edge/src/vendor_common.rs), and talk to the host through a 12-function API. Full documentation: [**Edge Modules Guide**](docs/edge-modules/README.md). See the [complete implemented module list](#edge-module-list) below.

</details>

<details id="edge-module-list">
<summary><strong>🧩 Edge Intelligence — <a href="docs/edge-modules/README.md">All 65 Modules Implemented</a></strong> (ADR-041 complete)</summary>

All 60 modules are implemented, tested (609 tests passing), and ready to deploy. They compile to `wasm32-unknown-unknown`, run on ESP32-S3 via WASM3, and share a [common utility library](v2/crates/wifi-densepose-wasm-edge/src/vendor_common.rs). Source: [`crates/wifi-densepose-wasm-edge/src/`](v2/crates/wifi-densepose-wasm-edge/src/)

**Core modules** (ADR-040 flagship + early implementations):

| Module | File | What It Does |
|--------|------|-------------|
| Gesture Classifier | [`gesture.rs`](v2/crates/wifi-densepose-wasm-edge/src/gesture.rs) | DTW template matching for hand gestures |
| Coherence Filter | [`coherence.rs`](v2/crates/wifi-densepose-wasm-edge/src/coherence.rs) | Phase coherence gating for signal quality |
| Adversarial Detector | [`adversarial.rs`](v2/crates/wifi-densepose-wasm-edge/src/adversarial.rs) | Detects physically impossible signal patterns |
| Intrusion Detector | [`intrusion.rs`](v2/crates/wifi-densepose-wasm-edge/src/intrusion.rs) | Human vs non-human motion classification |
| Occupancy Counter | [`occupancy.rs`](v2/crates/wifi-densepose-wasm-edge/src/occupancy.rs) | Zone-level person counting |
| Vital Trend | [`vital_trend.rs`](v2/crates/wifi-densepose-wasm-edge/src/vital_trend.rs) | Long-term breathing and heart rate trending |
| RVF Parser | [`rvf.rs`](v2/crates/wifi-densepose-wasm-edge/src/rvf.rs) | RVF container format parsing |

**Vendor-integrated modules** (24 modules, ADR-041 Category 7):

**📡 Signal Intelligence** — Real-time CSI analysis and feature extraction

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Flash Attention | [`sig_flash_attention.rs`](v2/crates/wifi-densepose-wasm-edge/src/sig_flash_attention.rs) | Tiled attention over 8 subcarrier groups — finds spatial focus regions and entropy | S (<5ms) |
| Coherence Gate | [`sig_coherence_gate.rs`](v2/crates/wifi-densepose-wasm-edge/src/sig_coherence_gate.rs) | Z-score phasor gating with hysteresis: Accept / PredictOnly / Reject / Recalibrate | L (<2ms) |
| Temporal Compress | [`sig_temporal_compress.rs`](v2/crates/wifi-densepose-wasm-edge/src/sig_temporal_compress.rs) | 3-tier adaptive quantization (8-bit hot / 5-bit warm / 3-bit cold) | L (<2ms) |
| Sparse Recovery | [`sig_sparse_recovery.rs`](v2/crates/wifi-densepose-wasm-edge/src/sig_sparse_recovery.rs) | ISTA L1 reconstruction for dropped subcarriers | H (<10ms) |
| Person Match | [`sig_mincut_person_match.rs`](v2/crates/wifi-densepose-wasm-edge/src/sig_mincut_person_match.rs) | Hungarian-lite bipartite assignment for multi-person tracking | S (<5ms) |
| Optimal Transport | [`sig_optimal_transport.rs`](v2/crates/wifi-densepose-wasm-edge/src/sig_optimal_transport.rs) | Sliced Wasserstein-1 distance with 4 projections | L (<2ms) |

**🧠 Adaptive Learning** — On-device learning without cloud connectivity

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| DTW Gesture Learn | [`lrn_dtw_gesture_learn.rs`](v2/crates/wifi-densepose-wasm-edge/src/lrn_dtw_gesture_learn.rs) | User-teachable gesture recognition — 3-rehearsal protocol, 16 templates | S (<5ms) |
| Anomaly Attractor | [`lrn_anomaly_attractor.rs`](v2/crates/wifi-densepose-wasm-edge/src/lrn_anomaly_attractor.rs) | 4D dynamical system attractor classification with Lyapunov exponents | H (<10ms) |
| Meta Adapt | [`lrn_meta_adapt.rs`](v2/crates/wifi-densepose-wasm-edge/src/lrn_meta_adapt.rs) | Hill-climbing self-optimization with safety rollback | L (<2ms) |
| EWC Lifelong | [`lrn_ewc_lifelong.rs`](v2/crates/wifi-densepose-wasm-edge/src/lrn_ewc_lifelong.rs) | Elastic Weight Consolidation — remembers past tasks while learning new ones | S (<5ms) |

**🗺️ Spatial Reasoning** — Location, proximity, and influence mapping

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| PageRank Influence | [`spt_pagerank_influence.rs`](v2/crates/wifi-densepose-wasm-edge/src/spt_pagerank_influence.rs) | 4x4 cross-correlation graph with power iteration PageRank | L (<2ms) |
| Micro HNSW | [`spt_micro_hnsw.rs`](v2/crates/wifi-densepose-wasm-edge/src/spt_micro_hnsw.rs) | 64-vector navigable small-world graph for nearest-neighbor search | S (<5ms) |
| Spiking Tracker | [`spt_spiking_tracker.rs`](v2/crates/wifi-densepose-wasm-edge/src/spt_spiking_tracker.rs) | 32 LIF neurons + 4 output zone neurons with STDP learning | S (<5ms) |

**⏱️ Temporal Analysis** — Activity patterns, logic verification, autonomous planning

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Pattern Sequence | [`tmp_pattern_sequence.rs`](v2/crates/wifi-densepose-wasm-edge/src/tmp_pattern_sequence.rs) | Activity routine detection and deviation alerts | S (<5ms) |
| Temporal Logic Guard | [`tmp_temporal_logic_guard.rs`](v2/crates/wifi-densepose-wasm-edge/src/tmp_temporal_logic_guard.rs) | LTL formula verification on CSI event streams | S (<5ms) |
| GOAP Autonomy | [`tmp_goap_autonomy.rs`](v2/crates/wifi-densepose-wasm-edge/src/tmp_goap_autonomy.rs) | Goal-Oriented Action Planning for autonomous module management | S (<5ms) |

**🛡️ AI Security** — Tamper detection and behavioral anomaly profiling

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Prompt Shield | [`ais_prompt_shield.rs`](v2/crates/wifi-densepose-wasm-edge/src/ais_prompt_shield.rs) | FNV-1a replay detection, injection detection (10x amplitude), jamming (SNR) | L (<2ms) |
| Behavioral Profiler | [`ais_behavioral_profiler.rs`](v2/crates/wifi-densepose-wasm-edge/src/ais_behavioral_profiler.rs) | 6D behavioral profile with Mahalanobis anomaly scoring | S (<5ms) |

**⚛️ Quantum-Inspired** — Quantum computing metaphors applied to CSI analysis

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Quantum Coherence | [`qnt_quantum_coherence.rs`](v2/crates/wifi-densepose-wasm-edge/src/qnt_quantum_coherence.rs) | Bloch sphere mapping, Von Neumann entropy, decoherence detection | S (<5ms) |
| Interference Search | [`qnt_interference_search.rs`](v2/crates/wifi-densepose-wasm-edge/src/qnt_interference_search.rs) | 16 room-state hypotheses with Grover-inspired oracle + diffusion | S (<5ms) |

**🤖 Autonomous Systems** — Self-governing and self-healing behaviors

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Psycho-Symbolic | [`aut_psycho_symbolic.rs`](v2/crates/wifi-densepose-wasm-edge/src/aut_psycho_symbolic.rs) | 16-rule forward-chaining knowledge base with contradiction detection | S (<5ms) |
| Self-Healing Mesh | [`aut_self_healing_mesh.rs`](v2/crates/wifi-densepose-wasm-edge/src/aut_self_healing_mesh.rs) | 8-node mesh with health tracking, degradation/recovery, coverage healing | S (<5ms) |

**🔮 Exotic (Vendor)** — Novel mathematical models for CSI interpretation

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Time Crystal | [`exo_time_crystal.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_time_crystal.rs) | Autocorrelation subharmonic detection in 256-frame history | S (<5ms) |
| Hyperbolic Space | [`exo_hyperbolic_space.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_hyperbolic_space.rs) | Poincare ball embedding with 32 reference locations, hyperbolic distance | S (<5ms) |

**🏥 Medical & Health** (Category 1) — Contactless health monitoring

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Sleep Apnea | [`med_sleep_apnea.rs`](v2/crates/wifi-densepose-wasm-edge/src/med_sleep_apnea.rs) | Detects breathing pauses during sleep | S (<5ms) |
| Cardiac Arrhythmia | [`med_cardiac_arrhythmia.rs`](v2/crates/wifi-densepose-wasm-edge/src/med_cardiac_arrhythmia.rs) | Monitors heart rate for irregular rhythms | S (<5ms) |
| Respiratory Distress | [`med_respiratory_distress.rs`](v2/crates/wifi-densepose-wasm-edge/src/med_respiratory_distress.rs) | Alerts on abnormal breathing patterns | S (<5ms) |
| Gait Analysis | [`med_gait_analysis.rs`](v2/crates/wifi-densepose-wasm-edge/src/med_gait_analysis.rs) | Tracks walking patterns and detects changes | S (<5ms) |
| Seizure Detection | [`med_seizure_detect.rs`](v2/crates/wifi-densepose-wasm-edge/src/med_seizure_detect.rs) | 6-state machine for tonic-clonic seizure recognition | S (<5ms) |

**🔐 Security & Safety** (Category 2) — Perimeter and threat detection

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Perimeter Breach | [`sec_perimeter_breach.rs`](v2/crates/wifi-densepose-wasm-edge/src/sec_perimeter_breach.rs) | Detects boundary crossings with approach/departure | S (<5ms) |
| Weapon Detection | [`sec_weapon_detect.rs`](v2/crates/wifi-densepose-wasm-edge/src/sec_weapon_detect.rs) | Metal anomaly detection via CSI amplitude shifts | S (<5ms) |
| Tailgating | [`sec_tailgating.rs`](v2/crates/wifi-densepose-wasm-edge/src/sec_tailgating.rs) | Detects unauthorized follow-through at access points | S (<5ms) |
| Loitering | [`sec_loitering.rs`](v2/crates/wifi-densepose-wasm-edge/src/sec_loitering.rs) | Alerts when someone lingers too long in a zone | S (<5ms) |
| Panic Motion | [`sec_panic_motion.rs`](v2/crates/wifi-densepose-wasm-edge/src/sec_panic_motion.rs) | Detects fleeing, struggling, or panic movement | S (<5ms) |

**🏢 Smart Building** (Category 3) — Automation and energy efficiency

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| HVAC Presence | [`bld_hvac_presence.rs`](v2/crates/wifi-densepose-wasm-edge/src/bld_hvac_presence.rs) | Occupancy-driven HVAC control with departure countdown | S (<5ms) |
| Lighting Zones | [`bld_lighting_zones.rs`](v2/crates/wifi-densepose-wasm-edge/src/bld_lighting_zones.rs) | Auto-dim/off lighting based on zone activity | S (<5ms) |
| Elevator Count | [`bld_elevator_count.rs`](v2/crates/wifi-densepose-wasm-edge/src/bld_elevator_count.rs) | Counts people entering/leaving with overload warning | S (<5ms) |
| Meeting Room | [`bld_meeting_room.rs`](v2/crates/wifi-densepose-wasm-edge/src/bld_meeting_room.rs) | Tracks meeting lifecycle: start, headcount, end, availability | S (<5ms) |
| Energy Audit | [`bld_energy_audit.rs`](v2/crates/wifi-densepose-wasm-edge/src/bld_energy_audit.rs) | Tracks after-hours usage and room utilization rates | S (<5ms) |

**🛒 Retail & Hospitality** (Category 4) — Customer insights without cameras

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Queue Length | [`ret_queue_length.rs`](v2/crates/wifi-densepose-wasm-edge/src/ret_queue_length.rs) | Estimates queue size and wait times | S (<5ms) |
| Dwell Heatmap | [`ret_dwell_heatmap.rs`](v2/crates/wifi-densepose-wasm-edge/src/ret_dwell_heatmap.rs) | Shows where people spend time (hot/cold zones) | S (<5ms) |
| Customer Flow | [`ret_customer_flow.rs`](v2/crates/wifi-densepose-wasm-edge/src/ret_customer_flow.rs) | Counts ins/outs and tracks net occupancy | S (<5ms) |
| Table Turnover | [`ret_table_turnover.rs`](v2/crates/wifi-densepose-wasm-edge/src/ret_table_turnover.rs) | Restaurant table lifecycle: seated, dining, vacated | S (<5ms) |
| Shelf Engagement | [`ret_shelf_engagement.rs`](v2/crates/wifi-densepose-wasm-edge/src/ret_shelf_engagement.rs) | Detects browsing, considering, and reaching for products | S (<5ms) |

**🏭 Industrial & Specialized** (Category 5) — Safety and compliance

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Forklift Proximity | [`ind_forklift_proximity.rs`](v2/crates/wifi-densepose-wasm-edge/src/ind_forklift_proximity.rs) | Warns when people get too close to vehicles | S (<5ms) |
| Confined Space | [`ind_confined_space.rs`](v2/crates/wifi-densepose-wasm-edge/src/ind_confined_space.rs) | OSHA-compliant worker monitoring with extraction alerts | S (<5ms) |
| Clean Room | [`ind_clean_room.rs`](v2/crates/wifi-densepose-wasm-edge/src/ind_clean_room.rs) | Occupancy limits and turbulent motion detection | S (<5ms) |
| Livestock Monitor | [`ind_livestock_monitor.rs`](v2/crates/wifi-densepose-wasm-edge/src/ind_livestock_monitor.rs) | Animal presence, stillness, and escape alerts | S (<5ms) |
| Structural Vibration | [`ind_structural_vibration.rs`](v2/crates/wifi-densepose-wasm-edge/src/ind_structural_vibration.rs) | Seismic events, mechanical resonance, structural drift | S (<5ms) |

**🔮 Exotic & Research** (Category 6) — Experimental sensing applications

| Module | File | What It Does | Budget |
|--------|------|-------------|--------|
| Dream Stage | [`exo_dream_stage.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_dream_stage.rs) | Contactless sleep stage classification (wake/light/deep/REM) | S (<5ms) |
| Emotion Detection | [`exo_emotion_detect.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_emotion_detect.rs) | Arousal, stress, and calm detection from micro-movements | S (<5ms) |
| Gesture Language | [`exo_gesture_language.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_gesture_language.rs) | Sign language letter recognition via WiFi | S (<5ms) |
| Music Conductor | [`exo_music_conductor.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_music_conductor.rs) | Tempo and dynamic tracking from conducting gestures | S (<5ms) |
| Plant Growth | [`exo_plant_growth.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_plant_growth.rs) | Monitors plant growth, circadian rhythms, wilt detection | S (<5ms) |
| Ghost Hunter | [`exo_ghost_hunter.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_ghost_hunter.rs) | Environmental anomaly classification (draft/insect/wind/unknown) | S (<5ms) |
| Rain Detection | [`exo_rain_detect.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_rain_detect.rs) | Detects rain onset, intensity, and cessation via signal scatter | S (<5ms) |
| Breathing Sync | [`exo_breathing_sync.rs`](v2/crates/wifi-densepose-wasm-edge/src/exo_breathing_sync.rs) | Detects synchronized breathing between multiple people | S (<5ms) |

</details>

---

<details>
<summary><strong>🧠 Self-Learning WiFi AI (ADR-024)</strong> — Adaptive recognition, self-optimization, and intelligent anomaly detection</summary>

Every WiFi signal that passes through a room creates a unique fingerprint of that space. WiFi-DensePose already reads these fingerprints to track people, but until now it threw away the internal "understanding" after each reading. The Self-Learning WiFi AI captures and preserves that understanding as compact, reusable vectors — and continuously optimizes itself for each new environment.

**What it does in plain terms:**
- Turns any WiFi signal into a 128-number "fingerprint" that uniquely describes what's happening in a room
- Learns entirely on its own from raw WiFi data — no cameras, no labeling, no human supervision needed
- Recognizes rooms, detects intruders, identifies people, and classifies activities using only WiFi
- Runs on an $8 ESP32 chip (the entire model fits in 55 KB of memory)
- Produces both body pose tracking AND environment fingerprints in a single computation

**Key Capabilities**

| What | How it works | Why it matters |
|------|-------------|----------------|
| **Self-supervised learning** | The model watches WiFi signals and teaches itself what "similar" and "different" look like, without any human-labeled data | Deploy anywhere — just plug in a WiFi sensor and wait 10 minutes |
| **Room identification** | Each room produces a distinct WiFi fingerprint pattern | Know which room someone is in without GPS or beacons |
| **Anomaly detection** | An unexpected person or event creates a fingerprint that doesn't match anything seen before | Automatic intrusion and fall detection as a free byproduct |
| **Person re-identification** | Each person disturbs WiFi in a slightly different way, creating a personal signature | Track individuals across sessions without cameras |
| **Environment adaptation** | MicroLoRA adapters (1,792 parameters per room) fine-tune the model for each new space | Adapts to a new room with minimal data — 93% less than retraining from scratch |
| **Memory preservation** | EWC++ regularization remembers what was learned during pretraining | Switching to a new task doesn't erase prior knowledge |
| **Hard-negative mining** | Training focuses on the most confusing examples to learn faster | Better accuracy with the same amount of training data |

**Architecture**

```
WiFi Signal [56 channels] → Transformer + Graph Neural Network
                                  ├→ 128-dim environment fingerprint (for search + identification)
                                  └→ 17-joint body pose (for human tracking)
```

**Quick Start**

```bash
# Step 1: Learn from raw WiFi data (no labels needed)
cargo run -p wifi-densepose-sensing-server -- --pretrain --dataset data/csi/ --pretrain-epochs 50

# Step 2: Fine-tune with pose labels for full capability
cargo run -p wifi-densepose-sensing-server -- --train --dataset data/mmfi/ --epochs 100 --save-rvf model.rvf

# Step 3: Use the model — extract fingerprints from live WiFi
cargo run -p wifi-densepose-sensing-server -- --model model.rvf --embed

# Step 4: Search — find similar environments or detect anomalies
cargo run -p wifi-densepose-sensing-server -- --model model.rvf --build-index env
```

**Training Modes**

| Mode | What you need | What you get |
|------|--------------|-------------|
| Self-Supervised | Just raw WiFi data | A model that understands WiFi signal structure |
| Supervised | WiFi data + body pose labels | Full pose tracking + environment fingerprints |
| Cross-Modal | WiFi data + camera footage | Fingerprints aligned with visual understanding |

**Fingerprint Index Types**

| Index | What it stores | Real-world use |
|-------|---------------|----------------|
| `env_fingerprint` | Average room fingerprint | "Is this the kitchen or the bedroom?" |
| `activity_pattern` | Activity boundaries | "Is someone cooking, sleeping, or exercising?" |
| `temporal_baseline` | Normal conditions | "Something unusual just happened in this room" |
| `person_track` | Individual movement signatures | "Person A just entered the living room" |

**Model Size**

| Component | Parameters | Memory (on ESP32) |
|-----------|-----------|-------------------|
| Transformer backbone | ~28,000 | 28 KB |
| Embedding projection head | ~25,000 | 25 KB |
| Per-room MicroLoRA adapter | ~1,800 | 2 KB |
| **Total** | **~55,000** | **55 KB** (of 520 KB available) |

The self-learning system builds on the [AI Backbone (RuVector)](#ai-backbone-ruvector) signal-processing layer — attention, graph algorithms, and compression — adding contrastive learning on top.

See [`docs/adr/ADR-024-contrastive-csi-embedding-model.md`](docs/adr/ADR-024-contrastive-csi-embedding-model.md) for full architectural details.

</details>

---

## 🧩 Claude Code & Codex Plugin

RuView ships a [Claude Code](https://docs.anthropic.com/en/docs/claude-code) plugin (and Codex prompt mirror) that wraps the whole workflow — onboarding, ESP32 setup, configuration, sensing apps, model training, advanced multistatic sensing, CLI/API/WASM, mmWave radar, and witness verification — as 9 skills, 7 `/ruview-*` commands, and 3 agents. It lives in [`plugins/ruview/`](plugins/ruview/README.md); the marketplace manifest is [`.claude-plugin/marketplace.json`](.claude-plugin/marketplace.json) at the repo root.

```bash
# In Claude Code — add this repo as a plugin marketplace, then install:
/plugin marketplace add ruvnet/RuView
/plugin install ruview@ruview

# Or try it for one session without installing (from a local clone of the repo):
claude --plugin-dir ./plugins/ruview

# Then, in Claude Code:
#   /ruview-start      → onboarding (Docker demo / repo build / live ESP32)
#   /ruview-flash      → build + flash ESP32 firmware
#   /ruview-provision  → provision WiFi creds, sink IP, channel/MAC, mesh slots
#   /ruview-app        → run a sensing application (presence / vitals / pose / sleep / MAT / point cloud)
#   /ruview-train      → train / evaluate / publish a model (incl. GPU on GCloud)
#   /ruview-advanced   → multistatic / tomography / cross-viewpoint / mesh-security
#   /ruview-verify     → tests + deterministic proof + witness bundle
```

**Codex (OpenAI CLI):** `cp plugins/ruview/codex/prompts/*.md ~/.codex/prompts/` — the seven `/ruview-*` commands are mirrored as Codex prompts; [`plugins/ruview/codex/AGENTS.md`](plugins/ruview/codex/AGENTS.md) carries the project rules. See [`plugins/ruview/codex/README.md`](plugins/ruview/codex/README.md).

Verify the plugin structure: `bash plugins/ruview/scripts/smoke.sh`. Full details: [`plugins/ruview/README.md`](plugins/ruview/README.md).

---

## 📖 Documentation

| Document | Description |
|----------|-------------|
| [User Guide](docs/user-guide.md) | Step-by-step guide: installation, first run, API usage, hardware setup, training |
| [Build Guide](docs/build-guide.md) | Building from source (Rust and Python) |
| [Claude Code / Codex Plugin](plugins/ruview/README.md) | The `ruview` plugin + marketplace — skills, `/ruview-*` commands, agents, and the Codex prompt mirror |
| [Architecture Decisions](docs/adr/README.md) | 96 ADRs — why each technical choice was made, organized by domain (hardware, signal processing, ML, platform, infrastructure) |
| [Domain Models](docs/ddd/README.md) | 8 DDD models (RuvSense, Signal Processing, Training Pipeline, Hardware Platform, Sensing Server, WiFi-Mat, CHCI, rvCSI) — bounded contexts, aggregates, domain events, and ubiquitous language |
| [rvCSI — edge RF sensing runtime](https://github.com/ruvnet/rvcsi) | Rust-first / TypeScript-accessible / hardware-abstracted CSI runtime: multi-source ingestion (incl. real nexmon_csi `.pcap` from a **Raspberry Pi 5** / Pi 4 / Pi 3B+ — CYW43455 / BCM43455c0) → validation → DSP → typed events → RuVector RF memory ([ADR-095](docs/adr/ADR-095-rvcsi-edge-rf-sensing-platform.md), [ADR-096](docs/adr/ADR-096-rvcsi-ffi-crate-layout.md), [domain model](docs/ddd/rvcsi-domain-model.md)). Now its own repo — [`ruvnet/rvcsi`](https://github.com/ruvnet/rvcsi) — vendored here under `vendor/rvcsi`; 9 `rvcsi-*` crates on crates.io, `@ruv/rvcsi` on npm, plus a Claude Code plugin. |
| [Desktop App](v2/crates/wifi-densepose-desktop/README.md) | **WIP** — Tauri v2 desktop app for node management, OTA updates, WASM deployment, and mesh visualization |
| [Medical Examples](examples/medical/README.md) | Contactless blood pressure, heart rate, breathing rate via 60 GHz mmWave radar — $15 hardware, no wearable |
| [Extended Documentation](docs/readme-details.md) | Latest additions, key features, installation, quick start, signal processing, training, CLI, testing, deployment, and changelog |

---

## 📄 License

MIT License — see [LICENSE](LICENSE) for details.

## 📞 Support

[GitHub Issues](https://github.com/ruvnet/RuView/issues) | [Discussions](https://github.com/ruvnet/RuView/discussions) | [PyPI](https://pypi.org/project/wifi-densepose/)

---

**WiFi DensePose** — Privacy-preserving human pose estimation through WiFi signals.
