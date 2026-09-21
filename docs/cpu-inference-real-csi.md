# CPU-First Inference on Real Public CSI

This guide covers the CPU inference track for RuView: preparing real public
WiFi CSI datasets, running the explicit preprocessing pipeline, and
benchmarking the `cpu-micro-fp32` / `cpu-micro-int8` profiles. Background
decisions live in ADR-015 (public-dataset strategy), ADR-173 (metric lock),
and ADR-175 (measured INT8 trade-off).

> The models are trained and evaluated on publicly released real CSI
> measurements. These results establish performance on the listed datasets
> only. They do not establish equivalent performance on RuView ESP32 hardware
> or in a new room. Hardware-specific validation, calibration, and local
> fine-tuning remain necessary.

## Why CPU inference

Edge RuView nodes (x86-64 gateways, ARM64 companions, and — via
the ONNX path — constrained devices) cannot assume a GPU. CPU is the default
backend: CPU-only builds require no CUDA, no GPU libraries, and no cloud
services. `cpu-micro-fp32` is the numerical reference; `cpu-micro-int8` is
the first deployment target (smaller, faster where int8 kernels exist — but
see ADR-175: INT8 is not automatically a win and must be measured per model).

## Datasets

### MM-Fi (primary)

- Source: <https://github.com/ybhbingo/MMFi_dataset>
- Paper: Yang et al., "MM-Fi: Multi-Modal Non-Intrusive 4D Human Dataset for
  Versatile Wireless Sensing", NeurIPS 2023 Datasets & Benchmarks
  (arXiv:2305.10345).
- Expected characteristics (verify against your download, then record them in
  the manifest): real WiFi CSI, ~40 subjects, ~27 actions, ~320,000 frames,
  114 subcarriers at 40 MHz, 1 TX × 3 RX (Atheros CSI Tool, TP-Link N750),
  ~100 Hz, 17-keypoint COCO-3D + DensePose UV labels.
- License: **CC BY-NC 4.0** — research and internal use; weights trained on
  MM-Fi must **not** be deployed commercially.

### Wi-Pose (secondary)

- Source: <https://github.com/NjtechCVLab/Wi-PoseDataset>
- Expected characteristics: real WiFi CSI, 12 volunteers × 12 actions,
  ~166,600 packets, 30 subcarriers, 3 TX × 3 RX, 18-keypoint AlphaPose
  labels, `.mat` file format.
- License: **research use** — confirm the current license text in the Wi-Pose
  repository before anything beyond research.
- Status in this slice: manifest template only (`DatasetManifest::
  wi_pose_template`). The `.mat` files must first be converted to the
  documented `.npy` layout; a Wi-Pose loader is a follow-up.

### Manual download (no bypassing access controls)

Both datasets are gated by their authors (Drive / Netdisk / request forms).
Download them by hand into an **ignored** data directory — never commit
dataset files to Git:

```bash
# suggested layout (data/ is git-ignored)
data/public/mmfi/          # S01/A01/wifi_csi.npy, wifi_csi_phase.npy, gt_keypoints.npy, …
data/processed/mmfi/       # generated: dataset_manifest.json, split.json
```

## Preparation workflow

The preparation command is Rust (the dataset layer lives in
`wifi-densepose-train`, per ADR-015 — a Python duplicate would drift):

```bash
cargo run -p wifi-densepose-train --no-default-features \
  --bin prepare-public-dataset -- \
  --input data/public/mmfi \
  --output data/processed/mmfi \
  --window-frames 10 \
  --test-fraction 0.2 \
  --seed 42 \
  --environment-count 4 \
  --sha256
```

What it does:

1. Discovers `SXX/AXX` recordings (`MmFiDataset::discover`).
2. **Measures** the native subcarrier width from the first `wifi_csi.npy`
   header — never assumed.
3. Counts files, subjects, activities; subjects counted, environments taken
   from `--environment-count` (the directory layout does not encode rooms;
   `0` means unknown).
4. Builds the default **subject-disjoint** split (seeded, leak-checked).
5. Optionally SHA-256-hashes every `.npy` file (`--sha256`; slow on full data).
6. Validates and writes `dataset_manifest.json` + `split.json`. Anything
   unverifiable fails the run instead of writing a manifest.

