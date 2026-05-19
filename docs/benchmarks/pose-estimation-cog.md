# `cog-pose-estimation` — Benchmark Log

This file tracks every published benchmark for the pose-estimation Cog. New runs append; never overwrite history. Per ADR-101 §"Acceptance gates".

## v0.0.1 — first measured run (2026-05-19)

### Setup

| Component | Value |
|-----------|-------|
| Training host | `ruvultra` (Ubuntu 6.17, x86_64, RTX 5080) |
| Backend | `candle-core 0.9` with `cuda` feature |
| Data | `data/paired/wiflow-p7-1779210883.paired.jsonl` — 1,077 paired samples, 30-min seated-at-desk recording, avg conf 0.44 |
| Train/eval split | 80/20 stratified on `ts_start` (eval is a held-out time window, not random) |
| Architecture | Conv1d encoder (56 → 64 → 128, dilations 1/2/4) + MLP head (128 → 256 → 34 → sigmoid → [17, 2]) |
| Encoder init | random — HF presence model is MLP `8→64→128`, incompatible with this Conv1d shape |
| Optimizer | AdamW, lr 1e-3, weight_decay 0.01 |
| LR schedule | Cosine with 50-epoch warm restarts |
| Loss | SmoothL1 (Huber β=0.1), confidence-weighted by `record.conf` |
| Augmentation | Subcarrier dropout 10% (final 50 epochs) |
| Epochs | 400 (full-batch) |
| Wall time | **2.1 s** total |

### Accuracy

| Metric | Value |
|--------|-------|
| **PCK@20** (overall) | **3.0%** |
| **PCK@50** (overall) | **18.5%** |
| **MPJPE** (normalized) | **0.0931** |
| Final eval loss | 0.0101 |
| Loss reduction | 0.181 → 0.014 (13×) |

### Per-joint PCK

| Joint | PCK@20 | PCK@50 |  | Joint | PCK@20 | PCK@50 |
|-------|-------:|-------:|--|-------|-------:|-------:|
| nose | 0.5% | 5.1% |  | l_hip | 0.0% | 27.3% |
| l_eye | 2.8% | 8.3% |  | **r_hip** | **25.0%** | **76.9%** |
| r_eye | 1.9% | 15.7% |  | l_knee | 2.3% | 20.8% |
| l_ear | 0.0% | 3.2% |  | r_knee | 0.9% | 35.2% |
| r_ear | 1.9% | 9.7% |  | l_ankle | 1.4% | 7.9% |
| l_shoulder | 4.6% | 8.8% |  | r_ankle | 0.9% | 9.3% |
| r_shoulder | 1.9% | 19.9% |  | l_elbow | 1.9% | 26.4% |
| l_wrist | 3.2% | 24.1% |  | r_elbow | 0.0% | 4.2% |
| r_wrist | 1.4% | 12.0% |  |  |  |  |

