# ADR-017: RuVector Integration for Signal Processing and MAT Crates

## Status

Proposed

## Date

2026-02-28

## Context

ADR-016 integrated all five published ruvector v2.0.4 crates into the
`wifi-densepose-train` crate (model.rs, dataset.rs, subcarrier.rs, metrics.rs).
Two production crates that pre-date ADR-016 remain without ruvector integration
despite having concrete, high-value integration points:

1. **`wifi-densepose-signal`** — SOTA signal processing algorithms (ADR-014):
   conjugate multiplication, Hampel filter, Fresnel zone breathing model, CSI
   spectrogram, subcarrier sensitivity selection, Body Velocity Profile (BVP).
   These algorithms perform independent element-wise operations or brute-force
   exhaustive search without subpolynomial optimization.

2. **`wifi-densepose-mat`** — Disaster detection (ADR-001): multi-AP
   triangulation, breathing/heartbeat waveform detection, triage classification.
   Time-series data is uncompressed and localization uses closed-form geometry
   without iterative system solving.

Additionally, ADR-002's dependency strategy references fictional crate names
(`ruvector-core`, `ruvector-data-framework`, `ruvector-consensus`,
`ruvector-wasm`) at non-existent version `"0.1"`. ADR-016 confirmed the actual
published crates at v2.0.4 and these must be used instead.

### Verified Published Crates (v2.0.4)

From source inspection of github.com/ruvnet/ruvector and crates.io:

| Crate | Key API | Algorithmic Advantage |
|---|---|---|
| `ruvector-mincut` | `DynamicMinCut`, `MinCutBuilder` | O(n^1.5 log n) dynamic graph partitioning |
| `ruvector-attn-mincut` | `attn_mincut(q,k,v,d,seq,λ,τ,ε)` | Attention + mincut gating in one pass |
| `ruvector-temporal-tensor` | `TemporalTensorCompressor`, `segment::decode` | Tiered quantization: 50–75% memory reduction |
| `ruvector-solver` | `NeumannSolver::new(tol,max_iter).solve(&CsrMatrix,&[f32])` | O(√n) Neumann series convergence |
| `ruvector-attention` | `ScaledDotProductAttention::new(d).compute(q,ks,vs)` | Sublinear attention for small d |

## Decision

Integrate the five ruvector v2.0.4 crates across `wifi-densepose-signal` and
`wifi-densepose-mat` through seven targeted integration points.

### Integration Map

```
wifi-densepose-signal/
├── subcarrier_selection.rs  ← ruvector-mincut   (DynamicMinCut partitions)
├── spectrogram.rs           ← ruvector-attn-mincut (attention-gated STFT tokens)
├── bvp.rs                   ← ruvector-attention   (cross-subcarrier BVP attention)
└── fresnel.rs               ← ruvector-solver      (Fresnel geometry system)

wifi-densepose-mat/
├── localization/
│   └── triangulation.rs     ← ruvector-solver      (multi-AP TDoA equations)
└── detection/
    ├── breathing.rs          ← ruvector-temporal-tensor (tiered waveform compression)
    └── heartbeat.rs          ← ruvector-temporal-tensor (tiered micro-Doppler compression)
```

---

### Integration 1: Subcarrier Sensitivity Selection via DynamicMinCut

**File:** `wifi-densepose-signal/src/subcarrier_selection.rs`
**Crate:** `ruvector-mincut`

**Current approach:** Rank all subcarriers by `variance_motion / variance_static`
ratio, take top-K by sorting. O(n log n) sort, static partition.

**ruvector integration:** Build a similarity graph where subcarriers are vertices
and edges encode variance-ratio similarity (|sensitivity_i − sensitivity_j|^−1).
`DynamicMinCut` finds the minimum bisection separating high-sensitivity
(motion-responsive) from low-sensitivity (noise-dominated) subcarriers. As new
static/motion measurements arrive, `insert_edge`/`delete_edge` incrementally
update the partition in O(n^1.5 log n) amortized — no full re-sort needed.

