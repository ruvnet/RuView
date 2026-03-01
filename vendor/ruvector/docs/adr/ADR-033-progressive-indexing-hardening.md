# ADR-033: Progressive Indexing Hardening — Centroid Stability, Adversarial Resilience, Recall Framing, and Mandatory Signatures

**Status**: Accepted
**Date**: 2026-02-15
**Supersedes**: Partially amends ADR-029 (RVF Canonical Format), ADR-030 (Cognitive Container)
**Affects**: `rvf-types`, `rvf-runtime`, `rvf-manifest`, `rvf-crypto`, `rvf-wasm`

---

## Context

Analysis of the progressive indexing system (spec chapters 02-04) revealed four structural weaknesses that convert engineered guarantees into opportunistic behavior:

1. **Centroid stability** depends on physical layout, not logical identity
2. **Layer A recall** collapses silently under adversarial distributions
3. **Recall targets** are empirical, presented as if they were bounds
4. **Manifest integrity** is optional, leaving the hotset attack surface open

Each issue individually is tolerable. Together they form a compound vulnerability: an adversary who controls the data distribution AND the file tail can produce a structurally valid RVF file that returns confident, wrong answers with no detection mechanism.

This ADR converts all four from "known limitations" to "engineered defenses."

---

## Decision

### 1. Content-Addressed Centroid Stability

**Invariant**: Logical identity must not depend on physical layout.

#### 1.1 Content-Addressed Segment References

Hotset pointers in the Level 0 manifest currently store raw byte offsets:

```
0x058   8   centroid_seg_offset      Byte offset in file
```

Add a parallel content hash field for each hotset pointer:

```
Offset  Size  Field                    Description
------  ----  -----                    -----------
0x058   8     centroid_seg_offset      Byte offset (for fast seek)
0x0A0   16    centroid_content_hash    First 128 bits of SHAKE-256 of segment payload
```

The runtime validates:
1. Seek to `centroid_seg_offset`
2. Read segment header + payload
3. Compute SHAKE-256 of payload
4. Compare first 128 bits against `centroid_content_hash`
5. If mismatch: reject pointer, fall back to Level 1 directory scan

This makes compaction physically destructive but logically stable. The manifest re-points by offset for speed but verifies by hash for correctness.

#### 1.2 Centroid Epoch Monotonic Counter

Add to Level 0 root manifest:

```
Offset  Size  Field                    Description
------  ----  -----                    -----------
0x0B0   4     centroid_epoch           Monotonic counter, incremented on recomputation
0x0B4   4     max_epoch_drift          Maximum allowed drift before forced recompute
```

**Semantics**:
- `centroid_epoch` increments each time centroids are recomputed
- The manifest's global `epoch` counter tracks all mutations
- `epoch_drift = manifest.epoch - centroid_epoch`
- If `epoch_drift > max_epoch_drift`: runtime MUST either recompute centroids or widen `n_probe`

Default `max_epoch_drift`: 64 epochs.

#### 1.3 Automatic Quality Elasticity

When epoch drift is detected, the runtime applies controlled quality degradation instead of silent recall loss:

```rust
fn effective_n_probe(base_n_probe: u32, epoch_drift: u32, max_drift: u32) -> u32 {
    if epoch_drift <= max_drift / 2 {
        // Within comfort zone: no adjustment
        base_n_probe
    } else if epoch_drift <= max_drift {
        // Drift zone: linear widening up to 2x
        let scale = 1.0 + (epoch_drift - max_drift / 2) as f64 / max_drift as f64;
        (base_n_probe as f64 * scale).ceil() as u32
    } else {
        // Beyond max drift: double n_probe, schedule recomputation
        base_n_probe * 2
    }
}
```

This turns degradation into **controlled quality elasticity**: recall trades against latency in a predictable, bounded way.

#### 1.4 Wire Format Changes

Add content hash fields to Level 0 at reserved offsets (using the `0x094-0x0FF` reserved region before the signature):

```
Offset  Size  Field
------  ----  -----
0x0A0   16    entrypoint_content_hash
0x0B0   16    toplayer_content_hash
0x0C0   16    centroid_content_hash
0x0D0   16    quantdict_content_hash
0x0E0   16    hot_cache_content_hash
0x0F0   4     centroid_epoch
0x0F4   4     max_epoch_drift
0x0F8   8     reserved_hardening
```

Total: 96 bytes. Fits within the existing reserved region before the signature at `0x094`.

**Note**: The signature field at `0x094` must move to accommodate this. New signature offset: `0x100`. This is a breaking change to the Level 0 layout. Files written before ADR-033 are detected by `version < 2` in the root manifest and use the old layout.

---

### 2. Layer A Adversarial Resilience

**Invariant**: Silent catastrophic degradation must not be possible.

#### 2.1 Distance Entropy Detection

After computing distances to the top-K centroids, measure the discriminative power:

```rust
/// Detect adversarial or degenerate centroid distance distributions.
/// Returns true if the distribution is too uniform to trust centroid routing.
fn is_degenerate_distribution(distances: &[f32], k: usize) -> bool {
    if distances.len() < 2 * k {
        return true; // Not enough centroids
    }

    // Sort and take top-2k
    let mut sorted = distances.to_vec();
    sorted.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap());
    let top = &sorted[..2 * k];

    // Compute coefficient of variation (CV = stddev / mean)
    let mean = top.iter().sum::<f32>() / top.len() as f32;
    if mean < f32::EPSILON {
        return true; // All distances near zero
    }

    let variance = top.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / top.len() as f32;
    let cv = variance.sqrt() / mean;

    // CV < 0.05 means top distances are within 5% of each other
    // This indicates centroids provide no discriminative power
    cv < DEGENERATE_CV_THRESHOLD
}

const DEGENERATE_CV_THRESHOLD: f32 = 0.05;
```

#### 2.2 Adaptive n_probe Widening

When degeneracy is detected, widen the search:

```rust
fn adaptive_n_probe(
    base_n_probe: u32,
    centroid_distances: &[f32],
    total_centroids: u32,
) -> u32 {
    if is_degenerate_distribution(centroid_distances, base_n_probe as usize) {
        // Degenerate: widen to sqrt(K) or 4x base, whichever is smaller
        let widened = (total_centroids as f64).sqrt().ceil() as u32;
        base_n_probe.max(widened).min(base_n_probe * 4)
    } else {
        base_n_probe
    }
}
```

#### 2.3 Multi-Centroid Fallback

When distance variance is below threshold AND Layer B is not yet loaded, fall back to a lightweight multi-probe strategy:

1. Compute distances to ALL centroids (not just top-K)
2. If all distances are within `mean +/- 2*stddev`: treat as uniform
3. For uniform distributions: scan the hot cache linearly (if available)
4. If no hot cache: return results with a `quality_flag = APPROXIMATE` in the response

This prevents silent wrong answers. The caller knows the result quality.

#### 2.4 Quality Flag at the API Boundary

`ResultQuality` is defined at two levels: per-retrieval and per-response.

**Per-retrieval** (internal, attached to each candidate):

```rust
/// Quality confidence for the retrieval candidate set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RetrievalQuality {
    /// Full index traversed, high confidence in candidate set.
    Full = 0x00,
    /// Partial index (Layer A+B), good confidence.
    Partial = 0x01,
    /// Layer A only, moderate confidence.
    LayerAOnly = 0x02,
    /// Degenerate distribution detected, low confidence.
    DegenerateDetected = 0x03,
    /// Brute-force fallback used within budget, exact over scanned region.
    BruteForceBudgeted = 0x04,
}
```

**Per-response** (external, returned to the caller at the API boundary):

```rust
/// Response-level quality signal. This is the field that consumers
/// (RAG pipelines, agent tool chains, MCP clients) MUST inspect.
///
/// If `response_quality < threshold`, the consumer should either:
/// - Wait and retry (progressive loading will improve quality)
/// - Widen the search (increase k or ef_search)
/// - Fall back to an alternative data source
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ResponseQuality {
    /// All results from full index. Trust fully.
    Verified = 0x00,
    /// Results from partial index. Usable but may miss neighbors.
    Usable = 0x01,
    /// Degraded retrieval detected. Results are best-effort.
    /// The `degradation_reason` field explains why.
    Degraded = 0x02,
    /// Insufficient candidates found. Results are unreliable.
    /// Caller SHOULD NOT use these for downstream decisions.
    Unreliable = 0x03,
}
```

**Derivation rule** — `ResponseQuality` is the minimum of all `RetrievalQuality` values in the result set:

```rust
fn derive_response_quality(results: &[SearchResult]) -> ResponseQuality {
    let worst = results.iter()
        .map(|r| r.retrieval_quality)
        .max_by_key(|q| *q as u8)
        .unwrap_or(RetrievalQuality::Full);

    match worst {
        RetrievalQuality::Full => ResponseQuality::Verified,
        RetrievalQuality::Partial => ResponseQuality::Usable,
        RetrievalQuality::LayerAOnly => ResponseQuality::Usable,
        RetrievalQuality::DegenerateDetected => ResponseQuality::Degraded,
        RetrievalQuality::BruteForceBudgeted => ResponseQuality::Degraded,
    }
}
```

**Mandatory outer wrapper** — `QualityEnvelope` is the top-level return type for all
query APIs. It is not a nested field. It is the outer wrapper. JSON flattening cannot
discard it. gRPC serialization cannot drop it. MCP tool responses must include it.

