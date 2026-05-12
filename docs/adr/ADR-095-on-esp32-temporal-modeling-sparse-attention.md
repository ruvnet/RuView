# ADR-095: On-ESP32-S3 Temporal Modeling at the Edge via `ruvllm_sparse_attention` (no_std)

| Field       | Value                                                                                                  |
|-------------|--------------------------------------------------------------------------------------------------------|
| **Status**  | Proposed (2026-05-07)                                                                                  |
| **Date**    | 2026-05-07                                                                                             |
| **Authors** | ruvnet, claude-flow                                                                                    |
| **Related** | ADR-018, ADR-024, ADR-039, ADR-040, ADR-061, ADR-081, ADR-091; upstream ADR-189, ADR-190, ADR-192      |
| **Branch**  | `feat/ruvllm-sparse-attention-edge`                                                                    |
| **Tracking**| #513                                                                                                   |

---

## 1. Context

Today the ESP32-S3 firmware in `firmware/esp32-csi-node/main/` does
**physics-only** sensing on-device. The pipeline in `edge_processing.c`
runs on Core 1 and produces:

- Adaptive presence detection (`presence_score`).
- Breathing-band (0.1–0.5 Hz) and heart-rate-band (0.8–2.0 Hz) biquad
  IIR bandpass + zero-crossing BPM estimators.
- A motion / fall flag (`flags` bits 0–2 in `edge_vitals_pkt_t` magic
  `0xC5110002`, plus fused mmWave variant `0xC5110004` per ADR-063).
- ADR-081 `rv_feature_state_t` (60 B at magic `0xC5110006`) emitted at
  1–10 Hz from the adaptive controller's fast loop.

There is **no learned model of any kind on the MCU**. The closest things
are: ADR-039 Tier-1 compressed-CSI emission, ADR-040 WASM modules
(Tier-3, but used by the user for ad-hoc DSP, not transformer
inference), and the Rust-side AETHER embeddings (ADR-024) which run
on the host, not the node. Anomaly detection that needs *temporal
context* — "is this fall pattern consistent with a fall, or just a
sit-down?" — is structurally absent. The fall debounce in v0.6.x
(3-frame consecutive + 5 s cooldown, raised threshold 2.0 → 15.0 rad/s²)
is a hand-tuned heuristic exactly because the firmware has nothing
better to reason with.

A second pressure point: the Tmr Svc / FreeRTOS stack is already
sensitive. `edge_processing.c` lines 47–48 explicitly note that
`process_frame + update_multi_person_vitals` combined used ~6.5–7.5 KB
of the 8 KB task stack and that **scratch buffers were moved to static
storage to avoid stack overflow.** Any new heavyweight workload — and
a transformer forward pass is heavyweight — must therefore live in
**its own FreeRTOS task with its own task stack**, not piggyback on
the existing edge DSP task.

The vendored crate `ruvllm_sparse_attention` v0.1.1 (released 2026-05-07,
synced today at `vendor/ruvector/crates/ruvllm_sparse_attention/`)
removes the previously-blocking `std` requirement. Per upstream
**ADR-192**, the crate now compiles cleanly to
`xtensa-esp32s3-none-elf` via `espup`, with a measured **376 KB
release rlib**, zero runtime dependencies beyond `libm`, and was
validated on a real ESP32-S3 (rev v0.2, 16 MB flash). It exposes
`SubquadraticSparseAttention`, `KvCache` / `KvCacheF16`, `FastGrnnGate`,
`IncrementalLandmarks`, `RuvLlmSparseBlock`, and a `Tensor3` value
type. The kernel is O(N log N) by default and near-linear O(N) when
the FastGRNN salience gate is enabled.

This is the first time we have had a credible path to **on-device
transformer inference for CSI** without a Python runtime, without
TFLite, and without a coprocessor. It is also the right moment to
decide *whether* we want it before code starts to land.

---

## 2. Decision