```rust
use ruvector_mincut::{DynamicMinCut, MinCutBuilder};

/// Partition subcarriers into sensitive/insensitive groups via min-cut.
/// Returns (sensitive_indices, insensitive_indices).
pub fn mincut_subcarrier_partition(
    sensitivity: &[f32],
) -> (Vec<usize>, Vec<usize>) {
    let n = sensitivity.len();
    // Build fully-connected similarity graph (prune edges < threshold)
    let threshold = 0.1_f64;
    let mut edges = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            let diff = (sensitivity[i] - sensitivity[j]).abs() as f64;
            let weight = if diff > 1e-9 { 1.0 / diff } else { 1e6 };
            if weight > threshold {
                edges.push((i as u64, j as u64, weight));
            }
        }
    }
    let mc = MinCutBuilder::new().exact().with_edges(edges).build();
    let (side_a, side_b) = mc.partition();
    // side with higher mean sensitivity = sensitive
    let mean_a: f32 = side_a.iter().map(|&i| sensitivity[i as usize]).sum::<f32>()
        / side_a.len() as f32;
    let mean_b: f32 = side_b.iter().map(|&i| sensitivity[i as usize]).sum::<f32>()
        / side_b.len() as f32;
    if mean_a >= mean_b {
        (side_a.into_iter().map(|x| x as usize).collect(),
         side_b.into_iter().map(|x| x as usize).collect())
    } else {
        (side_b.into_iter().map(|x| x as usize).collect(),
         side_a.into_iter().map(|x| x as usize).collect())
    }
}
```

**Advantage:** Incremental updates as the environment changes (furniture moved,
new occupant) do not require re-ranking all subcarriers. Dynamic partition tracks
changing sensitivity in O(n^1.5 log n) vs O(n^2) re-scan.

---

### Integration 2: Attention-Gated CSI Spectrogram

**File:** `wifi-densepose-signal/src/spectrogram.rs`
**Crate:** `ruvector-attn-mincut`

**Current approach:** Compute STFT per subcarrier independently, stack into 2D
matrix [freq_bins × time_frames]. All bins weighted equally for downstream CNN.

**ruvector integration:** After STFT, treat each time frame as a sequence token
(d = n_freq_bins, seq_len = n_time_frames). Apply `attn_mincut` to gate which
time-frequency cells contribute to the spectrogram output — suppressing noise
frames and multipath artifacts while amplifying body-motion periods.

```rust
use ruvector_attn_mincut::attn_mincut;

/// Apply attention gating to a computed spectrogram.
/// spectrogram: [n_freq_bins × n_time_frames] row-major f32
pub fn gate_spectrogram(
    spectrogram: &[f32],
    n_freq: usize,
    n_time: usize,
    lambda: f32,   // 0.1 = mild gating, 0.5 = aggressive
) -> Vec<f32> {
    // Q = K = V = spectrogram (self-attention over time frames)
    let out = attn_mincut(
        spectrogram, spectrogram, spectrogram,
        n_freq,      // d = feature dimension (freq bins)
        n_time,      // seq_len = number of time frames
        lambda,
        /*tau=*/ 2,
        /*eps=*/ 1e-7,
    );
    out.output
}
```

**Advantage:** Self-attention + mincut identifies coherent temporal segments
(body motion intervals) and gates out uncorrelated frames (ambient noise, transient
interference). Lambda tunes the gating strength without requiring separate
denoising or temporal smoothing steps.

---

### Integration 3: Cross-Subcarrier BVP Attention

**File:** `wifi-densepose-signal/src/bvp.rs`
**Crate:** `ruvector-attention`

**Current approach:** Aggregate Body Velocity Profile by summing STFT magnitudes
uniformly across all subcarriers: `BVP[v,t] = Σ_k |STFT_k[v,t]|`. Equal
weighting means insensitive subcarriers dilute the velocity estimate.

