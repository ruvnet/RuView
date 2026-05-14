# ADR-098: Evaluate `ruvnet/midstream` for RuView's CSI / WebSocket / mesh pipeline

| Field | Value |
|-------|-------|
| **Status** | Rejected (with crate-level carve-outs for future evaluation) |
| **Date** | 2026-05-13 |
| **Deciders** | ruv |
| **Codename** | **midstream-in-RuView** |
| **Relates to** | ADR-095 (rvCSI platform), ADR-096 (rvCSI crate topology), ADR-097 (adopt rvCSI as RuView's CSI runtime), ADR-012 (ESP32 CSI mesh), ADR-029 (RuvSense multistatic / TDM), ADR-031 (RuView sensing-first RF mode), ADR-043 (sensing-server UI API completion) |
| **midstream repo** | [github.com/ruvnet/midstream](https://github.com/ruvnet/midstream) — vendored at `vendor/midstream`, currently pinned at [`30fe5eb`](https://github.com/ruvnet/midstream/commit/30fe5eb7a1f1494aa1ad00d54160088a565ec766) |
| **Outcome** | Do **not** adopt as a system component. Two of midstream's six workspace crates (`temporal-compare`, `nanosecond-scheduler`) are plausible future-use building blocks; the rest do not fit. `vendor/midstream` is retained as a reference-only submodule. |

---

## 1. Context

`vendor/midstream` is a git submodule of RuView (`.gitmodules:1-4`) but, like `vendor/rvcsi` was before ADR-097, it is **vendored but not consumed**: no `v2/crates/*/Cargo.toml` depends on a `midstreamer-*` crate, no Rust source contains `use midstreamer_…`, and the ESP32 firmware and TypeScript dashboard have no midstream imports.

This ADR settles the standing question of *whether RuView should consume midstream at all*, and if so, where. The user-facing prompt enumerated four candidate seams to evaluate:

1. Streaming / pub-sub for the WebSocket fan-out (today: `tokio::sync::broadcast::channel::<String>(256)` at `v2/crates/wifi-densepose-sensing-server/src/main.rs:4769`).
2. Stream processing for the CSI → DSP → event pipeline (today: synchronous `EventPipeline` at `vendor/rvcsi/crates/rvcsi-events/src/pipeline.rs`, freshly adopted via ADR-097).
3. Multi-source merging / TDM coordination for the ESP32 mesh (ADR-029, ADR-073).
4. Backpressure / flow control between the UDP receiver and downstream consumers (`v2/crates/wifi-densepose-sensing-server/src/main.rs:3638` `udp_receiver_task`; firmware-side `stream_sender` ENOMEM backoff at `firmware/esp32-csi-node/main/csi_collector.c:223-228`).

To evaluate each, we read midstream's workspace `Cargo.toml` (`vendor/midstream/Cargo.toml:1-99`), the `README.md` and `BENCHMARKS_SUMMARY.md`, and every crate's `lib.rs`:

| Crate | File | LOC | Purpose (from header doc) |
|---|---|---:|---|
| `midstreamer-temporal-compare` | `vendor/midstream/crates/temporal-compare/src/lib.rs:1-697` | 697 | DTW, LCS, Levenshtein, generic pattern matching on `Sequence<T>` of `TemporalElement<T>` |
| `midstreamer-scheduler` | `vendor/midstream/crates/nanosecond-scheduler/src/lib.rs:1-406` | 406 | Priority + deadline-aware task scheduler (RM, EDF, LLF) for low-latency real-time tasks |
| `midstreamer-attractor` | `vendor/midstream/crates/temporal-attractor-studio/src/lib.rs:1-482` | 482 | Phase-space reconstruction, Lyapunov exponents, attractor classification |
| `midstreamer-neural-solver` | `vendor/midstream/crates/temporal-neural-solver/src/lib.rs:1-509` | 509 | LTL / CTL / MTL temporal-logic verification with neural reasoning |
| `midstreamer-strange-loop` | `vendor/midstream/crates/strange-loop/src/lib.rs:1-496` | 496 | Multi-level meta-learning, self-referential systems |
| `midstreamer-quic` | `vendor/midstream/crates/quic-multistream/src/lib.rs:1-255`, `native.rs:1-303`, `wasm.rs:1-307` | 865 | Thin wrapper over `quinn` (native) and `WebTransport` (WASM); generic QUIC streams |

Plus a TypeScript layer (`vendor/midstream/npm/`, `vendor/midstream/npm-wasm/`) whose product is "real-time LLM streaming" — OpenAI Realtime API client, RTMP / WebRTC / HLS for video, an in-console dashboard, a Whisper transcription scaffold, an MCP server for LLM agents.

The top-level identity is unambiguous: `Cargo.toml:16` describes the package as **`"Real-time LLM streaming with inflight analysis"`**, and the README (`vendor/midstream/README.md:45-80`) frames midstream as a platform that "analyzes [LLM] responses **as they stream in real-time** — enabling instant insights, pattern detection, and intelligent decision-making" — i.e. the streaming domain is **LLM tokens and dashboard telemetry**, not RF signals. A search for any of `csi`, `wifi`, `sensing`, or `sensor` across `vendor/midstream/crates/*/src/*.rs` returns zero hits.

This shapes the conclusion: midstream's *abstractions* (DTW pattern matching, attractor analysis, LTL verification, meta-learning) were chosen for a fundamentally different problem domain than CSI, and its *transport* (QUIC) is a thin `quinn` wrapper rather than a sensing-aware backplane. The candidate seams enumerated above are either already filled by simpler primitives in RuView, or filled better by rvCSI under ADR-097.

### 1.1 What this ADR is *not*

- Not a judgment on midstream's quality. It has 139 passing tests and clean Rust; it is well-engineered for its target domain.
- Not a decision to drop `vendor/midstream`. The submodule pin is cheap to keep, and the carve-outs in §3 may justify revisiting it.
- Not a position on the *standalone* midstream product (LLM streaming, OpenAI Realtime, dashboards). That product is unaffected by this ADR.

---

## 2. Decision

**Reject midstream as a system component of RuView.** The four candidate seams are either filled (well) by existing RuView primitives, or are filled by rvCSI's freshly-adopted `EventPipeline` and `RfMemoryStore`. The eight decisions below are the architectural contract.

### D1 — Streaming / pub-sub for the WebSocket fan-out: no change

RuView's sensing-server currently fans out updates to WebSocket clients via `tokio::sync::broadcast::channel::<String>(256)` (`v2/crates/wifi-densepose-sensing-server/src/main.rs:4769`). midstream offers no equivalent in-process broadcast primitive — its TypeScript dashboard fan-out is HTTP-server based (`vendor/midstream/npm/src/dashboard.ts`), and its Rust `midstreamer-quic` crate is a generic point-to-point QUIC wrapper (`vendor/midstream/crates/quic-multistream/src/native.rs:31-69`), not a pub-sub bus.

Tokio's `broadcast` channel is the standard Rust idiom for this pattern, costs effectively nothing per subscriber, integrates with the rest of the Axum + Tokio stack already in use (`v2/crates/wifi-densepose-sensing-server/src/main.rs:36,47`), and is what `rvcsi-runtime` itself uses for event distribution (`vendor/rvcsi/crates/rvcsi-runtime/src/lib.rs`). **Keep `tokio::sync::broadcast`.**
*Consequences:* zero migration; zero new dependency surface; the WebSocket handlers at `main.rs:1989,2030` continue to work unchanged.

### D2 — CSI → DSP → event pipeline: stay on rvCSI's `EventPipeline`

ADR-097 D2 just adopted `rvcsi-runtime::CaptureRuntime` + `rvcsi_events::EventPipeline` as the CSI ingestion / DSP / event-extraction path. `EventPipeline` is **deterministic, synchronous, single-frame-at-a-time** (`vendor/rvcsi/crates/rvcsi-events/src/pipeline.rs:1-5`: *"Feed it frames with `EventPipeline::process_frame` and drain the tail with `EventPipeline::flush`"*) — and that determinism is load-bearing for ADR-095 D9 (replayability) and ADR-095 D13 (quality scoring against learned baselines).

midstream's stream-processing primitives are designed for the opposite shape: `temporal-attractor-studio` (phase-space reconstruction, Lyapunov exponents) and `temporal-neural-solver` (LTL formula verification) operate on **trajectories** of multi-dimensional states over hundreds-to-thousands of samples (`vendor/midstream/README.md:528-531`: *"Attractor detection: <5ms for 1000-point series"*) — that is closer to RuView's existing RuvSense modules (`v2/crates/wifi-densepose-signal/src/ruvsense/longitudinal.rs`, `intention.rs`) than to anything the runtime DSP layer needs.

Replacing rvCSI's event detectors with midstream constructs would (a) break determinism, (b) re-introduce a parallel CSI-processing implementation — exactly the duplication ADR-097 was opened to remove — and (c) force RuView to invent a `Sequence<T: temporal-compare::TemporalElement>` shim around `CsiFrame` for marginal benefit. **Stay on `rvcsi-events::EventPipeline`.**
*Consequences:* the determinism / replay guarantees of ADR-095 D9 and ADR-097 D6 remain intact; the work to land `rvcsi-adapter-esp32` (ADR-097 D4, P3) is not duplicated.

### D3 — TDM / multi-source merging: stay on the existing aggregator

The ESP32 mesh's multi-source merging is in `v2/crates/wifi-densepose-hardware/src/aggregator/mod.rs:74-220` — a `UdpSocket`-backed aggregator (`mod.rs:74,85`) that receives parsed `CsiFrame`s from N nodes and forwards them on a `SyncSender<CsiFrame>` to the consumer. The TDM coordination (slot assignment, channel hopping, dwell time) lives in firmware (`firmware/esp32-csi-node/main/`) and is governed by ADR-029 and ADR-073. midstream offers nothing for either side: it has no UDP merger, no slot scheduler, and no firmware-side primitives.

`midstreamer-scheduler` is conceptually adjacent — it does priority + deadline-aware scheduling (`vendor/midstream/crates/nanosecond-scheduler/src/lib.rs:53-63`: `RateMonotonic`, `EarliestDeadlineFirst`, `LeastLaxityFirst`, `FixedPriority`) — but its target is **in-process tokio tasks on a 4-thread executor** (`vendor/midstream/README.md:466-477`: *"4 worker threads"*, *"<50 ns scheduling latency"*), not the cross-device, wall-clock-anchored TDM that RuvSense needs. **Keep the existing `wifi-densepose-hardware` aggregator and firmware-side TDM.**
*Consequences:* ADR-029 stays as-is; the work to migrate the parser to `rvcsi-adapter-esp32` (ADR-097 D4) is unaffected.

### D4 — UDP receiver backpressure / flow control: existing solutions are correct at each end

There are two distinct backpressure problems in RuView, and neither benefits from midstream:

- **Firmware side (`firmware/esp32-csi-node/main/csi_collector.c:64,223-228`):** lwIP pbuf exhaustion produces `ENOMEM` when the ESP32 tries to UDP-send faster than the network drains. The fix in code is a rate-limit on `stream_sender_send` *inside the CSI callback*. This is a C-level firmware concern with no Rust analogue — midstream cannot run on the ESP32.
- **Host side (`v2/crates/wifi-densepose-sensing-server/src/main.rs:3638-3640`, `4769`):** `udp_receiver_task` reads from `UdpSocket` and pushes onto `broadcast::channel::<String>(256)`. The bounded channel is itself the backpressure mechanism: lagged subscribers see `RecvError::Lagged`, the buffer wraps, no producer ever blocks. The 256-slot capacity is sized to one second of frame envelopes at the target rate; the per-second packet-yield collapse symptom (`adaptive_controller_decide.c:26-28`) is detected and surfaced by ADR-039 / ADR-081's `pkt_yield_per_sec` accessor, not by transport-layer flow control.

midstream's `quic-multistream` provides per-stream prioritization (`vendor/midstream/crates/quic-multistream/src/native.rs:1-303`), which is a useful flow-control primitive *for QUIC* but not for the UDP-CSI / WS-fan-out topology RuView actually uses. Adopting QUIC end-to-end would mean (a) replacing the ESP32's UDP sender — which would need a QUIC stack on a memory-constrained Xtensa MCU and is out of scope for this project — or (b) terminating QUIC at the aggregator only, which provides no benefit the current bounded `broadcast` channel doesn't. **Keep the existing two-tier backpressure.**
*Consequences:* the ENOMEM rate-limit at `csi_collector.c:223-228` and the bounded `broadcast::channel::<String>(256)` at `main.rs:4769` continue to be the load-bearing primitives.

### D5 — Carve-out: `temporal-compare` as a future RuvSense-side building block

`midstreamer-temporal-compare` (`vendor/midstream/crates/temporal-compare/src/lib.rs:1-697`) is a clean DTW / LCS / Levenshtein implementation with an LRU cache. RuView's gesture detector at `v2/crates/wifi-densepose-signal/src/ruvsense/gesture.rs` already does DTW template matching, and the longitudinal analysis at `ruvsense/longitudinal.rs` could plausibly benefit from cached pattern matching. If we ever need a *separate* DTW implementation that is decoupled from RuvSense's internal types, `temporal-compare` is a reasonable starting point — but only if and when that need arises.

We **do not adopt it today** because RuvSense's gesture matcher already exists, works, and uses RuView-native types, and pulling in `dashmap`, `lru`, and a generic `TemporalElement<T>` abstraction would be net-negative right now. **Tracked as a future evaluation, not a decision.**
*Consequences:* zero today; one named option for a future ADR if a "second" DTW pattern appears.

### D6 — Carve-out: `nanosecond-scheduler` for *host-side* edge tier scheduling (future)

If ADR-039's edge-intelligence tier scheduling ever moves from the ESP32 onto a host-side coordinator (e.g. a Raspberry Pi running the cluster aggregator), `nanosecond-scheduler`'s deadline-aware policies (`vendor/midstream/crates/nanosecond-scheduler/src/lib.rs:53-63`) could plausibly host that scheduler. Today the scheduling is firmware-side and the C-level RTOS handles it; there is nothing to schedule in Rust at the granularity midstream offers.

Again: **not a current decision, just an option kept open.**
*Consequences:* zero today.

### D7 — Submodule disposition: keep `vendor/midstream`

`vendor/midstream` is one git submodule pin; the build does not depend on it; it does not slow down `cargo build --workspace`; and the carve-outs in D5/D6 leave the door open. Removing the submodule would also remove the reference material that justified the carve-outs.

**Keep the submodule, no per-release pin advancement.** Unlike `vendor/rvcsi` (whose pin is bumped per RuView release under ADR-097 D7), `vendor/midstream` has no in-build consumer to validate against. If D5 or D6 ever activates, *that* ADR will start the per-release pin process. Until then the pin can drift freely.
*Consequences:* one line of `.gitmodules` (`.gitmodules:1-4`) stays; `git submodule update --init` remains a no-op for normal RuView development.

### D8 — Documentation: cross-reference, don't import

The ADR index (`docs/adr/README.md`) gets ADR-098 added under "Architecture and infrastructure". No other docs are updated. The README on the RuView side is untouched; midstream is not part of the RuView platform story.
*Consequences:* one row added to the ADR index; no churn elsewhere.

---

## 3. Why not adopt (the rejection record)

For institutional memory, the table below records what each midstream crate *would* solve and the alternative RuView already uses. This is the answer to "but we vendored midstream — what is it for?"

| midstream crate | Plausible RuView seam | Already filled by | Verdict |
|---|---|---|---|
| `midstreamer-temporal-compare` (DTW, LCS, Levenshtein) | Gesture template matching (`ruvsense/gesture.rs`); longitudinal biomechanics drift | RuvSense's existing DTW gesture matcher | Carve-out only (D5) — not adopted today |
| `midstreamer-scheduler` (nanosecond priority + deadline) | ESP32 edge-tier scheduling (ADR-039); RuvSense TDM (ADR-029) | Firmware-side RTOS (ESP32); ADR-029's wall-clock-anchored TDM | Carve-out only (D6) — wrong scope today |
| `midstreamer-attractor` (Lyapunov, phase-space) | RF-field stability detection in `ruvsense/field_model.rs`, `longitudinal.rs` | Welford stats + biomechanics drift (longitudinal.rs); SVD eigenstructure (field_model.rs) | Not adopted — RuvSense's approach is calibrated to RF signal scale and the project's existing dataset, not generic dynamical-systems theory |
| `midstreamer-neural-solver` (LTL / CTL / MTL verification) | Adversarial signal detection (`ruvsense/adversarial.rs`); coherence-gate decisions | Multi-link consistency checks (adversarial.rs); `coherence_gate.rs` state machine | Not adopted — RuView's adversarial detector is not a formal-verification problem; it's a multi-link physical-consistency check |
| `midstreamer-strange-loop` (meta-learning, self-modification) | None in RuView's scope | RuView is not a self-modifying learner; AETHER (ADR-024) is contrastive embedding, not meta-learning | Not adopted — out of scope |
| `midstreamer-quic` (QUIC native + WASM) | Sensing-server → external client transport (alternative to WS) | `tokio::sync::broadcast` + Axum WebSocket + UDP (`main.rs:36-47, 4769, 1989, 2030, 3638`) | Not adopted — see D1, D4 |

The shape of the rejection is consistent: **midstream's abstractions are LLM-token / dashboard-telemetry shaped, RuView's pipeline is RF-frame / event-detector shaped.** Where the two share vocabulary ("streaming", "temporal", "real-time"), the implementations diverge sharply — and the case-by-case analysis above shows that the closer one looks at each seam, the worse the fit gets.

---

## 4. Consequences

**Positive**

- Zero net change to RuView's build, runtime, or surface area; ADR-097's phased rvCSI adoption proceeds unaffected.
- The decision space around midstream is now bounded and documented; future contributors and AI agents see "ADR-098 already evaluated this; here is why not" before re-opening the question.
- The two crate-level carve-outs (D5, D6) are explicit, so if the relevant seams appear later, the evaluation can pick up from this ADR rather than start over.
- `vendor/midstream` (the submodule) remains as reference material, but is correctly marked as not part of the build path.

**Negative / costs**

- One more vendored repo with no in-build consumer — a small but non-zero cognitive load (mitigated by D7's explicit "do not bump the pin").
- If midstream's published crates evolve materially (e.g. a CSI-aware feature lands), the reasoning in §3 needs revisiting; this is the standard "rejected ADRs go stale" risk and applies to every Rejected ADR in the index.

**Risks**

- The most plausible failure mode of this ADR is *not* "we should have adopted midstream"; it is "we re-open the question in six months without re-reading this ADR." Mitigated by indexing ADR-098 in `docs/adr/README.md` and by the per-crate table in §3 being precise enough to short-circuit the next evaluator.

---

## 5. Alternatives considered

| Alternative | Why not |
|---|---|
| **Adopt midstream wholesale as RuView's streaming backbone** | Would force the CSI pipeline into the `Sequence<TemporalElement>` shape (`vendor/midstream/crates/temporal-compare/src/lib.rs:42-70`) and the `quic-multistream` transport (`vendor/midstream/crates/quic-multistream/src/native.rs:1-303`) — both are designed for LLM tokens / arbitrary streams, not validated RF frames with quality scoring. Conflicts directly with ADR-095 D5 (one `CsiFrame` schema), D6 (validate before crossing boundaries), and D9 (deterministic replay). |
| **Replace `tokio::sync::broadcast` with midstream's QUIC fan-out** | Solves no observed problem. `broadcast::channel::<String>(256)` at `v2/crates/wifi-densepose-sensing-server/src/main.rs:4769` handles N WebSocket subscribers at zero per-subscriber cost; the lagged-subscriber semantics (`RecvError::Lagged`) are exactly what an event-feed wants. QUIC adds TLS + congestion control + per-stream priority — useful for *external* clients across a network, but the sensing-server's clients connect over WS on the same host or LAN. |
| **Replace `EventPipeline` with `temporal-attractor-studio` / `temporal-neural-solver`** | `EventPipeline` is deterministic by contract (`vendor/rvcsi/crates/rvcsi-events/src/lib.rs:20`) and ADR-097 just made it RuView's event source of truth. Attractor analysis and LTL verification operate on entirely different abstractions; using them as event detectors would re-invent rvCSI's pipeline in a less-determined way. |
| **Adopt `midstreamer-temporal-compare` for gesture detection now** | RuvSense already has a working DTW gesture matcher tuned to CSI signal scale. Swapping it for a generic `TemporalElement<T>` matcher buys cleanliness but costs a re-tune and a new dep tree (`dashmap`, `lru`). Tracked as D5 for if/when a *second* DTW use case shows up. |
| **Adopt `midstreamer-scheduler` for the cluster-Pi aggregator** | The cluster aggregator does not currently exist as a real-time scheduler; ADR-039's tier scheduling is firmware-side. Until the host-side schedule appears, importing a deadline-aware scheduler is solution-looking-for-a-problem. Tracked as D6. |
| **Drop the `vendor/midstream` submodule entirely** | Cheap to keep, useful as the reference material this ADR cites. D7 keeps it on the explicit understanding that the pin is not advanced. |

---

## 6. Open questions / re-evaluation triggers

This ADR is `Rejected` today on the strength of the §1.1 / §3 analysis. The following events would justify re-opening it:

1. **A second DTW / LCS / Levenshtein use case appears in RuView** (e.g. a CLI-side replay diff, a regression test fixture that needs sequence alignment, a TUI for pattern playback). Then re-evaluate `midstreamer-temporal-compare` per D5.
2. **A host-side real-time scheduler enters RuView's scope** (e.g. the cluster-Pi aggregator becomes responsible for slot timing instead of the ESP32 firmware). Then re-evaluate `midstreamer-scheduler` per D6.
3. **midstream ships a CSI-aware adapter or RF-scale `Sequence<T>` extension** — i.e. midstream's own scope grows to include sensing primitives. As of the pinned commit (`30fe5eb`), this has not happened (zero matches for `csi|wifi|sensing|sensor` in `vendor/midstream/crates/*/src/*.rs`).
4. **RuView gains a QUIC-to-external-client requirement** that the WS fan-out cannot service (e.g. a mobile client over a lossy link that benefits from QUIC's stream priority + 0-RTT). Then re-evaluate `midstreamer-quic` per D1 / D4.

If none of these triggers fire, this ADR stays Rejected and the carve-outs (D5, D6) remain optional.

---

## 7. References

- [ADR-095 — rvCSI Edge RF Sensing Platform](ADR-095-rvcsi-edge-rf-sensing-platform.md) — sets the single-`CsiFrame` schema, deterministic replay, and quality-scoring constraints that midstream's abstractions conflict with.
- [ADR-096 — rvCSI Crate Topology, the napi-c Shim, the napi-rs Surface](ADR-096-rvcsi-ffi-crate-layout.md) — the crate topology that rvCSI fills the candidate seams with.
- [ADR-097 — Adopt rvCSI as RuView's primary CSI runtime](ADR-097-adopt-rvcsi-as-ruview-csi-runtime.md) — phased adoption (P1-P5) that this ADR explicitly does not duplicate.
- [ADR-012 — ESP32 CSI Sensor Mesh](ADR-012-esp32-csi-sensor-mesh.md) — the multi-source TDM context for D3.
- [ADR-029 — RuvSense Multistatic Sensing Mode](ADR-029-ruvsense-multistatic-sensing-mode.md) — the wall-clock-anchored TDM that `midstreamer-scheduler` is the wrong shape for.
- [ADR-039 — ESP32 Edge Intelligence Pipeline](ADR-039-esp32-edge-intelligence.md) — the firmware-side tier scheduling that would need to move host-side before D6 activates.
- [`github.com/ruvnet/midstream`](https://github.com/ruvnet/midstream) — 5 published crates on crates.io (`temporal-compare`, `nanosecond-scheduler`, `temporal-attractor-studio`, `temporal-neural-solver`, `strange-loop`) + 1 local crate (`quic-multistream`); 139 passing tests.
- `vendor/midstream` (submodule) — pinned at `30fe5eb` (`vendor/midstream/Cargo.toml:16` describes the package as *"Real-time LLM streaming with inflight analysis"*).
- RuView code paths cited in §1: `v2/crates/wifi-densepose-sensing-server/src/main.rs:36,47,1989,2030,3638-3640,4769`; `v2/crates/wifi-densepose-hardware/src/aggregator/mod.rs:74-220`; `firmware/esp32-csi-node/main/csi_collector.c:64,223-228`; `firmware/esp32-csi-node/main/adaptive_controller_decide.c:26-28`.
- RuvSense code paths cited in §3: `v2/crates/wifi-densepose-signal/src/ruvsense/gesture.rs`, `longitudinal.rs`, `field_model.rs`, `adversarial.rs`, `coherence_gate.rs`.
- rvCSI code paths cited in §2: `vendor/rvcsi/crates/rvcsi-events/src/lib.rs:1-37`, `vendor/rvcsi/crates/rvcsi-events/src/pipeline.rs:1-5`.
