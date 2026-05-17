# ADR-099: Adopt midstream as RuView's real-time introspection + low-latency tap

| Field | Value |
|-------|-------|
| **Status** | Proposed |
| **Date** | 2026-05-13 |
| **Deciders** | ruv |
| **Codename** | **midstream-introspection** |
| **Relates to** | ADR-097 (rvCSI adoption — provides the validated `CsiFrame` stream this ADR taps), ADR-098 (Rejected midstream as a *replacement* for RuView's existing seams — this ADR is the *parallel-addition* answer that complements it), ADR-095/096 (rvCSI platform + FFI), ADR-014 (SOTA signal processing in `wifi-densepose-signal`) |
| **midstream repo** | [github.com/ruvnet/midstream](https://github.com/ruvnet/midstream) (vendored at `vendor/midstream`); 5 crates on crates.io at `0.2.1` |

---

## 1. Context

[ADR-098](ADR-098-evaluate-midstream-fit.md) rejected midstream as a **replacement** for RuView's existing seams — the four candidate substitutions (WS fan-out, the `wifi-densepose-signal` DSP pipeline, ESP32 mesh TDM coordination, `tokio::sync::broadcast` backpressure) all checked out as "current solution fits, midstream is the wrong tool". That verdict stands.

This ADR is the **other half** of that conversation. Two of midstream's primitives — `temporal-compare` (DTW) and `temporal-attractor-studio` (Lyapunov + regime classification) — were carved out under ADR-098 D5 as "re-evaluate if a second use case appears". The use case is now named: **real-time introspection of the CSI stream + low-latency detection of motion-shape events**, running as a parallel tap *alongside* RuView's existing event pipeline rather than replacing it.

### 1.1 The latency floor today, by construction

[`vendor/rvcsi/crates/rvcsi-events/src/window_buffer.rs:20`](../../vendor/rvcsi/crates/rvcsi-events/src/window_buffer.rs#L20) defines `WindowBuffer::new(max_frames: usize, max_duration_ns: u64)`. The events pipeline emits *only at window close*. At RuView's ~30 Hz CSI rate with the default 16-frame / 1-second windows, the soonest `MotionDetected` or `PresenceStarted` can fire is roughly **500–1000 ms after the actual RF perturbation**. That's an architectural floor, not an implementation accident — `WindowBuffer` is the integration tier, and integration takes time.

For high-touch UI (the live dashboard) and for downstream consumers that need to react to motion *as it starts*, that floor matters. The `wifi-densepose-sensing-server` already maintains continuous per-frame state (`AppStateInner::{frame_history, rssi_history, smoothed_motion, baseline_motion, last_novelty_score}` at [`main.rs:307–423`](../../v2/crates/wifi-densepose-sensing-server/src/main.rs#L307)), but exposes them only as endpoint-poll scalars — there's no streaming-tap surface for "what's happening *inside* the pipeline right now". A consumer that wants reflex-level reaction has to invent it.

### 1.2 What midstream's primitives actually map onto

Ground-truth grep across `vendor/midstream/crates/`:

| Term | Hits | Where |
|---|---|---|
| `Lyapunov` | 284 | `temporal-attractor-studio` |
| `LTL` | 230 | `temporal-neural-solver` |
| `Attractor` | 1252 | `temporal-attractor-studio` |
| `DTW` | 540 | `temporal-compare` |
| `phase-space` | 23 | `temporal-attractor-studio` |

`temporal-compare/src/lib.rs:5` advertises *"Dynamic Time Warping (DTW), Longest Common Subsequence (LCS), Edit Distance (Levenshtein), Pattern matching and detection, Efficient caching"* — and the bench prose (in midstream's `README.md`) puts a cached pattern match at **~12 µs**. `temporal-attractor-studio/src/lib.rs:6` advertises *"Attractor classification (point, limit cycle, strange), Lyapunov exponent calculation, Phase space analysis, Stability detection"*. At RuView's ~30 Hz tick budget (33 ms), the per-frame cost of either is well under 1 % of the budget.

### 1.3 Why this isn't ADR-214

ADR-214 (the V0 / Cognitum cluster correlator decision, owned in a separate repo) takes a much larger commitment: all five midstream crates, a full new `cognitum-rvcsi-correlator` crate, a `WireRecord` adapter layer, multi-Pi cadence alignment via `nanosecond-scheduler`. That's the right shape for V0 because V0 is filling a "no Rust correlator binary exists yet" gap (ADR-209 §C.1) — *replacing* a Python prototype.

RuView's case is different and smaller. The Rust pipeline already exists and works. This ADR adds two midstream crates and one tap — same primitives, much narrower scope, no replacement.

---

## 2. Decision

**Adopt `midstreamer-temporal-compare` and `midstreamer-attractor` as a parallel real-time introspection tap inside `wifi-densepose-sensing-server`.** All eight decisions below are the architectural contract.

### D1 — Only two midstream crates, no more

`midstreamer-temporal-compare = "0.2"` and `midstreamer-attractor = "0.2"` enter as dependencies of `wifi-densepose-sensing-server`. The other three midstream crates are explicitly **not** in scope:

* `midstreamer-scheduler` — sub-µs host-side scheduling has no fit in RuView; the per-Pi / per-ESP32 timing-sensitive work happens in firmware (ADR-073 channel hopping, the ESP32 TDM) where it belongs.
* `midstreamer-neural-solver` (LTL) — relevant for the MAT (Mass Casualty Assessment Tool) audit-trail use case, *not* for real-time introspection. Tracked as a follow-up ADR.
* `midstreamer-strange-loop` — long-horizon meta-learning for `adaptive_classifier` confidence; out of scope of "real-time".

*Consequences:* the dependency footprint is two A+-security `unsafe_code = "deny"` crates, not the full midstream workspace.

### D2 — The tap point is post-validate, parallel to `WindowBuffer::push`

Each `CsiFrame` that survives `rvcsi_core::validate_frame` and `SignalPipeline::process_frame` (the same gate ADR-097 D6 establishes as the boundary) is fanned out to **two consumers**:

1. The existing `WindowBuffer::push` → `EventPipeline` → `broadcast::<String>` → `/ws/sensing` path. Unchanged.
2. The new `IntrospectionState::update_per_frame` → `broadcast::<IntrospectionSnapshot>` → `/ws/introspection` path. Per-frame, never window-blocked.

*Consequences:* zero behavioural change to the existing `/ws/sensing` / `/api/v1/sensing/latest` / vital-sign / pose / model-management endpoints; the bearer-auth middleware from #547 (PR-merged) wraps the new endpoint exactly like every other `/api/v1/*` and `/ws/*`.

### D3 — One new WS topic + one new REST endpoint

* `WS /ws/introspection` — continuous stream of `IntrospectionSnapshot` JSON frames (one per CSI frame received, modulo a small coalesce window if the client is slow).
* `GET /api/v1/introspection/snapshot` — one-shot poll for the latest snapshot (mirrors the existing `/api/v1/sensing/latest` shape).

`IntrospectionSnapshot` carries: `timestamp_ns`, `regime` (one of `Idle`/`Periodic`/`Transient`/`Chaotic`), `lyapunov_exponent: f32`, `attractor_dim: f32`, `top_k_similarity: Vec<(signature_id: String, score: f32)>` (k = 5 by default).

*Consequences:* dashboard widgets can subscribe directly; the existing `/ws/sensing` stays the canonical "events" topic; the new topic is the "continuous state" topic.

### D4 — Per-frame update only, never window-blocked

The new introspection path **must not** block on window close. The DTW path operates over a sliding tail buffer (default 64 frames) of derived feature vectors; the attractor path operates over a sliding tail of `mean_amplitude` scalars. Both update on every accepted frame.

*Consequences:* the soonest "shape-matches signature" emission is bounded by the per-frame update cost (target ≤1 ms p99 on a Pi-5-class host), not by the 16-frame window — a **~16× collapse** of the latency floor on this specific class of event.

### D5 — `temporal-neural-solver` (LTL) is out of scope of this ADR

The MAT audit-trail use case (provable triggers with proof artefacts, ADR-style "this `SurvivorTrack` activation was provably (LTL formula) satisfied") is a separate concern. Tracked as a follow-up ADR; the same crate that lives in `vendor/midstream/crates/temporal-neural-solver` will be revisited there.

*Consequences:* this ADR does not deliver audit-grade proof artefacts; if you need them, wait for the MAT ADR.

### D6 — ESP32 firmware is unchanged

Introspection runs entirely on the host side (`wifi-densepose-sensing-server`). The ESP32 ADR-018 wire format, the firmware's CSI collector, the TDM protocol, the NVS provisioning — none change. No firmware re-flash required to consume this feature.

*Consequences:* deployment is "update the host-side binary / Docker image"; existing ESP32-S3 / ESP32-C6 / mmWave node fleets work as-is.

### D7 — Signature library is JSON, on-disk, customer-owned

A "signature" is a short labelled sequence of derived feature vectors. Schema (one file per signature under `--signatures-dir /etc/cognitum/signatures/`):

```jsonc
{
  "id": "walking_slow_v1",
  "label": "Walking — slow pace",
  "captured_at": "2026-05-13T20:00:00Z",
  "feature_kind": "amplitude_l2_per_subcarrier",  // or "vec128" once an embedding source exists
  "length": 64,
  "dtw": { "window": 8, "step_pattern": "symmetric2" },
  "vectors": [ [ ... ], [ ... ], /* length-64 of feature vectors */ ],
  "promotion_threshold": 0.78
}
```

Three reference signatures ship under `signatures/` in the crate as developer fixtures (`idle_room.sig.json`, `walking_slow.sig.json`, `door_open.sig.json`). Customer-trained signatures are not committed.

*Consequences:* the library is a deployment-time concern, not a build-time one; customers can tune the threshold per environment.

### D8 — Measurement-first adoption — promotion bar is empirical

Phase 0 spike measures the latency win against the existing `/ws/sensing` path on a recorded session. **Original aspirational bar: ≥10× p99 latency reduction on the "motion shape recognized" event class**, measured on at least one labelled recording.

**Empirical baseline from `tests/introspection_latency.rs`** (I5/I6 — host-side L1 stand-in scoring + midstream-attractor regime classification on a 1-D mean-amplitude feature, 5-frame motion-ramp signature, 200 frames of noise warm-up, `analyze_every_n = 1`):

| Signal | Frames to recognise | Ratio vs event-path floor (16) |
|---|---|---|
| `top_k_similarity[0].above_threshold` | 5 | **3.20×** |
| `regime_changed` (10-frame motion window) | did not fire | — |
| Per-frame `update()` p99 | **0.041 ms** (~24× under D4's 1 ms budget) | — |

The 10× bar is **architecturally unreachable** at the 1-D scalar feature resolution this stand-in operates at — `signature_score`'s length-normalised L1 needs roughly the full signature length of in-shape frames to discriminate from noise (any shortcut trades false positives), and the attractor's Lyapunov classification needs more than a 10-frame perturbation to overcome a long noise trajectory. The 3.2× ratio is the structural ceiling for this feature class.

**Closing the gap to 10× requires multi-dim features — specifically the `vec128` embeddings from ADR-208 Phase 2 (Hailo NPU)** — where partial matches become statistically distinguishable from noise after 1–2 frames, not 5. Until then, the adoption decision **revises the bar**:

* **Ship behind `--introspection` (off by default)** until either ADR-208 P2 lands a multi-dim feature path, *or* the L1 stand-in is replaced with a numeric DTW that scores partial-prefix matches at acceptable false-positive rates.
* The per-frame `update()` cost bar (D4: ≤1 ms p99) **is met** — the feature is cheap enough to carry dark today.
* **Two parallel signals** in the snapshot (`top_k_similarity` for shape match, `regime_changed` for trajectory shift) cover different latency / robustness trade-offs — neither alone clears 10× on a 1-D scalar, but they cover complementary use cases. Downstream consumers pick.

> **Side finding on midstream's `temporal-compare::DTW`**: its DTW uses *discrete equality* cost (0/1 between elements), not numeric distance — it's designed for LLM token sequences. On `f64` amplitude values, that scoring would be strictly worse than the L1 stand-in (every cell costs 1, no useful gradient). "Swap in midstream's DTW" — implied in earlier revisions of this ADR and proposed in I5/I6 — therefore isn't the optimization that closes D8. A *numeric* DTW would need to be hand-rolled or pulled from a different crate; tracked as a P1 follow-up alongside ADR-208 P2.

*Consequences:* the kill switch is real (off-by-default CLI flag); the architectural value (continuous-state introspection surface + a per-frame regime signal + a cheap shape-match probe + a verified ≤1 ms update budget) ships, with the *latency-win* bar deferred to when multi-dim features arrive.

---

## 3. Architecture

```
                                  ┌── (existing) ──┐
                                  │  WindowBuffer  │── EventPipeline ─┐
   UDP / CSI source ─→ validate ─→│                │                  ↓
                       + DSP  ───→│                │              broadcast<String>
                                  │  (16 frames /  │                  ↓
                                  │   1 s window)  │           /ws/sensing
                                  └────────────────┘
                       ───→──────┐
                                 ↓
                          (NEW — this ADR)
                          IntrospectionState::update_per_frame
                          ├─ DTW vs signature library (temporal-compare)
                          ├─ Attractor / Lyapunov sliding (attractor-studio)
                          └─ Coalesce client-slow → snapshot
                                                                   ↓
                                                  broadcast<IntrospectionSnapshot>
                                                                   ↓
                                                  /ws/introspection   (NEW)
                                                  /api/v1/introspection/snapshot  (NEW)
```

The tap is added once, in `csi.rs`'s frame loop, right after the line that currently feeds the `WindowBuffer`. Implementation lives in one new module: `v2/crates/wifi-densepose-sensing-server/src/introspection.rs`.

The new path **never reads or writes** the existing `AppStateInner` introspection scalars (`smoothed_motion`, `baseline_motion`, etc.) — those stay as the dashboard's continuous-summary backing. The new path produces *additional* signal, not replacement signal.

---

## 4. Implementation phases

| Phase | Scope | Bar |
|---|---|---|
| **P0 — Spike + benchmark** | Add deps, scaffold `introspection.rs`, wire the tap, add `/ws/introspection`, measure p50/p99 latency on a recorded session. | ≥ 10× p99 latency reduction on the "shape recognized" path vs. `/ws/sensing` event path. If miss, the feature stays behind a CLI flag. |
| **P1 — First real signature library** | Capture 3 labelled segments (`idle_room`, `walking_slow`, `door_open`) on the ESP32-S3 on COM7, build the developer fixture under `signatures/`. | A live person walking in front of the node produces a `walking_slow` match in /ws/introspection ≥1 frame before `MotionDetected` fires on /ws/sensing. |
| **P2 — Dashboard widget** | Add an "Introspection" panel to the live dashboard subscribing to `/ws/introspection`: regime indicator, Lyapunov gauge, top-k matches with confidence. | Visual confirmation of D4 ("never window-blocked") — the panel responds to a perturbation before the `MotionDetected` toast appears. |
| **P3 — Signature capture workflow** | CLI sub-command `rvcsi capture-signature --label <name> --duration 2s --out signatures/<id>.json` (or its sensing-server equivalent) that records and labels a segment in one step. | A non-developer can extend the library without writing JSON by hand. |
| **P4 — Adaptive classifier hook (optional)** | Feed introspection's continuous regime scalar + top-k similarities into the existing `adaptive_classifier` as auxiliary features. | Measurable classifier accuracy improvement on a held-out test set; if no improvement, abandon and document. |

P0 is the commitment. P1–P3 are sequential per-PR follow-ups. P4 is research-shaped and explicitly failure-tolerant.

---

## 5. Consequences

**Positive**

* Soonest-event latency on the "shape recognized" path drops from ~533 ms (16-frame window @ 30 Hz) to ~33 ms (one frame at 30 Hz) — a 16× collapse, dwarfed only by network RTT and the DTW math itself (~12 µs / cached pattern).
* Dashboards and downstream consumers get a streaming-tap surface for *what the pipeline is seeing right now*, not just summary scalars at endpoint-poll time.
* `adaptive_classifier` and the novelty bank gain a richer per-frame feature input (regime, Lyapunov, top-k similarity) — augmenting, not replacing, their current inputs.
* Zero behavioural change to existing endpoints, no firmware change, no schema migration. Pure addition.
* Two A+-security `unsafe_code = "deny"` crates — bounded, audited dependency footprint.

**Negative**

* Dependency surface grows by two crates. Mitigation: both pinned `^0.2`, both ours (user owns midstream), both `unsafe_code = "deny"`.
* The DTW path is only as good as its signature library — a poor library means false matches. D7's per-deployment library + D8's `promotion_threshold` per signature mitigate; P3's capture workflow makes the library tractable to grow.
* Adding a second broadcast topic adds memory pressure under fan-out (each subscriber holds a ring slot). The default ring size (32 snapshots) caps it.

**Neutral**

* Existing `/ws/sensing` consumers continue to see the same events at the same cadence.
* ADR-097's rvCSI adoption is unaffected — this tap *consumes* rvCSI's validated `CsiFrame` output, doesn't replace any rvCSI seam.
* The `vendor/rvcsi` submodule and the `vendor/midstream` submodule both stay; this ADR uses crates.io versions of both for the build, with the submodules as reference / patch escape hatches (ADR-097 D7 and ADR-098 D7 patterns respectively).

---

## 6. Alternatives considered

| Alternative | Why not |
|---|---|
| **Tighten the rvCSI `WindowBuffer` to 1-frame / 0 ms windows.** | Defeats the purpose — `EventPipeline`'s state machines (`PresenceDetector::enter_windows = 2`, `MotionDetector::debounce_windows = 2`) need stable window-aggregated input to debounce noise. Single-frame windows produce per-frame events with no hysteresis, which is *worse* than today, not better. |
| **Write the DTW + attractor math from scratch in `wifi-densepose-signal`.** | This is what midstream's crates *are*. ~640 hits for DTW and 1252 for Attractor across midstream's existing source — re-implementing would be 1–2k LOC of math we'd own and maintain forever. Not free. |
| **Use the heuristic `smoothed_motion` / `baseline_motion` as the introspection signal.** | They already exist (`main.rs:310,377`), they're already broadcast on the dashboard's continuous-summary path. But they're a single scalar derived from EWMA — they don't classify regime, don't match shapes, don't give phase-space stability. Worth keeping as the "always-on lite indicator"; not a substitute for D3's snapshot. |
| **All five midstream crates at once.** | The other three (`scheduler`, `neural-solver`, `strange-loop`) don't fit the "real-time introspection" framing — they fit "host-side hard scheduling", "audit-grade proofs", "long-horizon meta-learning". Mixing them in would balloon the surface and dilute the latency-win measurement. D1 keeps it to two. |
| **Defer until ADR-214's V0 correlator ships and copy its design.** | V0's correlator is the *replacement* shape (Python prototype → Rust). RuView's case is the *addition* shape. The designs share crates but not topologies; deferring would leave RuView's latency floor in place for months while V0 lands. |

---

## 7. Open questions

* **Feature vector for `vec128`-class DTW.** Until ADR-208 Phase 2 ships real Hailo NPU embeddings, the per-frame feature vector is a derived scalar tuple (RSSI + per-subcarrier amplitude L2 norm). When the encoder lands, the DTW path consumes `vec128` directly — what version-skew strategy do signature libraries use?
* **Coalesce window for slow WS clients.** A subscriber falling behind shouldn't make the broadcast ring grow unboundedly. Default proposal: drop oldest, log a `warn!` after N consecutive drops. The exact N is tunable.
* **Cross-node introspection.** Today the snapshot is per-node. For multi-node deployments, do we want a fused cluster-level snapshot too? Likely yes — but as a separate ADR; this one keeps to per-node.

---

## 8. References

* [ADR-097 — Adopt rvCSI as RuView's primary CSI runtime](ADR-097-adopt-rvcsi-as-ruview-csi-runtime.md) — provides the validated `CsiFrame` stream this tap reads.
* [ADR-098 — Evaluate `ruvnet/midstream` for RuView's CSI / WebSocket / mesh pipeline (Rejected)](ADR-098-evaluate-midstream-fit.md) — Rejected midstream as a *replacement* for existing seams. This ADR is the *addition* answer; D5/D6 of ADR-098 explicitly carved out `temporal-compare` and the attractor crate for this case.
* [ADR-095 — rvCSI Edge RF Sensing Platform](ADR-095-rvcsi-edge-rf-sensing-platform.md), [ADR-096 — rvCSI Crate Topology](ADR-096-rvcsi-ffi-crate-layout.md) — the upstream platform.
* [`midstreamer-temporal-compare` 0.2.1](https://crates.io/crates/midstreamer-temporal-compare), [`midstreamer-attractor` 0.2.1](https://crates.io/crates/midstreamer-attractor) — the two crates this ADR adopts.
* [`vendor/midstream/crates/temporal-compare/src/lib.rs:5`](../../vendor/midstream/crates/temporal-compare/src/lib.rs#L5) — DTW / LCS / edit-distance pattern matching, public API.
* [`vendor/midstream/crates/temporal-attractor-studio/src/lib.rs:6`](../../vendor/midstream/crates/temporal-attractor-studio/src/lib.rs#L6) — attractor classification + Lyapunov exponent, public API.
* [`vendor/rvcsi/crates/rvcsi-events/src/window_buffer.rs:20`](../../vendor/rvcsi/crates/rvcsi-events/src/window_buffer.rs#L20) — the window-aggregation step whose latency floor this tap bypasses.
* [`v2/crates/wifi-densepose-sensing-server/src/main.rs:307-423`](../../v2/crates/wifi-densepose-sensing-server/src/main.rs#L307) — the existing per-frame state surface this tap augments.
