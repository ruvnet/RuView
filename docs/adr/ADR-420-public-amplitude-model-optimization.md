# ADR-420: Public amplitude activity model optimization

Status: Proposed, candidate implementation

Date: 2026-10-12

## Context

ADR-419 reproduces the eight-feature temporal embedding contract. Public
ESP-Fi HAR samples contain 52 amplitude channels over 950 time points, not
those eight features. Reinterpreting subcarriers as engineered features would
invalidate the pretrained model's input semantics. Activity recognition is
also a different task from temporal retrieval or dense pose estimation.

## Decision

Implement a separate amplitude classifier experiment with its own artifacts.
The original model and production sensing remain the rollback baseline.

Pin the upstream revision and full archive SHA256. Retain the upstream CC BY
4.0 attribution, numeric activity identifiers and explicit provenance. The
upstream README and code disagree about activity names, so no candidate may
claim named activity or fall detection performance until that conflict is
resolved. Never commit captures or prepared signal arrays.

Separate rooms and participants before fitting any preprocessing. Exclude
crossed validation/test identities from the training set. Reject duplicates,
nonfinite arrays, unsafe archive paths, unsupported shapes and unverifiable
source bytes. Software fixture tests must retain synthetic evidence labels.

Use a bounded declared search over a statistical baseline and compact temporal
networks. Fit scaling on training only. Select architecture and checkpoint on
validation macro F1. Freeze that choice before final test predictions. Report
the fixed majority baseline, confusion matrix, per-class F1, artifact bytes,
parameter count, runtime and batch-one CPU latency scope.

Fold convolution batch normalization only after training, check validation
logit parity, then measure the actual inference result. A smaller file is not
proof of lower device RAM, a CPU timing is not an ESP32 timing, and an activity
score is not a pose score. Record exact code and artifact digests.

## Acceptance and deployment

Unit and integration tests cover partition isolation, provenance, malformed
data, real gradients, export reload and failed lifecycle rejection. A real
public data run must be reproduced separately from synthetic CI.

No candidate automatically replaces the published Hugging Face encoder or a
production model. Public benchmark performance does not establish transfer
to RuView field hardware. Deployment requires independent field validation,
confirmed label semantics, target runtime integration and explicit promotion.
Repeated inspection of the same final test invalidates an untouched test
claim for later tuning; retain an external confirmation set for that work.

## Sources

* https://github.com/AutoSmartGroup/ESP-Fi-HAR
* https://doi.org/10.1016/j.adhoc.2026.104192
* ADR-419 and ADR-299

## Development-only generalization extension

After the initial test was opened, further optimization must not treat it as
an untouched evaluation. A separate development loader validates the complete
archive-derived bundle but deserializes only training and validation arrays.
The original test is absent from the model search API.

Freeze four feature families and four ridge regularization strengths before
searching. Select using two inner folds entirely within the original training
partition: room 1 participants 1 through 4 to room 2 participants 5 and 6,
and the reverse room direction with the same participant separation. Fit every
scaler on the fitting side of its fold. Select by mean inner macro F1, then fit
all original training data. Compare once on existing validation against the
original frozen bundle and label this development evidence, not a new test.

Only a fresh independent confirmation dataset can authorize a stronger
generalization claim. Feature extraction, model latency and artifact integrity
must be measured together for the new artifact family.

## Field context research boundary

The RuField and WorldGraph exploration uses two explicitly separate paths.
The Python amplitude experiment may add temporal descriptors within a trial;
it cannot invent continuous chronology, verified empty room calibration,
positions or room topology. The standalone Rust integrity probe calls pinned
actual RuField and WorldGraph crates using authored synthetic events. Public
unlabeled replay is inspected for format only and never promoted to labeled
activity evidence or graph beliefs.

Root sensor evidence and derived graph beliefs remain distinct. The probe
bridge rejects derived input as new independent roots, duplicate event hashes,
stale observations and expired or untraceable beliefs. The bridge is not a
production graph cycle detector or an enrollment authority. Native WorldGraph
permits cycles, and native RuField simulation trust does not enforce wall clock
freshness. Neither behavior proves a production defect when used as documented.

The frozen temporal extension improved internal training folds but regressed
development accuracy and macro F1. Reject it and preserve the retained motion
ACF model. Evidence, executable commands and future field acceptance are in
[`espfi-field-context-research.md`](../espfi-field-context-research.md).
## Native resolution research boundary, 2026-10-13

