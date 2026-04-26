# ADR-085: RaBitQ Similarity Sensor — Pipeline Expansion (Seven Additional Sites)

| Field          | Value                                                                                                                                          |
|----------------|------------------------------------------------------------------------------------------------------------------------------------------------|
| **Status**     | Proposed                                                                                                                                       |
| **Date**       | 2026-04-25                                                                                                                                     |
| **Authors**    | ruv                                                                                                                                            |
| **Refines**    | ADR-084 (RaBitQ similarity sensor, five-site baseline)                                                                                         |
| **Touches**    | ADR-027 (cross-environment generalization), ADR-028 (capability audit / witness bundle), ADR-066 (swarm-bridge to coordinator), ADR-073 (multifrequency mesh scan), ADR-076 (CSI spectrogram embeddings), ADR-081 (5-layer firmware kernel), ADR-082 (confirmed-track filter), ADR-083 (per-cluster Pi compute hop) |
| **Companion**  | `v2/crates/wifi-densepose-ruvector/src/sketch.rs` (ADR-084 Pass 1 — `Sketch`, `SketchBank`, `SketchError`; on branch `feat/adr-084-pass-1-sketch-module`, commits `6fd5b7d` + `1df9d5f7d`) |

## Context

ADR-084 committed RuView to **RaBitQ-style binary sketches as a cheap
similarity sensor** (Gao & Long, SIGMOD 2024 — arxiv 2405.12497) at
five pipeline sites: AETHER re-ID pre-filter, cluster-Pi novelty,
mincut subcarrier maintenance, mesh-exchange compression, and the
privacy-preserving event log. Pass 1 of that work landed the
`wifi-densepose-ruvector::sketch` module and benched at **43–51×
compare speedup at d=512** and **7.5× top-K speedup at k=8 over 1024
sketches** — comfortably above the ADR-084 acceptance threshold of
8×. The sketch primitive is no longer an open question; the question
is where else in the pipeline the same sensor pattern earns its keep.

Seven additional sites have been identified, all outside the ADR-084
five but matching the same shape — code that asks "is this familiar?"
against a stored set, today by way of a full float compare or model
invocation. The unifying rule articulated alongside ADR-084 — *sketch
first, refine on miss, store the witness hash instead of the raw
embedding* — applies to all seven.

This ADR formalizes those seven sites in one document rather than
seven small ADRs because (a) they share one primitive and one
acceptance shape, so evaluating in isolation hides the pattern;
(b) most involve modest code surgery (< 200 LOC at the call site)
and an ADR-per-site would inflate the ledger without buying
decision-resolution; (c) the few sites that *do* raise novel
questions (Mahalanobis pre-filtering, REST similarity API shape,
witness-hash format for non-vector data) are flagged under Open
Questions and may spin out as follow-ups if their answers prove
load-bearing. ADR-084 owns the primitive; ADR-085 owns the
*deployment surface*.

## Decision

Apply the ADR-084 sketch sensor pattern at seven additional sites,
listed in the order they will be implemented (cheapest-first /
lowest-risk-first). Each entry states (a) **what is sketched**,
(b) **what triggers the comparison**, (c) **what the refinement step
on a miss is**, and (d) **what artifact stands in for the raw
embedding** — i.e., the witness hash.

### Site 1 — Per-room adaptive classifier short-circuit

**Crate:** `wifi-densepose-sensing-server` —
`src/adaptive_classifier.rs::classify` (per-class centroids and spread,
Mahalanobis-like distance per frame).

- **Sketched:** Each per-class centroid `µ_k` (already a fixed-dim
  feature vector). Sketches live in a `SketchBank` keyed by class id,
  rebuilt whenever a class is re-trained.
- **Trigger:** Every classification call, before the float Mahalanobis
  distance loop runs.
- **Refinement on miss / first cut:** Hamming top-K (K = 3) selects
  candidate classes; full Mahalanobis runs only on those K. If the
  hamming top-1 disagrees with the eventual Mahalanobis winner, log
  the disagreement and fall back to full evaluation against all
  classes for that frame.
- **Witness hash:** `sha256(centroid_bytes || spread_bytes ||
  sketch_version)` per class, recorded once at classifier-train time
  and stored alongside the sketch.

