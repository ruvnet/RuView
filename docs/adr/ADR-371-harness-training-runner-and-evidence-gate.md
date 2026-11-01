# ADR-371: Harness training runner and mean-pose evidence gate

- **Status**: proposed
- **Date**: 2026-09-30
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: training, evaluation, evidence, pose, honesty
- **Related**: ADR-079 (camera-supervised pose), ADR-151 (room calibration),
  ADR-181 (baseline-relative PCK), ADR-369

## Context

RuView has a retracted pose result (a CLAIMED 92.9%/100% that did not survive
baseline and leakage review). The project rule is that pose
PCK is quoted only with the mean-pose baseline, on a leakage-free held-out
split, with an evidence tag. The `train-pose` skill says this in prose, but
nothing enforces it. Running the trainers also needs repository knowledge:
the `tch-backend` feature, a libtorch version match, and where checkpoints go.

## Decision

### Gate (`ruview_train_gate`, pure, read-only)

The input is a small report: metric, model and baseline score, split,
train/test subjects, train end and test start, `n_test`, reproducer, and data
source.

**Hard failures:**
- no baseline;
- a random-frame split, or any split outside chronological / blocked-gap /
  grouped-subject / grouped-session;
- subject overlap in a grouped-subject split;
- missing or overlapping time bounds for chronological or blocked-gap splits;
- no improvement over the baseline.

**Warnings:**
- subjects not reported;
- subjects overlapping in non-subject splits;
- `n_test` < 100;
- a near-perfect score: at or above the `implausible_score` threshold in
  `src/training.js`.

**Evidence tag:** `SYNTHETIC` when `data: synthetic`, `MEASURED` only with a
reproducer, otherwise `CLAIMED`.

On PASS the gate returns the one acceptable sentence:
`Held-out <metric> X% vs Y% mean-pose baseline = +Z pp (<tag>, …, <split> split).`

### Runner (`ruview_train`, `ruview_train_plan`)

Modes:
- `pose-smoke`: `wifi-densepose-train` `train --dry-run` on the synthetic
  dataset.
- `pose`: `--data-dir`, MM-Fi recordings.
- `room`: `wifi-densepose train-room`, or `cargo run -p wifi-densepose-cli`
  when the binary is absent.

Rules:
- All caller paths (config, data, checkpoints, enrollment, output) must
  resolve inside the trusted checkout. Realpaths are checked for inputs that
  already exist.
- Pose runs use `cargo run --manifest-path v2/Cargo.toml` with the working
  directory set to the checkpoint directory, default
  `v2/target/ruview-train/<mode>`. The trainer's relative `logs/` therefore
  never lands in the source tree.
- Without `confirm` it returns a plan. MCP requires `workspace-write` plus
  `confirm`.
- Failures are classified with remedies:
  - libtorch absent;
  - libtorch version mismatch (tch 0.24 expects torch 2.11.0);
  - link failure.

## Evidence

- **SYNTHETIC** (`npx @ruvnet/ruview train --mode pose-smoke --samples 16
  --confirm`, CPU PyTorch 2.11.0, `LIBTORCH_USE_PYTORCH=1`, Linux x64): the
  real trainer compiled, ran 20 epochs with early stopping, and wrote
  safetensors checkpoints and `logs/training_log.csv` under
  `v2/target/ruview-train/pose-smoke`. The trainer labels its own smoke
  metrics "DO NOT REPORT". None are reported here.
- With PyTorch 2.14.1 the tch 0.24 C++ shim failed to compile. The runner
  classified this as a version mismatch rather than a missing install.
- The gate is covered by `test/training.test.mjs`, including the leakage
  patterns behind the retraction.

## Consequences

- A number can pass through the harness only with its baseline and split
  attached. The gate does not verify the arithmetic that produced the input;
  reviewers must still re-run the reproducer.
- Training needs the Rust toolchain and a matching libtorch. `doctor` covers
  Rust; libtorch is detected at run time.