Small development subsets without copying data:

```bash
... --subset-subjects 2   # first 2 sorted subjects only (recorded in manifest notes)
```

CI never downloads datasets: all tests use deterministic synthetic fixtures
(see `tests/test_public_dataset.rs`).

### Wi-Pose `.mat` conversion (documented path, operator-run)

```python
# One-off, on your machine: convert release .mat files to the SXX/AXX .npy
# layout above (amplitude/phase [T, n_tx, n_rx, 30] float32 + keypoints).
# Then prepare with the same command. Do NOT redistribute converted files
# beyond what the Wi-Pose license allows, and record the conversion
# (method + script hash) in the manifest notes.
import scipy.io  # operator-provided; not a repo dependency
```

## Preprocessing pipeline (`real-csi-v1`)

`RealCsiPipeline` (`wifi-densepose-train/src/real_csi.rs`) stages, in order:

```text
raw window [T, n_tx, n_rx, n_sc]
  -> shape validation (amp/phase must match, all dims > 0)
  -> NaN / ±inf rejection (fail-closed: error, no repair)
  -> phase sanitization + unwrapping per (t,tx,rx) lane along subcarriers
     (wifi-densepose-signal PhaseSanitizer)
  -> amplitude denoising: Hampel filter per (tx,rx,sc) time lane
     (wifi-densepose-signal hampel_filter; outliers replaced by local median)
  -> per-antenna-pair standardization  z = (x - mean) / (std + 1e-6)
     (population std over the (T, SC) window, f64 accumulation)
  -> subcarrier conversion to the target width (see below)
  -> model input tensors + per-sample provenance + quality report
```

Quality handling: insufficient frames (`< min_frames`, default 2),
Hampel outlier fraction above `max_outlier_fraction` (default 0.5, the
median/MAD breakdown point), and any non-finite input are all hard errors
(`PreprocessError`). Every output window carries `ProcessedSampleMeta`
(source dataset, subject, environment, recording, activity, frame,
original/processed subcarrier counts, antenna layout, sampling rate,
pipeline version) — converted data is never labeled as native.

### Subcarrier conversion math

Piecewise-linear interpolation over normalized positions
`x = k / (n_sc − 1)`; output lane `j` samples the input at
`x_j = j / (target − 1)`:

```text
out[j] = in[i] + (in[i+1] − in[i]) · (x_j − x_i) / (x_{i+1} − x_i)
```

- MM-Fi 114 → 56: interpolation (same Atheros family, 40 → 20 MHz).
- Native width → target: exact passthrough, recorded as
  `native-passthrough`.
- Wi-Pose 30 → 56: **not** silent — 30-sub Intel captures have different
  spectral occupancy than 56-sub Atheros captures, so the method (interpola-
  tion vs. zero-pad) must be chosen and recorded per evaluation. The loader
  refuses to equate them implicitly.

## Train / validation / test splits

Default: **subject-disjoint** (`MmFiDataset::subject_disjoint_split`,
seeded Fisher–Yates over sorted subjects, self-checked by
`assert_split_leak_free`). Report separately: random in-domain (only for
comparison with published numbers), subject-disjoint, environment-disjoint,
recording-disjoint, and cross-dataset. Every result cites its `split.json`
(policy + seed + subject lists). Never fine-tune on the test set.

## Model profiles

`wifi-densepose-nn/src/profile.rs`:

| Profile | Precision | Status |
|---|---|---|
| `cpu-micro-fp32` | FP32 | implemented (reference) |
| `cpu-micro-int8` | INT8 | implemented (deployment target) |
| `cpu-nano-int8` | INT8 | reserved (rejected loudly) |
| `cpu-tiny-int8` | INT8 | reserved (rejected loudly) |
| `cpu-micro-int4-qat` | INT4 | reserved (rejected loudly; no INT4 until FP32+INT8 are measured) |

`ModelMetadata` records profile, architecture, shapes, parameter count,
precision, quantization method, size, devices, training/validation
provenance, preprocessing version, accuracy status, and license notes — and
`validate()` refuses conflations (e.g. hardware accuracy claims without
hardware validation, missing license notes). The ONNX backend honors
`threads` (intra-op pool) and `deterministic` (forces 1 thread); mismatched
profile/precision fails at build time.