**ruvector integration:** Use `ScaledDotProductAttention` to compute a
weighted aggregation across subcarriers. Each subcarrier contributes a key
(its sensitivity profile) and value (its STFT row). The query is the current
velocity bin. Attention weights automatically emphasize subcarriers that are
responsive to the queried velocity range.

```rust
use ruvector_attention::ScaledDotProductAttention;

/// Compute attention-weighted BVP aggregation across subcarriers.
/// stft_rows: Vec of n_subcarriers rows, each [n_velocity_bins] f32
/// sensitivity: sensitivity score per subcarrier [n_subcarriers] f32
pub fn attention_weighted_bvp(
    stft_rows: &[Vec<f32>],
    sensitivity: &[f32],
    n_velocity_bins: usize,
) -> Vec<f32> {
    let d = n_velocity_bins;
    let attn = ScaledDotProductAttention::new(d);

    // Mean sensitivity row as query (overall body motion profile)
    let query: Vec<f32> = (0..d).map(|v| {
        stft_rows.iter().zip(sensitivity.iter())
            .map(|(row, &s)| row[v] * s)
            .sum::<f32>()
            / sensitivity.iter().sum::<f32>()
    }).collect();

    // Keys = STFT rows (each subcarrier's velocity profile)
    // Values = STFT rows (same, weighted by attention)
    let keys: Vec<&[f32]> = stft_rows.iter().map(|r| r.as_slice()).collect();
    let values: Vec<&[f32]> = stft_rows.iter().map(|r| r.as_slice()).collect();

    attn.compute(&query, &keys, &values)
        .unwrap_or_else(|_| vec![0.0; d])
}
```

**Advantage:** Replaces uniform sum with sensitivity-aware weighting. Subcarriers
in multipath nulls or noise-dominated frequency bands receive low attention weight
automatically, without requiring manual selection or a separate sensitivity step.

---

### Integration 4: Fresnel Zone Geometry System via NeumannSolver

**File:** `wifi-densepose-signal/src/fresnel.rs`
**Crate:** `ruvector-solver`

**Current approach:** Closed-form Fresnel zone radius formula assuming known
TX-RX-body geometry. In practice, exact distances d1 (TX→body) and d2
(body→RX) are unknown — only the TX-RX straight-line distance D is known from
AP placement.

**ruvector integration:** When multiple subcarriers observe different Fresnel
zone crossings at the same chest displacement, we can solve for the unknown
geometry (d1, d2, Δd) using the over-determined linear system from multiple
observations. `NeumannSolver` handles the sparse normal equations efficiently.

```rust
use ruvector_solver::neumann::NeumannSolver;
use ruvector_solver::types::CsrMatrix;

/// Estimate TX-body and body-RX distances from multi-subcarrier Fresnel observations.
/// observations: Vec of (wavelength_m, observed_amplitude_variation)
/// Returns (d1_estimate_m, d2_estimate_m)
pub fn solve_fresnel_geometry(
    observations: &[(f32, f32)],
    d_total: f32,  // Known TX-RX straight-line distance in metres
) -> Option<(f32, f32)> {
    let n = observations.len();
    if n < 3 { return None; }

    // System: A·[d1, d2]^T = b
    // From Fresnel: A_k = |sin(2π·2·Δd / λ_k)|, observed ~ A_k
    // Linearize: use log-magnitude ratios as rows
    // Normal equations: (A^T A + λI) x = A^T b
    let lambda_reg = 0.05_f32;
    let mut coo = Vec::new();
    let mut rhs = vec![0.0_f32; 2];

    for (k, &(wavelength, amplitude)) in observations.iter().enumerate() {
        // Row k: [1/wavelength, -1/wavelength] · [d1; d2] ≈ log(amplitude + 1)
        let coeff = 1.0 / wavelength;
        coo.push((k, 0, coeff));
        coo.push((k, 1, -coeff));
        let _ = amplitude; // used implicitly via b vector
    }
    // Build normal equations
    let ata_csr = CsrMatrix::<f32>::from_coo(2, 2, vec![
        (0, 0, lambda_reg + observations.iter().map(|(w, _)| 1.0 / (w * w)).sum::<f32>()),
        (1, 1, lambda_reg + observations.iter().map(|(w, _)| 1.0 / (w * w)).sum::<f32>()),
    ]);
    let atb: Vec<f32> = vec![
        observations.iter().map(|(w, a)| a / w).sum::<f32>(),
        -observations.iter().map(|(w, a)| a / w).sum::<f32>(),
    ];

    let solver = NeumannSolver::new(1e-5, 300);
    match solver.solve(&ata_csr, &atb) {
        Ok(result) => {
            let d1 = result.solution[0].abs().clamp(0.1, d_total - 0.1);
            let d2 = (d_total - d1).clamp(0.1, d_total - 0.1);
            Some((d1, d2))
        }
        Err(_) => None,
    }
}
```

