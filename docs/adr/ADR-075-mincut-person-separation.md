# ADR-075: Min-Cut Based Person Separation from Subcarrier Correlation

- **Status:** Proposed
- **Date:** 2026-04-02
- **Issue:** #348 — `n_persons` always reports 4 regardless of actual occupancy
- **Depends on:** ADR-016 (RuVector integration), ADR-041 (person tracking), ADR-073 (multifrequency mesh scan)

## Context

### The Bug

Issue #348 reports that the ESP32 firmware's multi-person counting always reports
`n_persons = 4`. The root cause is in the WASM edge module
`sig_mincut_person_match.rs`, which uses a fixed `MAX_PERSONS = 4` constant and a
threshold-based variance classifier to populate person slots. The classifier bins
subcarriers into "dynamic" vs "static" using a single fixed variance threshold
(`DYNAMIC_VAR_THRESH = 0.15`). In practice:

1. The threshold is miscalibrated for real-world CSI data — almost any room with
   multipath reflections pushes a majority of subcarriers above 0.15 variance.
2. The subcarrier-to-person assignment uses a greedy Hungarian-lite matcher that
   fills all 4 slots once there are >= 4 dynamic subcarriers (which is nearly
   always the case).
3. There is no mechanism to determine how many independent movers exist — the
   algorithm assumes all 4 slots should be filled.

### Prior Art

The Rust crate `ruvector-mincut` (vendored at `vendor/ruvector/crates/ruvector-mincut/`)
implements a full dynamic min-cut algorithm with O(n^{o(1)}) amortized update time,
Stoer-Wagner exact min-cut, and online edge insert/delete. It is already integrated
in the training pipeline (`wifi-densepose-train/src/metrics.rs`) via
`DynamicPersonMatcher`.

### WiFi Sensing Insight

When a person moves through a room, they perturb the Fresnel zones of specific
subcarrier frequencies. Subcarriers whose Fresnel zones overlap the person's body
change **together** — their amplitudes are temporally correlated. When two people
move independently, they create two **separate** groups of correlated subcarriers.
This correlation structure forms a natural graph partitioning problem.

## Decision

Replace the fixed-threshold person counter with a spectral min-cut algorithm
operating on the subcarrier temporal correlation graph. This runs in the bridge
script (`scripts/mincut-person-counter.js`) or on Cognitum Seed, and feeds the
corrected person count back to the feature vector before ingest.

### Algorithm

1. **Sliding window accumulation**: Maintain the last 2 seconds of subcarrier
   amplitude data (~40 frames at 20 fps). Each frame provides a 64-element
   amplitude vector (one per subcarrier).

2. **Pairwise Pearson correlation**: For all subcarrier pairs (i, j), compute
   the Pearson correlation coefficient over the sliding window:

   ```
   r(i,j) = cov(amp_i, amp_j) / (std(amp_i) * std(amp_j))
   ```

   This produces a 64x64 correlation matrix.

3. **Graph construction**: Build a weighted undirected graph:
   - **Nodes** = subcarriers (64 for single-antenna ESP32-S3, up to 128 for dual)
   - **Edges** = pairs with |r(i,j)| > 0.3 (correlation threshold)
   - **Weight** = |r(i,j)| (correlation strength)
   - Discard null subcarriers (amplitude consistently near zero)
   - Expected: ~1500-2500 edges for 64 active subcarriers

4. **Iterative Stoer-Wagner min-cut**: Apply the Stoer-Wagner algorithm to find
   the global minimum cut. If the min-cut weight is below a separation threshold
   (empirically 2.0), the cut represents a real boundary between independent
   movers. Split the graph at the cut and recurse on each partition.

5. **Person count**: The number of partitions after all valid cuts = number of
   independent movers = person count. A single connected component with high
   internal correlation and no low-weight cut = 1 person (or 0 if variance is
   also low).

6. **Empty room detection**: If the total variance across all subcarriers is
   below a noise floor threshold, report 0 persons regardless of graph structure.

### Stoer-Wagner Algorithm

Stoer-Wagner finds the exact global minimum cut of an undirected weighted graph
in O(V * E) time using a sequence of "minimum cut phases":

