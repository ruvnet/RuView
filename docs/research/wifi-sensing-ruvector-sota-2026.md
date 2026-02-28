# WiFi Sensing + Vector Intelligence: State of the Art and 20-Year Projection

**Date:** 2026-02-28
**Scope:** WiFi CSI-based human sensing, vector database signal intelligence (RuVector/HNSW), edge AI inference, post-quantum cryptography, and technology trajectory through 2046.

---

## 1. WiFi CSI Human Sensing: State of the Art (2023–2026)

### 1.1 Foundational Work: DensePose From WiFi

The seminal work by Geng, Huang, and De la Torre at Carnegie Mellon University ([arXiv:2301.00250](https://arxiv.org/abs/2301.00250), 2023) demonstrated that dense human pose correspondence can be estimated using WiFi signals alone. Their architecture maps CSI phase and amplitude to UV coordinates across 24 body regions, achieving performance comparable to image-based approaches.

The pipeline consists of three stages:
1. **Amplitude and phase sanitization** of raw CSI
2. **Two-branch encoder-decoder network** translating sanitized CSI to 2D feature maps
3. **Modified DensePose-RCNN** producing UV maps from the 2D features

This work established that commodity WiFi routers contain sufficient spatial information for dense human pose recovery, without cameras.

### 1.2 Multi-Person 3D Pose Estimation (CVPR 2024)

Yan et al. presented **Person-in-WiFi 3D** at CVPR 2024 ([paper](https://openaccess.thecvf.com/content/CVPR2024/papers/Yan_Person-in-WiFi_3D_End-to-End_Multi-Person_3D_Pose_Estimation_with_Wi-Fi_CVPR_2024_paper.pdf)), advancing the field from 2D to end-to-end multi-person 3D pose estimation using WiFi signals. This represents a significant leap — handling multiple subjects simultaneously in three dimensions using only wireless signals.

### 1.3 Cross-Site Generalization (IEEE IoT Journal, 2024)

Zhou et al. published **AdaPose** (IEEE Internet of Things Journal, 2024, vol. 11, pp. 40255–40267), addressing one of the critical challenges: cross-site generalization. WiFi sensing models trained in one environment often fail in others due to different multipath profiles. AdaPose demonstrates device-free human pose estimation that transfers across sites using commodity WiFi hardware.

### 1.4 Lightweight Architectures (ECCV 2024)

**HPE-Li** was presented at ECCV 2024 in Milan, introducing WiFi-enabled lightweight dual selective kernel convolution for human pose estimation. This work targets deployment on resource-constrained edge devices — a critical requirement for practical WiFi sensing systems.

### 1.5 Subcarrier-Level Analysis (2025)

**CSI-Channel Spatial Decomposition** (Electronics, February 2025, [MDPI](https://www.mdpi.com/2079-9292/14/4/756)) decomposes CSI spatial structure into dual-view observations — spatial direction and channel sensitivity — demonstrating that this decomposition is sufficient for unambiguous localization and identification. This work directly informs how subcarrier-level features should be extracted from CSI data.

**Deciphering the Silent Signals** (Springer, 2025) applies explainable AI to understand which WiFi frequency components contribute most to pose estimation, providing critical insight into feature selection for signal processing pipelines.

### 1.6 ESP32 CSI Sensing

The Espressif ESP32 has emerged as a practical, affordable CSI sensing platform:

| Metric | Result | Source |
|--------|--------|--------|
| Human identification accuracy | 88.9–94.5% | Gaiba & Bedogni, IEEE CCNC 2024 |
| Through-wall HAR range | 18.5m across 5 rooms | [Springer, 2023](https://link.springer.com/chapter/10.1007/978-3-031-44137-0_4) |
| On-device inference accuracy | 92.43% at 232ms latency | MDPI Sensors, 2025 |
| Data augmentation improvement | 59.91% → 97.55% | EMD-based augmentation, 2025 |

Key findings from ESP32 research:
- **ESP32-S3** is the preferred variant due to improved processing power and AI instruction set support
- **Directional biquad antennas** extend through-wall range significantly
- **On-device DenseNet inference** is achievable at 232ms per frame on ESP32-S3
- [Espressif ESP-CSI](https://github.com/espressif/esp-csi) provides official CSI collection tools

### 1.7 Hardware Comparison for CSI

| Parameter | ESP32-S3 | Intel 5300 | Atheros AR9580 |
|-----------|----------|------------|----------------|
| Subcarriers | 52–56 | 30 (compressed) | 56 (full) |
| Antennas | 1–2 TX/RX | 3 TX/RX (MIMO) | 3 TX/RX (MIMO) |
| Cost | $5–15 | $50–100 (discontinued) | $30–60 (discontinued) |
| CSI quality | Consumer-grade | Research-grade | Research-grade |
| Availability | In production | eBay only | eBay only |
| Edge inference | Yes (on-chip) | Requires host PC | Requires host PC |
| Through-wall range | 18.5m demonstrated | ~10m typical | ~15m typical |

---

## 2. Vector Databases for Signal Intelligence

### 2.1 WiFi Fingerprinting as Vector Search

WiFi fingerprinting is fundamentally a nearest-neighbor search problem. Rocamora and Ho (Expert Systems with Applications, November 2024, [ScienceDirect](https://www.sciencedirect.com/science/article/abs/pii/S0957417424026691)) demonstrated that deep learning vector embeddings (d-vectors and i-vectors, adapted from speech processing) provide compact CSI fingerprint representations suitable for scalable retrieval.

Their key insight: CSI fingerprints are high-dimensional vectors. The online positioning phase reduces to finding the nearest stored fingerprint vector to the current observation. This is exactly the problem HNSW solves.

### 2.2 HNSW for Sub-Millisecond Signal Matching

Hierarchical Navigable Small Worlds (HNSW) provides O(log n) approximate nearest-neighbor search through a layered proximity graph:

- **Bottom layer**: Dense graph connecting all vectors
- **Upper layers**: Sparse skip-list structure for fast navigation
- **Search**: Greedy descent through sparse layers, bounded beam search at bottom

For WiFi sensing, HNSW enables:
- **Real-time fingerprint matching**: <1ms query at 100K stored fingerprints
- **Environment adaptation**: Quickly find similar CSI patterns as the environment changes
- **Multi-person disambiguation**: Separate overlapping CSI signatures by similarity

### 2.3 RuVector's HNSW Implementation

RuVector provides a Rust-native HNSW implementation with SIMD acceleration, supporting:
- 329-dimensional CSI feature vectors (64 amplitude + 64 variance + 63 phase + 10 Doppler + 128 PSD)
- PQ8 product quantization for 8x memory reduction
- Hyperbolic embeddings (Poincaré ball) for hierarchical activity classification
- Copy-on-write branching for environment-specific fingerprint databases

### 2.4 Self-Learning Signal Intelligence (SONA)

The Self-Optimizing Neural Architecture (SONA) in RuVector adapts pose estimation models online through:
- **LoRA fine-tuning**: Only 0.56% of parameters (17,024 of 3M) are adapted per environment
- **EWC++ regularization**: Prevents catastrophic forgetting of previously learned environments
- **Feedback signals**: Temporal consistency, physical plausibility, multi-view agreement
- **Adaptation latency**: <1ms per update cycle

This enables a WiFi sensing system that improves its accuracy over time as it observes more data in a specific environment, without forgetting how to function in previously visited environments.

---

## 3. Edge AI and WASM Inference

### 3.1 ONNX Runtime Web

ONNX Runtime Web ([documentation](https://onnxruntime.ai/docs/tutorials/web/)) enables ML inference directly in browsers via WebAssembly:

- **WASM backend**: Near-native CPU inference, multi-threading via SharedArrayBuffer, SIMD128 acceleration
- **WebGPU backend**: GPU-accelerated inference (19x speedup on Segment Anything encoder)
- **WebNN backend**: Hardware-neutral neural network acceleration

Performance benchmarks (MobileNet V2):
- WASM + SIMD + 2 threads: **3.4x speedup** over plain WASM
- WebGPU: **19x speedup** for attention-heavy models

### 3.2 Rust-Native WASM Inference

[WONNX](https://github.com/webonnx/wonnx) provides a GPU-accelerated ONNX runtime written entirely in Rust, compiled to WASM. This aligns directly with the wifi-densepose Rust architecture and enables:
- Single-binary deployment as `.wasm` module
- WebGPU acceleration when available
- CPU fallback via WASM for older devices

### 3.3 Model Quantization for Edge

| Quantization | Size | Accuracy Impact | Target |
|-------------|------|----------------|--------|
| Float32 | 12MB | Baseline | Server |
| Float16 | 6MB | <0.5% loss | Tablets |
| Int8 (PTQ) | 3MB | <2% loss | Browser/mobile |
| Int4 (GPTQ) | 1.5MB | <5% loss | ESP32/IoT |

The wifi-densepose WASM module targets 5.5KB runtime + 0.7–62MB container depending on profile (IoT through Field deployment).

### 3.4 RVF Edge Containers

RuVector's RVF (Cognitive Container) format packages model weights, HNSW index, fingerprint vectors, and WASM runtime into a single deployable file:

| Profile | Container Size | Boot Time | Target |
|---------|---------------|-----------|--------|
| IoT | ~0.7 MB | <200ms | ESP32 |
| Browser | ~10 MB | ~125ms | Chrome/Firefox |
| Mobile | ~6 MB | ~150ms | iOS/Android |
| Field | ~62 MB | ~200ms | Disaster response |

---

## 4. Post-Quantum Cryptography for Sensor Networks

### 4.1 NIST PQC Standards (Finalized August 2024)

NIST released three finalized standards ([announcement](https://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-encryption-standards)):

| Standard | Algorithm | Type | Signature Size | Use Case |
|----------|-----------|------|---------------|----------|
| FIPS 203 (ML-KEM) | CRYSTALS-Kyber | Key encapsulation | 1,088 bytes | Key exchange |
| FIPS 204 (ML-DSA) | CRYSTALS-Dilithium | Digital signature | 2,420 bytes (ML-DSA-65) | General signing |
| FIPS 205 (SLH-DSA) | SPHINCS+ | Hash-based signature | 7,856 bytes | Conservative backup |

### 4.2 IoT Sensor Considerations

For bandwidth-constrained WiFi sensor mesh networks:
- **ML-DSA-65** signature size (2,420 bytes) is feasible for ESP32 UDP streams (~470 byte CSI frames + 2.4KB signature = ~2.9KB per authenticated frame)
- **FN-DSA** (FALCON, expected 2026–2027) will offer smaller signatures (~666 bytes) but requires careful Gaussian sampling implementation
- **Hybrid approach**: ML-DSA + Ed25519 dual signatures during transition period (as specified in ADR-007)

### 4.3 Transition Timeline

| Milestone | Date |
|-----------|------|
| NIST PQC standards finalized | August 2024 |
| First post-quantum certificates | 2026 |
| Browser-wide trust | 2027 |
| Quantum-vulnerable algorithms deprecated | 2030 |
| Full removal from NIST standards | 2035 |

WiFi-DensePose's early adoption of ML-DSA-65 positions it ahead of the deprecation curve, ensuring sensor mesh data integrity remains quantum-resistant.

---

## 5. Twenty-Year Projection (2026–2046)

### 5.1 WiFi Evolution and Sensing Resolution

#### WiFi 7 (802.11be) — Available Now
- **320 MHz channels** with up to 3,984 CSI tones (vs. 56 on ESP32 today)
- **16×16 MU-MIMO** spatial streams (vs. 2×2 on ESP32)
- **Sub-nanosecond RTT resolution** for centimeter-level positioning
- Built-in sensing capabilities in PHY/MAC layer

WiFi 7's 320 MHz bandwidth provides ~71x more CSI tones than current ESP32 implementations. This alone transforms sensing resolution.

#### WiFi 8 (802.11bn) — Expected ~2028
- Operations across **sub-7 GHz, 45 GHz, and 60 GHz** bands ([survey](https://www.sciencedirect.com/science/article/abs/pii/S1389128625005572))
- **WLAN sensing as a core PHY/MAC capability** (not an add-on)
- Formalized sensing frames and measurement reporting
- Higher-order MIMO configurations

#### Projected WiFi Sensing Resolution by Decade

| Timeframe | WiFi Gen | Subcarriers | MIMO | Spatial Resolution | Sensing Capability |
|-----------|----------|------------|------|-------------------|-------------------|
| 2024 | WiFi 6 (ESP32) | 56 | 2×2 | ~1m | Presence, coarse motion |
| 2025 | WiFi 7 | 3,984 | 16×16 | ~10cm | Pose, gestures, respiration |
| ~2028 | WiFi 8 | 10,000+ | 32×32 | ~2cm | Fine motor, vital signs |
| ~2033 | WiFi 9* | 20,000+ | 64×64 | ~5mm | Medical-grade monitoring |
| ~2040 | WiFi 10* | 50,000+ | 128×128 | ~1mm | Sub-dermal sensing |

*Projected based on historical doubling patterns in IEEE 802.11 standards.

### 5.2 Medical-Grade Vital Signs via Ambient WiFi

**Current state (2026):** Breathing detection at 85–95% accuracy with ESP32 mesh; heartbeat detection marginal and placement-sensitive.

**Projected trajectory:**
- **2028–2030**: WiFi 8's formalized sensing + 60 GHz millimeter-wave enables reliable heartbeat detection at ~95% accuracy. Hospital rooms equipped with sensing APs replace some wired patient monitors.
- **2032–2035**: Sub-centimeter Doppler resolution enables blood flow visualization, glucose monitoring via micro-Doppler spectroscopy. FDA Class II clearance for ambient WiFi vital signs monitoring.
- **2038–2042**: Ambient WiFi provides continuous, passive health monitoring equivalent to today's wearable devices. Elderly care facilities use WiFi sensing for fall detection, sleep quality, and early disease indicators.
- **2042–2046**: WiFi sensing achieves sub-millimeter resolution. Non-invasive blood pressure, heart rhythm analysis, and respiratory function testing become standard ambient measurements. Medical imaging grade penetration through walls.

### 5.3 Smart City Mesh Sensing at Scale

**Projected deployment:**
- **2028**: Major cities deploy WiFi 7/8 infrastructure with integrated sensing. Pedestrian flow monitoring replaces camera-based surveillance in privacy-sensitive zones.
- **2032**: Urban-scale mesh sensing networks provide real-time occupancy maps of public spaces, transit systems, and emergency shelters. Disaster response systems (like wifi-densepose-mat) operate as permanent city infrastructure.
- **2038**: Full-city coverage enables ambient intelligence: traffic optimization, crowd management, emergency detection — all without cameras, using only the WiFi infrastructure already deployed for connectivity.

### 5.4 Vector Intelligence at Scale

**Projected evolution of HNSW-based signal intelligence:**
- **2028**: HNSW indexes of 10M+ CSI fingerprints per city zone, enabling instant environment recognition and person identification across any WiFi-equipped space. RVF containers store environment-specific models that adapt in <1ms.
- **2032**: Federated learning across city-scale HNSW indexes. Each building's local index contributes to a global model without sharing raw CSI data. Post-quantum signatures ensure tamper-evident data provenance.
- **2038**: Continuous self-learning via SONA at city scale. The system improves autonomously from billions of daily observations. EWC++ prevents catastrophic forgetting across seasonal and environmental changes.
- **2042**: Exascale vector indexes (~1T fingerprints) with sub-microsecond queries via quantum-classical hybrid search. WiFi sensing becomes an ambient utility like electricity — invisible, always-on, universally available.

### 5.5 Privacy-Preserving Sensing Architecture

The critical challenge for large-scale WiFi sensing is privacy. Projected solutions:

- **2026–2028**: On-device processing (ESP32/edge WASM) ensures raw CSI never leaves the local network. RVF containers provide self-contained inference without cloud dependency.
- **2030–2033**: Homomorphic encryption enables cloud-based CSI processing without decryption. Federated learning trains global models without sharing local data.
- **2035–2040**: Post-quantum cryptography secures all sensor mesh communication against quantum adversaries. Zero-knowledge proofs enable presence verification without revealing identity.
- **2040–2046**: Fully decentralized sensing with CRDT-based consensus (no central authority). Individuals control their own sensing data via personal RVF containers signed with post-quantum keys.

---

## 6. Implications for WiFi-DensePose + RuVector

The convergence of these technologies creates a clear path for wifi-densepose:

1. **Near-term (2026–2028)**: ESP32 mesh with feature-level fusion provides practical presence/motion detection. RuVector's HNSW enables real-time fingerprint matching. WASM edge deployment eliminates cloud dependency. Trust kill switch proves pipeline authenticity.

2. **Medium-term (2028–2032)**: WiFi 7/8 CSI (3,984+ tones) transforms sensing from coarse presence to fine-grained pose estimation. SONA adaptation makes the system self-improving. Post-quantum signatures secure the sensor mesh.

3. **Long-term (2032–2046)**: WiFi sensing becomes ambient infrastructure. Medical-grade monitoring replaces wearables. City-scale vector intelligence operates autonomously. The architecture established today — RVF containers, HNSW indexes, witness chains, distributed consensus — scales directly to this future.

The fundamental insight: **the software architecture for ambient WiFi sensing at scale is being built now, using technology available today.** The hardware (WiFi 7/8, faster silicon) will arrive to fill the resolution gap. The algorithms (HNSW, SONA, EWC++) are already proven. The cryptography (ML-DSA, SLH-DSA) is standardized. What matters is building the correct abstractions — and that is exactly what the RuVector integration provides.

---

## References

### WiFi Sensing
- [DensePose From WiFi](https://arxiv.org/abs/2301.00250) — Geng, Huang, De la Torre (CMU, 2023)
- [Person-in-WiFi 3D](https://openaccess.thecvf.com/content/CVPR2024/papers/Yan_Person-in-WiFi_3D_End-to-End_Multi-Person_3D_Pose_Estimation_with_Wi-Fi_CVPR_2024_paper.pdf) — Yan et al. (CVPR 2024)
- [CSI-Channel Spatial Decomposition](https://www.mdpi.com/2079-9292/14/4/756) — Electronics, Feb 2025
- [WiFi CSI-Based Through-Wall HAR with ESP32](https://link.springer.com/chapter/10.1007/978-3-031-44137-0_4) — Springer, 2023
- [Espressif ESP-CSI](https://github.com/espressif/esp-csi) — Official CSI tools
- [WiFi Sensing Survey](https://dl.acm.org/doi/10.1145/3705893) — ACM Computing Surveys, 2025
- [WiFi-Based Human Identification Survey](https://pmc.ncbi.nlm.nih.gov/articles/PMC11479185/) — PMC, 2024

### Vector Search & Fingerprinting
- [WiFi CSI Fingerprinting with Vector Embedding](https://www.sciencedirect.com/science/article/abs/pii/S0957417424026691) — Rocamora & Ho (Expert Systems with Applications, 2024)
- [HNSW Explained](https://milvus.io/blog/understand-hierarchical-navigable-small-worlds-hnsw-for-vector-search.md) — Milvus Blog
- [WiFi Fingerprinting Survey](https://pmc.ncbi.nlm.nih.gov/articles/PMC12656469/) — PMC, 2024

### Edge AI & WASM
- [ONNX Runtime Web](https://onnxruntime.ai/docs/tutorials/web/) — Microsoft
- [WONNX: Rust ONNX Runtime](https://github.com/webonnx/wonnx) — WebGPU-accelerated
- [In-Browser Deep Learning on Edge Devices](https://arxiv.org/html/2309.08978v2) — arXiv, 2023

### Post-Quantum Cryptography
- [NIST PQC Standards](https://www.nist.gov/news-events/news/2024/08/nist-releases-first-3-finalized-post-quantum-encryption-standards) — FIPS 203/204/205 (August 2024)
- [NIST IR 8547: PQC Transition](https://nvlpubs.nist.gov/nistpubs/ir/2024/NIST.IR.8547.ipd.pdf) — Transition timeline
- [State of PQC Internet 2025](https://blog.cloudflare.com/pq-2025/) — Cloudflare

### WiFi Evolution
- [Wi-Fi 7 (802.11be)](https://en.wikipedia.org/wiki/Wi-Fi_7) — Finalized July 2025
- [From Wi-Fi 7 to Wi-Fi 8 Survey](https://www.sciencedirect.com/science/article/abs/pii/S1389128625005572) — ScienceDirect, 2025
- [Wi-Fi 7 320MHz Channels](https://www.netgear.com/hub/network/wifi-7-320mhz-channels/) — Netgear