## CPU benchmark

```bash
# Pipeline-only smoke (no model needed; this is what CI exercises)
cargo run -p wifi-densepose-nn --bin cpu-inference-bench -- \
  --profile cpu-micro-fp32 --threads 1 --iterations 200 --report smoke.json

# Full benchmark (needs a model file + real or prepared input tensor)
cargo run --release -p wifi-densepose-nn --bin cpu-inference-bench -- \
  --model cpu-micro-int8.onnx --input-npy data/processed/mmfi/sample.npy \
  --profile cpu-micro-int8 --threads 1 --iterations 1000 --report int8.json

# FP32 vs INT8 comparison (refuses mismatched inputs)
cargo run -p wifi-densepose-nn --bin cpu-inference-bench -- \
  --compare fp32.json int8.json
```

Each report prints and stores: OS/arch/CPU/rustc, profile, precision,
threads, input shape/source, warmup/iterations, model-load time, P50/P95
latency, throughput (batch 1), model size, peak RSS (Linux), and per-output
statistics (+ full values for small heads). Preprocessing-stage timings live
in the `real_csi_bench` criterion bench (synthetic windows, software timing
only).

Results table to fill per evaluation (all cells MEASURED or explicitly
not-run):

| Training data | Evaluation data | Split | Model | Precision | Metric | Result |
|---|---|---|---|---|---|---|
| MM-Fi | MM-Fi | subject-disjoint | cpu-micro-fp32 | FP32 | PCK/F1 | not run (no dataset locally) |
| MM-Fi | MM-Fi | subject-disjoint | cpu-micro-int8 | INT8 | PCK/F1 | not run (no dataset locally) |
| MM-Fi | Wi-Pose | cross-dataset | cpu-micro-fp32 | FP32 | PCK/F1 | not run (loader follow-up) |
| MM-Fi | Wi-Pose | cross-dataset | cpu-micro-int8 | INT8 | PCK/F1 | not run (loader follow-up) |

## What was measured in this slice (this machine)

- `cargo test -p wifi-densepose-train --no-default-features`: 279 lib +
  all integration suites green (incl. 5 new `test_public_dataset` tests).
- `cargo test -p wifi-densepose-nn` (default and `--no-default-features`):
  green, incl. profile/options/bench-parser unit tests.
- `cargo test -p wifi-densepose-signal --no-default-features`: 496 green.
- `prepare-public-dataset` on a 2-subject synthetic fixture: manifest +
  subject-disjoint split written, native width measured as 114.
- `cpu-inference-bench` smoke: header + `SMOKE-OK`.
- Clippy: zero warnings in new code (remaining repo warnings pre-date this
  change; two clippy errors in `accuracy.rs`/`metrics_core.rs` come from a
  newer clippy than the pinned 1.89 toolchain, untouched here).
- Doc-tests could not execute in this sandbox (Application Control blocks
  `rustdoc.exe`); the two new doc examples mirror assertions already
  covered by unit tests.

## Limitations (read before citing any number)

1. No accuracy numbers exist yet: no MM-Fi/Wi-Pose files were downloaded
   here and no `cpu-micro-*` weights were trained.
2. Public-dataset accuracy never equals RuView ESP32 accuracy: different
   chipsets (Atheros/Intel vs ESP32), antenna counts (1×3 / 3×3 vs mesh),
   rooms, sampling rates, and subcarrier grids.
3. 114→56 interpolation loses frequency resolution; cross-dataset gaps are
   expected and must be reported, not tuned away on the test set.
4. MM-Fi is CC BY-NC 4.0: no commercial deployment of MM-Fi-trained
   weights. Wi-Pose is research-use.
5. x86 int8 numbers do not transfer to edge-SoC int8 latency (cf. ADR-175).

## Future ESP32 validation path

The interfaces are shaped so local hardware slots in without rewriting the
runtime: ESP32 frame capture → packet validation → calibration → the same
`RealCsiPipeline` stages → temporal windowing → CPU inference (profile-
selected) → temporal smoothing → pose/activity output, with
`LocalHardwareValidation::Validated` flipped only on captured evidence.
Collect across rooms, people, placements, routers, empty-room baselines,
and interference conditions; pose claims need synchronized ground truth.