The sketch only narrows; Mahalanobis still decides on the K
candidates, preserving the original distance-to-class semantics.
Substituting Mahalanobis for the standard RaBitQ exact-distance
re-rank step (Gao & Long 2024) is, to our knowledge, novel — Open Q1.

### Site 2 — Recording-search REST endpoint

**Crate:** `wifi-densepose-sensing-server` —
`src/recording.rs` plus a new HTTP handler in `src/main.rs`.

- **Sketched:** Each recording's pooled CSI/embedding signature (mean
  AETHER embedding over the recording, or mean spectrogram embedding
  per ADR-076). One sketch per recording, stored next to the recording
  metadata.
- **Trigger:** `GET /api/v1/recordings/similar?to=<id>&k=N` request.
- **Refinement on miss:** Hamming top-K returns a candidate list of
  recording ids. Full embedding refinement is **opt-in** via a
  `&refine=true` query param that loads the candidate recordings'
  full embeddings (if stored) and re-ranks. Default behavior is
  sketch-only — the endpoint trades exact ranking for the ability to
  ship without storing full embeddings server-side.
- **Witness hash:** `sha256(sketch_bytes || recording_id ||
  sketch_version)` returned in the response payload as the result row
  identifier. The raw embedding is **not retained** by default; the
  hash is the artifact a client can use to assert which sketch
  produced the match.

Delivers "find recordings that look like this one" without
long-term embedding storage. The shape is closer to SimHash dedup
APIs than to Qdrant's `/collections/{name}/points/search` (the
closest Rust-native vector-DB endpoint, which returns full vectors)
— deliberate; see Open Q4.

### Site 3 — WiFi BSSID fingerprinting (channel-hop scheduler input)

**Crate:** `wifi-densepose-wifiscan` —
new `bssid_sketch` module beside the existing scan/result types.

- **Sketched:** A short per-BSSID time-series feature vector — recent
  RSSI, SNR, channel, beacon interval, capability flags — pooled over
  a rolling window (e.g., last 60 s). One sketch per (BSSID, window).
- **Trigger:** Each scan tick, after the multi-BSSID scan completes.
  The current window's sketch is compared against the prior window's
  bank.
- **Refinement on miss:** A sketch whose nearest neighbor's hamming
  distance exceeds a threshold flags the BSSID as **novel** (newly
  appeared, or known-AP-changed-beyond-recognition). The hop scheduler
  (ADR-073) reads novelty as a hint to give the affected channel
  more dwell time on the next rotation.
- **Witness hash:** `sha256(bssid || pooled_features || sketch_version
  || window_end_unix)` stored in the per-AP novelty log; raw
  per-BSSID time series is dropped after the sketch is taken.

Anomaly detection over a heterogeneous low-dim vector; acceptance
is **false-positive rate on stable deployments**, not top-K
coverage. IEEE 802.11bf-2025 (published March 2025) standardizes
sensing measurement frames but not BSSID-novelty heuristics, so
this site does not duplicate the standard's scope.

### Site 4 — mmWave radar signature memory

**Crate:** `wifi-densepose-vitals` —
`src/preprocessor.rs` and `src/anomaly.rs` (LD2410 / MR60BHA2 input
path).

- **Sketched:** A per-frame radar signature vector — range bins,
  Doppler bins, peak frequencies — sketched at the same cadence as
  the radar input (~10 Hz).
- **Trigger:** Every incoming radar frame, before the heavy vital
  signs DSP runs. The current sketch is compared against a small
  per-room "have we seen this kind of frame before" bank.
- **Refinement on miss:** A sketch within hamming distance of a known
  signature short-circuits to "no new event"; vital signs DSP stays
  asleep. A sketch beyond threshold wakes the full breathing/heart
  pipeline (`vitals::breathing`, `vitals::heartrate`) for one or more
  frames, then re-sleeps once the bank update settles.
- **Witness hash:** `sha256(signature_bytes || sensor_kind ||
  sketch_version)` stored in the vitals event log; the raw radar
  frame is not retained beyond the rolling preprocessor buffer.