Add a learned **temporal head** to the ESP32-S3 firmware running on
the node itself, using `ruvllm_sparse_attention` compiled
`--no-default-features` (no_std + alloc, optionally `+fp16`), driven
by a small Rust component integrated into the ESP-IDF build. The
temporal head runs **alongside** the existing physics-only pipeline,
not as a replacement — physics gives us breathing/heart-rate/presence,
the temporal head gives us classification and sequence-aware reasoning.

Concretely:

1. The temporal head consumes a rolling window of feature vectors
   (initially the same `rv_feature_state_t` floats already produced
   by ADR-081, plus optionally a small projection of recent CSI
   amplitude statistics), length `N` ∈ [100, 500] frames, sampled at
   the controller's fast-loop rate.
2. It outputs a small set of **class logits** for the active
   detection task. The first three deployable tasks are listed in
   §4.
3. It runs in its own FreeRTOS task on Core 1 (or pinned to whichever
   core the WiFi driver is *not* on), at a cadence slower than the
   fast loop — initially 1 Hz, classification-on-demand.
4. The kernel is invoked through a thin C ABI (`ruv_temporal_init`,
   `ruv_temporal_push_frame`, `ruv_temporal_classify`) exported from
   a Rust static library linked into the ESP-IDF build the same way
   the existing Tier-3 components are linked.