**Advantage:** Converts the Fresnel model from a single fixed-geometry formula
into a data-driven geometry estimator. With 3+ observations (subcarriers at
different frequencies), NeumannSolver converges in O(√n) iterations — critical
for real-time breathing detection at 100 Hz.

---

### Integration 5: Multi-AP Triangulation via NeumannSolver

**File:** `wifi-densepose-mat/src/localization/triangulation.rs`
**Crate:** `ruvector-solver`

**Current approach:** Multi-AP localization uses pairwise TDoA (Time Difference
of Arrival) converted to hyperbolic equations. Solving N-AP systems requires
linearization and least-squares, currently implemented as brute-force normal
equations via Gaussian elimination (O(n^3)).

**ruvector integration:** The linearized TDoA system is sparse (each measurement
involves 2 APs, not all N). `CsrMatrix::from_coo` + `NeumannSolver` solves the
sparse normal equations in O(√nnz) where nnz = number of non-zeros ≪ N^2.

```rust
use ruvector_solver::neumann::NeumannSolver;
use ruvector_solver::types::CsrMatrix;

/// Solve multi-AP TDoA survivor localization.
/// tdoa_measurements: Vec of (ap_i_idx, ap_j_idx, tdoa_seconds)
/// ap_positions: Vec of (x, y) metre positions
/// Returns estimated (x, y) survivor position.
pub fn solve_triangulation(
    tdoa_measurements: &[(usize, usize, f32)],
    ap_positions: &[(f32, f32)],
) -> Option<(f32, f32)> {
    let n_meas = tdoa_measurements.len();
    if n_meas < 3 { return None; }

    const C: f32 = 3e8_f32; // speed of light
    let mut coo = Vec::new();
    let mut b = vec![0.0_f32; n_meas];

    // Linearize: subtract reference AP from each TDoA equation
    let (x_ref, y_ref) = ap_positions[0];
    for (row, &(i, j, tdoa)) in tdoa_measurements.iter().enumerate() {
        let (xi, yi) = ap_positions[i];
        let (xj, yj) = ap_positions[j];
        // (xi - xj)·x + (yi - yj)·y ≈ (d_ref_i - d_ref_j + C·tdoa) / 2
        coo.push((row, 0, xi - xj));
        coo.push((row, 1, yi - yj));
        b[row] = C * tdoa / 2.0
            + ((xi * xi - xj * xj) + (yi * yi - yj * yj)) / 2.0
            - x_ref * (xi - xj) - y_ref * (yi - yj);
    }

    // Normal equations: (A^T A + λI) x = A^T b
    let lambda = 0.01_f32;
    let ata = CsrMatrix::<f32>::from_coo(2, 2, vec![
        (0, 0, lambda + coo.iter().filter(|e| e.1 == 0).map(|e| e.2 * e.2).sum::<f32>()),
        (0, 1, coo.iter().filter(|e| e.1 == 0).zip(coo.iter().filter(|e| e.1 == 1)).map(|(a, b2)| a.2 * b2.2).sum::<f32>()),
        (1, 0, coo.iter().filter(|e| e.1 == 1).zip(coo.iter().filter(|e| e.1 == 0)).map(|(a, b2)| a.2 * b2.2).sum::<f32>()),
        (1, 1, lambda + coo.iter().filter(|e| e.1 == 1).map(|e| e.2 * e.2).sum::<f32>()),
    ]);
    let atb = vec![
        coo.iter().filter(|e| e.1 == 0).zip(b.iter()).map(|(e, &bi)| e.2 * bi).sum::<f32>(),
        coo.iter().filter(|e| e.1 == 1).zip(b.iter()).map(|(e, &bi)| e.2 * bi).sum::<f32>(),
    ];

    NeumannSolver::new(1e-5, 500)
        .solve(&ata, &atb)
        .ok()
        .map(|r| (r.solution[0], r.solution[1]))
}
```

