# ADR-040: Causal Atlas RVF Runtime — Planet Detection & Life Candidate Scoring

**Status:** Proposed
**Date:** 2026-02-18
**Author:** System Architect (AgentDB v3)
**Supersedes:** None
**Related:** ADR-003 (RVF Format), ADR-006 (Unified Self-Learning RVF), ADR-007 (Full Capability Integration), ADR-008 (Chat UI RVF)
**Package:** `@agentdb/causal-atlas`

## Context

ADR-008 demonstrated that a single RVF artifact can embed a minimal Linux
userspace, an LLM inference engine, and a self-learning pipeline into one
portable file. This ADR extends that pattern to scientific computing: a
portable RVF runtime that ingests public astronomy and physics datasets,
builds a multi-scale interaction graph, maintains a dynamic coherence field,
and emits replayable witness logs for every derived claim.

The design draws engineering inspiration from causal sets, loop-gravity-style
discretization, and holographic boundary encoding, but it is implemented as a
practical data system, not a physics simulator. The holographic principle
manifests as a concrete design choice: primarily store and index boundaries,
and treat interior state as reconstructable from boundary witnesses and
retained archetypes.

### Existing Capabilities (ADR-003 through ADR-008)

| Component | Package | Relevant APIs |
|-----------|---------|---------------|
| **RVF segments** | `@ruvector/rvf`, `@ruvector/rvf-node` | `embedKernel`, `extractKernel`, `embedEbpf`, `segments`, `derive` |
| **HNSW indexing** | `@ruvector/rvf-node` | `ingestBatch`, `query`, `compact`, HNSW with metadata filters |
| **Witness chains** | `@ruvector/rvf-node`, `RvfSolver` | `verifyWitness`, SHAKE-256 witness chains, signed root hash |
| **Graph transactions** | `NativeAccelerator` | `graphTransaction`, `graphBatchInsert`, Cypher queries |
| **SIMD embeddings** | `@ruvector/ruvllm` | 768-dim SIMD embed, cosine/dot/L2, HNSW memory search |
| **SONA learning** | `SonaLearningBackend` | Micro-LoRA, trajectory recording, EWC++ |
| **Federated coordination** | `FederatedSessionManager` | Cross-agent trajectories, warm-start patterns |
| **Contrastive training** | `ContrastiveTrainer` | InfoNCE, hard negative mining, 3-stage curriculum |
| **Adaptive index** | `AdaptiveIndexTuner` | 5-tier compression, Matryoshka truncation, health monitoring |
| **Kernel embedding** | `KernelBuilder` (ADR-008) | Minimal Linux boot from KERNEL_SEG + INITRD_SEG |
| **Lazy model download** | `ChatInference` (ADR-008) | Deferred GGUF load on first inference call |

### What This ADR Adds

1. Domain adapters for astronomy data (light curves, spectra, galaxy catalogs)
2. Compressed causal atlas with partial-order event graph
3. Coherence field index with cut pressure and partition entropy
4. Multi-scale interaction memory with budget-controlled tiered retention
5. Boundary evolution tracker with holographic-style boundary-first storage
6. Planet detection pipeline (Kepler/TESS transit search)
7. Life candidate scoring pipeline (spectral disequilibrium signatures)
8. Progressive data download from public sources on first activation

## Goal State