Energy is the headline: vital signs DSP (band-pass + phase-fusion +
heart/breath FFT) is the most expensive cluster-Pi operation per
minute of quiet-room time. Published FMCW pipelines treat the DSP
stage as always-on after presence; **no primary source** found for
"binary-sketch wake-gate over a per-room radar signature bank" —
this is a direct extension of ADR-084's novelty sensor.

### Site 5 — Witness bundle similarity (ADR-028 release-CI signal)

**Crate:** Out-of-tree — addition to `scripts/generate-witness-bundle.sh`
plus a new `scripts/witness_drift_check.py`.

- **Sketched:** Each release's witness bundle "fingerprint" — a fixed
  vector built from per-component SHA-256 prefixes plus numeric
  attestation values (test count, proof hash byte-segments,
  per-firmware sizes). One sketch per release.
- **Trigger:** Run during the CI release job, after the witness
  bundle is generated and before publication.
- **Refinement on miss:** A sketch whose hamming distance to the prior
  release exceeds threshold flags the release as **drifted** and
  surfaces the changed components in the CI summary. The release is
  not blocked; the signal is a ratchet that says "these components
  changed by more than the recent baseline, take a second look."
- **Witness hash:** `sha256(sketch_bytes || release_tag ||
  sketch_version)` published alongside the witness bundle as
  `WITNESS-LOG-<sha>.sketch`. The full bundle is the existing artifact;
  the sketch hash is a 32-byte add-on.

Conservative use of the sensor — drift detection over a *very*
small candidate set (last 5–10 releases). Existing CI drift prior
art is autoencoder/SHAP-based commit-anomaly detection plus
PKI-signed artifact integrity; **no primary source** for
"binary-sketch over release-bundle fingerprint" as a CI signal.
Acceptance: "useful ratchet without false-firing on every
dependency bump." If no, the sketch step drops from the release
script — most readily revertible of the seven.

### Site 6 — Agent / swarm memory routing

**Crate:** `wifi-densepose-sensing-server` —
`src/multistatic_bridge.rs` (ADR-066 swarm-bridge channel) and the
peer Cognitum Seed registration metadata.

- **Sketched:** Each Cognitum Seed's accumulated **historical bank**
  signature — a pooled mean of the sketches it has stored over a
  rolling horizon. One sketch per peer Seed; refreshed at peer
  heartbeat cadence.
- **Trigger:** A sensor node escalates an event to the swarm. Before
  broadcasting to all peer Seeds, the cluster Pi computes the event's
  sketch and routes it to the **closest peer** by hamming distance.
- **Refinement on miss:** No nearby peer (all hammings above threshold)
  → broadcast to all. Nearby peer hits → unicast to that Seed first;
  only escalate to broadcast if the routed Seed cannot resolve.
- **Witness hash:** `sha256(event_sketch || origin_seed_id ||
  routed_seed_id || sketch_version || event_unix)` recorded in the
  swarm-bridge audit log. The full event sketch is exchanged; the
  hash is the routing-decision attestation.

A 12-Seed swarm broadcasting every event is O(n) message storm per
event; sketch-routing turns the common case into O(1) with O(n)
fallback. Closest published comparator: **MasRouter** (ACL 2025),
which routes LLM queries via a learned DeBERTa router; ADR-085's
variant is structurally similar but uses unlearned hamming compare
against each peer's pooled bank — cheaper, and resilient to peer
churn.

### Site 7 — Log / event-stream pattern detection

**Crate:** `wifi-densepose-sensing-server` —
new `src/event_anomaly.rs` module reading the cluster Pi's
existing event stream.

- **Sketched:** A pooled feature vector over the recent-events window
  (last hour by default) — counts per event type, mean inter-event
  interval, sources distribution. One sketch per cluster, refreshed
  every 5 minutes.
- **Trigger:** Every refresh tick. The current-hour sketch is compared
  against the historical bank (last 24 hours of hourly sketches).
- **Refinement on miss:** Hamming distance above threshold flags the
  hour as **anomalous behavior**; the cluster Pi raises a single
  cluster-level alert with a pointer to the witness hash, **not** to
  the raw events. No raw events leave the Pi as part of the alert
  payload.
- **Witness hash:** `sha256(hourly_sketch || cluster_id || hour_unix
  || sketch_version)` recorded as the alert body. Raw events stay on
  the cluster Pi behind the existing privacy boundary.