```rust
/// The mandatory outer return type for all query APIs.
/// This is not optional. This is not a nested field.
/// Consumers that ignore this are misusing the API.
pub struct QualityEnvelope {
    /// The search results.
    pub results: Vec<SearchResult>,
    /// Top-level quality signal. Consumers MUST inspect this.
    pub quality: ResponseQuality,
    /// Structured evidence for why the quality is what it is.
    pub evidence: SearchEvidenceSummary,
    /// Resource consumption report for this query.
    pub budgets: BudgetReport,
    /// If quality is degraded, the structured reason.
    pub degradation: Option<DegradationReport>,
}

/// Evidence chain: what index state was actually used.
pub struct SearchEvidenceSummary {
    /// Which index layers were available and used.
    pub layers_used: IndexLayersUsed,
    /// Effective n_probe (after any adaptive widening).
    pub n_probe_effective: u32,
    /// Whether degenerate distribution was detected.
    pub degenerate_detected: bool,
    /// Coefficient of variation of top-K centroid distances.
    pub centroid_distance_cv: f32,
    /// Number of candidates found by HNSW before safety net.
    pub hnsw_candidate_count: u32,
    /// Number of candidates added by safety net scan.
    pub safety_net_candidate_count: u32,
    /// Content hashes of index segments actually touched.
    pub index_segments_touched: Vec<[u8; 16]>,
}

#[derive(Clone, Copy, Debug)]
pub struct IndexLayersUsed {
    pub layer_a: bool,
    pub layer_b: bool,
    pub layer_c: bool,
    pub hot_cache: bool,
}

/// Resource consumption report.
pub struct BudgetReport {
    /// Wall-clock time per stage.
    pub centroid_routing_us: u64,
    pub hnsw_traversal_us: u64,
    pub safety_net_scan_us: u64,
    pub reranking_us: u64,
    pub total_us: u64,
    /// Distance evaluations performed.
    pub distance_ops: u64,
    pub distance_ops_budget: u64,
    /// Bytes read from storage.
    pub bytes_read: u64,
    /// Candidates scanned in linear scan (safety net).
    pub linear_scan_count: u64,
    pub linear_scan_budget: u64,
}

/// Why quality is degraded.
pub struct DegradationReport {
    /// Which fallback path was chosen.
    pub fallback_path: FallbackPath,
    /// Why it was chosen (structured, not prose).
    pub reason: DegradationReason,
    /// What guarantee is lost relative to Full quality.
    pub guarantee_lost: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub enum FallbackPath {
    /// Normal HNSW traversal, no fallback needed.
    None,
    /// Adaptive n_probe widening due to epoch drift.
    NProbeWidened,
    /// Adaptive n_probe widening due to degenerate distribution.
    DegenerateWidened,
    /// Selective safety net scan on hot cache.
    SafetyNetSelective,
    /// Safety net budget exhausted before completion.
    SafetyNetBudgetExhausted,
}

#[derive(Clone, Copy, Debug)]
pub enum DegradationReason {
    /// Centroid epoch drift exceeded threshold.
    CentroidDrift { epoch_drift: u32, max_drift: u32 },
    /// Degenerate distance distribution detected.
    DegenerateDistribution { cv: f32, threshold: f32 },
    /// Brute-force budget exhausted.
    BudgetExhausted { scanned: u64, total: u64, budget_type: &'static str },
    /// Index layer not yet loaded.
    IndexNotLoaded { available: &'static str, needed: &'static str },
}
```

**Hard enforcement rule**: If `quality` is `Degraded` or `Unreliable`, the runtime MUST
either:

1. Return the `QualityEnvelope` with the structured warning (which cannot be dropped
   because it is the outer type, not a nested field), OR
2. Require an explicit caller override flag to proceed:

```rust
pub enum QualityPreference {
    /// Runtime decides. Default. Fastest path that meets internal thresholds.
    Auto,
    /// Caller prefers quality over latency. Runtime may widen n_probe,
    /// extend budgets up to 4x, and block until Layer B loads.
    PreferQuality,
    /// Caller prefers latency over quality. Runtime may skip safety net,
    /// reduce n_probe. ResponseQuality honestly reports what it gets.
    PreferLatency,
    /// Caller explicitly accepts degraded results. Required to proceed
    /// when ResponseQuality would be Degraded or Unreliable under Auto.
    /// Without this flag, Degraded queries return an error, not results.
    AcceptDegraded,
}
```

Without `AcceptDegraded`, a `Degraded` result is returned as
`Err(RvfError::QualityBelowThreshold(envelope))` — the caller gets the evidence
but must explicitly opt in to use the results. This prevents silent misuse.

#### 2.5 Distribution Assumption Declaration

The spec MUST explicitly state:

> **Distribution Assumption**: Recall targets (0.70/0.85/0.95) assume sub-Gaussian embedding distributions typical of neural network outputs (sentence-transformers, OpenAI ada-002, Cohere embed-v3, etc.). For adversarial, synthetic, or uniform-random distributions, recall may be lower. When degenerate distributions are detected at query time, the runtime automatically widens its search and signals reduced confidence via `ResultQuality`.

This converts an implicit assumption into an explicit contract.

---

### 3. Recall Bound Framing

**Invariant**: Never claim theoretical guarantees without distribution assumptions.