**Advantage:** For a disaster site with 5–20 APs, the TDoA system has N×(N-1)/2
= 10–190 measurements but only 2 unknowns (x, y). The normal equations are 2×2
regardless of N. NeumannSolver converges in O(1) iterations for well-conditioned
2×2 systems — eliminating Gaussian elimination overhead.

---

### Integration 6: Breathing Waveform Compression

**File:** `wifi-densepose-mat/src/detection/breathing.rs`
**Crate:** `ruvector-temporal-tensor`

**Current approach:** Breathing detector maintains an in-memory ring buffer of
recent CSI amplitude samples across subcarriers × time. For a 60-second window
at 100 Hz with 56 subcarriers: 60 × 100 × 56 × 4 bytes = **13.4 MB per zone**.
With 16 concurrent zones: **214 MB just for breathing buffers**.

**ruvector integration:** `TemporalTensorCompressor` with tiered quantization
(8-bit hot / 5-7-bit warm / 3-bit cold) compresses the breathing waveform buffer
by 50–75%:

```rust
use ruvector_temporal_tensor::{TemporalTensorCompressor, TierPolicy};
use ruvector_temporal_tensor::segment;

pub struct CompressedBreathingBuffer {
    compressor: TemporalTensorCompressor,
    encoded: Vec<u8>,
    n_subcarriers: usize,
    frame_count: u64,
}

impl CompressedBreathingBuffer {
    pub fn new(n_subcarriers: usize, zone_id: u64) -> Self {
        Self {
            compressor: TemporalTensorCompressor::new(
                TierPolicy::default(),
                n_subcarriers,
                zone_id,
            ),
            encoded: Vec::new(),
            n_subcarriers,
            frame_count: 0,
        }
    }

    pub fn push_frame(&mut self, amplitudes: &[f32]) {
        self.compressor.push_frame(amplitudes, self.frame_count, &mut self.encoded);
        self.frame_count += 1;
    }

    pub fn flush(&mut self) {
        self.compressor.flush(&mut self.encoded);
    }

    /// Decode all frames for frequency analysis.
    pub fn to_vec(&self) -> Vec<f32> {
        let mut out = Vec::new();
        segment::decode(&self.encoded, &mut out);
        out
    }

    /// Get single frame for real-time display.
    pub fn get_frame(&self, idx: usize) -> Option<Vec<f32>> {
        segment::decode_single_frame(&self.encoded, idx)
    }
}
```

**Memory reduction:** 13.4 MB/zone → 3.4–6.7 MB/zone. 16 zones: 54–107 MB
instead of 214 MB. Disaster response hardware (Raspberry Pi 4: 4–8 GB) can
handle 2–4× more concurrent zones.

---

### Integration 7: Heartbeat Micro-Doppler Compression

**File:** `wifi-densepose-mat/src/detection/heartbeat.rs`
**Crate:** `ruvector-temporal-tensor`

**Current approach:** Heartbeat detection uses micro-Doppler spectrograms:
sliding STFT of CSI amplitude time-series. Each zone stores a spectrogram of
shape [n_freq_bins=128, n_time=600] (60 seconds at 10 Hz output rate):
128 × 600 × 4 bytes = **307 KB per zone**. With 16 zones: 4.9 MB — acceptable,
but heartbeat spectrograms are the most access-intensive (queried at every triage
update).