The most genuinely "anomaly detection" of the seven, and most
exposed to the non-vector witness-hash open question (event
features are mixed counts and rates needing normalization before
sketching). Closest published comparator: **LogAI** (Salesforce,
Drain parser → counter vectors → unsupervised detection); ADR-085's
variant sketches the counter vector, trading recall for constant
memory and sub-ms compare on the cluster Pi.

### Witness-hash discipline

In every site above, the witness hash replaces the raw embedding /
feature vector at the storage boundary — the same privacy posture
ADR-084 introduced for the cluster-Pi event log, generalized across
seven new contexts. The format is uniform:
`sha256(sketch_bytes || stable_metadata || sketch_version)`. Where
the input is not natively a dense vector (Sites 5 and 7), the
encoding into a sketchable shape is itself a design choice — see
Open Questions.

## Consequences

### Positive

- **The "is this familiar?" pattern becomes a first-class deployment
  primitive across REST APIs, scanning subsystems, mmWave gating,
  CI, swarm routing, and event analytics.** Each site is a modest
  win individually; together they remove the last excuses to keep
  full embeddings on every storage and exchange path.
- **Energy and bandwidth wins compound at the cluster boundary.**
  Site 4 cuts vital signs DSP duty cycle; Site 6 cuts cross-cluster
  broadcast load. Both are at the cluster Pi, where wattage matters.
- **Privacy story strengthens.** Every site stores a witness hash,
  not raw data. Sites 2 and 7 are explicitly designed to ship
  without retaining the embeddings or event payloads they index.
- **Reuses ADR-084 Pass 1 with no new dependency.** The
  `wifi-densepose-ruvector::sketch` module already exposes
  `Sketch`, `SketchBank`, `SketchError` at 43–51× compare speedup.
- **Each site is independently testable and revertible.** The seven
  passes share no data paths; failure at any one rolls back without
  touching the others.

### Negative / risks

- **Mahalanobis distributional assumption (Site 1).** Pure 1-bit
  sign quantization performs best on zero-centered, isotropic
  embeddings; Mahalanobis explicitly encodes covariance structure
  hamming distance is insensitive to. The sketch is used **only**
  as a candidate-narrower; the Mahalanobis re-score preserves
  semantics. But if hamming top-K systematically excludes the true
  winner, the short-circuit is worse than no short-circuit. The
  Validation acceptance test guards this; randomized rotation
  pre-pass (RaBitQ-paper-style) may be needed — see Open Q1.
- **REST endpoint shape (Site 2) is an API surface commitment.**
  A `GET /api/v1/recordings/similar` with a sketch-only default
  is a contract; clients expect approximate-recall behavior.
  Documenting "sketch-only by default, `&refine=true` for full
  re-ranking" is part of the acceptance bar.
- **False-positive risk on Site 3 (BSSID novelty)** in dynamic
  environments. Coffee-shop / co-working deployments see BSSIDs
  rotate constantly; the signal must flag *unexpected* change,
  not background churn — acceptance is framed accordingly.
- **Witness-hash format for non-vector inputs (Sites 5 and 7).**
  Witness bundles and event streams are not natively dense-vector
  data; the encoding into sketchable form (numeric SHA-prefix
  segments; normalized event-type histograms) is itself a design
  choice future model changes can break. `sketch_version` bumps
  invalidate banks everywhere, but only Sites 5 and 7 must
  re-encode raw inputs.
- **Operational surface area.** Seven banks each with their own
  persistence, version-skew, and refresh story. The cluster Pi
  gains non-trivial state. ADR-083's secure-boot / OTA story
  holds, but state-rebuild cost on `sketch_version` bump is now
  seven banks, not one.

### Neutral

- The five ADR-084 sites and the seven sites here are independent.
  Acceptance or rollback at any one site does not propagate.
- ADR-082 (confirmed-track filter) remains upstream of every sketch
  call. ADR-081 (5-layer firmware kernel) is unchanged — every new
  bank lives at the cluster Pi or higher.
- ADR-027 (cross-environment generalization, MERIDIAN) interacts
  cleanly: Site 1's per-class sketches are *per environment* by
  construction, which is the same shape MERIDIAN already assumes.