#### 3.1 Monotonic Recall Improvement Property

Replace hard recall bounds with a provable structural property:

> **Monotonic Recall Property**: For any query Q and any two index states S1 and S2 where S2 includes all segments of S1 plus additional INDEX_SEGs:
>
> `recall(Q, S2) >= recall(Q, S1)`
>
> Proof: S2's candidate set is a superset of S1's (append-only segments, no removal). More candidates cannot reduce recall.

This is provable from the append-only invariant and requires no distribution assumption.

#### 3.2 Recall Target Classes

Replace the single recall table with benchmark-class-specific targets:

| State | Natural Embeddings | Synthetic Uniform | Adversarial Clustered |
|-------|-------------------|-------------------|----------------------|
| Layer A | >= 0.70 | >= 0.40 | >= 0.20 (with detection) |
| A + B | >= 0.85 | >= 0.70 | >= 0.60 |
| A + B + C | >= 0.95 | >= 0.90 | >= 0.85 |

"Natural Embeddings" = sentence-transformers, OpenAI, Cohere on standard corpora.

#### 3.3 Brute-Force Safety Net (Dual-Budgeted)

When the candidate set from HNSW search is smaller than `2 * k`, the safety net
activates. It is capped by **both** a time budget and a candidate budget to prevent
unbounded work. An adversarial query cannot force O(N) compute.

**Three required caps** (all enforced, none optional):

```rust
/// Budget caps for the brute-force safety net.
/// All three are enforced simultaneously. The scan stops at whichever hits first.
/// These are RUNTIME limits, not caller-adjustable above the defaults.
/// Callers may reduce them but not exceed them (unless PreferQuality mode,
/// which extends to 4x).
pub struct SafetyNetBudget {
    /// Maximum wall-clock time for the safety net scan.
    /// Default: 2,000 us (2 ms) in Layer A mode, 5,000 us (5 ms) in partial mode.
    pub max_scan_time_us: u64,
    /// Maximum number of candidate vectors to scan.
    /// Default: 10,000 in Layer A mode, 50,000 in partial mode.
    pub max_scan_candidates: u64,
    /// Maximum number of distance evaluations (the actual compute cost).
    /// This is the hardest cap — it bounds CPU work directly.
    /// Default: 10,000 in Layer A mode, 50,000 in partial mode.
    pub max_distance_ops: u64,
}

impl SafetyNetBudget {
    /// Layer A only defaults: tight budget for instant first query.
    pub const LAYER_A: Self = Self {
        max_scan_time_us: 2_000,     // 2 ms
        max_scan_candidates: 10_000,
        max_distance_ops: 10_000,
    };
    /// Partial index defaults: moderate budget.
    pub const PARTIAL: Self = Self {
        max_scan_time_us: 5_000,     // 5 ms
        max_scan_candidates: 50_000,
        max_distance_ops: 50_000,
    };
    /// PreferQuality mode: 4x extension of the applicable default.
    pub fn extended_4x(&self) -> Self {
        Self {
            max_scan_time_us: self.max_scan_time_us * 4,
            max_scan_candidates: self.max_scan_candidates * 4,
            max_distance_ops: self.max_distance_ops * 4,
        }
    }
    /// Disabled: all zeros. Safety net will not scan anything.
    pub const DISABLED: Self = Self {
        max_scan_time_us: 0,
        max_scan_candidates: 0,
        max_distance_ops: 0,
    };
}
```

All three are in `QueryOptions`:

```rust
pub struct QueryOptions {
    pub k: usize,
    pub ef_search: u32,
    pub quality_preference: QualityPreference,
    /// Safety net budget. Callers may tighten but not loosen beyond
    /// the mode default (unless QualityPreference::PreferQuality).
    pub safety_net_budget: SafetyNetBudget,
}
```

**Policy response**: When any budget is exceeded, the scan stops immediately and returns:
- `FallbackPath::SafetyNetBudgetExhausted`
- `DegradationReason::BudgetExhausted` with which budget triggered and how far the scan got
- A partial candidate set (whatever was found before the budget hit)
- `ResponseQuality::Degraded`

**Selective scan strategy** — the safety net does NOT scan the entire hot cache. It
scans a targeted subset to stay sparse even under fallback:

```rust
fn selective_safety_net_scan(
    query: &[f32],
    k: usize,
    hnsw_candidates: &[Candidate],
    centroid_distances: &[(u32, f32)], // (centroid_id, distance)
    store: &RvfStore,
    budget: &SafetyNetBudget,
) -> (Vec<Candidate>, BudgetReport) {
    let deadline = Instant::now() + Duration::from_micros(budget.max_scan_time_us);
    let mut scanned: u64 = 0;
    let mut dist_ops: u64 = 0;
    let mut candidates = Vec::new();
    let mut budget_report = BudgetReport::default();

    // Phase 1: Multi-centroid union
    // Scan hot cache entries whose centroid_id is in top-T centroids.
    // T = min(adaptive_n_probe, sqrt(total_centroids))
    let top_t = centroid_distances.len().min(
        (centroid_distances.len() as f64).sqrt().ceil() as usize
    );
    let top_centroid_ids: Vec<u32> = centroid_distances[..top_t]
        .iter().map(|(id, _)| *id).collect();

    for block in store.hot_cache_blocks_by_centroid(&top_centroid_ids) {
        if scanned >= budget.max_scan_candidates { break; }
        if dist_ops >= budget.max_distance_ops { break; }
        if Instant::now() >= deadline { break; }

        let block_results = scan_block(query, block);
        scanned += block.len() as u64;
        dist_ops += block.len() as u64;
        candidates.extend(block_results);
    }

    // Phase 2: HNSW neighbor expansion
    // For each existing HNSW candidate, scan their neighbors' vectors
    // in the hot cache (1-hop expansion).
    if scanned < budget.max_scan_candidates && dist_ops < budget.max_distance_ops {
        for candidate in hnsw_candidates.iter().take(k) {
            if scanned >= budget.max_scan_candidates { break; }
            if dist_ops >= budget.max_distance_ops { break; }
            if Instant::now() >= deadline { break; }

            if let Some(neighbors) = store.hot_cache_neighbors(candidate.id) {
                for neighbor in neighbors {
                    if dist_ops >= budget.max_distance_ops { break; }
                    let d = distance(query, &neighbor.vector);
                    dist_ops += 1;
                    scanned += 1;
                    candidates.push(Candidate { id: neighbor.id, distance: d });
                }
            }
        }
    }

    // Phase 3: Recency window (if budget remains)
    // Scan the most recently ingested vectors in the hot cache,
    // which are most likely to be missing from the HNSW index.
    if scanned < budget.max_scan_candidates && dist_ops < budget.max_distance_ops {
        let remaining_budget = budget.max_scan_candidates - scanned;
        for vec in store.hot_cache_recent(remaining_budget as usize) {
            if dist_ops >= budget.max_distance_ops { break; }
            if Instant::now() >= deadline { break; }
            let d = distance(query, &vec.vector);
            dist_ops += 1;
            scanned += 1;
            candidates.push(Candidate { id: vec.id, distance: d });
        }
    }

    budget_report.linear_scan_count = scanned;
    budget_report.linear_scan_budget = budget.max_scan_candidates;
    budget_report.distance_ops = dist_ops;
    budget_report.distance_ops_budget = budget.max_distance_ops;

    (candidates, budget_report)
}
```

**Why selective, not exhaustive:**

The safety net scans three targeted sets in priority order:
1. **Multi-centroid union**: vectors near the best-matching centroids (spatial locality)
2. **HNSW neighbor expansion**: 1-hop neighbors of existing candidates (graph locality)
3. **Recency window**: recently ingested vectors not yet in any index (temporal locality)

Each phase respects all three budget caps. Even under the safety net, the scan stays
**sparse and deterministic**.

**Why three budget caps:**

- **Time alone** is insufficient: fast CPUs burn millions of ops in 5 ms.
- **Candidates alone** is insufficient: slow storage makes 50K scans take 50 ms.
- **Distance ops alone** is insufficient: a scan that reads but doesn't compute still
  consumes I/O bandwidth.
- **All three together** bound the work in every dimension. The scan stops at whichever
  limit hits first.

**Invariant**: The brute-force safety net is bounded in time, candidates, and compute.
A fuzzed query generator cannot push p95 latency above the budgeted ceiling. If all
three budgets are set to 0, the safety net is disabled entirely and the system returns
`ResponseQuality::Degraded` immediately when HNSW produces insufficient candidates.

#### 3.3.1 DoS Hardening

Three additional protections for public-facing deployments:

**Budget tokens**: Each query consumes a fixed budget of distance ops and bytes. The
runtime tracks a per-connection token bucket. No tokens remaining = query rejected with
`429 Too Many Requests` equivalent. Prevents sustained DoS via repeated adversarial queries.