```
function stoerWagner(G):
    best_cut = infinity
    while |V(G)| > 1:
        (s, t, cut_of_phase) = minimumCutPhase(G)
        if cut_of_phase < best_cut:
            best_cut = cut_of_phase
            best_partition = partition induced by t
        merge(s, t)  // contract vertices s and t
    return best_cut, best_partition

function minimumCutPhase(G):
    A = {arbitrary start vertex}
    while A != V(G):
        z = vertex most tightly connected to A
        // "most tightly connected" = max sum of edge weights to A
        add z to A
    s = second-to-last vertex added
    t = last vertex added (most tightly connected)
    cut_of_phase = sum of weights of edges incident to t
    return (s, t, cut_of_phase)
```

For V=64 subcarriers and E~2000 edges, this runs in ~8 million operations,
well under 1ms on modern hardware and under 10ms even on ESP32-S3.

### Integration Points

```
ESP32 Node 1 ──UDP 5006──┐
                          ├──> mincut-person-counter.js ──> corrected n_persons
ESP32 Node 2 ──UDP 5006──┘         │
                                   ├──> seed_csi_bridge.py (feature dim 5 override)
                                   └──> csi-graph-visualizer.js (debug view)
```

The person counter runs as a standalone Node.js process alongside the existing
`rf-scan.js` and `seed_csi_bridge.py` bridge scripts. It can also replay
recorded `.csi.jsonl` files for offline analysis.

## Alternatives Considered

### 1. Threshold-based peak counting (current, broken)

Count subcarriers with variance above a threshold, then cluster by proximity.
**Problem:** threshold is environment-dependent, miscalibrates easily, and
cannot distinguish correlated from independent motion.

### 2. PCA / spectral clustering on correlation matrix

Compute eigenvectors of the correlation matrix; the number of large eigenvalues
indicates the number of independent sources. **Problem:** requires choosing an
eigenvalue gap threshold, which is as fragile as the current variance threshold.
Also does not give per-person subcarrier assignments.

### 3. Min-cut on correlation graph (this ADR)

**Advantages:**
- Directly models the physical structure (Fresnel zone groupings)
- Threshold-free person counting (cut weight is a natural separation metric)
- Produces per-person subcarrier groups as a side effect
- Stoer-Wagner is simple to implement (~100 lines) and runs in polynomial time
- Already validated in Rust via `ruvector-mincut` integration

## Performance

| Metric | Value |
|--------|-------|
| Graph size | V=64, E~2000 |
| Stoer-Wagner complexity | O(V * E) = O(128,000) per cut |
| Iterative cuts (max 4) | O(512,000) total |
| Wall time (Node.js) | < 5 ms per 2-second window |
| Wall time (Rust/WASM) | < 0.5 ms |
| Memory | ~32 KB for correlation matrix + graph |
| Sliding window | 2 seconds = ~40 frames * 64 subcarriers * 8 bytes = 20 KB |

## Consequences

### Positive

- Fixes #348: person count now reflects actual independent movers
- Robust across environments (no per-room threshold calibration)
- Per-person subcarrier groups enable per-person feature extraction
- Graph visualization aids debugging and room mapping
- Algorithm is well-understood (Stoer-Wagner, 1997)

### Negative

- Adds a new process to the sensing pipeline
- 2-second latency for person count changes (sliding window)
- Correlation-based: cannot detect stationary persons (no motion = no signal)
- Assumes independent motion — two people walking in sync may be counted as one

### Migration

1. Deploy `scripts/mincut-person-counter.js` alongside existing bridge
2. Override feature vector dimension 5 (`n_persons`) with corrected count
3. Once validated, port Stoer-Wagner to C for direct ESP32-S3 firmware integration
4. Deprecate the fixed-threshold `PersonMatcher` in `sig_mincut_person_match.rs`

## References

- Stoer, M. & Wagner, F. (1997). "A Simple Min-Cut Algorithm." JACM 44(4).
- `vendor/ruvector/crates/ruvector-mincut/src/algorithm/mod.rs` — DynamicMinCut API
- `v2/.../sig_mincut_person_match.rs` — current (broken) WASM edge matcher
- `scripts/rf-scan.js` — CSI packet parsing and subcarrier classification