## Implementation

Seven passes, ordered cheapest-first / lowest-risk-first. Each is
independently shippable; each has a single-line acceptance test that
must pass before the next pass starts.

| # | Pass | Target crate | Acceptance test (one line) |
|---|------|--------------|----------------------------|
| 1 | **Witness bundle drift sketch** (Site 5) | `scripts/witness_drift_check.py` | CI run on the last 5 releases produces ≥ 1 drift flag on a known dependency-bump release and 0 flags on a known no-op release. |
| 2 | **BSSID fingerprint novelty** (Site 3) | `wifi-densepose-wifiscan::bssid_sketch` | 24-hour soak in a stable office: novelty rate ≤ 5 events / hour; controlled new-AP injection: novelty fires within 2 scan cycles. |
| 3 | **mmWave signature gate** (Site 4) | `wifi-densepose-vitals::preprocessor` | Vitals DSP CPU time / hour ≥ 4× lower in steady-state empty-room compared to no-gate baseline; missed-detection regression ≤ 1 pp on the existing breathing/heart fixtures. |
| 4 | **Adaptive classifier short-circuit** (Site 1) | `wifi-densepose-sensing-server::adaptive_classifier` | Per-frame `classify` time reduced ≥ 2× at K = 3 candidates; classification accuracy regression ≤ 1 pp on the held-out test set. |
| 5 | **Event-stream anomaly sketch** (Site 7) | `wifi-densepose-sensing-server::event_anomaly` | 7-day rolling deployment: ≤ 1 false anomaly / day; injection of a synthetic anomalous hour fires within one refresh tick. |
| 6 | **Swarm memory routing** (Site 6) | `wifi-densepose-sensing-server::multistatic_bridge` | 12-Seed simulated swarm: per-event broadcast-message count drops ≥ 5× vs. unrouted baseline; routed-Seed-resolution rate ≥ 80%. |
| 7 | **Recording-search REST endpoint** (Site 2) | `wifi-densepose-sensing-server::recording` + HTTP route | `GET /api/v1/recordings/similar` returns a top-K with ≥ 90% candidate-set agreement vs. full-embedding re-rank on the recorded dataset; response time < 50 ms at K = 10 over 1000 recordings. |

ADR-084's general acceptance numbers — **8–30× compare cost
reduction, ≥ 90% top-K coverage, < 1 pp accuracy regression** —
apply unchanged to Sites 1 (classifier) and 2 (recording search),
where the candidate set is large and top-K coverage is the right
framing. Sites 3, 4, 5, 6 are gating / anomaly / routing problems
measured against site-specific criteria above (false-positive rate,
DSP duty cycle, broadcast count, drift-flag precision). Each pass
adds three tests under `v2/crates/<target>/tests/`: property test
(sketch ↔ float top-K where applicable), criterion bench
(compare-cost ratio), end-to-end regression against recorded data.
Benches reuse the ADR-084 Pass 1 harness.

## Validation

This ADR is **Proposed**. Acceptance requires **at least four of
seven passes** to meet their per-row acceptance test. The four
must-haves are: **Site 1** (per-frame cost; Mahalanobis assumption
load-bearing), **Site 4** (cluster-Pi energy), **Site 6**
(cross-cluster bandwidth), **Site 7** (privacy-preserving anomaly).
Sites 2, 3, 5 are nice-to-haves and may ship or revert
independently.

Validation runs against:

- existing workspace tests (must stay green at
  `cargo test --workspace --no-default-features` on `v2/`);
- a 7-day cluster-Pi soak at the lab fixture (3 sensor nodes + 1 Pi
  per ADR-083) with recordings, mmWave, and BSSID scans active —
  per-site logs graded against the Implementation table;
- Python proof harness unchanged (`archive/v1/data/proof/verify.py`
  must still print `VERDICT: PASS`);
- regenerated witness bundle (ADR-028) including the Site 5 sketch.

When the four must-haves pass and the soak holds, ADR moves
**Proposed → Accepted** and README hardware/feature tables gain a
sketch-bank row.

## Open questions