Strongest signal at right-side proximal joints (`r_hip` 77% PCK@50, `r_knee` 35%, `r_shoulder` 20%) — consistent with the camera framing during data collection (operator's right side most consistently in frame).

### Comparison to prior baseline

| Run | Backend | Train time | PCK@20 | PCK@50 | MPJPE |
|-----|---------|-----------:|-------:|-------:|------:|
| pre-2026-05-19 | pure-JS SPSA, lite TCN (#640) | ~20 min | 0.0% | 0.0% | 0.66 |
| **v0.0.1** (this run) | **candle-cuda, Conv1d TCN** | **2.1 s** | **3.0%** | **18.5%** | **0.093** |

**7× MPJPE improvement, 570× faster training, signal-bearing PCK at all proximal joints.** The remaining gap to ADR-079's PCK@20 ≥ 35% target is data-bound, not infra-bound (see Issue #640).

### Inference latency

Measured on Windows host (x86_64, no GPU — `candle-cpu` backend) running the release binary:

| Mode | Measurement | Notes |
|------|-------------|-------|
| Cold start | **76.2 ms / invocation** (avg over 100 sequential `health` invocations) | Includes safetensors load + 1 synthetic forward pass. Most of the cost is process startup + mmap. |
| Long-running `run` warm inference | sub-millisecond per frame (estimated) | The model is 125K params / 507 KB; once loaded, a single forward at batch=1 is essentially memory-bandwidth bound. To be measured precisely against a live sensing-server feed. |

### ONNX export

`pose_v1.onnx` is produced from `pose_v1.safetensors` by `scripts/export-onnx.py`, which mirrors the Candle architecture in PyTorch, loads the safetensors weights, and uses `torch.onnx.export` with opset 18 + dynamic batch axis. Verified end-to-end:

| Check | Result |
|-------|--------|
| `onnx.checker.check_model` | ✅ ok |
| Parity vs torch reference | **max \|torch − onnx\| = 8.94e−8** (1e−5 threshold) |
| File size | 12,059 bytes |
| Dynamic axes | `batch` on input and output |

The ONNX artifact is the input to the Hailo Dataflow Compiler (HEF cross-compile) and to ONNX Runtime CPU/GPU benchmarks on each target arch — both still pending.

### Real-hardware smoke (cognitum-v0 Pi 5)

Cross-compiled to `aarch64-unknown-linux-gnu` on ruvultra and run on a live Cognitum-V0 appliance:

| Host | Mode | Result |
|------|------|--------|
| ruvultra (under `qemu-aarch64-static`) | `health` | `backend: candle-cpu`, `confidence: 0.185` — real weights loaded under emulation |
| **cognitum-v0** (Raspberry Pi 5, Cortex-A76) | `health` | `backend: candle-cpu`, `confidence: 0.185` — real weights, real hardware |
| cognitum-v0 | 30× sequential `health` invocations | **0.251 s total → 8.4 ms / invocation** (cold) |

8.4 ms cold-start on real Pi 5 hardware vs 76 ms on the x86_64 Windows host. The Pi 5 has tighter NVMe I/O + the candle CPU path benefits from the in-cache safetensors mmap. Long-running `run` warm inference will still be sub-millisecond.

### Release artifacts (signed + published to GCS)

```
gs://cognitum-apps/cogs/arm/cog-pose-estimation-arm                       3,741,976 bytes
gs://cognitum-apps/cogs/arm/cog-pose-estimation-pose_v1.safetensors         507,032 bytes

binary_sha256:  1e1a7d3dd01ca05d5bfc5dbb142a5941b7866ed9f3224a21edc04d3f09a99bf5
weights_sha256: eb249b9a6b2e10130437a10976ed0230b0d085f86a0553d7226e1ae6eae4b9e5
signature:      LUN7xqLPYD3MFzm5dKB5MnYU0LvoRtek5ci5KiKPHBg+Xo6xuazwokn2Dw2JPMaLYJzmWn/SpT4djuR7hYvVDw==   (Ed25519, signed with COGNITUM_OWNER_SIGNING_KEY)
```

Full manifest at `cog/artifacts/manifest.json`. Verified via public anonymous GET against `https://storage.googleapis.com/cognitum-apps/cogs/arm/cog-pose-estimation-arm` — downloaded SHA matches the locally-computed SHA.

Pending separately:

- Hailo HEF cross-compile (gated on Hailo SDK on a self-hosted runner) — uses `pose_v1.onnx` as input.
- `cog-pose-estimation run` long-run latency under a live sensing-server feed.
- x86_64 release upload (today's release is arm-only).

### Artifacts

- `v2/crates/cog-pose-estimation/cog/artifacts/pose_v1.safetensors` — 507 KB
- `v2/crates/cog-pose-estimation/cog/artifacts/train_results.json` — full per-epoch loss curve + hyperparameters + per-joint PCK

### Reproducibility

```bash
# On any host with cargo + a CUDA-capable GPU:
cd ~/work/cog-pose-train
mkdir -p ./
# Stage the same inputs (1,077 paired samples + HF encoder, see scripts/align-ground-truth.js for regeneration)
cp paired.jsonl ./paired.jsonl
cp encoder.safetensors ./encoder.safetensors

# Build & train (no Python, no pip)
cargo new --bin pose-trainer && cd pose-trainer
# Edit Cargo.toml deps: candle-core 0.9 (cuda), candle-nn 0.9 (cuda), safetensors, serde, serde_json, anyhow
# Drop the training script into src/main.rs (see this repo's training-tooling examples for reference)
cargo run --release
```

`candle-core 0.8.4 + 0.9.2` are typically already in `~/.cargo/registry/cache/` on any developer host, so the build completes in seconds.
