# π RuView

<p align="center">
  <a href="https://cognitum.one/seed">
    <img src="assets/ruview-hero-h3-v3.gif" alt="RuView - WiFi DensePose — animated visualization of real-time pose estimation, breathing, and heart-rate sensing through WiFi" width="100%">
  </a>
</p>


## **See through walls with WiFi** ##

**Turn ordinary WiFi into a spatial intelligence / sensing system.** Detect people, measure breathing and heart rate, track movement, and monitor rooms — through walls, in the dark, with no cameras or wearables. Just physics.

Works natively with the four major smart-home ecosystems: **[Home Assistant](docs/integrations/home-assistant.md)** via the HA-DISCO MQTT publisher, **[Apple Home & HomePod](docs/user-guide-apple-homepod.md)** as a discoverable HAP-1.1 bridge, **[Google Home](docs/integrations/home-assistant.md)** + **[Amazon Alexa](docs/integrations/home-assistant.md)** via the same HA bridge or a [Matter](docs/adr/ADR-122-bfld-ruview-ha-matter-exposure.md) endpoint. Siri, Google Assistant, and Alexa can voice presence and vitals by room with zero custom skills.

[![Works with Home Assistant](https://img.shields.io/badge/Works%20with-Home%20Assistant-blue?logo=home-assistant&logoColor=white&labelColor=41BDF5)](docs/integrations/home-assistant.md) [![Works with Matter](https://img.shields.io/badge/Works%20with-Matter-blue?labelColor=4285F4)](docs/adr/ADR-122-bfld-ruview-ha-matter-exposure.md) [![Works with Apple Home](https://img.shields.io/badge/Works%20with-Apple%20Home-black?logo=apple)](docs/user-guide-apple-homepod.md) [![Works with Google Home](https://img.shields.io/badge/Works%20with-Google%20Home-blue?logo=googlehome)](docs/integrations/home-assistant.md) [![Works with Alexa](https://img.shields.io/badge/Works%20with-Alexa-blue?logo=amazon&logoColor=white&labelColor=00CAFF)](docs/integrations/home-assistant.md)

> Drop into any **Home Assistant** install with one `--mqtt` flag. Or pair into **Apple Home / Google Home / Alexa / SmartThings** as a Matter Bridge. Ships 21 entities per node (11 raw signals + 10 inferred semantic states: someone-sleeping, possible-distress, room-active, elderly-inactivity-anomaly, meeting-in-progress, bathroom-occupied, fall-risk-elevated, bed-exit, no-movement, multi-room-transition) plus 3 starter HA Blueprints. See [`docs/integrations/home-assistant.md`](docs/integrations/home-assistant.md) · [ADR-115](docs/adr/ADR-115-home-assistant-integration.md).

### π RuView is a WiFi sensing platform that turns radio signals into spatial intelligence.

Every WiFi router already fills your space with radio waves. When people move, breathe, or even sit still, they disturb those waves in measurable ways. RuView captures these disturbances using Channel State Information (CSI) from low-cost ESP32 sensors and turns them into actionable data: who's there, what they're doing, and whether they're okay.

**What it senses:**
- **Presence and occupancy** — detect people through walls, count them, track entries and exits
- **Vital signs** — breathing rate and heart rate, contactless, while sleeping or sitting
- **Activity recognition** — walking, sitting, gestures, falls — from temporal CSI patterns
- **Environment mapping** — RF fingerprinting identifies rooms, detects moved furniture, spots new objects
- **Sleep quality** — overnight monitoring with sleep stage classification and apnea screening

**Also included:**

- **Camera-free pose** — estimate 17 body keypoints from WiFi CSI
- **Built-in model workflow** — record CSI, train models, load RVF files, and switch LoRA profiles
- **Local automation** — HOMECORE provides state, history, automations, signed Wasm plugins, voice hooks, and HomeKit support
- **Unified RF world model** — combine WiFi CSI, radar, UWB, and cellular sensing in one privacy-bounded scene model; accuracy is still synthetic until real-data validation
- **Governed evidence** — attach privacy policy, uncertainty, provenance, and witness records to sensing events
- **RuView MetaHarness** — use an AI operator to onboard, calibrate, train, verify, and check sensing claims

<details>
<summary><strong>RuView MetaHarness</strong> — guided operation for humans and AI agents</summary>

The RuView-specific metaharness we created is published as [`@ruvnet/ruview`](harness/ruview/README.md). It provides source-cited guidance, guarded Claude Code/Codex agents, deterministic verification, an honesty check for accuracy claims, and an explicitly granted OAuth-only Cognitum Spaces read.

```bash
# Check the local setup and get source-cited guidance
npx @ruvnet/ruview@0.4.0 doctor
npx @ruvnet/ruview@0.4.0 guidance --topic sensing --query "model loading"

# Run a read-only RuView agent through Codex
npx @ruvnet/ruview@0.4.0 agent run --host codex --repo . \
  --prompt "Find the nearest tests and cite the source files"

# Search or verify the reviewed contributor brain
npx @ruvnet/ruview@0.4.0 brain search --query "calibration"
npx @ruvnet/ruview@0.4.0 brain verify --repo .

# Check claims, replay the deterministic proof, or expose the MCP server
npx @ruvnet/ruview@0.4.0 claim-check --file REPORT.md
npx @ruvnet/ruview@0.4.0 verify
npx @ruvnet/ruview@0.4.0 spaces
npx @ruvnet/ruview@0.4.0 mcp start
```

Agent runs are read-only by default. Workspace writes require both `--allow-write` and `--confirm`; retrieved brain content is evidence, not authority.

</details>

Built on [RuVector](https://github.com/ruvnet/ruvector/) and [Cognitum Seed](https://cognitum.one), RuView runs entirely on edge hardware — an ESP32 mesh (as low as $9 per node) paired with a Cognitum Seed for persistent memory, cryptographic attestation, and AI integration. No cloud, no cameras, no internet required.

The system learns each environment locally using spiking neural networks that adapt in under 30 seconds, with multi-frequency mesh scanning across 6 WiFi channels that uses your neighbors' routers as free radar illuminators. Every measurement is cryptographically attested via an Ed25519 witness chain.

RuView turns ordinary WiFi into a contactless sensor. A $9 ESP32 board reads the radio reflections off the people in a room, and a small pretrained model — published on Hugging Face at [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained) — tells you who's there, how they're breathing, and how their heart rate is trending. The model fits in 8 KB (4-bit quantized) and runs in microseconds on a Raspberry Pi. (The [v2 encoder](https://huggingface.co/ruvnet/wifi-densepose-pretrained) reports an honest, label-free held-out **temporal-triplet accuracy of 82.3%** — up from 66.4% raw; the older "100% presence" figure was measured on a single-class recording and has been retracted in favor of this.) No cameras, no wearables, no app on the user's phone.

### Built for low-power edge applications

[Edge modules](#edge-intelligence-adr-041) are small programs that run directly on the ESP32 sensor — no internet needed, no cloud fees, instant response.

[![Rust 1.85+](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests: 1463](https://img.shields.io/badge/tests-1463%20passed-brightgreen.svg)](https://github.com/ruvnet/RuView)
[![Docker: multi-arch](https://img.shields.io/badge/docker-amd64%20%2B%20arm64-blue.svg)](https://hub.docker.com/r/ruvnet/wifi-densepose)
[![Vital Signs](https://img.shields.io/badge/vital%20signs-breathing%20%2B%20heartbeat-red.svg)](#vital-sign-detection)
[![ESP32 Ready](https://img.shields.io/badge/ESP32--S3-CSI%20streaming-purple.svg)](#esp32-s3-hardware-pipeline)
[![crates.io](https://img.shields.io/crates/v/wifi-densepose-ruvector.svg)](https://crates.io/crates/wifi-densepose-ruvector)
[![Downloads](https://img.shields.io/badge/downloads-10M%2B-brightgreen.svg)](#-edge-module-catalog)

 
> | What | How | Speed / scale |
> |------|-----|---------------|
> | 🫁 **Breathing rate** | Bandpass 0.1–0.5 Hz on wrapped phase, circular variance, zero-crossing BPM ([#593](https://github.com/ruvnet/RuView/issues/593)) | 6–30 BPM, real-time |
> | 💓 **Heart rate** | Bandpass 0.8–2.0 Hz, zero-crossing BPM | 40–120 BPM, real-time |
> | 👤 **Presence detection** | Trained head on Hugging Face ([`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained); v2 encoder = 82.3% held-out temporal-triplet acc, honestly re-benchmarked) + a phase-variance fallback that needs no model | < 1 ms, ~30 s ambient calibration |
> | 🧬 **CSI embeddings** | 128-dim contrastive encoder shipped on Hugging Face, 4-bit quantised variant fits in 8 KB | **164,183 emb/s** on M4 Pro |
> | 🦴 **17-keypoint pose estimation** | `cog-pose-estimation` Cog v0.0.1 — signed aarch64 + x86_64 binaries on GCS, loads `pose_v1.safetensors` via Candle (the committed `pose_v1` is a **first-cut** on-device model: PCK@20 = 3.0%, below the ADR-079 ≥35% target, and its runtime path is still a `confidence=0` stub — see [Model weights: what's real, what's not](#model-weights-whats-real-whats-not); the **82.69%** figure below is the separate published MM-Fi benchmark, not this live cog). Train your own from paired data in 2.1 s on an RTX 5080 ([ADR-101](docs/adr/ADR-101-pose-estimation-cog.md), [benchmarks](docs/benchmarks/pose-estimation-cog.md)). **SOTA on MM-Fi:** [`ruvnet/wifi-densepose-mmfi-pose`](https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose) hits **82.69% torso-PCK@20** (ensemble 83.59%), beating MultiFormer (72.25%) and CSI2Pose (68.41%) on the matched MM-Fi `random_split` protocol — self-corrected and auditable on [AetherArena](https://huggingface.co/spaces/ruvnet/aether-arena) | 8.4 ms cold-start on a Pi 5 |
> | 🚶 **Motion / activity** | Motion-band power + phase acceleration | Real-time |
> | 🤸 **Fall detection** | Phase-acceleration threshold + 3-frame debounce + 5 s cooldown ([#263](https://github.com/ruvnet/RuView/issues/263)) | < 200 ms |
> | 🧮 **Multi-person count** | Adaptive P95 normalisation + runtime-tunable dedup factor (`/api/v1/config/dedup-factor`, [#491](https://github.com/ruvnet/RuView/pull/491)). Six specialised learned counters available as Cogs: `occupancy-zones`, `elevator-count`, `queue-length`, `customer-flow`, `clean-room`, `person-matching` | Real-time, self-calibrating |
> | 🌍 **World model prediction** | OccWorld TransVQVAE — 15-frame future occupancy prediction, 209 ms inference, 3.4 GB VRAM on RTX 5080; fine-tune on your space with `occworld_retrain.py` ([ADR-147](docs/adr/ADR-147-nvidia-cosmos-world-foundation-model-integration.md)) | 15 frames × 200×200×16 vox |
> | 🧱 **Through-wall sensing** | Fresnel-zone geometry + multipath modeling | Up to ~5 m, signal-dependent |
> | 🧠 **Edge intelligence** | **105-cog catalog** ([ADR-102](docs/adr/ADR-102-edge-module-registry.md)) live from `app-registry.json` — health, security, building, retail, industrial, research, AI, swarm, signal, network, and developer modules. Optional Cognitum Seed adds persistent vector store + kNN + witness chain | $140 total BOM |
> | 🎯 **Camera-free pre-training** | Self-supervised contrastive encoder, 12.2M training steps on 60K frames, shipped on Hugging Face | 84 s/epoch retrain on M4 Pro |
> | 📷 **Camera-supervised fine-tune** | MediaPipe + ESP32 CSI paired training, end-to-end Candle pipeline on RTX 5080 ([ADR-079](docs/adr/ADR-079-camera-supervised-pose-finetune.md)) | 2.1 s for 400 epochs (~5 ms/epoch) |
> | 📡 **Multi-frequency mesh** | Channel hopping across 6 bands, TDM slot scheduling ([ADR-029](docs/adr/ADR-029-multifrequency-mesh.md)) | 3× sensing bandwidth |
> | 🌐 **3D point cloud fusion** | Camera depth (MiDaS) + WiFi CSI + mmWave radar → unified spatial model | 22 ms pipeline · 19K+ points/frame |
>
> Browse the full 105-module catalog (with practical descriptions, sizes, and difficulty) below in [🧩 Edge Module Catalog](#-edge-module-catalog), or visit [seed.cognitum.one/store](https://seed.cognitum.one/store).
>
> 🤗 **Pretrained weights**: download from [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained) — see [Loading the pretrained model](#loading-the-pretrained-model) below for one-command setup.

<details>
<summary><strong>Quick start options</strong> — Docker, ESP32-S3/C6, Cognitum Seed, and Python</summary>

```bash
# Option 1: Docker (simulated data, no hardware needed)
docker pull ruvnet/wifi-densepose:latest
docker run -p 3000:3000 ruvnet/wifi-densepose:latest
# Open http://localhost:3000

# Option 2a: Live sensing with ESP32-S3 hardware ($9)
# Flash firmware, provision WiFi, and start sensing:
python -m esptool --chip esp32s3 --port COM9 --baud 460800 \
  write_flash 0x0 bootloader.bin 0x8000 partition-table.bin \
  0xf000 ota_data_initial.bin 0x20000 esp32-csi-node.bin
python firmware/esp32-csi-node/provision.py --port COM9 \
  --ssid "YourWiFi" --password "secret" --target-ip 192.168.1.20

# Option 2b: WiFi 6 + 802.15.4 research sensing with ESP32-C6 ($6-10, ADR-110)
# Same csi-node firmware compiled for the C6 target — picks up the C6
# overlay (sdkconfig.defaults.esp32c6) automatically.
cd firmware/esp32-csi-node
idf.py set-target esp32c6 && idf.py build
idf.py -p COM6 flash
# C6 boot extras (vs S3): HE-LTF subcarrier tagging in ADR-018 bytes 18-19,
#   802.15.4 mesh time-sync on channel 15, TWT setup when the AP supports it,
#   opt-in LP-core wake-on-motion for ~5 µA battery seed nodes.
# v0.6.7 adds: real LP-core RISC-V motion-gate program (debounce + motion
#   counter) and a Wi-Fi 6 soft-AP with TWT Responder so two C6 boards can
#   benchmark real iTWT without buying an 11ax router. Both default off,
#   flip CONFIG_C6_{LP_CORE,SOFTAP_HE}_ENABLE to turn them on.

# Option 3: Full system with Cognitum Seed ($140)
# ESP32 streams CSI → bridge forwards to Seed for persistent storage + kNN + witness chain
node scripts/rf-scan.js --port 5006           # Live RF room scan
node scripts/snn-csi-processor.js --port 5006  # SNN real-time learning
node scripts/mincut-person-counter.js --port 5006  # Correct person counting

# Option 4: Python — live on PyPI (ADR-117)
pip install ruview                        # or: pip install wifi-densepose
# Both ship the same compiled PyO3 wheel (~250 KB, abi3-py310, Linux/macOS/Windows).
# Add [client] for the asyncio WebSocket + paho-mqtt clients:
pip install "ruview[client]"              # or: pip install "wifi-densepose[client]"

# from ruview import BreathingExtractor, HeartRateExtractor   # equivalent to:
# from wifi_densepose import BreathingExtractor, HeartRateExtractor
# from ruview.client import SensingClient, RuViewMqttClient
```

</details>

[![PyPI ruview](https://img.shields.io/pypi/v/ruview?label=ruview)](https://pypi.org/project/ruview/) [![PyPI wifi-densepose](https://img.shields.io/pypi/v/wifi-densepose?label=wifi-densepose)](https://pypi.org/project/wifi-densepose/)

> [!NOTE]
> **CSI-capable hardware recommended.** Presence, vital signs, through-wall sensing, and all advanced capabilities require Channel State Information (CSI) from an ESP32-S3 ($9) or research NIC. The Docker image runs with simulated data for evaluation. Consumer WiFi laptops provide RSSI-only presence detection.

> **Hardware options** for live CSI capture:
>
> | Option | Hardware | Cost | Full CSI | Capabilities |
> |--------|----------|------|----------|-------------|
> | **ESP32 + Cognitum Seed** (recommended) | ESP32-S3 + [Cognitum Seed](https://cognitum.one) | ~$140 | Yes | Presence, motion, breathing, heart rate, fall detection, multi-person counting, 17-keypoint pose (signed Cog binary — first-cut on-device model, see [Model weights: what's real, what's not](#model-weights-whats-real-whats-not)), 105-cog catalog, persistent vector store, kNN search, witness chain, MCP proxy |
> | **ESP32 Mesh** | 3-6× ESP32-S3 + WiFi router | ~$54 | Yes | Same capabilities as above without the persistent-memory features |
> | **ESP32-C6 research node** ([ADR-110](docs/adr/ADR-110-esp32-c6-firmware-extension.md), [witness](docs/WITNESS-LOG-110.md), [reviewer guide](docs/ADR-110-REVIEW-GUIDE.md), [firmware v0.7.0](https://github.com/ruvnet/RuView/releases/tag/v0.7.0-esp32)) | ESP32-C6-DevKit ($6–10) | ~$10 | Yes (Wi-Fi 6 capable) | Dual-target CSI with **99.56% measured ESP-NOW sync match** and measured HE-LTF capture on IDF 5.5.2. TWT and ~5 µA operation still need hardware validation. |
> | **Research NIC** | Intel 5300 / Atheros AR9580 | ~$50-100 | Yes | Full CSI with 3x3 MIMO |
> | **Qualcomm CSI beta** ([ADR-268](docs/adr/ADR-268-qualcomm-atheros-csi-platform.md)) | QCA9300 now; QCN9074/QCN9274 experimental | ~$30-200 | Simulator now; hardware adapter gated | Rust `QCS1` codec, deterministic replay, UDP/API integration; modern ath11k/ath12k profiles do not claim public CSI export |
> | **Vendor provider beta** ([ADR-270](docs/adr/ADR-270-vendor-rf-sensing-integration-program.md)) | Origin, Plume, Mist, NETGEAR, Electric Imp, RF Solutions, Luma, Nest, Linksys, Wifigarden | Varies | Capability-dependent | Bounded Rust adapters and deterministic fixtures; telemetry/network-only/unsupported states cannot masquerade as CSI |
> | **Any WiFi** | Windows, macOS, or Linux laptop | $0 | No | RSSI-only: coarse presence and motion (see [tutorial #36](https://github.com/ruvnet/RuView/issues/36)) |
>
> No hardware? Verify the signal processing pipeline with the deterministic reference signal: `python archive/v1/data/proof/verify.py`
>
---


  <a href="https://ruvnet.github.io/RuView/">
    <img src="assets/v2-screen.png" alt="WiFi DensePose — Live pose detection with setup guide" width="800">
  </a>
  <br>
  <em>Real-time pose skeleton from WiFi CSI signals — no cameras, no wearables (demo visualization; the live CSI-only single-ESP32 17-keypoint model is still first-cut — see <a href="#model-weights-whats-real-whats-not">Model weights: what's real, what's not</a>)</em>
  <br><br>
  <a href="https://ruvnet.github.io/RuView/"><strong>▶ Live Observatory Demo</strong></a>
  &nbsp;|&nbsp;
  <a href="https://ruvnet.github.io/RuView/pose-fusion.html"><strong>▶ Dual-Modal Pose Fusion Demo</strong></a>
  &nbsp;|&nbsp;
  <a href="https://ruvnet.github.io/RuView/pointcloud/"><strong>▶ Live 3D Point Cloud</strong></a>
  &nbsp;|&nbsp;
  <a href="https://ruvnet.github.io/RuView/three.js/"><strong>▶ three.js Demos (5)</strong></a>

> The [server](#-quick-start) is optional for visualization and aggregation — the ESP32 [runs independently](#esp32-s3-hardware-pipeline) for presence detection, vital signs, and fall alerts.
>
> **Live ESP32 pipeline**: Connect an ESP32-S3 node → run the [sensing server](#sensing-server) → open the [pose fusion demo](https://ruvnet.github.io/RuView/pose-fusion.html) for real-time dual-modal pose estimation (webcam + WiFi CSI). See [ADR-059](docs/adr/ADR-059-live-esp32-csi-pipeline.md). (The webcam supplies ground-truth pose in this dual-modal demo; the CSI-only on-device 17-keypoint model is still first-cut — see [Model weights: what's real, what's not](#model-weights-whats-real-whats-not).)
>
> **three.js scene gallery** at [`/three.js/`](https://ruvnet.github.io/RuView/three.js/) — five progressively richer ADR-097 demos: helpers, cinematic, GLTF skinned, FBX skinned, and a live MediaPipe→Mixamo retargeting feed driven by ESP32 CSI. Demos 04 and 05 require a local Mixamo `X Bot.fbx` (license boundary — not redistributed).


## 🤗 Pretrained model on Hugging Face

Pretrained CSI weights live at [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained) — 12.2M training steps on 60K frames / 610K contrastive triplets, **82.3% held-out temporal-triplet accuracy** (up from 66.4% raw; the older "100% presence" figure was measured on a single-class recording and has been retracted), 4-bit quantized variant fits in 8 KB. The release includes a contrastive **CSI encoder** producing 128-dim embeddings (164,183 emb/s on M4 Pro) and a **presence-detection head**. Per-node LoRA adapters are included for environment-specific fine-tuning.

```bash
# Download the model bundle
pip install huggingface_hub
huggingface-cli download ruvnet/wifi-densepose-pretrained --local-dir models/wifi-densepose-pretrained
```

**What works today vs. what's pending wiring:**

| Consumer | Format used | Status |
|----------|-------------|--------|
| Python training / evaluation / embedding extraction | `model.safetensors` | ⚠️ The published file's header is NUL-padded, which the reference `safetensors.torch.load_file` rejects (issue [#1522](https://github.com/ruvnet/RuView/issues/1522)) — pending a corrected re-upload. `csi-embed-v2.safetensors` in the same repo is unaffected and loads normally. |
| Inspect / re-export the bundle | `model.rvf.jsonl` (line-by-line JSON) | ✅ Works — plain JSONL |
| Sensing-server `--model <PATH>` flag | native RVF, `model.safetensors`, or `model.rvf.jsonl` | ✅ Native RVF loads directly; safetensors and JSONL auto-convert in memory |

**Loader scope:** `--model` now accepts native RVF and auto-converts the published safetensors or JSONL files. The quantized `model-q*.bin` files still need a compatible reader, and loading weights does not supply the matching pose-decoder architecture or establish end-to-end pose accuracy.

**Quantization choices** (all in the HF repo): `model-q2.bin` (4 KB) · `model-q4.bin` ⭐ recommended (8 KB) · `model-q8.bin` (16 KB) · `model.safetensors` full (48 KB)

The separate **17-keypoint pose-estimation model** is now published at [`ruvnet/wifi-densepose-mmfi-pose`](https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose) — **82.69% torso-PCK@20** on MM-Fi (single model) / **83.59%** (3-model ensemble + TTA), beating the prior published SOTA MultiFormer (72.25%) and CSI2Pose (68.41%) on the matched `random_split` protocol. See **Results & proof** below.

### Results & proof

See the measured benchmarks, witness records, and one-command reproducibility check.

<details>
<summary><strong>View benchmark and proof details</strong></summary>

| What | Where | Numbers |
|------|-------|---------|
| **MM-Fi pose model (SOTA)** | [`ruvnet/wifi-densepose-mmfi-pose`](https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose) | 82.69% torso-PCK@20 (single) · 83.59% (ensemble+TTA) · 75K-param micro variant 74.30% |
| **AetherArena benchmark Space** | [`ruvnet/aether-arena`](https://huggingface.co/spaces/ruvnet/aether-arena) | self-correcting, auditable MM-Fi leaderboard |
| **Full MM-Fi study (honest picture)** | [`docs/benchmarks/mmfi-wifi-sensing-study.md`](docs/benchmarks/mmfi-wifi-sensing-study.md) | pose + action; zero-shot cross-subject ~64%, labeled in-room calibration → 72.2% |
| **Efficiency frontier** | [`docs/benchmarks/wifi-pose-efficiency-frontier.md`](docs/benchmarks/wifi-pose-efficiency-frontier.md) | SOTA-beating MM-Fi pose in a ~37 KB int4 model; live ESP32 compatibility not established |
| **Pretrained encoder** | [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained) | 82.3% held-out temporal-triplet, 8 KB int4 |
| **Reproducible proof (Trust Kill Switch)** | [`archive/v1/data/proof/verify.py`](archive/v1/data/proof/verify.py) + [`expected_features.sha256`](archive/v1/data/proof/expected_features.sha256) | one-command deterministic pipeline replay (SHA-256 of output vs published hash) |
| **Benchmark-proof ADR** | [ADR-168](docs/adr/ADR-168-benchmark-proof.md) | how the numbers are produced and verified |
| **Witness attestation** | [`docs/WITNESS-LOG-028.md`](docs/WITNESS-LOG-028.md) | 33-row capability attestation matrix with per-claim evidence |

```bash
# Reproduce the deterministic pipeline proof yourself (must print VERDICT: PASS):
python archive/v1/data/proof/verify.py
```

Tracked in [#509](https://github.com/ruvnet/RuView/issues/509); see [ADR-079](docs/adr/ADR-079-camera-ground-truth-training.md) phases P7–P9 for the camera-supervised fine-tune path.

</details>

### Model weights: what's real, what's not

See which checkpoints are validated, experimental, or architecture-only.

<details>
<summary><strong>View model maturity details</strong></summary>

"WiFi → pose" means three different things in this repo, at three different maturity
levels. Read the label, not the headline ([ADR-187](docs/adr/ADR-187-archive-v1-deprecation-honest-labeling.md)):

| Tier | Checkpoint(s) | Honest status |
|------|---------------|---------------|
| **Real & validated** | [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained) (CSI encoder + presence head) · [`ruvnet/wifi-densepose-mmfi-pose`](https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose) (17-keypoint pose) · `cog-person-count/count_v1` | **MEASURED / published.** Presence = 82.3% held-out temporal-triplet accuracy (the old "100% presence" figure was retracted); MM-Fi pose = 82.69% torso-PCK@20 on the `random_split` protocol. These are the pose/presence numbers the project stands behind today. |
| **Real but weak (honestly labeled)** | committed `v2/crates/cog-pose-estimation/cog/artifacts/pose_v1.safetensors` | First-cut on-device model. **PCK@20 = 3.0% / PCK@50 = 18.5%** on a 217-sample holdout — **below the ADR-079 target of ≥ 35%.** Learns coarse structure (`r_hip` 77% PCK@50); distal/face joints near-random. Its runtime path in `cog-pose-estimation/src/inference.rs` is still a centred-skeleton **stub returning `confidence=0`** — the weights are not yet wired in. Full disclosure in the [cog README](v2/crates/cog-pose-estimation/cog/README.md). |
| **Architecture only, no weights** | `archive/v1` `DensePoseHead` | Random `kaiming_normal_` init, **no checkpoint of any kind** (zero `.pth`/`.onnx`/`.safetensors` files under `archive/v1/`). Deprecated and superseded — see [`archive/v1/DEPRECATED.md`](archive/v1/DEPRECATED.md). Do not expect real pose output from it. |

**On the ESP32-SISO question ([#509](https://github.com/ruvnet/RuView/issues/509)):** a
single-antenna, 56-subcarrier CSI stream at a 20-frame window does *not* carry the
fine-grained spatial information the multi-antenna NIC research relies on — the cog
measurements above show distal/face joints near-random. The shippable pose accuracy the
project can stand behind today is the **MM-Fi benchmark number**, not a live single-ESP32
number. The path to a first *reproducible* on-device baseline (PCK@20 ≥ 35%) is tracked in
[ADR-079](docs/adr/ADR-079-camera-ground-truth-training.md) / [#645](https://github.com/ruvnet/RuView/issues/645) — do not advertise the live single-ESP32 17-keypoint feature without the "first-cut, below-target, runtime-stub" caveat until that baseline is measured.

</details>


## 🧩 Edge Module Catalog

Add signed modules for health, security, buildings, industry, research, AI, and more.

<details>
<summary><strong>Browse the full edge module catalog</strong></summary>

Browse and install modules at [seed.cognitum.one/store](https://seed.cognitum.one/store) or on your appliance at `http://<appliance>:9000/cogs`. Each module is a small signed binary that runs beside the sensing stack. The appliance updates the catalog over the air and verifies every module before installation ([ADR-100](docs/adr/ADR-100-cog-packaging-specification.md), [ADR-102](docs/adr/ADR-102-edge-module-registry.md)).

### 🫀 Health &mdash; <sub>14 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `air-quality-index` | Track indoor air quality with CO2 and particle sensors | 8 KB | Easy |
| `baby-cry` | Sustained mid-band energy detector for nursery / infant monitoring. Audio-only, no camera. | 451 KB | Easy |
| `breathing-sync` | Detects when two people breathe in sync | 10 KB | Hard |
| `cardiac-arrhythmia` | Spots irregular heartbeats and abnormal heart rhythms | 8 KB | Hard |
| `cough-detect` | Acoustic transient + spectral cough detector with 30s cluster aggregation. Early-warning signal for respiratory illness. | 451 KB | Easy |
| `dream-stage` | Tracks your sleep stages — light, deep, and dreaming | 14 KB | Hard |
| `fall-detect` | Two-stage impact + stillness fall detector over ambient feature stream (ESP32 motion / mic). Optional ruview-mode for CSI-based pose reinforcement. | 402 KB | Easy |
| `gait-analysis` | Detects walking problems and scores fall risk | 12 KB | Hard |
| `health-monitor` | Contactless heart rate, breathing, sleep, and fall alerts | 30 KB | Med |
| `respiratory-distress` | Alerts when breathing becomes labored or dangerously fast | 10 KB | Hard |
| `seizure-detect` | Recognizes seizures and sends immediate alerts | 10 KB | Hard |
| `sleep-apnea` | Detects when someone stops breathing during sleep | 4 KB | Easy |
| `snore-monitor` | Periodic low-band energy tracker for sleep-quality / apnea-risk trending. Companion to sleep-apnea cog. | 451 KB | Easy |
| `vital-trend` | Tracks breathing and heart rate trends over weeks | 6 KB | Med |

### 🔒 Security &mdash; <sub>14 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `audit-logger` | Record every action for compliance — tamper-proof log | 8 KB | Easy |
| `behavioral-profiler` | Learns normal behavior and flags anything unusual | 12 KB | Hard |
| `fleet-auth` | Manage device certificates and access across all seeds | 12 KB | Med |
| `glass-break` | Two-phase bang + shatter acoustic detector. Distinguishes glass break from ordinary impulse noise. | 451 KB | Easy |
| `gunshot-detect` | Saturating peak + exponential decay acoustic detector with optional ruview CSI motion-drop reinforcement. | 451 KB | Easy |
| `intrusion` | Alerts when an unauthorized person enters a room | 6 KB | Med |
| `intrusion-detect-ml` | Detect network attacks using machine learning | 14 KB | Hard |
| `loitering` | Alerts when someone lingers too long in one spot | 3 KB | Easy |
| `network-firewall` | Block unauthorized network access per cog | 6 KB | Easy |
| `panic-motion` | Detects sudden panicked or erratic movement | 6 KB | Med |
| `perimeter-breach` | Guards multiple zones and shows entry direction | 10 KB | Med |
| `prompt-shield` | Blocks signal replay and injection attacks on the seed | 10 KB | Med |
| `tailgating` | Catches when someone sneaks in behind a badge holder | 6 KB | Med |
| `weapon-detect` | Detects concealed metal objects on a person | 8 KB | Hard |

### 🏢 Building &mdash; <sub>11 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `beehive-monitor` | Acoustic hive state classifier. Detects healthy / chaotic / queenless / swarming / robbing via hum-band energy + chaos + piping autocorr. | 451 KB | Easy |
| `elevator-count` | Counts how many people are in an elevator | 8 KB | Med |
| `energy-audit` | Learns your schedule and cuts wasted energy | 6 KB | Med |
| `frost-warning` | Predicts frost 6 hours ahead via temperature trend + dewpoint-depression gate. Field/orchard agriculture. | 451 KB | Easy |
| `hvac-presence` | Turns heating and cooling on when you arrive | 3 KB | Easy |
| `lighting-zones` | Turns lights on and off as people move between rooms | 4 KB | Easy |
| `meeting-room` | Shows if a meeting room is free or occupied | 5 KB | Easy |
| `occupancy-zones` | Counts people in each room through walls | 8 KB | Med |
| `predictive-maintenance` | Vibration harmonic analyzer for rotating equipment. Tracks F1 / 2×F1 / high-order / sideband energy to score degradation severity. | 451 KB | Easy |
| `smoke-fire` | Multi-signal smoke and fire detector. Fuses acoustic crackle, thermal drift proxy, and optional ruview CSI plume signature. Not a UL-listed replacement for code-required smoke alarms. | 451 KB | Easy |
| `water-leak` | Persistent low-amplitude hiss + periodic drip acoustic detector with multi-minute persistence gate. Two-stage likely → confirmed. | 451 KB | Easy |

### 🛍️ Retail &mdash; <sub>7 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `customer-flow` | Counts foot traffic in and out of each entrance | 8 KB | Med |
| `dwell-heatmap` | Shows where customers spend the most time | 6 KB | Med |
| `package-detect` | Sustained CSI-shift detector for porch / loading bay package arrivals and departures. Requires ESP32 CSI ruview input. | 451 KB | Easy |
| `parking-occupancy` | Per-zone parking occupancy via ESP32 CSI subcarrier-amplitude shift. Tracks utilization and churn-per-hour. Requires ruview. | 451 KB | Easy |
| `queue-length` | Estimates line length and wait time | 6 KB | Med |
| `shelf-engagement` | Detects when customers interact with products | 6 KB | Med |
| `table-turnover` | Tracks which restaurant tables are free or occupied | 4 KB | Easy |

### 🏭 Industrial &mdash; <sub>7 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `clean-room` | Enforces max headcount in controlled environments | 4 KB | Easy |
| `confined-space` | Monitors workers in tight spaces for safety | 5 KB | Med |
| `forklift-proximity` | Warns if a forklift gets too close to workers | 10 KB | Hard |
| `livestock-monitor` | Monitors animals for distress, escape, or illness | 6 KB | Med |
| `ppe-compliance` | Cog-composition layer: alerts when ruview-densepose detects presence in a restricted zone without an accompanying PPE-camera-cog confirmation vector. | 387 KB | Easy |
| `slip-fall-zone` | Pre-fall risk detector. Fires when motion-variance drop, splash audio, and optional cautious-gait CSI all signal elevated slip risk. | 451 KB | Easy |
| `structural-vibration` | Detects dangerous vibrations in buildings or machines | 8 KB | Hard |

### 🔬 Research &mdash; <sub>12 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `emotion-detect` | Reads stress and calm from body language and breathing | 10 KB | Hard |
| `energy-harvester` | Optimize solar and battery for off-grid seed deployment | 6 KB | Med |
| `gesture-language` | Recognizes sign language gestures in real time | 12 KB | Hard |
| `ghost-hunter` | Finds unexplained environmental anomalies — for fun | 10 KB | Hard |
| `happiness-score` | Estimates well-being from movement and mood signals | 8 KB | Med |
| `hyperbolic-space` | Maps data into curved space for tree-like structures | 12 KB | Hard |
| `music-conductor` | Reads a conductor's gestures for tempo and dynamics | 12 KB | Hard |
| `plant-growth` | Tracks plant growth rate and day/night cycles | 8 KB | Med |
| `rain-detect` | Detects when rain starts, stops, and how heavy it is | 6 KB | Med |
| `ruview-densepose` | Full body pose tracking from WiFi — no cameras needed | 50 KB | Hard |
| `sound-classifier` | Identify sounds like glass break, alarm, or baby cry | 16 KB | Hard |
| `time-crystal` | Experiments with repeating time-pattern symmetry | 12 KB | Hard |

### 🤖 Ai &mdash; <sub>15 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `anomaly-attractor` | Learns what's normal and catches anything weird | 10 KB | Hard |
| `cognitive-pipeline` | FastGRNN anomaly gate + SmolLM2 sparse-LLM inference for on-device Pi Zero 2W cognitive events | 320 KB | Hard |
| `dtw-gesture-learn` | Teach custom hand gestures by showing examples | 14 KB | Med |
| `ewc-lifelong` | Learns new things without forgetting old lessons | 8 KB | Hard |
| `federated-learning` | Train AI across seeds without sharing raw data | 18 KB | Hard |
| `goap-autonomy` | Plans and executes goals on its own | 14 KB | Hard |
| `meta-adapt` | Automatically tunes itself for best performance | 10 KB | Hard |
| `micro-hnsw` | Fast on-device fingerprinting and classification | 12 KB | Med |
| `neural-trader` | Spot market patterns and trends from live data | 20 KB | Hard |
| `pagerank-influence` | Finds the most influential person in a group | 12 KB | Med |
| `pattern-sequence` | Detects daily routines and repeated habits | 10 KB | Med |
| `rag-local` | Search your documents using AI — runs on the seed | 14 KB | Med |
| `spiking-tracker` | Brain-inspired tracker that runs on tiny hardware | 16 KB | Hard |
| `temporal-logic` | Enforces safety rules on live event streams | 12 KB | Hard |
| `time-series-forecast` | Predict sensor trends using historical patterns | 12 KB | Med |

### 🐝 Swarm &mdash; <sub>11 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `swarm-backup-restore` | Auto-backup data to other seeds — one-click restore | 8 KB | Easy |
| `swarm-cluster-monitor` | Live dashboard of every seed's health and status | 6 KB | Easy |
| `swarm-consensus` | Seeds vote before making critical changes together | 16 KB | Hard |
| `swarm-delta-sync` | Auto-sync data between seeds — only sends changes | 8 KB | Med |
| `swarm-deploy` | Install or remove cogs on all seeds at once | 10 KB | Med |
| `swarm-distributed-store` | Spread data across seeds and search them all at once | 14 KB | Hard |
| `swarm-edge-orchestrator` | Manage all ESP32 sensor nodes from one place | 14 KB | Hard |
| `swarm-load-balancer` | Spread queries across seeds so no single one overloads | 10 KB | Med |
| `swarm-mesh-manager` | Find, connect, and monitor all seeds on your network | 12 KB | Easy |
| `swarm-mqtt-bridge` | Share events between seeds over MQTT messaging | 6 KB | Easy |
| `swarm-witness-federation` | Share tamper-proof audit trails across seeds | 12 KB | Hard |

### 📡 Signal &mdash; <sub>6 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `coherence-gate` | Filters out noisy signals and keeps clean ones | 8 KB | Med |
| `flash-attention` | Focuses sensing on specific areas for better accuracy | 12 KB | Med |
| `optimal-transport` | Measures motion using shape-aware signal comparison | 12 KB | Hard |
| `person-matching` | Tells apart multiple people in the same room | 18 KB | Hard |
| `sparse-recovery` | Recovers missing signal data from partial readings | 16 KB | Hard |
| `temporal-compress` | Shrinks old data to save memory without losing meaning | 14 KB | Med |

### 🌐 Network &mdash; <sub>1 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `tailscale` | Reach the seed from anywhere via a private WireGuard mesh (Tailscale). Userspace mode — no root. | 700 KB | Med |

### 🛠️ Developer &mdash; <sub>7 modules</sub>

| ID | What it does | Size | Difficulty |
|----|--------------|-----:|:----------:|
| `adversarial` | Detects tampered or spoofed sensor signals | 4 KB | Easy |
| `coherence` | Monitors signal quality across multiple channels | 4 KB | Easy |
| `gesture` | Core gesture recognition building block for cogs | 6 KB | Med |
| `interference-search` | Searches many possibilities at once for fast answers | 14 KB | Hard |
| `psycho-symbolic` | Reasons over knowledge graphs with multiple styles | 16 KB | Hard |
| `quantum-coherence` | Quantum-inspired model for advanced signal states | 16 KB | Hard |
| `self-healing-mesh` | Keeps sensor mesh running even when nodes drop out | 14 KB | Hard |

> ℹ️ Build your own cog: see [ADR-100](docs/adr/ADR-100-cog-packaging-specification.md) for the packaging spec. The first cog this repo ships into the catalog lives in [v2/crates/cog-pose-estimation/](v2/crates/cog-pose-estimation/) (17-keypoint WiFi pose, [ADR-101](docs/adr/ADR-101-pose-estimation-cog.md)).

</details>


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

The [Self-Learning system (ADR-024)](docs/adr/ADR-024-contrastive-csi-embedding-model.md) provides
camera-free representation-learning components. Cross-room pose remains a separate, data-gated
problem: [MERIDIAN (ADR-027)](docs/adr/ADR-027-cross-environment-domain-generalization.md) is
**Proposed**, while the measured calibration reference requires labeled CSI/keypoint pairs and
model-specific adapters. See the [model compatibility boundary](docs/user-guide.md#model-and-capture-compatibility).

---

## 🏢 Use Cases & Applications

> **Safety boundary:** these are research and prototype applications, not medical devices,
> emergency systems, or safety-certified controls. Vital-sign and pose outputs require independent
> validation on the exact hardware, room, subjects, and failure conditions before operational use.

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
| **Fitness & sports research** | Explore motion and breathing cadence without a wearable or camera; reliable posture correction requires a validated compatible pose model | 3+ ESP32-S3 mesh + edge host | Prototype; no live S3 pose accuracy claim | [Breathing Sync](docs/edge-modules/exotic.md), [Gait Analysis](docs/edge-modules/medical.md) |
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


---

## 🧠 Self-Learning WiFi AI

Learn compact room fingerprints from raw CSI and adapt the model to each environment.

<details>
<summary><strong>View self-learning architecture and commands</strong></summary>

Every WiFi signal that passes through a room creates a unique fingerprint of that space. WiFi-DensePose already reads these fingerprints to track people, but until now it threw away the internal "understanding" after each reading. The Self-Learning WiFi AI captures and preserves that understanding as compact, reusable vectors — and continuously optimizes itself for each new environment.

**What it does in plain terms:**
- Turns any WiFi signal into a 128-number "fingerprint" that uniquely describes what's happening in a room
- Learns entirely on its own from raw WiFi data — no cameras, no labeling, no human supervision needed
- Recognizes rooms, detects intruders, and classifies activities using only WiFi (named person-identity is an experimental, data-gated research capability — see below, not a shipped feature)
- Runs on an $8 ESP32 chip (the entire model fits in 55 KB of memory)
- Produces both body pose tracking AND environment fingerprints in a single computation

**Key Capabilities**

| What | How it works | Why it matters |
|------|-------------|----------------|
| **Self-supervised learning** | The model watches WiFi signals and teaches itself what "similar" and "different" look like, without any human-labeled data | Deploy anywhere — just plug in a WiFi sensor and wait 10 minutes |
| **Room identification** | Each room produces a distinct WiFi fingerprint pattern | Know which room someone is in without GPS or beacons |
| **Anomaly detection** | An unexpected person or event creates a fingerprint that doesn't match anything seen before | Automatic intrusion and fall detection as a free byproduct |
| **Person re-identification** *(experimental, research)* | A real per-channel similarity matcher (Soul Signature §3.6, `wifi-densepose-bfld`); **measured** result: on WiFi-only cardiac+respiratory channels alone two people are *not* separable (gap ~0.0005) | Honest research capability — **named identity is not claimed** and is data-gated on enrollment with the decisive AETHER/body-resonance channel. See [#1021](https://github.com/ruvnet/RuView/issues/1021) |
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

Use the in-repo plugin for guided setup, sensing, training, and verification in Claude Code or Codex.

<details>
<summary><strong>View plugin installation and commands</strong></summary>

RuView's [Claude Code](https://docs.anthropic.com/en/docs/claude-code) plugin and Codex prompt mirror cover onboarding, ESP32 setup, sensing apps, model training, advanced sensing, CLI/API/WASM, mmWave radar, and witness verification. The source lives in [`plugins/ruview/`](plugins/ruview/README.md); the marketplace manifest is [`.claude-plugin/marketplace.json`](.claude-plugin/marketplace.json).

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

For the portable RuView MetaHarness, use `npx @ruvnet/ruview@0.4.0`; the quick commands and fuller explanation are in the collapsed MetaHarness section near the top of this README and in [`harness/ruview/`](harness/ruview/README.md).

</details>

---

## 📖 Documentation

Start with the user, build, and calibration guides; expand for the full reference map.

<details>
<summary><strong>Browse all documentation</strong></summary>

| Document | Description |
|----------|-------------|
| [User Guide](docs/user-guide.md) | Step-by-step guide: installation, first run, API usage, hardware setup, training |
| [Build Guide](docs/build-guide.md) | Building from source (Rust and Python) |
| [Calibration & Room Training Guide](docs/calibration-guide.md) | What `calibrate`/`enroll`/`train-room` actually enforce: minimum frame counts, per-anchor quality gates, the pet/small-motion presence-detection caveat, and empty-room baseline conditions — grounded in the real code, not just ADR-135/151 |
| [Trust State & Engine Errors](docs/trust-and-engine-errors.md) | What `engine_error_count` and `demoted` mean on `/api/v1/status`, exact trigger conditions, the current diagnostic gap (no per-cause breakdown), the `WDP_GUARD_INTERVAL_US` recovery path, and why a converted Hugging Face model isn't shown to be the cause in code |
| [**Home Assistant + Matter Integration**](docs/integrations/home-assistant.md) | **Works with Home Assistant** via MQTT auto-discovery + **Works with Matter** (Apple Home / Google Home / Alexa / SmartThings) — full entity catalog, 3 starter blueprints, Lovelace dashboards, privacy mode, threshold tuning ([ADR-115](docs/adr/ADR-115-home-assistant-integration.md)). |
| [**BFLD — Beamforming Feedback Layer for Detection**](v2/crates/wifi-densepose-bfld/README.md) | New privacy-gated WiFi sensing layer that measures + structurally prevents identity leakage from 802.11ac/ax Beamforming Feedback Information. Three type-enforced invariants (raw BFI never exits node, identity embedding is in-RAM-only, cross-site correlation cryptographically impossible via per-site BLAKE3 keyed hash + daily rotation). Ships full operator surface (`BfldPipeline`, `BfldPipelineHandle`, the Soul Signature §3.6 per-channel matcher `EnrolledMatcher`/`SoulMatchOracle` — experimental; named identity is data-gated, **measured** as not-separable on WiFi-only channels alone), MQTT topic router + HA-DISCO + availability + LWT, 3 operator HA blueprints, two runnable examples, eclipse-mosquitto:2 CI service container. 327+ tests. [ADR-118](docs/adr/ADR-118-bfld-beamforming-feedback-layer-for-detection.md) umbrella + sub-ADRs [119](docs/adr/ADR-119-bfld-frame-format-and-wire-protocol.md)/[120](docs/adr/ADR-120-bfld-privacy-class-and-hash-rotation.md)/[121](docs/adr/ADR-121-bfld-identity-risk-scoring.md)/[122](docs/adr/ADR-122-bfld-ruview-ha-matter-exposure.md)/[123](docs/adr/ADR-123-bfld-capture-path-nexmon-and-esp32.md). Research dossier: [`docs/research/BFLD/`](docs/research/BFLD/) (11 files, 13,544 words). |
| [**SENSE-BRIDGE — rvagent MCP server**](tools/ruview-mcp/README.md) | Dual-transport MCP server (`@ruvnet/rvagent`) bridging the RuView sensing stack to AI agents (Claude Code, Cursor, ruflo swarms). 6 tools wired: `ruview.presence.now`, `ruview.vitals.get_{breathing,heart_rate,all}`, `ruview.bfld.last_scan`, `ruview.bfld.subscribe`. stdio + Streamable HTTP (`POST /mcp`, Origin-validated, bearer-token auth, `127.0.0.1` bind). Full 20-tool Zod schema barrel + 5 RUVIEW-POLICY governance tools. 93 tests. [ADR-124](docs/adr/ADR-124-rvagent-mcp-ruvector-npm-integration.md). Try: `npx @ruvnet/rvagent stdio`. |
| [Semantic Primitives — Precision/Recall](docs/integrations/semantic-primitives-metrics.md) | Per-primitive F1 on the held-out paired-capture set: someone-sleeping, possible-distress, room-active, elderly-inactivity-anomaly, meeting, bathroom, fall-risk, bed-exit, no-movement, multi-room. |
| [Claude Code / Codex Plugin](plugins/ruview/README.md) | The `ruview` plugin + marketplace — skills, `/ruview-*` commands, agents, and the Codex prompt mirror |
| [Portable harness — `npx @ruvnet/ruview`](harness/ruview/README.md) | MetaHarness-minted, host-portable RuView operator harness — `ruview.*` MCP tools + the MEASURED-vs-CLAIMED honesty guardrail enforced in code ([ADR-182](docs/adr/ADR-182-npx-ruview-harness-via-metaharness.md)). A lighter, multi-host companion to the in-repo plugin. |
| [CSI Frame Selection — Measured](docs/csi-frame-selection-measured.md) | Which rate gate actually buys cross-node pairing, measured on a nine-node ESP32-C6 fleet as a 2x2 factorial in three Latin-square blocks. Selecting frames by 802.11 `rx_seq` roughly doubled pairing (+31.7 pp) while mesh-time alignment bought +0.5 pp, inside noise. Includes the trade it makes, a prediction that turned out wrong, the limits of the measurement, and `run_arms.py` so the comparison can be re-run rather than taken on trust. |
| [ESP32-C6 Firmware Runbook](firmware/esp32-csi-node/RUNBOOK.md) | What to run, in what order, and what to check: the three-file build that keeps OTA rollback compiled in, a five-key pre-flash gate, which flash regions OTA can and cannot reach, a single-board diagnostic flash with a revert path, and the tunables whose right value depends on your house rather than on the chip. |
| [Architecture Decisions](docs/adr/README.md) | 205 ADRs — why each technical choice was made, organized by domain (hardware, signal processing, ML, platform, infrastructure) |
| [Domain Models](docs/ddd/README.md) | 8 DDD models (RuvSense, Signal Processing, Training Pipeline, Hardware Platform, Sensing Server, WiFi-Mat, CHCI, rvCSI) — bounded contexts, aggregates, domain events, and ubiquitous language |
| [rvCSI — edge RF sensing runtime](https://github.com/ruvnet/rvcsi) | Rust-first / TypeScript-accessible / hardware-abstracted CSI runtime: multi-source ingestion (incl. real nexmon_csi `.pcap` from a **Raspberry Pi 5** / Pi 4 / Pi 3B+ — CYW43455 / BCM43455c0) → validation → DSP → typed events → RuVector RF memory ([ADR-095](docs/adr/ADR-095-rvcsi-edge-rf-sensing-platform.md), [ADR-096](docs/adr/ADR-096-rvcsi-ffi-crate-layout.md), [domain model](docs/ddd/rvcsi-domain-model.md)). Now its own repo — [`ruvnet/rvcsi`](https://github.com/ruvnet/rvcsi) — vendored here under `vendor/rvcsi`; 9 `rvcsi-*` crates on crates.io, `@ruv/rvcsi` on npm, plus a Claude Code plugin. |
| [Desktop App](v2/crates/wifi-densepose-desktop/README.md) | **WIP** — Tauri v2 desktop app for node management, OTA updates, WASM deployment, and mesh visualization |
| `ruview-swarm` | Drone swarm control system (ADR-148) — hierarchical-mesh topology, Raft consensus, MARL, CSI sensing payload, MAVLink/PX4/ArduPilot compatibility, Ruflo AI-agent integration |
| `ruview-unified` | Unified RF spatial world model ([ADR-273](docs/adr/ADR-273-unified-rf-spatial-world-model.md)..[277](docs/adr/ADR-277-edge-sensing-control-plane.md)) — canonical RF tensor + hardware adapters (WiFi CSI / FMCW radar / UWB / 5G SRS), universal RF foundation encoder with ≤1% task adapters, RF-aware Gaussian spatial memory with channel-gain queries + inverse updates, physics-guided synthetic RF worlds, and an 802.11bf/ETSI-ISAC-aligned sensing policy plane (raw RF structurally unexportable). All accuracy numbers SYNTHETIC until real-data validation. |
| [Medical Examples](examples/medical/README.md) | Contactless blood pressure, heart rate, breathing rate via 60 GHz mmWave radar — $15 hardware, no wearable |
| [Extended Documentation](docs/readme-details.md) | Latest additions, key features, installation, quick start, signal processing, training, CLI, testing, deployment, and changelog |

</details>

---

## 🚧 Beta software

> **Beta Software** — Under active development. APIs and firmware may change. Known limitations:
> - ESP32-C3 and original ESP32 are not supported (single-core, insufficient for CSI DSP)
> - Single ESP32 deployments have limited spatial resolution — use 2+ nodes or add a [Cognitum Seed](https://cognitum.one) for best results
> - Camera-free pose accuracy is limited (PCK@20 ≈ 2.5% with proxy labels) — [camera ground-truth training](docs/adr/ADR-079-camera-ground-truth-training.md) targets **35%+ PCK@20**; the pipeline is implemented, but the data-collection and evaluation phases (ADR-079 P7–P9) are still pending.
>
> Contributions and bug reports welcome at [Issues](https://github.com/ruvnet/RuView/issues).

## 📄 License

MIT License — see [LICENSE](LICENSE) for details.

## 🤝 Creator Affiliate Program

**For TikTok · Instagram · YouTube creators** — earn **25% on every Cognitum sale** you refer. The RuFlo, RuView, and RuVector videos you're already making have done millions of views; get paid for the orders they drive. Click-tracking activates instantly; commissions activate after a quick manual review (usually under 24 hours).

[Apply now → cognitum.one/affiliate](https://cognitum.one/affiliate)

## 📞 Support

[GitHub Issues](https://github.com/ruvnet/RuView/issues) | [Discussions](https://github.com/ruvnet/RuView/discussions) | [PyPI](https://pypi.org/project/wifi-densepose/)

---

**WiFi DensePose** — Privacy-preserving human pose estimation through WiFi signals.