**Negative caching**: If a query signature (hash of the query vector's quantized form)
triggers degenerate mode more than N times in a window, the runtime caches it and forces
`SafetyNetBudget::DISABLED` for subsequent matches. The adversary cannot keep burning budget
on the same attack vector.

**Proof-of-work option**: For open-internet endpoints only. The caller must include a
nonce proving O(work) computation before the query is accepted. This is opt-in, not
default — only relevant for unauthenticated public endpoints.

#### 3.4 Acceptance Test Update

Update `benchmarks/acceptance-tests.md` to:

1. Test against three distribution classes (natural, synthetic, adversarial)
2. Verify `ResponseQuality` flag accuracy at the API boundary
3. Verify monotonic recall improvement across progressive load phases
4. Measure brute-force fallback frequency and latency impact
5. Verify brute-force scan terminates within both time and candidate budgets

#### 3.5 Acceptance Test: Malicious Tail Manifest (MANDATORY)

**Test**: A maliciously rewritten tail manifest that preserves CRC32C but
changes hotset pointers must fail to mount under `Strict` policy, and must
produce a logged, deterministic failure reason.

```
Test: Malicious Hotset Pointer Redirection
==========================================

Setup:
  1. Create signed RVF file with 100K vectors, full HNSW index
  2. Record the original centroid_seg_offset and centroid_content_hash
  3. Identify a different valid INDEX_SEG in the file (e.g., Layer B)
  4. Craft a new Level 0 manifest:
     - Replace centroid_seg_offset with the Layer B segment offset
     - Keep ALL other fields identical
     - Recompute CRC32C at 0xFFC to match the modified manifest
     - Do NOT re-sign (signature becomes invalid)
  5. Overwrite last 4096 bytes of file with crafted manifest

Verification under Strict policy:
  1. Attempt: RvfStore::open_with_policy(&path, opts, SecurityPolicy::Strict)
  2. MUST return Err(SecurityError::InvalidSignature)
  3. The error MUST include:
     - error_code: a stable, documented error code (not just a string)
     - manifest_offset: byte offset of the rejected manifest
     - expected_signer: public key fingerprint (if known)
     - rejection_phase: "signature_verification" (not "content_hash")
  4. The error MUST be logged at WARN level or higher
  5. The file MUST NOT be queryable (no partial mount, no fallback)

Verification under Paranoid policy:
  Same as Strict, identical behavior.

Verification under WarnOnly policy:
  1. File opens successfully (warning logged)
  2. Content hash verification runs on first hotset access
  3. centroid_content_hash mismatches the actual segment payload
  4. MUST return Err(SecurityError::ContentHashMismatch) on first query
  5. The error MUST include:
     - pointer_name: "centroid_seg_offset"
     - expected_hash: the hash stored in Level 0
     - actual_hash: the hash of the segment at the pointed offset
     - seg_offset: the byte offset that was followed
  6. System transitions to read-only mode, refuses further queries

Verification under Permissive policy:
  1. File opens successfully (no warning)
  2. Queries execute against the wrong segment
  3. Results are structurally valid but semantically wrong
  4. ResponseQuality is NOT required to detect this (Permissive = no safety)
  5. This is the EXPECTED AND DOCUMENTED behavior of Permissive mode

Pass criteria:
  - Strict/Paranoid: deterministic rejection, logged error, no mount
  - WarnOnly: mount succeeds, content hash catches mismatch on first access
  - Permissive: mount succeeds, no detection (by design)
  - Error messages are stable across versions (code, not prose)
  - No panic, no undefined behavior, no partial state leakage
```

**Test: Malicious Manifest with Re-signed Forgery**

```
Setup:
  1. Same as above, but attacker also re-signs with a DIFFERENT key
  2. File now has valid CRC32C AND valid signature — but wrong signer

Verification under Strict policy:
  1. MUST return Err(SecurityError::UnknownSigner)
  2. Error includes the actual signer fingerprint
  3. Error includes the expected signer fingerprint (from trust store)
  4. File does not mount

Pass criteria:
  - The system distinguishes "no signature" from "wrong signer"
  - Both produce distinct, documented error codes
```

#### 3.6 Acceptance Tests: QualityEnvelope Enforcement (MANDATORY)

**Test 1: Consumer Cannot Ignore QualityEnvelope**

```
Test: Schema Enforcement of QualityEnvelope
============================================

Setup:
  1. Create RVF file with 10K vectors, full index
  2. Issue a query that returns Degraded results (use degenerate query vector)

Verification:
  1. The query API returns QualityEnvelope, not Vec<SearchResult>
  2. Attempt to deserialize the response as Vec<SearchResult> (without envelope)
  3. MUST fail at schema validation — the envelope is the outer type
  4. JSON response: top-level keys MUST include "quality", "evidence", "budgets"
  5. gRPC response: QualityEnvelope is the response message type
  6. MCP tool response: "quality" field is at top level, not nested

Pass criteria:
  - No API path exists that returns raw results without the envelope
  - Schema validation rejects any consumer that skips the quality field
  - The envelope cannot be flattened away by middleware or serialization
```

**Test 2: Adversarial Query Respects max_distance_ops Under Safety Net**

```
Test: Budget Cap Enforcement Under Adversarial Query
=====================================================

Setup:
  1. Create RVF file with 1M vectors, Layer A only (no HNSW loaded)
  2. Set SafetyNetBudget to LAYER_A defaults (10,000 distance ops)
  3. Craft adversarial query that triggers degenerate detection
     (uniform-random vector or equidistant from all centroids)

Verification:
  1. Issue query with quality_preference = Auto
  2. Safety net activates (candidate set < 2*k from HNSW)
  3. BudgetReport.distance_ops MUST be <= SafetyNetBudget.max_distance_ops
  4. BudgetReport.distance_ops MUST be <= 10,000
  5. Total query wall-clock MUST be <= SafetyNetBudget.max_scan_time_us
  6. DegradationReport.reason MUST be BudgetExhausted if budget was hit
  7. ResponseQuality MUST be Degraded (not Verified or Usable)

Stress test:
  1. Repeat with 10,000 adversarial queries in sequence
  2. No single query may exceed max_distance_ops
  3. Aggregate p95 latency MUST stay below max_scan_time_us ceiling
  4. No OOM, no panic, no unbounded allocation

Pass criteria:
  - max_distance_ops is a hard cap, never exceeded by even 1 operation
  - Budget enforcement works under all three safety net phases
  - Each phase independently respects all three budget caps
```

**Test 3: Degenerate Conditions Produce Partial Results, Not Hangs**

```
Test: Graceful Degradation Under Degenerate Conditions
=======================================================

Setup:
  1. Create RVF file with 1M uniform-random vectors (worst case)
  2. Load with Layer A only (no HNSW, no Layer B/C)
  3. All centroids equidistant from query (maximum degeneracy)

Verification:
  1. Issue query with quality_preference = Auto
  2. Runtime MUST return within max_scan_time_us (not hang)
  3. Return type MUST be Err(RvfError::QualityBelowThreshold(envelope))
  4. The envelope MUST contain:
     a. A partial result set (whatever was found before budget hit)
     b. quality = ResponseQuality::Degraded or Unreliable
     c. degradation.reason = BudgetExhausted or DegenerateDistribution
     d. degradation.guarantee_lost describes what is missing
     e. budgets.distance_ops <= budgets.distance_ops_budget
  5. The caller can then choose:
     a. Retry with PreferQuality (extends budget 4x)
     b. Retry with AcceptDegraded (uses partial results as-is)
     c. Wait for Layer B to load and retry

  6. With AcceptDegraded:
     a. Same partial results are returned as Ok(envelope)
     b. ResponseQuality is still Degraded (honesty preserved)
     c. No additional scanning beyond what was already done

Pass criteria:
  - No hang, no scan-to-completion, no unbounded work
  - Partial results are always available (not empty unless truly zero candidates)
  - Clear, structured reason for degradation (not a string, a typed enum)
  - Caller can always recover by choosing a different QualityPreference
```

#### 3.7 Benchmark: Fuzzed Query Latency Ceiling (MANDATORY)

```
Benchmark: Fuzzed Query Generator vs Budget Ceiling
=====================================================

Setup:
  1. Create RVF file with 10M vectors, 384 dimensions, fp16
  2. Generate a fuzzed query corpus:
     a. 1000 natural embedding queries (sentence-transformer outputs)
     b. 1000 uniform-random queries
     c. 1000 adversarial queries (equidistant from top-K centroids)
     d. 1000 degenerate queries (zero vector, max-norm vector, NaN-adjacent)
  3. Load file progressively: measure at Layer A, A+B, A+B+C

Test:
  1. Execute all 4000 queries at each progressive load stage
  2. Measure p50, p95, p99, max latency per query class per stage

Pass criteria:
  - p95 latency MUST NOT exceed SafetyNetBudget.max_scan_time_us at any stage
  - p99 latency MUST NOT exceed 2x SafetyNetBudget.max_scan_time_us at any stage
    (allowing for OS scheduling jitter, not algorithmic overshoot)
  - max_distance_ops is NEVER exceeded (hard invariant, no exceptions)
  - Recall improves monotonically across stages for all query classes:
    recall@10(Layer A) <= recall@10(A+B) <= recall@10(A+B+C)
  - No query class achieves recall@10 = 0.0 at any stage
    (even degenerate queries must return SOME results)

Report:
  JSON report per stage with:
    stage, query_class, p50_us, p95_us, p99_us, max_us,
    avg_recall_at_10, min_recall_at_10, avg_distance_ops,
    max_distance_ops, safety_net_trigger_rate, budget_exhaustion_rate
```

---

### 4. Mandatory Manifest Signatures

**Invariant**: No signature, no mount in secure mode.

#### 4.1 Security Mount Policy

Add a `SecurityPolicy` enum to `RvfOptions`:

```rust
/// Manifest signature verification policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SecurityPolicy {
    /// No signature verification. For development and testing only.
    /// Files open regardless of signature state.
    Permissive = 0x00,
    /// Warn on missing or invalid signatures, but allow open.
    /// Log events for auditing.
    WarnOnly = 0x01,
    /// Require valid signature on Level 0 manifest.
    /// Reject files with missing or invalid signatures.
    /// DEFAULT for production.
    Strict = 0x02,
    /// Require valid signatures on Level 0, Level 1, and all
    /// hotset-referenced segments. Full chain verification.
    Paranoid = 0x03,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::Strict
    }
}
```

**Default is `Strict`**, not `Permissive`.

#### 4.2 Verification Chain

Under `Strict` policy, the open path becomes:

```
1. Read Level 0 (4096 bytes)
2. Validate CRC32C (corruption check)
3. Validate ML-DSA-65 signature (adversarial check)
4. If signature missing: REJECT with SecurityError::UnsignedManifest
5. If signature invalid: REJECT with SecurityError::InvalidSignature
6. Extract hotset pointers
7. For each hotset pointer: validate content hash (ADR-033 §1.1)
8. If any content hash fails: REJECT with SecurityError::ContentHashMismatch
9. System is now queryable with verified pointers
```

Under `Paranoid` policy, add:

```
10. Read Level 1 manifest
11. Validate Level 1 signature
12. For each segment in directory: verify content hash matches on first access
```

#### 4.3 Unsigned File Handling

Files without signatures can still be opened under `Permissive` or `WarnOnly` policies. This supports:

- Development and testing workflows
- Legacy files created before signature support
- Performance-critical paths where verification latency is unacceptable

But the default is `Strict`. If an enterprise deploys with defaults, they get signature enforcement. They must explicitly opt out.

#### 4.4 Signature Generation on Write

Every `write_manifest()` call MUST:

1. Compute SHAKE-256-256 content hashes for all hotset-referenced segments
2. Store hashes in Level 0 at the new offsets (§1.4)
3. If a signing key is available: sign Level 0 with ML-DSA-65
4. If no signing key: write `sig_algo = 0` (unsigned)

The `create()` and `open()` methods accept an optional signing key:

```rust
impl RvfStore {
    pub fn create_signed(
        path: &Path,
        options: RvfOptions,
        signing_key: &MlDsa65SigningKey,
    ) -> Result<Self, RvfError>;
}
```

#### 4.5 Runtime Policy Flag

The security policy is set at store open time and cannot be downgraded:

```rust
let store = RvfStore::open_with_policy(
    &path,
    RvfOptions::default(),
    SecurityPolicy::Strict,
)?;
```

A store opened with `Strict` policy will reject any hotset pointer that fails content hash verification, even if the CRC32C passes. This prevents the segment-swap attack identified in the analysis.

---

## Consequences

### Positive

- Centroid stability becomes a **logical invariant**, not a physical accident
- Adversarial distribution degradation becomes **detectable and bounded**
- Recall claims become **honest** — empirical targets with explicit assumptions
- Manifest integrity becomes **mandatory by default** — enterprises are secure without configuration
- Quality elasticity replaces silent degradation — the system tells you when it's uncertain

### Negative

- Level 0 layout change is **breaking** (version 1 -> version 2)
- Content hash computation adds ~50 microseconds per manifest write
- Strict signature policy adds ~200 microseconds per file open (ML-DSA-65 verify)
- Adaptive n_probe increases query latency by up to 4x under degenerate distributions

### Migration

- Level 0 version field (`0x004`) distinguishes v1 (pre-ADR-033) from v2
- v1 files are readable under `Permissive` policy (no content hashes, no signature)
- v1 files trigger a warning under `WarnOnly` policy
- v1 files are rejected under `Strict` policy unless explicitly migrated
- Migration tool: `rvf migrate --sign --key <path>` rewrites manifest with v2 layout

---

## Size Impact

| Component | Additional Bytes | Where |
|-----------|-----------------|-------|
| Content hashes (5 pointers * 16 bytes) | 80 B | Level 0 manifest |
| Centroid epoch + drift fields | 8 B | Level 0 manifest |
| ResponseQuality + DegradationReason | ~64 B | Per query response |
| SecurityPolicy in options | 1 B | Runtime config |
| Total Level 0 overhead | 96 B | Within existing 4096 B page |

No additional segments. No file size increase beyond the 96 bytes in Level 0.

---

## Implementation Order

| Phase | Component | Estimated Effort |
|-------|-----------|-----------------|
| 1 | Content hash fields in `rvf-types` Level 0 layout | Small |
| 2 | `centroid_epoch` + `max_epoch_drift` in manifest | Small |
| 3 | `ResultQuality` enum in `rvf-runtime` | Small |
| 4 | `is_degenerate_distribution()` + adaptive n_probe | Medium |
| 5 | Content hash verification in read path | Medium |
| 6 | `SecurityPolicy` enum + enforcement in open path | Medium |
| 7 | ML-DSA-65 signing in write path | Large (depends on rvf-crypto) |
| 8 | Brute-force safety net in query path | Medium |
| 9 | Acceptance test updates (3 distribution classes) | Medium |
| 10 | Migration tool (`rvf migrate --sign`) | Medium |

---

## References

- RVF Spec 02: Manifest System (hotset pointers, Level 0 layout)
- RVF Spec 04: Progressive Indexing (Layer A/B/C recall targets)
- RVF Spec 03: Temperature Tiering (centroid refresh, sketch epochs)
- ADR-029: RVF Canonical Format (universal adoption across libraries)
- ADR-030: Cognitive Container (three-tier execution model)
- FIPS 204: ML-DSA (Module-Lattice Digital Signature Algorithm)
- Malkov & Yashunin (2018): HNSW search complexity analysis