A single RVF artifact that boots a minimal Linux userspace, progressively
downloads and ingests public astronomy and physics datasets on first
activation (lazy, like ADR-008's GGUF model download), builds a multi-scale
interaction graph, maintains a dynamic coherence field, and emits replayable
witness logs for every derived claim.

### Primary Outputs

| # | Output | Description |
|---|--------|-------------|
| 1 | **Atlas snapshots** | Queryable causal partial order plus embeddings |
| 2 | **Coherence field** | Partition tree plus cut pressure signals over time |
| 3 | **Multi-scale memory** | Delta-encoded interaction history from seconds to micro-windows |
| 4 | **Boundary tracker** | Boundary changes, drift, and anomaly alerts |
| 5 | **Planet candidates** | Ranked list with traceable evidence |
| 6 | **Life candidates** | Ranked list of spectral disequilibrium signatures with traceable evidence |

### Non-Goals

1. Proving quantum gravity
2. Replacing astrophysical pipelines end-to-end
3. Claiming life detection without conventional follow-up observation

## Public Data Sources

All data is progressively downloaded from public archives on first activation.
The RVF artifact ships with download manifests and integrity hashes, not the
raw data itself.

### Planet Finding

| Source | Access | Reference |
|--------|--------|-----------|
| Kepler light curves and pixel files | MAST bulk and portal | [archive.stsci.edu/kepler](https://archive.stsci.edu/missions-and-data/kepler) |
| TESS light curves and full-frame images | MAST portal | [archive.stsci.edu/tess](https://archive.stsci.edu/missions-and-data/tess) |

### Life-Relevant Spectra

| Source | Access | Reference |
|--------|--------|-----------|
| JWST exoplanet spectra | exo.MAST and MAST holdings | [archive.stsci.edu](https://archive.stsci.edu/home) |
| NASA Exoplanet Archive parameters | Cross-linking to spectra and mission products | [exoplanetarchive.ipac.caltech.edu](https://exoplanetarchive.ipac.caltech.edu/) |

### Large-Scale Structure

| Source | Access | Reference |
|--------|--------|-----------|
| SDSS public catalogs (spectra, redshifts) | DR17 | [sdss4.org/dr17](https://www.sdss4.org/dr17/) |

### Progressive Download Strategy

Following the lazy-download pattern established in ADR-008 for GGUF models:

1. **Manifest-first**: RVF ships with `MANIFEST_SEG` containing download URLs,
   SHA-256 hashes, expected sizes, and priority tiers
2. **Tier 0 (boot)**: Minimal curated dataset (~50 MB) for offline demo —
   100 Kepler targets with known confirmed planets, embedded in VEC_SEG
3. **Tier 1 (first run)**: Download 1,000 Kepler targets on first pipeline
   activation. Background download, progress reported via CLI/HTTP
4. **Tier 2 (expansion)**: Full Kepler/TESS catalog download on explicit
   `rvf ingest --expand` command
5. **Tier 3 (spectra)**: JWST and archive spectra downloaded when life
   candidate pipeline is first activated
6. **Seal-on-complete**: After download, data is ingested into VEC_SEG and
   INDEX_SEG, a new witness root is committed, and the RVF is sealed into
   a reproducible snapshot

```
Download state machine:

  [boot] ──first-inference──> [downloading-tier-1]
           │                        │
           │ (offline demo works)   │ (progress: 0-100%)
           │                        │
           ▼                        ▼
  [tier-0-only]              [tier-1-ready]
                                    │
                         rvf ingest --expand
                                    │
                                    ▼
                             [tier-2-ready]
                                    │
                         life pipeline activated
                                    │
                                    ▼
                             [tier-3-ready] ──seal──> [sealed-snapshot]
```

Each tier download:
- Resumes from last byte on interruption (HTTP Range headers)
- Validates SHA-256 after download
- Commits a witness record for the download event
- Can be skipped with `--offline` flag (uses whatever is already present)

## RVF Artifact Layout

Extends the ADR-003 segment model with domain-specific segments.

| # | Segment | Contents |
|---|---------|----------|
| 1 | `MANIFEST_SEG` | Segment table, hashes, policy, budgets, version gates, **download manifests** |
| 2 | `KERNEL_SEG` | Minimal Linux kernel image for portable boot (reuse ADR-008) |
| 3 | `INITRD_SEG` | Minimal userspace: busybox, RuVector binaries, data ingest tools, query server |
| 4 | `EBPF_SEG` | Socket allow-list and syscall reduction. Default: local loopback + explicit download ports only |
| 5 | `VEC_SEG` | Embedding vectors: light-curve windows, spectrum windows, graph node descriptors, partition boundary descriptors |
| 6 | `INDEX_SEG` | HNSW unified attention index for vectors and boundary descriptors |
| 7 | `GRAPH_SEG` | Dynamic interaction graph: nodes, edges, timestamps, authority, provenance |
| 8 | `DELTA_SEG` | Append-only change log of graph updates and field updates |
| 9 | `WITNESS_SEG` | Deterministic witness chain: canonical serialization, signed root hash progression |
| 10 | `POLICY_SEG` | Data provenance requirements, candidate publishing thresholds, deny rules, confidence floors |
| 11 | `DASHBOARD_SEG` | Vite-bundled Three.js visualization app — static assets served by runtime HTTP server |

## Data Model

### Core Entities

```typescript
interface Event {
  id: string;
  t_start: number;          // epoch seconds
  t_end: number;
  domain: 'kepler' | 'tess' | 'jwst' | 'sdss' | 'derived';
  payload_hash: string;      // SHA-256 of raw data window
  provenance: Provenance;
}

interface Observation {
  id: string;
  instrument: string;        // 'kepler-lc' | 'tess-ffi' | 'jwst-nirspec' | ...
  target_id: string;         // e.g., KIC or TIC identifier
  data_pointer: string;      // segment offset into VEC_SEG
  calibration_version: string;
  provenance: Provenance;
}

interface InteractionEdge {
  src_event_id: string;
  dst_event_id: string;
  type: 'causal' | 'periodicity' | 'shape_similarity' | 'co_occurrence' | 'spatial';
  weight: number;
  lag: number;               // temporal lag in seconds
  confidence: number;
  provenance: Provenance;
}

interface Boundary {
  boundary_id: string;
  partition_left_set_hash: string;
  partition_right_set_hash: string;
  cut_weight: number;
  cut_witness: string;       // witness chain reference
  stability_score: number;
}

interface Candidate {
  candidate_id: string;
  category: 'planet' | 'life';
  evidence_pointers: string[];   // event and edge IDs
  score: number;
  uncertainty: number;
  publishable: boolean;          // based on POLICY_SEG rules
  witness_trace: string;         // WITNESS_SEG reference for replay
}

interface Provenance {
  source: string;            // 'mast-kepler' | 'mast-tess' | 'mast-jwst' | ...
  download_witness: string;  // witness chain entry for the download
  transform_chain: string[]; // ordered list of transform IDs applied
  timestamp: string;         // ISO-8601
}
```

### Domain Adapters

#### Planet Transit Adapter

```
Input:  flux time series + cadence metadata (Kepler/TESS FITS)
Output: Event nodes for windows
        InteractionEdges for periodicity hints and shape similarity
        Candidate nodes for dip detections
```

#### Spectrum Adapter

```
Input:  wavelength, flux, error arrays (JWST NIRSpec, etc.)
Output: Event nodes for band windows
        InteractionEdges for molecule feature co-occurrence
        Disequilibrium score components
```

#### Cosmic Web Adapter (optional, Phase 2+)

```
Input:  galaxy positions and redshifts (SDSS)
Output: Graph of spatial adjacency and filament membership
```

## The Four System Constructs

### 1. Compressed Causal Atlas

**Definition**: A partial order of events plus minimal sufficient descriptors
to reproduce derived edges.

**Construction**:

1. **Windowing** — Light curves into overlapping windows at multiple scales
   - Scales: 2 hours, 12 hours, 3 days, 27 days

2. **Feature extraction** — Robust features per window
   - Flux derivative statistics
   - Autocorrelation peaks
   - Wavelet energy bands
   - Transit-shaped matched filter response

3. **Embedding** — RuVector SIMD embed per window, stored in VEC_SEG

4. **Causal edges** — Add edge when window A precedes window B and improves
   predictability of B (conditional mutual information proxy or prediction gain,
   subject to POLICY_SEG constraints)
   - Edge weight: prediction gain magnitude
   - Provenance: exact windows, transform IDs, threshold used

5. **Atlas compression**
   - Keep only top-k causal parents per node
   - Retain stable boundary witnesses
   - Delta-encode updates into DELTA_SEG

**Output API**:

| Endpoint | Returns |
|----------|---------|
| `atlas.query(event_id)` | Parents, children, plus provenance |
| `atlas.trace(candidate_id)` | Minimal causal chain for a candidate |

### 2. Coherence Field Index

**Definition**: A field over the atlas graph that assigns coherence pressure
and cut stability over time.

**Signals**:

| Signal | Description |
|--------|-------------|
| Cut pressure | Minimum cut values over selected subgraphs |
| Partition entropy | Distribution of cluster sizes and churn rate |
| Disagreement | Cross-detector disagreement rate |
| Drift | Embedding distribution shift in sliding window |

**Algorithm**:

1. Maintain a partition tree. Update with dynamic min-cut on incremental
   graph changes
2. For each update epoch:
   - Compute cut witnesses for top boundaries
   - Emit boundary events into GRAPH_SEG
   - Append witness record into WITNESS_SEG
3. Index boundaries via descriptor vector:
   - Cut value, partition sizes, local graph curvature proxy, recent churn

**Query API**:

| Endpoint | Returns |
|----------|---------|
| `coherence.get(target_id, epoch)` | Field values for target at epoch |
| `boundary.nearest(descriptor)` | Similar historical boundary states via INDEX_SEG |

### 3. Multi-Scale Interaction Memory

**Definition**: A memory that retains interactions at multiple time resolutions
with strict budget control.

**Three tiers**:

| Tier | Resolution | Content |
|------|-----------|---------|
| **S** | Seconds to minutes | High-fidelity deltas |
| **M** | Hours to days | Aggregated deltas |
| **L** | Weeks to months | Boundary summaries and archetypes |

**Retention rules**:
1. Preserve events that are boundary-critical
2. Preserve events that are candidate evidence
3. Compress everything else via archetype clustering in INDEX_SEG

**Mechanism**:
- DELTA_SEG is append-only
- Periodic compaction produces a new RVF root with a witness proof of
  preservation rules applied

### 4. Boundary Evolution Tracker

**Definition**: A tracker that treats boundaries as primary objects that evolve
over time.

**This is where the holographic flavor is implemented.** You primarily store
and index boundaries, and treat interior state as reconstructable from boundary
witnesses and retained archetypes.

**Output API**:

| Endpoint | Returns |
|----------|---------|
| `boundary.timeline(target_id)` | Boundary evolution over time |
| `boundary.alerts` | Alerts when: cut pressure spikes, boundary identity flips, disagreement exceeds threshold, drift persists beyond policy |

## Planet Detection Pipeline

### Stage P0: Ingest

**Input**: Kepler or TESS light curves from MAST (progressively downloaded)

1. Normalize flux
2. Remove obvious systematics (detrending)
3. Segment into windows and store as Event nodes

### Stage P1: Candidate Generation

1. Matched filter bank for transit-like dips
2. Period search on candidate dip times (BLS or similar)
3. Create Candidate node per period hypothesis

### Stage P2: Coherence Gating

Candidate must pass all gates:

| Gate | Requirement |
|------|-------------|
| Multi-scale stability | Stable across multiple window scales |
| Boundary consistency | Consistent boundary signature around transit times |
| Low drift | Drift below threshold across adjacent windows |

**Score components**:

| Component | Description |
|-----------|-------------|
| SNR-like strength | Signal-to-noise of transit dip |
| Shape consistency | Cross-transit shape agreement |
| Period stability | Variance of period estimates |
| Coherence stability | Coherence field stability around candidate |

**Emit**: Candidate with evidence pointers + witness trace listing exact
windows, transforms, and thresholds used.

## Life Candidate Pipeline

Life detection here means pre-screening for non-equilibrium atmospheric
chemistry signatures, not proof.

### Stage L0: Ingest

**Input**: Published or mission spectra tied to targets via MAST and NASA
Exoplanet Archive (progressively downloaded on first pipeline activation)

1. Normalize and denoise within instrument error model
2. Window spectra by wavelength bands
3. Create band Event nodes

### Stage L1: Feature Extraction

1. Identify absorption features and confidence bands
2. Encode presence vectors for key molecule families (H2O, CO2, CH4, O3, NH3, etc.)
3. Build InteractionEdges between features that co-occur in physically
   meaningful patterns

### Stage L2: Disequilibrium Scoring

**Core concept**: Life-like systems maintain chemical ratios that resist
thermodynamic relaxation.

**Implementation as graph scoring**:

1. Build a reaction plausibility graph (prior rule set in POLICY_SEG)
2. Compute inconsistency score between observed co-occurrences and expected
   equilibrium patterns
3. Track stability of that score across epochs and observation sets

**Score components**:

| Component | Description |
|-----------|-------------|
| Persistent multi-molecule imbalance | Proxy for non-equilibrium chemistry |
| Feature repeatability | Agreement across instruments or visits |
| Contamination risk penalty | Instrument artifact and stellar contamination |
| Stellar activity confound penalty | Host star variability coupling |

**Output**: Life candidate list with explicit uncertainty + required follow-up
observations list generated by POLICY_SEG rules.

## Runtime and Portability

### Boot Sequence

1. RVF boots minimal Linux from KERNEL_SEG and INITRD_SEG (reuse ADR-008 `KernelBuilder`)
2. Starts `rvf-runtime` daemon exposing local HTTP and CLI
3. On first inference/query, progressively downloads required data tier

### Local Interfaces

**CLI**:
```bash
rvf run artifact.rvf                    # boot the runtime
rvf query planet list                   # ranked planet candidates
rvf query life list                     # ranked life candidates
rvf trace <candidate_id>               # full witness trace for any candidate
rvf ingest --expand                     # download tier-2 full catalog
rvf status                              # download progress, segment sizes, witness count
```

**HTTP**:
```
GET /                                   # Three.js dashboard (served from DASHBOARD_SEG)
GET /assets/*                           # Dashboard static assets

GET /api/atlas/query?event_id=...       # causal parents/children
GET /api/atlas/trace?candidate_id=...   # minimal causal chain
GET /api/coherence?target_id=...&epoch= # field values
GET /api/boundary/timeline?target_id=...
GET /api/boundary/alerts
GET /api/candidates/planet              # ranked planet list
GET /api/candidates/life                # ranked life list
GET /api/candidates/:id/trace           # witness trace
GET /api/status                         # system health + download progress
GET /api/memory/tiers                   # tier S/M/L utilization

WS  /ws/live                            # real-time boundary alerts, pipeline progress, candidate updates
```

### Determinism

1. Fixed seeds for all stochastic operations
2. Canonical serialization of every intermediate artifact
3. Witness chain commits after each epoch
4. Two-machine reproducibility: identical RVF root hash for identical input

### Security Defaults

1. Network off by default
2. If enabled, eBPF allow-list: MAST/archive download ports + local loopback only
3. No remote writes without explicit policy toggle in POLICY_SEG
4. Downloaded data verified against MANIFEST_SEG hashes before ingestion

## Three.js Visualization Dashboard

The RVF embeds a Vite-bundled Three.js dashboard in `DASHBOARD_SEG`. The
runtime HTTP server serves it at `/` (root). All visualizations are driven
by the same API endpoints the CLI uses, so every rendered frame corresponds
to queryable, witness-backed data.

### Architecture

```
DASHBOARD_SEG (inside RVF)
  dist/
    index.html            # Vite SPA entry
    assets/
      main.[hash].js      # Three.js + D3 + app logic (tree-shaken)
      main.[hash].css     # Tailwind/minimal styles
      worker.js           # Web Worker for graph layout

Runtime serves:
  GET /                   -> DASHBOARD_SEG/dist/index.html
  GET /assets/*           -> DASHBOARD_SEG/dist/assets/*
  GET /api/*              -> JSON API (atlas, coherence, candidates, etc.)
  WS  /ws/live            -> Live streaming of boundary alerts and pipeline progress
```

**Build pipeline**: Vite builds the dashboard at package time into a single
tree-shaken bundle. The bundle is embedded into `DASHBOARD_SEG` during RVF
assembly. No Node.js required at runtime — the dashboard is pure static
assets served by the existing HTTP server.

### Dashboard Views

#### V1: Causal Atlas Explorer (Three.js 3D)

Interactive 3D force-directed graph of the causal atlas.

| Feature | Implementation |
|---------|---------------|
| **Node rendering** | `THREE.InstancedMesh` for events — color by domain (Kepler=blue, TESS=cyan, JWST=gold, derived=white) |
| **Edge rendering** | `THREE.LineSegments` with opacity mapped to edge weight |
| **Causal flow** | Animated particles along causal edges showing temporal direction |
| **Scale selector** | Toggle between window scales (2h, 12h, 3d, 27d) — re-layouts graph |
| **Candidate highlight** | Click candidate in sidebar to trace its causal chain in 3D, dimming unrelated nodes |
| **Witness replay** | Step through witness chain entries, animating graph state forward/backward |
| **LOD** | Level-of-detail: far=boundary nodes only, mid=top-k events, close=full subgraph |

Data source: `GET /api/atlas/query`, `GET /api/atlas/trace`

#### V2: Coherence Field Heatmap (Three.js + shader)

Real-time coherence field rendered as a colored surface over the atlas graph.

| Feature | Implementation |
|---------|---------------|
| **Field surface** | `THREE.PlaneGeometry` subdivided grid, vertex colors from coherence values |
| **Cut pressure** | Red hotspots where cut pressure is high, cool blue where stable |
| **Partition boundaries** | Glowing wireframe lines at partition cuts |
| **Time scrubber** | Scrub through epochs to see coherence evolution |
| **Drift overlay** | Toggle to show embedding drift as animated vector arrows |
| **Alert markers** | Pulsing icons at boundary alert locations |

Data source: `GET /api/coherence`, `GET /api/boundary/timeline`, `WS /ws/live`

#### V3: Planet Candidate Dashboard (2D panels + 3D orbit)

Split view combining data panels with 3D orbital visualization.

| Panel | Content |
|-------|---------|
| **Ranked list** | Sortable table: candidate ID, score, uncertainty, period, SNR, publishable status |
| **Light curve viewer** | Interactive D3 chart: raw flux, detrended flux, transit model overlay, per-window score |
| **Phase-folded plot** | All transits folded at detected period, with confidence band |
| **3D orbit preview** | `THREE.Line` showing inferred orbital path around host star, sized by uncertainty |
| **Evidence trace** | Expandable tree showing witness chain from raw data to final score |
| **Score breakdown** | Radar chart: SNR, shape consistency, period stability, coherence stability |

Data source: `GET /api/candidates/planet`, `GET /api/candidates/:id/trace`

#### V4: Life Candidate Dashboard (2D panels + 3D molecule)

Split view for spectral disequilibrium analysis.

| Panel | Content |
|-------|---------|
| **Ranked list** | Sortable table: candidate ID, disequilibrium score, uncertainty, molecule flags, publishable |
| **Spectrum viewer** | Interactive D3 chart: wavelength vs flux, molecule absorption bands highlighted |
| **Molecule presence matrix** | Heatmap of detected molecule families vs confidence |
| **3D molecule overlay** | `THREE.Sprite` labels at absorption wavelengths in a 3D wavelength space |
| **Reaction graph** | Force-directed graph of molecule co-occurrences vs equilibrium expectations |
| **Confound panel** | Bar chart: stellar activity penalty, contamination risk, repeatability score |

Data source: `GET /api/candidates/life`, `GET /api/candidates/:id/trace`

#### V5: System Status Dashboard

Operational health and download progress.

| Panel | Content |
|-------|---------|
| **Download progress** | Per-tier progress bars with byte counts and ETA |
| **Segment sizes** | Stacked bar chart of RVF segment utilization |
| **Memory tiers** | S/M/L tier fill levels and compaction history |
| **Witness chain** | Scrolling log of recent witness entries with hash preview |
| **Pipeline status** | P0/P1/P2 and L0/L1/L2 stage indicators with event counts |
| **Performance** | Query latency histogram, events/second throughput |

Data source: `GET /api/status`, `GET /api/memory/tiers`, `WS /ws/live`

### WebSocket Live Stream

```typescript
// WS /ws/live — server pushes events as they happen
interface LiveEvent {
  type: 'boundary_alert' | 'candidate_new' | 'candidate_update' |
        'download_progress' | 'witness_commit' | 'pipeline_stage' |
        'coherence_update';
  timestamp: string;
  data: Record<string, unknown>;
}
```

The dashboard subscribes on connect and updates all views in real-time as
pipelines process data and boundaries evolve.

### Vite Build Configuration

```typescript
// vite.config.ts for dashboard build
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    outDir: 'dist/dashboard',
    assetsDir: 'assets',
    rollupOptions: {
      output: {
        manualChunks: {
          three: ['three'],         // ~150 KB gzipped
          d3: ['d3-scale', 'd3-axis', 'd3-shape', 'd3-selection'],
        },
      },
    },
  },
});
```

**Bundle budget**: < 500 KB gzipped total (Three.js ~150 KB, D3 subset ~30 KB,
app logic ~50 KB, styles ~10 KB). The dashboard adds minimal overhead to the
RVF artifact.

### Design Decision: D5 — Dashboard Embedded in RVF

The Three.js dashboard is bundled at build time and embedded in `DASHBOARD_SEG`
rather than served from an external CDN or requiring a separate install. This
ensures:

1. **Fully offline**: Works without network after boot
2. **Version-locked**: Dashboard always matches the API version it queries
3. **Single artifact**: One RVF file = runtime + data + visualization
4. **Witness-aligned**: Dashboard renders exactly the data the witness chain
   can verify

## Package Structure

```
packages/agentdb-causal-atlas/
  src/
    index.ts                    # createCausalAtlasServer() factory
    CausalAtlasServer.ts        # HTTP + CLI runtime + dashboard serving + WS
    CausalAtlasEngine.ts        # Core atlas, coherence, memory, boundary
    adapters/
      PlanetTransitAdapter.ts   # Kepler/TESS light curve ingestion
      SpectrumAdapter.ts        # JWST/archive spectral ingestion
      CosmicWebAdapter.ts       # SDSS spatial graph (Phase 2)
    pipelines/
      PlanetDetection.ts        # P0-P2 planet detection pipeline
      LifeCandidate.ts          # L0-L2 life candidate pipeline
    constructs/
      CausalAtlas.ts            # Compressed causal partial order
      CoherenceField.ts         # Partition tree + cut pressure
      MultiScaleMemory.ts       # Tiered S/M/L retention
      BoundaryTracker.ts        # Boundary evolution + alerts
    download/
      ProgressiveDownloader.ts  # Tiered lazy download with resume
      DataManifest.ts           # URL + hash + size manifests
    KernelBuilder.ts            # Reuse/extend from ADR-008
  dashboard/                    # Vite + Three.js visualization app
    vite.config.ts              # Build config — outputs to dist/dashboard/
    index.html                  # SPA entry point
    src/
      main.ts                   # App bootstrap, router, WS connection
      api.ts                    # Typed fetch wrappers for /api/* endpoints
      ws.ts                     # WebSocket client for /ws/live
      views/
        AtlasExplorer.ts        # V1: 3D causal atlas (Three.js force graph)
        CoherenceHeatmap.ts     # V2: Coherence field surface + cut pressure
        PlanetDashboard.ts      # V3: Planet candidates + light curves + 3D orbit
        LifeDashboard.ts        # V4: Life candidates + spectra + molecule graph
        StatusDashboard.ts      # V5: System health, downloads, witness log
      three/
        AtlasGraph.ts           # InstancedMesh nodes, LineSegments edges, particles
        CoherenceSurface.ts     # PlaneGeometry with vertex-colored field
        OrbitPreview.ts         # Orbital path visualization
        CausalFlow.ts           # Animated particles along causal edges
        LODController.ts        # Level-of-detail: boundary → top-k → full
      charts/
        LightCurveChart.ts      # D3 flux time series with transit overlay
        SpectrumChart.ts        # D3 wavelength vs flux with molecule bands
        RadarChart.ts           # Score breakdown radar
        MoleculeMatrix.ts       # Heatmap of molecule presence vs confidence
      components/
        Sidebar.ts              # Candidate list, filters, search
        TimeScrubber.ts         # Epoch scrubber for coherence replay
        WitnessLog.ts           # Scrolling witness chain entries
        DownloadProgress.ts     # Tier progress bars
      styles/
        main.css                # Minimal Tailwind or hand-rolled styles
  tests/
    causal-atlas.test.ts
    planet-detection.test.ts
    life-candidate.test.ts
    progressive-download.test.ts
    coherence-field.test.ts
    boundary-tracker.test.ts
    dashboard.test.ts           # Dashboard build + API integration tests
```

## Implementation Phases

### Phase 1: Core Atlas + Planet Detection + Dashboard Shell (v0.1)

**Scope**: Kepler and TESS only. No spectra. No life scoring.

1. Implement `ProgressiveDownloader` with tier-0 curated dataset (100 Kepler targets)
2. Implement `PlanetTransitAdapter` for FITS light curve ingestion
3. Implement `CausalAtlas` with windowing, feature extraction, SIMD embedding
4. Implement `PlanetDetection` pipeline (P0-P2)
5. Implement `WITNESS_SEG` with SHAKE-256 chain
6. CLI: `rvf run`, `rvf query planet list`, `rvf trace`
7. HTTP: `/api/candidates/planet`, `/api/atlas/trace`
8. Dashboard: Vite scaffold, V1 Atlas Explorer (Three.js 3D graph), V3 Planet
   Dashboard (ranked list + light curve chart), V5 Status Dashboard (download
   progress + witness log). Embedded in `DASHBOARD_SEG`, served at `/`
9. WebSocket `/ws/live` for real-time pipeline progress

**Acceptance**: 1,000 Kepler targets, top-100 ranked list includes >= 80
confirmed planets, every item replays to same score and witness root on two
machines. Dashboard renders atlas graph and candidate list in browser.

### Phase 2: Coherence Field + Boundary Tracker + Dashboard V2 (v0.2)

1. Implement `CoherenceField` with dynamic min-cut, partition entropy
2. Implement `BoundaryTracker` with timeline and alerts
3. Implement `MultiScaleMemory` with S/M/L tiers and budget control
4. Add coherence gating to planet pipeline
5. HTTP: `/api/coherence`, `/api/boundary/*`, `/api/memory/tiers`
6. Dashboard: V2 Coherence Heatmap (Three.js field surface + cut pressure
   overlay + time scrubber), boundary alert markers via WebSocket

### Phase 3: Life Candidate Pipeline + Dashboard V4 (v0.3)

1. Implement `SpectrumAdapter` for JWST/archive spectral data
2. Implement `LifeCandidate` pipeline (L0-L2)
3. Implement disequilibrium scoring with reaction plausibility graph
4. Tier-3 progressive download for spectral data
5. CLI: `rvf query life list`
6. HTTP: `/api/candidates/life`
7. Dashboard: V4 Life Dashboard (spectrum viewer + molecule presence matrix
   + reaction graph + confound panel)

**Acceptance**: Published spectra with known atmospheric detections vs nulls,
AUC > 0.8, every score includes confound penalties and provenance trace.
Dashboard renders spectrum analysis in browser.

### Phase 4: Cosmic Web + Full Integration (v0.4)

1. `CosmicWebAdapter` for SDSS spatial graph
2. Cross-domain coherence (planet candidates enriched by large-scale context)
3. Dashboard: 3D cosmic web view, cross-domain candidate linking
4. Full offline demo with sealed RVF snapshot
5. `rvf ingest --expand` for tier-2 bulk download
6. Dashboard polish: LOD optimization, mobile-responsive layout, dark/light theme

## Evaluation Plan

### Planet Detection Acceptance Test

| Metric | Requirement |
|--------|-------------|
| Recall@100 | >= 80 confirmed planets in top 100 |
| False positives@100 | Documented with witness traces |
| Median time per star | Measured and reported |
| Reproducibility | Identical root hash on two machines |

### Life Candidate Acceptance Test

| Metric | Requirement |
|--------|-------------|
| AUC (detected vs null) | > 0.8 |
| Confound penalties | Present on every score |
| Provenance trace | Complete for every score |

### System Acceptance Test

| Test | Requirement |
|------|-------------|
| Boot reproducibility | Identical root hash across two machines |
| Query determinism | Identical results for same dataset snapshot |
| Witness verification | `verifyWitness` passes for all chains |
| Progressive download | Resumes correctly after interruption |

## Failure Modes and Fix Path

| Failure | Fix |
|---------|-----|
| Noise dominates coherence field | Strengthen policy priors, add confound penalties, enforce multi-epoch stability |
| Over-compression kills rare signals | Boundary-critical retention rules + candidate evidence pinning |
| Spurious life signals from stellar activity | Model stellar variability as its own interaction graph, penalize coupling |
| Compute blow-up | Strict budgets in POLICY_SEG, tiered memory, boundary-first indexing |
| Download interruption | HTTP Range resume, partial-ingest checkpoint, witness for partial state |

## Design Decisions

### D1: Kepler/TESS only in v1, spectra in v3

Phase 1 delivers a concrete, testable planet-detection system. Life scoring
requires additional instrument-specific adapters and more nuanced policy
rules. Separating them de-risks the schedule.

### D2: Progressive download with embedded demo subset

The RVF artifact ships with a curated ~50 MB tier-0 dataset for fully offline
demonstration. Full catalog data is downloaded lazily, following the pattern
proven in ADR-008 for GGUF model files. This keeps the initial artifact small
(< 100 MB without kernel) while supporting the full 1,000+ target benchmark.

### D3: Boundary-first storage (holographic principle)

Boundaries are stored as first-class indexed objects. Interior state is
reconstructed on-demand from boundary witnesses and retained archetypes.
This reduces storage by 10-50x for large graphs while preserving
queryability and reproducibility.

### D4: Witness chain for every derived claim

Every candidate, every coherence measurement, and every boundary change is
committed to the SHAKE-256 witness chain. This enables two-machinevisu
reproducibility verification and provides a complete audit trail from raw
data to final score.

## References

1. [MAST — Kepler](https://archive.stsci.edu/missions-and-data/kepler)
2. [MAST — TESS](https://archive.stsci.edu/missions-and-data/tess)
3. [MAST Home](https://archive.stsci.edu/home)
4. [NASA Exoplanet Archive](https://exoplanetarchive.ipac.caltech.edu/)
5. [SDSS DR17](https://www.sdss4.org/dr17/)
6. ADR-003: RVF Native Format Integration
7. ADR-006: Unified Self-Learning RVF Integration
8. ADR-007: RuVector Full Capability Integration
9. ADR-008: Chat UI RVF Kernel Embedding