5. Weights are stored as a flat `f32` (or `f16` with the `fp16`
   feature) blob in the ESP32-S3 flash, loadable from either an
   embedded `EMBED_FILES` resource (compile-time bake-in) or NVS
   (post-flash provisioning, mirroring ADR-040's WASM-upload path).
6. The temporal head is gated behind a Kconfig option
   `CONFIG_CSI_TEMPORAL_HEAD_ENABLED`, **default off**, and is only
   compiled into the 8 MB build profile until the flash math in §6
   demonstrates 4 MB headroom.

This ADR authorizes the architecture; it does **not** ship any of
the firmware-side or training-side changes. Implementation lands in
follow-up issues per the roadmap in §7.

---

## 3. Approach

### 3.1 Build integration

ESP-IDF v5.4 already supports Rust components via the
`rust-esp32`-style template (a CMake `idf_component_register` shim
that runs `cargo build --target xtensa-esp32s3-none-elf` and links
the resulting static library). The new component lives at
`firmware/esp32-csi-node/components/ruv_temporal/`:

```
ruv_temporal/
  CMakeLists.txt          # component manifest, Rust build invocation
  Cargo.toml              # crate config: no_std, deps on ruvllm_sparse_attention
  build.rs                # generates the C header from #[no_mangle] exports
  src/lib.rs              # public C ABI: init/push/classify/teardown
  src/window.rs           # rolling frame buffer
  src/weights.rs          # NVS / EMBED_FILES weight loader
  include/ruv_temporal.h  # generated; consumed by edge_processing.c
```

Cargo features compiled in: `["fp16"]`. **Not** `parallel` (rayon
needs threads, breaks no_std). **Not** `std`.

### 3.2 Interface

The C ABI is intentionally narrow. It does not expose `Tensor3`,
attention configs, or any Rust types — only `float*` buffers and
opaque handles:

```c
typedef struct ruv_temporal_ctx ruv_temporal_ctx_t;

esp_err_t ruv_temporal_init(const uint8_t *weights, size_t wlen,
                            uint32_t input_dim, uint32_t window,
                            ruv_temporal_ctx_t **out_ctx);
esp_err_t ruv_temporal_push(ruv_temporal_ctx_t *ctx, const float *frame);
esp_err_t ruv_temporal_classify(ruv_temporal_ctx_t *ctx,
                                float *logits, uint32_t n_classes);
void      ruv_temporal_destroy(ruv_temporal_ctx_t *ctx);
```

`push` is the hot path and must be cheap (it just writes into a
ring buffer in PSRAM if available, IRAM/DRAM otherwise). `classify`
runs the actual sparse attention forward and is the budget-heavy
call.

### 3.3 Task topology

A new task `ruv_temporal_task` with its own 16 KB stack, pinned to
the same core as the edge DSP task (Core 1), fed via a FreeRTOS
queue from the adaptive controller's fast loop. We do **not** call
the kernel from the existing edge task — the edge stack is already
near-full per the comment at `edge_processing.c:47-48` and recent
fall-debounce / Tmr-Svc-stack work.

### 3.4 Memory budget (per inference)

With `N = 256` (window), `d_model = 32`, `n_heads = 4`, `head_dim = 8`,
1–2 `RuvLlmSparseBlock` layers, `block_size = 64`, `window = 64`:

- Weights: ~5–15 KB (single block, INT8 quant deferred to a later
  ADR; FP16 default).
- KV cache (FP16, full window): `2 * 256 * 4 * 8 * 2 B ≈ 16 KB`.
- Activations (peak, with `forward_flash` tiling): ≈ 2 KB.
- Working set: < 64 KB. Comfortable in PSRAM, possible in ISR-safe
  internal SRAM.

These are first-pass estimates; the precise numbers come out of the
`forward_flash` benchmark on real hardware, which is exit criterion
in §7.

### 3.5 Compatibility with ADR-081 / ADR-039 / ADR-018

The temporal head is a **consumer** of the same feature stream
already flowing in the firmware. It does not alter:

- ADR-018 raw CSI frame layout (`0xC5110001`).
- ADR-039 Tier-1 compressed CSI (`0xC5110005`) or vitals
  (`0xC5110002`).
- ADR-063 fused vitals (`0xC5110004`).
- ADR-081 `rv_feature_state_t` (`0xC5110006`) — this is the primary
  input we tap.

If the temporal head fires a classification, the result rides on a
new `0xC5110007` packet (small: class id, confidence, monotonic seq,
ts_us, CRC32). Allocation of that magic is deferred to the
implementation PR — this ADR reserves the *concept*, not the byte
layout.

---

## 4. Use cases that motivate this

| Task | Why temporal context matters | Window | Class count |
|------|------------------------------|--------|-------------|
| **Gesture recognition** (wave / point / clap / kick) | Single-frame CSI snapshots can't disambiguate gestures from random motion. ~100-frame windows capture full gesture trajectories. | 100 frames @ 50 Hz = 2 s | 4–8 |
| **Fall classification with sequence context** | The current heuristic ("> 15 rad/s² for 3 consecutive frames + 5 s cooldown") was raised to suppress false positives. A learned temporal head can distinguish a fall (rapid descent then stillness) from a sit-down (descent then sustained micro-motion) using the same input window. | 200 frames @ 50 Hz = 4 s | 3 (fall / sit / nothing) |
| **Breathing-quality scoring** | Today's pipeline emits a BPM and a confidence float. A temporal head trained on labeled apnea / shallow / paradoxical / normal sequences can output a 4-class quality label that downstream consumers can render in one glance. | 500 frames @ 50 Hz = 10 s | 4 |
| **"Is this normal for this room/time" anomaly detection** | Per-room SONA profiles (ADR-005) capture environment statistics, but anomaly *temporal shape* is currently checked host-side via embedding distance (ADR-024 §2.4 `temporal_baseline` index). A small on-device classifier can flag ahead of host roundtrip. | 300 frames | 2 (normal / anomalous) |

These four cover the visible product gaps in the v0.6.x line.
Gesture recognition is the headline; fall classification is the
highest-impact for the eldercare scenarios v0.5.4 was tuned for.

---

## 5. Alternatives considered

| Option | Why rejected |
|--------|--------------|
| **TFLite Micro** | Heavier runtime (~150 KB code + interpreter), pulls in C++ STL surface, no Rust-native API. Does not benefit from sparse attention specifically. We'd be re-paying the cost of a full inference framework when we only need one kernel. |
| **Run all classifiers server-side** | Costs a full Tier-1 CSI uplink (~50–70 KB/s/node per ADR-039) just to feed a remote classifier, then a roundtrip back. Defeats the point of ADR-081's compact feature stream and makes the system worthless when the backhaul is down. Also leaks raw CSI to the network for purposes the user did not opt into. |
| **Stay physics-only forever** | Cleanest from a maintenance standpoint, but loses gesture, structurally, and the fall-debounce hack will keep accreting per-deployment knobs. The product space already has commodity physics-only firmware (Bosch presence sensors, etc.); on-device transformer inference for CSI is what would *differentiate* RuView. |
| **Use `ruvector-attention` (already in workspace) on-device** | `ruvector-attention` is `std`-bound today; doesn't compile to `xtensa-esp32s3-none-elf` without a port comparable in scope to upstream ADR-192. Even if ported, it doesn't give us GQA + streaming KV cache, which is the structural capability the new crate adds. |
| **Wait for IEEE 802.11bf** | Different problem (standardised CSI exposure across vendors). Doesn't address whether the model runs on-device or off. |

---

## 6. Consequences

### Positive

- **Genuinely novel.** No competing CSI-sensing project ships
  transformer inference on the MCU itself. The closest peers
  (Espressif's ESP-DL, Edge Impulse) are non-attention CNN/RNN
  pipelines.
- **Latency.** Classification result is local — no backhaul,
  no host roundtrip, sub-100 ms gesture-to-action.
- **Privacy.** Raw CSI never leaves the node for these tasks.
- **Reuses the ADR-081 feature stream** — the temporal head is a
  consumer of the existing 60 B `rv_feature_state_t`, not a new
  uplink format.
- **Validated kernel.** Per upstream ADR-192, the no_std build was
  validated on real ESP32-S3 hardware (MAC `ac:a7:04:e2:66:24`).
  We are not betting on a paper crate.

### Negative / tradeoffs

- **Flash budget pressure on 4 MB boards.** Per `partitions_4mb.csv`,
  each OTA slot is 1.875 MB (`0x1D0000`). The current build is
  ~853 KiB. Adding a 376 KB rlib plus weights brings us to ~1.3 MB —
  still under the slot ceiling but with little headroom for other
  growth. **Decision: temporal head is 8 MB-only initially**, gated
  behind `CONFIG_CSI_TEMPORAL_HEAD_ENABLED`. 4 MB enablement is a
  separate ADR after we measure the actual incremental link size
  (the 376 KB upstream number is for the rlib in isolation; the
  linked-and-stripped final binary delta will be smaller).
- **Rust toolchain dependency.** The ESP-IDF build now needs
  `espup` + `cargo +esp` to be present on every developer machine
  and CI runner. This is a real hurdle on Windows — see
  `CLAUDE.local.md` for the existing Python-subprocess wrapper
  required to run ESP-IDF cleanly. CI will need a parallel
  Rust-toolchain step.
- **One more thing to test.** QEMU (ADR-061) does not run the
  ESP32-S3 Xtensa Rust binary today. The QEMU validator pipeline
  will need a build matrix entry for "Rust component compiled but
  classifier disabled" at minimum.
- **Stack overflow risk.** Same hazard the v0.6.4 work just
  navigated. Mitigated by §3.3 (own task, own stack); this needs
  to be a code-review checklist item.
- **Weights provenance.** Once we ship a model, we need a story
  for *which model*, signed by *whom*, retrained *how often*. See
  Open Questions §8.

### Neutral

- ADR-040's WASM Tier-3 path is **not** superseded. WASM remains
  the right choice for user-uploaded modules. The temporal head is
  a first-party signed-by-us component, with a different deploy
  story.
- The host-side ADR-024 AETHER pipeline is unchanged by this ADR.
  ADR-096 covers the host-side use of the same crate.

---

## 7. Roadmap

| Phase | Scope | Gating |
|-------|-------|--------|
| 0 | This ADR + ADR-096 land. No code. | Maintainer review of #513. |
| 1 | New crate `wifi-densepose-temporal` (host-side only): defines the temporal-head architecture, training script, weight serialization format. | Phase 0 accepted. |
| 2 | `ruv_temporal` ESP-IDF component scaffolding — empty kernel, just the C ABI and ring buffer. Compiles cleanly into 8 MB firmware. Adds ~5 KB to binary. | Phase 1 produces a serialised set of weights. |
| 3 | Wire `ruvllm_sparse_attention` `forward` (not yet `forward_gated`) into the component. First on-target classification benchmark on COM7. Gate: end-to-end inference ≤ 50 ms with `N = 256`, no stack overflow under 24 h soak. | Phase 2 ABI stable. |
| 4 | First trained classifier (gesture or fall, whichever has labelled data first). Hardware A/B: temporal-head decision vs current heuristic on a held-out set. Promotion criterion: temporal head matches or beats heuristic on F1 *and* false-positive rate. | Phase 3 latency gate met. |
| 5 | 4 MB profile gating — measure actual binary delta, decide whether to enable on SuperMini. | Phase 4 in production on 8 MB. |
| 6 | `forward_gated_with_fastgrnn` for long-window tasks (breathing-quality at N = 500). | Phase 4 stable. |

---

## 8. Open questions

1. **Who trains the temporal heads?** Two options:
   (a) host-side training on captured `rv_feature_state_t` traces
   labelled in-app, then export to flat-buffer weights;
   (b) teacher-distillation from the larger AETHER model (ADR-024)
   running off-device, using soft labels. Option (b) is more
   data-efficient but couples this ADR's ship date to ADR-024's
   training-pipeline maturity. Open.
2. **How are weights flashed?** Three options, in increasing
   capability: NVS blob (small, safe, 4–8 KB ceiling per key),
   `EMBED_FILES` baked into the firmware image (no runtime update),
   OTA-updateable partition (mirrors ADR-040 RVF upload path,
   biggest engineering cost). Phase 2/3 will pick one; my prior is
   `EMBED_FILES` for the first model, OTA partition once we have
   more than one.
3. **Does the 376 KB rlib figure scale?** Upstream measured
   376 KB for the kernel + the embedding/projection
   weights for *their* test config. Adding 1–2
   `RuvLlmSparseBlock` layers with embedding/projection weights
   sized to actual CSI feature dimension may push this. Phase 2
   will measure the on-target stripped-binary delta directly; if
   the delta exceeds 600 KB we revisit the 4 MB story sooner.
4. **What window length is right for fall classification?**
   200 frames at 50 Hz = 4 s feels right based on the v0.6.4
   debounce numbers (3-frame consecutive + 5 s cooldown is
   essentially a 4-second decision window already). Empirical, not
   architectural — set in Phase 4.
5. **Quantisation.** First model ships FP16 (KV cache feature flag
   already supports this). INT8 for both weights and activations
   is a follow-up; the current crate has no INT8 path so it would
   be a separate kernel.
6. **What happens when the controller is in `RV_PROFILE_PASSIVE_LOW_RATE`?**
   The fast loop slows down, so the input frame rate to the
   temporal head drops. Either the head needs to handle variable
   sample rate (resample at push time) or it stops emitting until
   the controller goes back to active. Phase 1 design call.

---

## 9. Acceptance criteria

This ADR is **Accepted** once:

1. Maintainer review on #513 confirms the architecture.
2. The follow-up implementation issue is filed and references this
   ADR plus ADR-096 by number.
3. ADR index in `docs/adr/README.md` (if present) has an ADR-095
   row.

This ADR is **Implemented** once:

1. Phase 3 is in `main` with the gating Kconfig off by default.
2. A Phase-4 hardware A/B has been published (witness-bundle
   compatible per ADR-028).
3. The QEMU validator (ADR-061) has at minimum a "compiles, doesn't
   run" check for the Rust component.

---

## 10. Related

ADR-018 (binary CSI frame), ADR-024 (AETHER contrastive embedding —
host-side counterpart, see ADR-096), ADR-039 (edge intelligence
tiers), ADR-040 (WASM Tier-3 modules — the *other* extensibility
path), ADR-061 (QEMU CI), ADR-081 (adaptive controller, mesh plane,
`rv_feature_state_t`), ADR-091 (stand-off radar tier — adjacent
edge-intelligence ADR), upstream ADR-189 (KV cache incremental
decode), upstream ADR-190 (GQA/MQA), upstream ADR-192 (no_std +
alloc on ESP32-S3 — the structural unblock that makes this ADR
possible).
