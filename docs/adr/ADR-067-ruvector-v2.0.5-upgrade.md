# ADR-067: RuVector v2.0.4 to v2.0.5 Upgrade + New Crate Adoption

**Status:** Proposed
**Date:** 2026-03-23
**Deciders:** @ruvnet
**Related:** ADR-016 (RuVector training pipeline integration), ADR-017 (RuVector signal + MAT integration), ADR-029 (RuvSense multistatic sensing)

## Context

RuView currently pins all five core RuVector crates at **v2.0.4** (from crates.io) plus a vendored `ruvector-crv` v0.1.1 and optional `ruvector-gnn` v2.0.5. The upstream RuVector workspace has moved to **v2.0.5** with meaningful improvements to the crates we depend on, and has introduced new crates that could benefit RuView's detection pipeline.

### Current Integration Map

| RuView Module | RuVector Crate | Current Version | Purpose |
|---------------|----------------|-----------------|---------|
| `signal/subcarrier.rs` | ruvector-mincut | 2.0.4 | Graph min-cut subcarrier partitioning |
| `signal/spectrogram.rs` | ruvector-attn-mincut | 2.0.4 | Attention-gated spectrogram denoising |
| `signal/bvp.rs` | ruvector-attention | 2.0.4 | Attention-weighted BVP aggregation |
| `signal/fresnel.rs` | ruvector-solver | 2.0.4 | Fresnel geometry estimation |
| `mat/triangulation.rs` | ruvector-solver | 2.0.4 | TDoA survivor localization |
| `mat/breathing.rs` | ruvector-temporal-tensor | 2.0.4 | Tiered compressed breathing buffer |
| `mat/heartbeat.rs` | ruvector-temporal-tensor | 2.0.4 | Tiered compressed heartbeat spectrogram |
| `viewpoint/*` (4 files) | ruvector-attention | 2.0.4 | Cross-viewpoint fusion with geometric bias |
| `crv/` (optional) | ruvector-crv | 0.1.1 (vendored) | CRV protocol integration |
| `crv/` (optional) | ruvector-gnn | 2.0.5 | GNN graph topology |

### What Changed Upstream (v2.0.4 → v2.0.5 → HEAD)

**ruvector-mincut:**
- Flat capacity matrix + allocation reuse - **10-30% faster** for all min-cut operations
- Tier 2-3 Dynamic MinCut (ADR-124): Gomory-Hu tree construction for fast global min-cut, incremental edge insert/delete without full recomputation
- Source-anchored canonical min-cut with SHA-256 witness hashing
- Fixed: unsafe indexing removed, WASM Node.js panic from `std::time`

**ruvector-attention / ruvector-attn-mincut:**
- Migrated to workspace versioning (no API changes)
- Documentation improvements

**ruvector-temporal-tensor:**
- Formatting fixes only (no API changes)

**ruvector-gnn:**
- Panic replaced with `Result` in `MultiHeadAttention` and `RuvectorLayer` constructors (breaking improvement - safer)
- Bumped to v2.0.5

**sona (new - Self-Optimizing Neural Architecture):**
- v0.1.6 → v0.1.8: state persistence (`loadState`/`saveState`), trajectory counter fix
- Micro-LoRA and Base-LoRA for instant and background learning
- EWC++ (Elastic Weight Consolidation) to prevent catastrophic forgetting
- ReasoningBank pattern extraction and similarity search
- WASM support for edge devices

**ruvector-coherence (new):**
- Spectral coherence scoring for graph index health
- Fiedler eigenvalue estimation, effective resistance sampling
- HNSW health monitoring with alerts
- Batch evaluation of attention mechanism quality

**ruvector-core (new):**
- ONNX embedding support for real semantic embeddings
- HNSW index with SIMD-accelerated distance metrics
- Quantization (4-32x memory reduction)
- Arena allocator for cache-optimized operations

## Decision

### Phase 1: Version Bump (Low Risk)

Bump the 5 core crates from v2.0.4 to v2.0.5 in the workspace `Cargo.toml`:

```toml
ruvector-mincut = "2.0.5"        # was 2.0.4 - 10-30% faster, safer
ruvector-attn-mincut = "2.0.5"   # was 2.0.4 - workspace versioning
ruvector-temporal-tensor = "2.0.5" # was 2.0.4 - fmt only
ruvector-solver = "2.0.5"        # was 2.0.4 - workspace versioning
ruvector-attention = "2.0.5"     # was 2.0.4 - workspace versioning
```

**Expected impact:** The mincut performance improvement directly benefits `signal/subcarrier.rs` which runs subcarrier graph partitioning every tick. 10-30% faster partitioning reduces per-frame CPU cost.

### Phase 2: Add ruvector-coherence (Medium Value)

Add `ruvector-coherence` with `spectral` feature to `wifi-densepose-ruvector`:

**Use case:** Replace or augment the custom phase coherence logic in `viewpoint/coherence.rs` with spectral graph coherence scoring. The current implementation uses phasor magnitude for phase coherence - spectral Fiedler estimation would provide a more robust measure of multi-node CSI consistency, especially for detecting when a node's signal quality degrades.

**Integration point:** `viewpoint/coherence.rs` - add `SpectralCoherenceScore` as a secondary coherence metric alongside existing phase phasor coherence. Use spectral gap estimation to detect structural changes in the multi-node CSI graph (e.g., a node dropping out or a new reflector appearing).

### Phase 3: Add SONA for Adaptive Learning (High Value)

Replace the logistic regression adaptive classifier in the sensing server with a SONA-backed learning engine:

**Current state:** The sensing server's adaptive training (`POST /api/v1/adaptive/train`) uses a hand-rolled logistic regression on 15 CSI features. It requires explicit labeled recordings and provides no cross-session persistence.

**Proposed improvement:** Use `sona::SonaEngine` to:
1. **Learn from implicit feedback** - trajectory tracking on person-count decisions (was the count stable? did the user correct it?)
2. **Persist across sessions** - `saveState()`/`loadState()` replaces the current `adaptive_model.json`
3. **Pattern matching** - `find_patterns()` enables "this CSI signature looks like room X where we learned Y"
4. **Prevent forgetting** - EWC++ ensures learning in a new room doesn't overwrite patterns from previous rooms

**Integration point:** New `adaptive_sona.rs` module in `wifi-densepose-sensing-server`, behind a `sona` feature flag. The existing logistic regression remains the default.

### Phase 4: Evaluate ruvector-core for CSI Embeddings (Exploratory)

**Current state:** The person detection pipeline uses hand-crafted features (variance, change_points, motion_band_power, spectral_power) with fixed normalization ranges.

**Potential:** Use `ruvector-core`'s ONNX embedding support to generate learned CSI embeddings that capture room geometry, person count, and activity patterns in a single vector. This would enable:
- Similarity search: "is this CSI frame similar to known 2-person patterns?"
- Transfer learning: embeddings learned in one room partially transfer to similar rooms
- Quantized storage: 4-32x memory reduction for pattern databases

**Status:** Exploratory - requires training data collection and embedding model design. Not a near-term target.

## Consequences

### Positive
- **Phase 1:** Free 10-30% performance gain in subcarrier partitioning. Security fixes (unsafe indexing, WASM panic). Zero API changes required.
- **Phase 2:** More robust multi-node coherence detection. Helps with the "flickering persons" issue (#292) by providing a second opinion on signal quality.
- **Phase 3:** Fundamentally improves the adaptive learning pipeline. Users no longer need to manually record labeled data - the system learns from ongoing use.
- **Phase 4:** Path toward real ML-based detection instead of heuristic thresholds.

### Negative
- **Phase 1:** Minimal risk - semver minor bump, no API breaks.
- **Phase 2:** Adds a dependency. Spectral computation has O(n) cost per tick for Fiedler estimation (n = number of subcarriers, typically 56-128). Acceptable.
- **Phase 3:** SONA adds ~200KB to the binary. The learning loop needs careful tuning to avoid adapting to noise.
- **Phase 4:** Requires significant research and training data. Not guaranteed to outperform tuned heuristics for WiFi CSI.

### Risks
- `ruvector-gnn` v2.0.5 changed constructors from panic to `Result` - any existing `crv` feature users need to handle the `Result`. Our vendored `ruvector-crv` may need updates.
- SONA's WASM support is experimental - keep it behind a feature flag until validated.

## Implementation Plan

| Phase | Scope | Effort | Priority |
|-------|-------|--------|----------|
| 1 | Bump 5 crates to v2.0.5 | 1 hour | High - free perf + security |
| 2 | Add ruvector-coherence | 1 day | Medium - improves multi-node stability |
| 3 | SONA adaptive learning | 3 days | Medium - replaces manual training workflow |
| 4 | CSI embeddings via ruvector-core | 1-2 weeks | Low - exploratory research |

## Vendor Submodule

The `vendor/ruvector` git submodule has been updated from commit `f8f2c60` (v2.0.4 era) to `51a3557` (latest `origin/main`). This provides local reference for the full upstream source when developing Phases 2-4.

## References

- Upstream repo: https://github.com/ruvnet/ruvector
- ADR-124 (Dynamic MinCut): `vendor/ruvector/docs/adr/ADR-124*.md`
- SONA docs: `vendor/ruvector/crates/sona/src/lib.rs`
- ruvector-coherence spectral: `vendor/ruvector/crates/ruvector-coherence/src/spectral.rs`
- ruvector-core embeddings: `vendor/ruvector/crates/ruvector-core/src/embeddings.rs`