The standalone resolution intake preserves the original manifest's admitted
training and development identities and duplicate quarantine. It skips final
test and excluded MATLAB payloads before decoding. Native amplitudes retain
lossless float64 values; pooled control values preserve original source dtype,
averaging order and float32 output. Every control tensor must match the parent
dataset exactly, and the refitted control state must match the retained ACF
artifact exactly.

The frozen 18-configuration experiment compares pooled190 with raw950 features
at matched sample-index spans and shorter spans. Selection uses only the two
original training folds. The original test was opened in the first pilot and
is not reopened here. The reused development partition remains adaptive
research evidence. No partition is relabeled as fresh confirmation.

See [protocol and results](../espfi-resolution-experiment.md) and the
[semantic label audit](../espfi-label-audit-20261013.md). Numeric label renaming
alone cannot improve accuracy or macro F1. Deployment requires independent
confirmation and target device measurement even if a research gate passes.

## Cross-carrier and compact runtime boundary, 2026-10-13

The cross-carrier experiment keeps the original 190-sample inputs and training
folds. Deterministic centered log-amplitude correlations add no physical
coordinates or field history. Its frozen 18-configuration search selects the
unchanged control; reject both new representations and retain the negative
result in [the experiment report](../espfi-spatial-experiment.md).

Compact storage is a separate numerical transformation of the retained ACF
classifier. Reconstruct its exact seeded random map and preserve the original
float64 feature extraction. Require exact unfused logits and class predictions,
bounded folded error, provenance and artifact integrity before retaining it.
Storage savings do not establish runtime memory savings or speed gains. See
[the compact protocol](../espfi-motion-compact.md).

The [independent data audit](../espfi-independent-data-20261013.md) distinguishes
dataset terms from code licenses, anonymous metadata access from signal intake,
and compatible labels from similar activity names. No external dataset is
silently admitted as independent confirmation for this seven-class model.

## Buffered inference boundary, 2026-10-13

The retained compact ACF artifact can be executed by a standalone NumPy and
safetensors runtime. Its loader validates bounded hashed bytes and the exact
seed recipe. A lock protects fixed workspace, returned outputs own their
storage, and the import path excludes training dependencies.

Preserve float64 floor, centering, normalization and difference arithmetic.
Direct contraction of the six autocorrelation products changes reduction order
only; require absolute logit error at most 1e-10 and identical predictions on
all admitted recordings. Keep the original artifact and reference runtime as
rollback. Three paired timing rounds must each improve median and p95 by at
least 10% before retention. Initial failure and final passing measurements are
both preserved in [the runtime report](../espfi-motion-runtime.md).

This accepts a measured shared CPU inference improvement, not a new activity
model, field accuracy result, target hardware guarantee or production release.

## Optional mixed classifier boundary, 2026-10-13

Preserve all feature calculations in float64. An explicitly selected subclass
may cast the folded classifier to float32 after validating and folding the
unchanged artifact in float64. Reject cast overflow, retain locking and output
ownership, and leave the existing float64 runtime unchanged.

This separate precision contract requires exact admitted-record predictions
and maximum absolute logit error 1e-4 against both retained references. Retain
the memory option only with at most half the model-array bytes, at most 65%
of combined model/workspace bytes and p95 no more than 5% worse in each of
three frozen paired rounds. Keep the separate 10% speed gate unchanged.

The [measured mixed report](../espfi-motion-mixed.md) passes the memory gate,
with 37.2239% fewer retained array bytes, but fails the speed gate. Preserve
the second-round median and third-round p95 regressions. No default switch,
training, new accuracy claim, final test reopening or deployment is implied.

## Explicit native reduction boundary, 2026-10-13

The reduced NumPy candidate preserves numerical parity but fails all declared
speed gates. Preserve that outcome and evaluate the native followup separately.
Keep amplitude flooring, logs and mean centering in the original NumPy float64
path. A fixed C ABI may compute only the remaining centered reductions.

Native loading is explicit trusted code execution. Require a separately supplied
library path and SHA-256, bounded regular-file reads, private exact-byte loading,
strict contiguous float64 buffers and fixed dimensions. Model metadata cannot
select code. Compile reviewed source explicitly without fast math or contraction;
bind source, compiler, flags, build receipt and library hashes to measurements.
Preserve the existing model loader, instance lock and owned output API.

Require <=1e-10 absolute logits, exact predictions on every admitted record,
passing sanitizer/boundary tests and >=10% median and p95 improvement in every
paired timing round. The [native report](../espfi-motion-native.md) passes these
research gates. Keep the NumPy default and rollback path; target hardware,
independent accuracy, mixed precision composition and deployment remain separate.
