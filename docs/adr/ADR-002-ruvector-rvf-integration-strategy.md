# ADR-002: RuVector RVF Integration Strategy

## Status
Superseded by [ADR-016](ADR-016-ruvector-integration.md) and [ADR-017](ADR-017-ruvector-signal-mat-integration.md)

> **Note:** The vision in this ADR has been fully realized. ADR-016 integrates all 5 RuVector crates into the training pipeline. ADR-017 adds 7 signal + MAT integration points. The `wifi-densepose-ruvector` crate is [published on crates.io](https://crates.io/crates/wifi-densepose-ruvector). See also [ADR-027](ADR-027-cross-environment-domain-generalization.md) for how RuVector is extended with domain generalization.

## Date
2026-02-28

## Context

### Current System Limitations

The WiFi-DensePose system processes Channel State Information (CSI) from WiFi signals to estimate human body poses. The current architecture (Python v1 + Rust port) has several areas where intelligence and performance could be significantly improved:

1. **No persistent vector storage**: CSI feature vectors are processed transiently. Historical patterns, fingerprints, and learned representations are not persisted in a searchable vector database.

2. **Static inference models**: The modality translation network (`ModalityTranslationNetwork`) and DensePose head use fixed weights loaded at startup. There is no online learning, adaptation, or self-optimization.

3. **Naive pattern matching**: Human detection in `CSIProcessor` uses simple threshold-based confidence scoring (`amplitude_indicator`, `phase_indicator`, `motion_indicator` with fixed weights 0.4, 0.3, 0.3). No similarity search against known patterns.

4. **No cryptographic audit trail**: Life-critical disaster detection (wifi-densepose-mat) lacks tamper-evident logging for survivor detections and triage classifications.

5. **Limited edge deployment**: The WASM crate (`wifi-densepose-wasm`) provides basic bindings but lacks a self-contained runtime capable of offline operation with embedded models.

6. **Single-node architecture**: Multi-AP deployments for disaster scenarios require distributed coordination, but no consensus mechanism exists for cross-node state management.

### RuVector Capabilities

RuVector (github.com/ruvnet/ruvector) provides a comprehensive cognitive computing platform:

- **RVF (Cognitive Containers)**: Self-contained files with 25 segment types (VEC, INDEX, KERNEL, EBPF, WASM, COW_MAP, WITNESS, CRYPTO) that package vectors, models, and runtime into a single deployable artifact
- **HNSW Vector Search**: Hierarchical Navigable Small World indexing with SIMD acceleration and Hyperbolic extensions for hierarchy-aware search
- **SONA**: Self-Optimizing Neural Architecture providing <1ms adaptation via LoRA fine-tuning with EWC++ memory preservation
- **GNN Learning Layer**: Graph Neural Networks that learn from every query through message passing, attention weighting, and representation updates
- **46 Attention Mechanisms**: Including Flash Attention, Linear Attention, Graph Attention, Hyperbolic Attention, Mincut-gated Attention
- **Post-Quantum Cryptography**: ML-DSA-65, Ed25519, SLH-DSA-128s signatures with SHAKE-256 hashing
- **Witness Chains**: Tamper-evident cryptographic hash-linked audit trails
- **Raft Consensus**: Distributed coordination with multi-master replication and vector clocks
- **WASM Runtime**: 5.5 KB runtime bootable in 125ms, deployable on servers, browsers, phones, IoT
- **Git-like Branching**: Copy-on-write structure (1M vectors + 100 edits ≈ 2.5 MB branch)

## Decision

We will integrate RuVector's RVF format and intelligence capabilities into the WiFi-DensePose system through a phased, modular approach across 9 integration domains, each detailed in subsequent ADRs (ADR-003 through ADR-010).

### Integration Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        WiFi-DensePose + RuVector                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │   CSI Input   │  │  RVF Store   │  │    SONA      │  │   GNN Layer  │   │
│  │   Pipeline    │──▶│  (Vectors,  │──▶│  Self-Learn  │──▶│  Pattern     │   │
│  │              │  │   Indices)   │  │              │  │  Enhancement │   │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘   │
│         │                 │                 │                 │            │
│         ▼                 ▼                 ▼                 ▼            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐   │
│  │  Feature     │  │   HNSW       │  │  Adaptive    │  │   Pose       │   │
│  │  Extraction  │  │   Search     │  │  Weights     │  │  Estimation  │   │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘   │
│         │                 │                 │                 │            │
│         └─────────────────┴─────────────────┴─────────────────┘            │
│                                     │                                      │
│                          ┌──────────▼──────────┐                          │
│                          │    Output Layer      │                          │
│                          │  • Pose Keypoints    │                          │
│                          │  • Body Segments     │                          │
│                          │  • UV Coordinates    │                          │
│                          │  • Confidence Maps   │                          │
│                          └──────────┬──────────┘                          │
│                                     │                                      │
│         ┌───────────────────────────┼───────────────────────────┐          │
│         ▼                           ▼                           ▼          │
│  ┌──────────────┐           ┌──────────────┐           ┌──────────────┐   │
│  │  Witness     │           │    Raft       │           │   WASM       │   │
│  │  Chains      │           │  Consensus    │           │   Edge       │   │
│  │  (Audit)     │           │  (Multi-AP)   │           │  Runtime     │   │
│  └──────────────┘           └──────────────┘           └──────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                  Post-Quantum Crypto Layer                          │   │
│  │          ML-DSA-65 │ Ed25519 │ SLH-DSA-128s │ SHAKE-256           │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

### New Crate: `wifi-densepose-rvf`

A new workspace member crate will serve as the integration layer:

```
crates/wifi-densepose-rvf/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Public API surface
│   ├── container.rs           # RVF cognitive container management
│   ├── vector_store.rs        # HNSW-backed CSI vector storage
│   ├── search.rs              # Similarity search for fingerprinting
│   ├── learning.rs            # SONA integration for online learning
│   ├── gnn.rs                 # GNN pattern enhancement layer
│   ├── attention.rs           # Attention mechanism selection
│   ├── witness.rs             # Witness chain audit trails
│   ├── consensus.rs           # Raft consensus for multi-AP
│   ├── crypto.rs              # Post-quantum crypto wrappers
│   ├── edge.rs                # WASM edge runtime integration
│   └── adapters/
│       ├── mod.rs
│       ├── signal_adapter.rs  # Bridges wifi-densepose-signal
│       ├── nn_adapter.rs      # Bridges wifi-densepose-nn
│       └── mat_adapter.rs     # Bridges wifi-densepose-mat
```

### Phased Rollout

| Phase | Timeline | ADR | Capability | Priority |
|-------|----------|-----|------------|----------|
| 1 | Weeks 1-3 | ADR-003 | RVF Cognitive Containers for CSI Data | Critical |
| 2 | Weeks 2-4 | ADR-004 | HNSW Vector Search for Signal Fingerprinting | Critical |
| 3 | Weeks 4-6 | ADR-005 | SONA Self-Learning for Pose Estimation | High |
| 4 | Weeks 5-7 | ADR-006 | GNN-Enhanced CSI Pattern Recognition | High |
| 5 | Weeks 6-8 | ADR-007 | Post-Quantum Cryptography for Secure Sensing | Medium |
| 6 | Weeks 7-9 | ADR-008 | Distributed Consensus for Multi-AP | Medium |
| 7 | Weeks 8-10 | ADR-009 | RVF WASM Runtime for Edge Deployment | Medium |
| 8 | Weeks 9-11 | ADR-010 | Witness Chains for Audit Trail Integrity | High (MAT) |

### Dependency Strategy

**Verified published crates** (crates.io, all at v2.0.4 as of 2026-02-28):

```toml
# In Cargo.toml workspace dependencies
[workspace.dependencies]
ruvector-mincut = "2.0.4"           # Dynamic min-cut, O(n^1.5 log n) graph partitioning
ruvector-attn-mincut = "2.0.4"     # Attention + mincut gating in one pass
ruvector-temporal-tensor = "2.0.4"  # Tiered temporal compression (50-75% memory reduction)
ruvector-solver = "2.0.4"           # NeumannSolver — O(√n) Neumann series convergence
ruvector-attention = "2.0.4"        # ScaledDotProductAttention
```

> **Note (ADR-017 correction):** Earlier versions of this ADR specified
> `ruvector-core`, `ruvector-data-framework`, `ruvector-consensus`, and
> `ruvector-wasm` at version `"0.1"`. These crates do not exist at crates.io.
> The five crates above are the verified published API surface at v2.0.4.
> Capabilities such as RVF cognitive containers (ADR-003), HNSW search (ADR-004),
> SONA (ADR-005), GNN patterns (ADR-006), post-quantum crypto (ADR-007),
> Raft consensus (ADR-008), and WASM runtime (ADR-009) are internal capabilities
> accessible through these five crates or remain as forward-looking architecture.
> See ADR-017 for the corrected integration map.

Feature flags control which ruvector capabilities are compiled in:

```toml
[features]
default = ["mincut-matching", "solver-interpolation"]
mincut-matching = ["ruvector-mincut"]
attn-mincut = ["ruvector-attn-mincut"]
temporal-compress = ["ruvector-temporal-tensor"]
solver-interpolation = ["ruvector-solver"]
attention = ["ruvector-attention"]
full = ["mincut-matching", "attn-mincut", "temporal-compress", "solver-interpolation", "attention"]
```

## Consequences

### Positive

- **10-100x faster pattern lookup**: HNSW replaces linear scan for CSI fingerprint matching
- **Continuous improvement**: SONA enables online adaptation without full retraining
- **Self-contained deployment**: RVF containers package everything needed for field operation
- **Tamper-evident records**: Witness chains provide cryptographic proof for disaster response auditing
- **Future-proof security**: Post-quantum signatures resist quantum computing attacks
- **Distributed operation**: Raft consensus enables coordinated multi-AP sensing
- **Ultra-light edge**: 5.5 KB WASM runtime enables browser and IoT deployment
- **Git-like versioning**: COW branching enables experimental model variations with minimal storage

### Negative

- **Increased binary size**: Full feature set adds significant dependencies (~15-30 MB)
- **Complexity**: 9 integration domains require careful coordination
- **Learning curve**: Team must understand RuVector's cognitive container paradigm
- **API stability risk**: RuVector is pre-1.0; APIs may change
- **Testing surface**: Each integration point requires dedicated test suites

### Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| RuVector API breaking changes | High | Pin versions, adapter pattern isolates impact |
| Performance regression from abstraction layers | Medium | Benchmark each integration point, zero-cost abstractions |
| Feature flag combinatorial complexity | Medium | CI matrix testing for key feature combinations |
| Over-engineering for current use cases | Medium | Phased rollout, each phase independently valuable |
| Binary size bloat for edge targets | Low | Feature flags ensure only needed capabilities compile |

## Related ADRs

- **ADR-001**: WiFi-Mat Disaster Detection Architecture (existing)
- **ADR-003**: RVF Cognitive Containers for CSI Data
- **ADR-004**: HNSW Vector Search for Signal Fingerprinting
- **ADR-005**: SONA Self-Learning for Pose Estimation
- **ADR-006**: GNN-Enhanced CSI Pattern Recognition
- **ADR-007**: Post-Quantum Cryptography for Secure Sensing
- **ADR-008**: Distributed Consensus for Multi-AP Coordination
- **ADR-009**: RVF WASM Runtime for Edge Deployment
- **ADR-010**: Witness Chains for Audit Trail Integrity

## References

- [RuVector Repository](https://github.com/ruvnet/ruvector)
- [HNSW Algorithm](https://arxiv.org/abs/1603.09320)
- [LoRA: Low-Rank Adaptation](https://arxiv.org/abs/2106.09685)
- [Elastic Weight Consolidation](https://arxiv.org/abs/1612.00796)
- [Raft Consensus](https://raft.github.io/raft.pdf)
- [ML-DSA (FIPS 204)](https://csrc.nist.gov/pubs/fips/204/final)
- [WiFi-DensePose Rust ADR-001: Workspace Structure](../rust-port/wifi-densepose-rs/docs/adr/ADR-001-workspace-structure.md)