1. **Does Mahalanobis pre-filtering survive sign-quantization bias
   on Site 1?** Pure 1-bit sketches discard the covariance
   structure Mahalanobis uses. The pass-1 framing — sketch narrows,
   Mahalanobis decides — preserves correctness in expectation, but
   adversarial centroid geometries can let the hamming top-K
   systematically exclude the true winner. **No primary source
   found** for "binary-sketch + Mahalanobis-refine" as a published
   pipeline; marked as conjecture, gated by the Site-1 acceptance
   test. If it fails, the next experiment is the randomized
   rotation pre-pass from Gao & Long (SIGMOD 2024, arxiv
   2405.12497), which ADR-084 also flagged for AETHER /
   spectrogram embeddings. A standalone follow-up ADR is the
   likely outcome if rotation is needed.
2. **Witness-hash format for non-vector data (Sites 5, 7).** The
   release bundle (Site 5) and event stream (Site 7) are not
   natively dense-vector inputs. The proposed encodings — numeric
   SHA-256-prefix segments plus attestation values for Site 5;
   normalized event-type histograms for Site 7 — are plausible
   but unvalidated against drift in the underlying distributions.
   A small follow-up ADR formalizing the "non-vector → sketchable"
   canonical path is plausible if the two sites diverge.
3. **Cross-environment domain generalization interaction
   (ADR-027).** Per-class sketches in Site 1 and per-room banks at
   Sites 4 and 7 are implicitly per-environment artifacts; ADR-027
   (MERIDIAN) handles cross-environment generalization at the model
   layer. When MERIDIAN's domain detector flags an environment
   shift, do banks rebuild, swap, or merge? Default here is
   **rebuild on shift**; a merge story may be cheaper and is open
   for the eventual MERIDIAN-aware deployment.
4. **REST API shape for Site 2.** The choice between
   Qdrant/Pinecone/Weaviate-style endpoints (Qdrant being the
   closest Rust-native comparator with HTTP `/points/search`) and
   a thin sketch-only response is intentionally opinionated
   toward the thin shape. **No Rust-idiom primary source** was
   located for "sketch-only similarity search over recordings"
   specifically; closest analog is SimHash-over-documents
   deduplication, which lacks time-series-recording prior art.
   If a clean Rust crate emerges owning this idiom, Site 2 may
   delegate rather than ship bespoke.
5. **BSSID novelty and 802.11bf-2025 interaction.** IEEE 802.11bf
   was published in March 2025 and standardizes WLAN sensing
   measurement frames; Site 3's novelty sketch operates above the
   measurement layer (on RSSI/SNR/channel time-series) and should
   not duplicate what 802.11bf eventually exposes natively. **No
   primary source found** for "RSSI-fingerprint anomaly + 802.11bf"
   — marked as conjecture; revisit when client/AP support arrives.

## Related

- **ADR-027** (Proposed) — MERIDIAN cross-environment generalization.
  Per-environment sketch banks (Sites 1, 4, 7) need an explicit
  swap/rebuild story under MERIDIAN-detected domain shifts.
- **ADR-028** (Accepted) — ESP32 capability audit / witness bundle.
  Site 5 adds a sketch ratchet to the existing release artifact.
- **ADR-066** (Proposed) — Swarm bridge to coordinator. Site 6 routes
  over the bridge channel ADR-066 defines.
- **ADR-073** (Proposed) — Multifrequency mesh scan. Site 3's
  BSSID novelty feeds the hop scheduler ADR-073 owns.
- **ADR-076** (Proposed) — CSI spectrogram embeddings. Site 2's
  recording-search sketch can pool over spectrogram embeddings
  when present, or fall back to AETHER means.
- **ADR-081** (Accepted) — 5-layer adaptive CSI mesh firmware kernel.
  No firmware change; every new sketch bank is at the cluster Pi
  or higher.
- **ADR-082** (Accepted) — Pose tracker confirmed-track filter.
  Upstream of every sketch call; unchanged.
- **ADR-083** (Proposed) — Per-cluster Pi compute hop. The Pi is
  the host for all seven new banks; ADR-083's deployment story is
  the prerequisite.
- **ADR-084** (Proposed) — RaBitQ similarity sensor (five-site
  baseline). This ADR refines and extends; it does not duplicate
  ADR-084's compare-cost / top-K / accuracy acceptance numbers
  where unchanged.
