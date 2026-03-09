# RF Topological Sensing — Research Index

## SOTA Research Compendium

**Generated**: 2026-03-08
**Total Documents**: 12
**Total Lines**: 14,322
**Branch**: `claude/rf-mincut-sensing-uHnQX`

---

## Core Concept

RF Topological Sensing treats a room as a dynamic signal graph where ESP32 nodes
are vertices and TX-RX links are edges weighted by CSI coherence. Instead of
estimating position, minimum cut detects where the RF field topology changes —
revealing physical boundaries corresponding to objects, people, and environmental
shifts. This creates a "radio nervous system" that is structurally aware of space.

---

## Document Index

### Foundations (Documents 1-2)

| # | Document | Lines | Key Topics |
|---|----------|-------|------------|
| 01 | [RF Graph Theory & Mincut Foundations](01-rf-graph-theory-foundations.md) | 1,112 | Max-flow/min-cut theorem, Stoer-Wagner/Karger algorithms, Fiedler vector, Cheeger inequality, spectral graph theory, comparison to classical RF sensing |
| 02 | [CSI Edge Weight Computation](02-csi-edge-weight-computation.md) | 1,059 | CSI feature extraction, coherence metrics, MUSIC/ESPRIT multipath decomposition, Kalman filtering of edges, noise robustness, normalization |

### Machine Learning (Documents 3-4)

| # | Document | Lines | Key Topics |
|---|----------|-------|------------|
| 03 | [Attention Mechanisms for RF Sensing](03-attention-mechanisms-rf-sensing.md) | 1,110 | GAT for RF graphs, self-attention for CSI, cross-attention fusion, differentiable mincut, antenna-level attention, efficient attention variants |
| 04 | [Transformer Architectures for Graph Sensing](04-transformer-architectures-graph-sensing.md) | 896 | Graphormer/SAN/GPS, temporal graph transformers, ViT for spectrograms, transformer-based mincut prediction, foundation models for RF, edge deployment |

### Algorithms (Document 5)

| # | Document | Lines | Key Topics |
|---|----------|-------|------------|
| 05 | [Sublinear Mincut Algorithms](05-sublinear-mincut-algorithms.md) | 1,170 | Sublinear approximation, dynamic mincut, streaming algorithms, Benczúr-Karger sparsification, local partitioning, Rust implementation |

### Hardware & Systems (Documents 6, 10)

| # | Document | Lines | Key Topics |
|---|----------|-------|------------|
| 06 | [ESP32 Mesh Hardware Constraints](06-esp32-mesh-hardware-constraints.md) | 1,122 | ESP32 CSI capabilities, 16-node topology, TDM synchronization, computational budget, channel hopping, power analysis, firmware architecture |
| 10 | [System Architecture & Prototype Design](10-system-architecture-prototype.md) | 1,625 | End-to-end pipeline, crate integration, DDD module design, 100ms latency budget, 3-phase prototype, benchmark design, ADR-044, Rust traits |

### Learning & Temporal (Documents 7-8)

| # | Document | Lines | Key Topics |
|---|----------|-------|------------|
| 07 | [Contrastive Learning for RF Coherence](07-contrastive-learning-rf-coherence.md) | 1,226 | SimCLR/MoCo for CSI, AETHER-Topo extension, delta-driven updates, self-supervised pre-training, triplet edge classification, MERIDIAN transfer |
| 08 | [Temporal Graph Evolution & RuVector](08-temporal-graph-evolution-ruvector.md) | 1,528 | TGN/TGAT/DyRep, RuVector graph memory, cut trajectory tracking, event detection, compressed storage, cross-room transitions, drift detection |

### Analysis (Document 9)

| # | Document | Lines | Key Topics |
|---|----------|-------|------------|
| 09 | [Resolution & Spatial Granularity](09-resolution-spatial-granularity.md) | 1,383 | Fresnel zone analysis, node density vs resolution, Cramér-Rao bounds, graph cut resolution theory, multi-frequency enhancement, scaling laws |

### Quantum Sensing (Documents 11-12)

| # | Document | Lines | Key Topics |
|---|----------|-------|------------|
| 11 | [Quantum-Level Sensors](11-quantum-level-sensors.md) | 934 | NV centers, Rydberg atoms, SQUIDs, quantum illumination, quantum graph algorithms, hybrid architecture, quantum ML, NISQ applications |
| 12 | [Quantum Biomedical Sensing](12-quantum-biomedical-sensing.md) | 1,157 | Biomagnetic mapping, neural field imaging, circulation sensing, coherence diagnostics, non-contact vitals, ambient health monitoring, BCI |

---

## Key Findings

### Resolution
- 16 ESP32 nodes at 1m spacing → **30-60 cm** spatial granularity
- Dual-band (2.4 + 5 GHz) → **6 cm** theoretical coherent limit
- Information-theoretic limit: **8.8 cm** for dense deployment

### Computational Feasibility
- Stoer-Wagner on 16-node graph: **~2,000 operations** per sweep
- At 20 Hz: **0.07%** of one ESP32 core
- Full pipeline CSI → mincut: **< 100 ms** latency budget

### Quantum Enhancement
- NV diamond: 100-1000× sensitivity improvement at room temperature
- Rydberg atoms: self-calibrated, SI-traceable RF field measurement
- D-Wave quantum annealing: native QUBO solver for graph cuts

### Biomedical Extension
- Non-contact cardiac monitoring at 1-3m with quantum sensors
- Coherence-based diagnostics: disease as topological change in body's EM graph
- Same graph algorithms (mincut, spectral) apply to both room sensing and medical

---

## Proposed ADRs
- **ADR-044**: RF Topological Sensing (Document 10)
- **ADR-045**: Quantum Biomedical Sensing Extension (Document 12)

## Implementation Phases
1. **Phase 1** (4 weeks): 4-node POC — detect person in room
2. **Phase 2** (8 weeks): 16-node room — track movement boundaries < 50 cm
3. **Phase 3** (16 weeks): Multi-room mesh — cross-room transition detection
4. **Phase 4** (2027-2028): Quantum-enhanced — NV diamond + ESP32 hybrid
5. **Phase 5** (2029+): Biomedical — coherence diagnostics, ambient health
