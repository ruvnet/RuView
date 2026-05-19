# ADR-101: Pose Estimation Cog (WiFi-DensePose side)

- **Status:** Accepted — **v0.0.1 shipped 2026-05-19** (merged in PRs #642 + #643, signed binaries on GCS, live install on cognitum-v0)
- **Date:** 2026-05-19
- **Deciders:** ruv
- **Companion ADR (v0-appliance side):** v0-appliance ADR-225 (cognitum-pose-estimation crate)

## Context

ADR-079 designed the 17-keypoint COCO pose-estimation training pipeline. ADR-100 formalised the Cognitum Cog packaging spec. This ADR is the bridge: it specifies how the wifi-densepose training pipeline produces an artifact that ships as a Cog (`cog-pose-estimation`) onto the Cognitum V0 appliance and out to the Pi+Hailo cluster.

It is the next product step beyond the published `presence` Cog (binary head trained from the contrastive encoder on Hugging Face at `ruvnet/wifi-densepose-pretrained`). Where `presence` reports a single boolean per tick, `cog-pose-estimation` reports 17 (x, y) keypoints per person, per tick.

## Decision

### Pipeline

```
                         (training side — ruvultra GPU)
ESP32 / rvcsi  ─►  collect-ground-truth.py + sensing-server recording
                         │
                         ▼
                   data/paired/*.paired.jsonl   (CSI window + camera keypoints)
                         │
                         ▼
                   v2/crates/wifi-densepose-train  ──►  Rust + libtorch trainer
                   (uses RTX 5080 / CUDA 12.x)         │
                   init from ruvnet/wifi-densepose-pretrained
                                                       │
                                                       ▼
                                                  model.safetensors  (encoder + pose head)
                                                       │
                                          ─────────────┴─────────────
                                          │                         │
                                          ▼                         ▼
                                  v2/crates/cog-pose-estimation     export to ONNX
                                  (this repo)                       │
                                   • emits manifest.json            ▼
                                   • produces cog binary       cognitum-hailo
                                   • signs + uploads to GCS    (v0-appliance side)
                                                                    │
                                                                    ▼
                                                           cog-pose-estimation.hef
                                                                    │
                                                                    ▼
                              (appliance side — cognitum-v0 + Pi+Hailo cluster)
                                                           
                              gs://cognitum-apps/cogs/{arm,hailo8,hailo10}/cog-pose-estimation-<arch>
                                                                    │
                                                                    ▼
                              `cognitum-cog-gateway` pulls artifact + manifest, verifies signature, installs
                              into /var/lib/cognitum/apps/pose-estimation/
                                                                    │
                                                                    ▼
                              run loop: read CSI frames from local sensing-server
                              → encoder → pose head → emit `{ts, persons: [{keypoints: [...17 x,y...] }]}`
                              on stdout as the Cog runtime contract requires
```

### Architecture (model)

| Stage | Module | Notes |
|-------|--------|-------|
| Input | `[56 subcarriers × 20 frames]` per CSI window | matches today's `data/paired/wiflow-p7-*.paired.jsonl` |
| Encoder | TCN-lite or contrastive encoder lifted from HF presence model | 128-dim embedding; weights init from `ruvnet/wifi-densepose-pretrained/model.safetensors` |
| Pose head | 2-layer MLP `(128 → 256 → 34)` | 34 = 17 × (x, y) |
| Output | `[B, 17, 2]` keypoints in `[0, 1]` image-normalised coords | confidence is implicit in keypoint variance over time; ADR-079 P9 will add explicit per-joint confidence |
| Loss | Confidence-weighted SmoothL1 (frame-level) + bone-length regulariser + temporal smoothness | per ADR-079 Phase 3 refinement |
| Init | Encoder = HF presence weights (frozen for 50 epochs, then jointly fine-tuned) | unblocks the sigmoid-saturation failure mode observed in #640 |
| Training | `v2/crates/wifi-densepose-train` with libtorch backend on RTX 5080 | replaces the pure-JS SPSA trainer that produced 0% PCK in #640 |

### Repo layout

```
v2/crates/cog-pose-estimation/        # NEW (this ADR)
├── Cargo.toml
├── src/
│   ├── main.rs                # CLI: run | health | version | manifest
│   ├── lib.rs
│   ├── inference.rs           # ONNX runtime + Hailo HEF runtime dispatch
│   ├── frame_subscriber.rs    # local sensing-server subscriber
│   └── publisher.rs           # emits structured JSON events per Cog contract
├── cog/
│   ├── manifest.template.json
│   ├── config.schema.json
│   ├── README.md
│   ├── icon.svg
│   └── Makefile               # build-arm | build-x86_64 | sign | upload
└── tests/
    ├── manifest_signature.rs
    └── inference_smoke.rs
```

### Runtime contract

Honours ADR-100's per-Cog CLI contract:

- `cog-pose-estimation version` → `pose-estimation 0.0.1`
- `cog-pose-estimation manifest` → JSON
- `cog-pose-estimation health` → 0 if encoder+head load and a synthetic frame produces a finite output
- `cog-pose-estimation run --config /etc/cognitum/cogs/pose-estimation/config.json` → long-running; emits one JSON event per inferred frame:

```json
{
  "ts": 1779210883.444,
  "level": "info",
  "event": "pose.frame",
  "fields": {
    "tick": 12345,
    "n_persons": 1,
    "persons": [
      {"keypoints": [[0.48, 0.31], [0.52, 0.28], ...], "confidence": 0.81}
    ]
  }
}
```

### Hardware deployment

| Target | arch | runtime | notes |
|--------|------|---------|-------|
| ruvultra (dev) | `x86_64` | ONNX Runtime CPU/CUDA | development & smoke tests |
| cognitum-v0 (Pi 5) | `arm` | ONNX Runtime ARM | reference deploy; ~20 ms/frame |
| Pi + Hailo-8 hat | `hailo8` | Hailo HEF runtime via `cognitum-hailo` | ~2 ms/frame, 26 TOPS budget |
| Pi + Hailo-10 hat | `hailo10` | Hailo HEF runtime via `cognitum-hailo` | ~1 ms/frame, 40 TOPS budget |

### Acceptance gates

1. **Validates:** `cargo test -p cog-pose-estimation` green; `cog-pose-estimation health` returns 0 against a synthetic CSI window.
2. **Benchmarks:** end-to-end frame latency on each target arch logged in `target/criterion/`; published in `docs/benchmarks/pose-estimation-cog.md`.
3. **Optimised:** the Hailo-targeted ONNX graph passes through Hailo Dataflow Compiler without quantisation-aware-training warnings.
4. **Published:** signed binary at `gs://cognitum-apps/cogs/<arch>/cog-pose-estimation-<arch>`; manifest valid against the JSON schema in ADR-100; appliance installer can pull and run it.

PCK@20 is intentionally **not** an acceptance gate of this ADR. Achieving the ADR-079 ≥35% target is a separate, data-bound milestone tracked in #640. This ADR ships the **vehicle**, not the model accuracy.

### First measured run — v0.0.1 (2026-05-19)

A Candle-on-CUDA training run on `ruvultra`'s RTX 5080 against the same 1,077-sample paired session that produced the 0%/0% baseline in #640 yielded:

- **PCK@20 = 3.0%**, **PCK@50 = 18.5%**, **MPJPE = 0.093** (normalized).
- 400 epochs in **2.1 s** wall time (~5 ms/epoch, full-batch).
- Loss reduction 13× (0.181 → 0.014, eval 0.010).
- Strongest signal at `r_hip` (PCK@50 = 76.9%), `r_knee` (35.2%), `l_elbow` (26.4%).

This confirms the pipeline trains end-to-end and produces a signal-bearing model. The remaining gap to PCK@20 ≥ 35% is data-bound (1,077 samples is ≪ the ADR-079 target of ~30K). See `docs/benchmarks/pose-estimation-cog.md` for the full result dump.

## Consequences

### Positive

- First Cog from this repo that integrates with the appliance/cog-gateway pipeline. Future cogs (e.g. `cog-vitals`, `cog-fall-alert`) follow the same template.
- Closes the loop from data collection → training → quantisation → cluster deployment with a single repo-anchored artifact.
- Forces a real signature on cog binaries (per ADR-100), which improves supply-chain hygiene across the whole appliance.

### Negative

- Adds a hard dependency on the Hailo Dataflow Compiler, which lives behind a self-hosted runner — Hailo-targeted PRs land more slowly.
- The first published binary will have low PCK (data + training time gap, #640) — UX needs to surface this clearly so end users do not interpret bad keypoints as a bug.

### Risks

- **Model size on Hailo**: the encoder fits comfortably in Hailo-8's on-chip SRAM, but the pose-head expansion to `[17×2]` plus required temporal stacking pushes us close to the Hailo-8 envelope. Mitigation: Hailo-10 path is the primary deploy target; Hailo-8 is a stretch.
- **Sensing-server schema drift**: the cog subscribes to `/api/v1/sensing/latest` JSON. If the appliance's sensing-server schema changes, the cog fails open (logs warning, emits nothing). The `frame_subscriber.rs` module pins to schema version `2`.

## Migration / rollout

1. Land this ADR + ADR-100 on `main` of RuView.
2. Land companion ADR-225 + crate on `main` of v0-appliance.
3. First release `cog-pose-estimation@0.0.1` ships **only** to `ruvultra` and `cognitum-v0`. Not pushed to the cluster Pis yet.
4. After P7→P9 data work (#640) brings PCK above a usable threshold, rebuild + re-publish; only then enable cluster rollout via `cognitum-cog-gateway`'s OTA channel.

## v0.0.1 shipping status — 2026-05-19

PRs `#642` (scaffold + arm release + ONNX + live install) and `#643` (x86_64 release) landed on `main`. Acceptance gates from ADR-100 met as follows:

| Gate | Status |
|------|--------|
| Cog binary exists per arch | ✅ arm (`3,741,976 B`) + x86_64 (`4,548,856 B`) on GCS |
| Manifest matches schema | ✅ `cog/artifacts/manifests/{arm,x86_64}/manifest.json` |
| Binary sha256 + Ed25519 signature | ✅ both signed with `COGNITUM_OWNER_SIGNING_KEY`, round-trip verified |
| Public-readable GCS | ✅ anonymous HTTP GET works, SHA matches |
| Live install on a real appliance | ✅ `/var/lib/cognitum/apps/pose-estimation/` on `cognitum-v0` (Pi 5), same layout as `anomaly-detect` |
| Runtime contract (`version \| manifest \| health \| run`) | ✅ all four return correct output; `run` emits `pose.frame` events |
| Real weights loaded (not stub) | ✅ `cargo test` asserts `backend.starts_with("candle-")` + non-zero confidence |
| ONNX artifact (for downstream HEF) | ✅ `pose_v1.onnx` (12 KB), parity vs torch = 8.94e-8 |

| Metric | Value |
|--------|-------|
| Training time (RTX 5080 / Candle CUDA) | 2.1 s for 400 epochs |
| PCK@20 / PCK@50 / MPJPE (1,077-sample seated-desk session) | 3.0% / 18.5% / 0.093 |
| Cold-start: Windows x86_64 | 76 ms |
| Cold-start: ruvultra x86_64 | **5.4 ms** |
| Cold-start: Pi 5 aarch64 | **8.4 ms** |
| Tests | 5/5 pass |

Open follow-ups carried forward from this ADR's "Acceptance gates" section:

- **Hailo HEF cross-compile** — `pose_v1.onnx` is ready; still gated on Hailo Dataflow Compiler + self-hosted runner provisioning. Tracked separately.
- **PCK@20 ≥ 35%** — explicitly not an acceptance gate of this ADR, but the limiting factor on practical usefulness. Tracked in [#640](https://github.com/ruvnet/RuView/issues/640): needs ~30× more paired samples + multi-room camera framing. Today's seated-desk session is the demonstrated bottleneck.

## See also

- ADR-079: Camera-supervised pose training pipeline (the model we're shipping).
- ADR-100: Cog packaging specification (the format we're shipping in).
- v0-appliance ADR-225: cognitum-pose-estimation crate (the appliance-side runtime).
- v0-appliance ADR-220: cog management surface (where this cog appears in the dashboard).
- Issue #640: PCK gap (current 3% / 18.5% → ≥35% target).
- `docs/benchmarks/pose-estimation-cog.md`: full benchmark log, all measured numbers.