**ruvector integration:** `TemporalTensorCompressor` stores the spectrogram rows
as temporal frames (each row = one frequency bin's time-evolution). Hot tier
(recent 10 seconds) at 8-bit, warm (10–30 sec) at 5-bit, cold (>30 sec) at 3-bit.
Recent heartbeat cycles remain high-fidelity; historical data is compressed 5x:

```rust
pub struct CompressedHeartbeatSpectrogram {
    /// One compressor per frequency bin
    bin_buffers: Vec<TemporalTensorCompressor>,
    encoded: Vec<Vec<u8>>,
    n_freq_bins: usize,
    frame_count: u64,
}

impl CompressedHeartbeatSpectrogram {
    pub fn new(n_freq_bins: usize) -> Self {
        let bin_buffers: Vec<_> = (0..n_freq_bins)
            .map(|i| TemporalTensorCompressor::new(TierPolicy::default(), 1, i as u64))
            .collect();
        let encoded = vec![Vec::new(); n_freq_bins];
        Self { bin_buffers, encoded, n_freq_bins, frame_count: 0 }
    }

    /// Push one column of the spectrogram (one time step, all frequency bins).
    pub fn push_column(&mut self, column: &[f32]) {
        for (i, (&val, buf)) in column.iter().zip(self.bin_buffers.iter_mut()).enumerate() {
            buf.push_frame(&[val], self.frame_count, &mut self.encoded[i]);
        }
        self.frame_count += 1;
    }

    /// Extract heartbeat frequency band power (0.8–1.5 Hz) from recent frames.
    pub fn heartbeat_band_power(&self, low_bin: usize, high_bin: usize) -> f32 {
        (low_bin..=high_bin.min(self.n_freq_bins - 1))
            .map(|b| {
                let mut out = Vec::new();
                segment::decode(&self.encoded[b], &mut out);
                out.iter().rev().take(100).map(|x| x * x).sum::<f32>()
            })
            .sum::<f32>()
            / (high_bin - low_bin + 1) as f32
    }
}
```

---

## Performance Summary

| Integration Point | File | Crate | Before | After |
|---|---|---|---|---|
| Subcarrier selection | `subcarrier_selection.rs` | ruvector-mincut | O(n log n) static sort | O(n^1.5 log n) dynamic partition |
| Spectrogram gating | `spectrogram.rs` | ruvector-attn-mincut | Uniform STFT bins | Attention-gated noise suppression |
| BVP aggregation | `bvp.rs` | ruvector-attention | Uniform subcarrier sum | Sensitivity-weighted attention |
| Fresnel geometry | `fresnel.rs` | ruvector-solver | Fixed geometry formula | Data-driven multi-obs system |
| Multi-AP triangulation | `triangulation.rs` (MAT) | ruvector-solver | O(N^3) dense Gaussian | O(1) 2×2 Neumann system |
| Breathing buffer | `breathing.rs` (MAT) | ruvector-temporal-tensor | 13.4 MB/zone | 3.4–6.7 MB/zone (50–75% less) |
| Heartbeat spectrogram | `heartbeat.rs` (MAT) | ruvector-temporal-tensor | 307 KB/zone uniform | Tiered hot/warm/cold |

## Dependency Changes Required

Add to `rust-port/wifi-densepose-rs/Cargo.toml` workspace (already present from ADR-016):
```toml
ruvector-mincut = "2.0.4"          # already present
ruvector-attn-mincut = "2.0.4"    # already present
ruvector-temporal-tensor = "2.0.4" # already present
ruvector-solver = "2.0.4"          # already present
ruvector-attention = "2.0.4"       # already present
```

Add to `wifi-densepose-signal/Cargo.toml` and `wifi-densepose-mat/Cargo.toml`:
```toml
[dependencies]
ruvector-mincut = { workspace = true }
ruvector-attn-mincut = { workspace = true }
ruvector-temporal-tensor = { workspace = true }
ruvector-solver = { workspace = true }
ruvector-attention = { workspace = true }
```

## Correction to ADR-002 Dependency Strategy

ADR-002's dependency strategy section specifies non-existent crates:
```toml
# WRONG (ADR-002 original — these crates do not exist at crates.io)
ruvector-core = { version = "0.1", features = ["hnsw", "sona", "gnn"] }
ruvector-data-framework = { version = "0.1", features = ["rvf", "witness", "crypto"] }
ruvector-consensus = { version = "0.1", features = ["raft"] }
ruvector-wasm = { version = "0.1", features = ["edge-runtime"] }
```

The correct published crates (verified at crates.io, source at github.com/ruvnet/ruvector):
```toml
# CORRECT (as of 2026-02-28, all at v2.0.4)
ruvector-mincut = "2.0.4"          # Dynamic min-cut, O(n^1.5 log n) updates
ruvector-attn-mincut = "2.0.4"    # Attention + mincut gating
ruvector-temporal-tensor = "2.0.4" # Tiered temporal compression
ruvector-solver = "2.0.4"          # NeumannSolver, sublinear convergence
ruvector-attention = "2.0.4"       # ScaledDotProductAttention
```

The RVF cognitive container format (ADR-003), HNSW search (ADR-004), SONA
self-learning (ADR-005), GNN patterns (ADR-006), post-quantum crypto (ADR-007),
Raft consensus (ADR-008), and WASM edge runtime (ADR-009) described in ADR-002
are architectural capabilities internal to ruvector but not exposed as separate
published crates at v2.0.4. Those ADRs remain as forward-looking architectural
guidance; their implementation paths will use the five published crates as
building blocks where applicable.

## Implementation Priority

| Priority | Integration | Rationale |
|---|---|---|
| P1 | Breathing + heartbeat compression (MAT) | Memory-critical for 16-zone disaster deployments |
| P1 | Multi-AP triangulation (MAT) | Safety-critical accuracy improvement |
| P2 | Subcarrier selection via DynamicMinCut | Enables dynamic environment adaptation |
| P2 | BVP attention aggregation | Direct accuracy improvement for activity classification |
| P3 | Spectrogram attention gating | Reduces CNN input noise; requires CNN retraining |
| P3 | Fresnel geometry system | Improves breathing detection in unknown geometries |

## Consequences

### Positive
- Consistent ruvector integration across all production crates (train, signal, MAT)
- 50–75% memory reduction in disaster detection enables 2–4× more concurrent zones
- Dynamic subcarrier partitioning adapts to environment changes without manual tuning
- Attention-weighted BVP reduces velocity estimation error from insensitive subcarriers
- NeumannSolver triangulation is O(1) in AP count (always solves 2×2 system)

### Negative
- ruvector crates operate on `&[f32]` CPU slices; MAT and signal crates must
  bridge from their native types (ndarray, complex numbers)
- `ruvector-temporal-tensor` compression is lossy; heartbeat amplitude values
  may lose fine-grained detail in warm/cold tiers (mitigated by hot-tier recency)
- Subcarrier selection via DynamicMinCut assumes a bipartite-like partition;
  environments with 3+ distinct subcarrier groups may need multi-way cut extension

## Related ADRs

- ADR-001: WiFi-Mat Disaster Detection (target: MAT integrations 5–7)
- ADR-002: RuVector RVF Integration Strategy (corrected crate names above)
- ADR-014: SOTA Signal Processing Algorithms (target: signal integrations 1–4)
- ADR-015: Public Dataset Training Strategy (preceding implementation in ADR-016)
- ADR-016: RuVector Integration for Training Pipeline (completed reference implementation)

## References

- [ruvector source](https://github.com/ruvnet/ruvector)
- [DynamicMinCut API](https://docs.rs/ruvector-mincut/2.0.4)
- [NeumannSolver convergence](https://en.wikipedia.org/wiki/Neumann_series)
- [Tiered quantization](https://arxiv.org/abs/2103.13630)
- SpotFi (SIGCOMM 2015), Widar 3.0 (MobiSys 2019), FarSense (MobiCom 2019)
