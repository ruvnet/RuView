# Public amplitude model optimization

This experiment trains a separate seven-class activity model from public
ESP-Fi HAR amplitudes. It does not replace the eight-feature Hugging Face
encoder, estimate pose or establish real hardware performance.

## Reproduce

Use Python 3.12, a fresh virtual environment and a data directory outside Git.
Install the CPU PyTorch wheel explicitly before the remaining dependencies.
Linux also needs the system libarchive shared library to read the RAR archive.

```sh
python -m pip install torch==2.11.0 --index-url https://download.pytorch.org/whl/cpu
python -m pip install -r scripts/requirements-espfi-training.txt
PYTHONPATH=scripts python -m unittest discover -s scripts/tests -p 'test_espfi*.py' -v

# This is a public 645 MB download, pinned to exact upstream bytes.
curl --fail --location --output /private/espfi/source.rar \
  https://media.githubusercontent.com/media/AutoSmartGroup/ESP-Fi-HAR/c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4/ESP-Fi_dataset.rar
PYTHONPATH=scripts python -m csi_training.espfi \
  --archive /private/espfi/source.rar --output /private/espfi/prepared
python scripts/train-espfi.py \
  --manifest /private/espfi/prepared/manifest.json \
  --output /private/espfi/run-001 --epochs 30 --seed 42 --max-seconds 600
```

Intake verifies the full archive SHA256 before decoding. It reads bounded MAT
members directly without extracting archive paths. It preserves 52 amplitude
channels and averages groups of five temporal samples into 190 time points.
Source timestamps are unavailable, so the preprocessing makes no calibrated
sampling frequency claim. Data are licensed CC BY 4.0; keep the recorded source
and Wen et al. citation with derived work.

## Frozen evaluation rules

Training uses rooms 1 and 2 with participants 1 through 6. Validation uses
room 3 with participant 7. Test uses room 4 with participant 8. Other
combinations are excluded to avoid sharing either room or participant across
partitions. Duplicate signals are quarantined, with counts in provenance.
The small final test measures one particular joint room/person transfer.

The source README and benchmark code disagree on activity names. Class labels
remain `activity_1` through `activity_7`; do not call any output a fall detector.

The declared search compares a statistics MLP with temporal networks of widths
16 and 32. Training statistics fit the scaler. Validation macro F1 selects the
checkpoint and architecture. The final test is opened after selection, with a
fixed training-majority control. Do not use that score to tune a later run and
continue calling it untouched.

## Artifacts and acceptance

The bundle contains safetensors, preprocessing, metrics and lifecycle status.
It records implementation/source hashes, per-class scores, confusion matrix,
parameter count and artifact bytes. Inference folding must preserve logits
within tolerance; exported reload is checked against the selected model.

```python
# PYTHONPATH=scripts; input shape N,52,190 after the same temporal pooling.
from csi_training.espfi_train import AmplitudeBundle
model = AmplitudeBundle('/private/espfi/run-001')
logits = model.predict(amplitudes)
```

CPU timings are batch one and exclude preprocessing and I/O. They are not
ESP32 timings or end-to-end response times. Failed and corrupted bundles are
rejected. `promotionAuthorized` remains false. Acceptance requires passing
contract tests, exact source verification, disjoint groups, successful bundle
reload and a recorded final evaluation. Production qualification additionally
requires resolved labels and independent field hardware data.

See [ADR-420](adr/ADR-420-public-amplitude-model-optimization.md).

## Measured public data pilot, October 12, 2026

The pinned archive contained two exact duplicate groups comprising four
recordings. All four were quarantined, leaving 837 training recordings and
70 each for validation and test under the declared joint split. Another
1,260 source recordings fall outside the split intersections, with one of
these also in the duplicate quarantine. No signal arrays are stored in Git.

| Candidate | Parameters before inference folding | Validation macro F1 |
| --- | ---: | ---: |
| Statistics MLP | 5,255 | 0.327905 |
| Temporal width 16 | 1,399 | 0.072092 |
| Temporal width 32 | 4,871 | 0.324024 |

The validation-selected statistics model achieved 22/70 correct (31.43%) and
0.288679 macro F1 on the final test. The fixed majority control achieved
10/70 (14.29%) and 0.035714 macro F1. Each test error changes accuracy by
1.43 percentage points. This is inadequate for deployment, and neither
room-wide nor population-wide performance can be inferred from one person
in one room.

The selected safetensors file is 21,316 bytes, SHA256
`42fad3cfb9bf532643c2bead4b4c5e51bd446c88e0c18686ad9f7df80d5cf781`.
The run used seed 42, a 30-epoch ceiling, early stopping patience 5, CPU one
thread, torch 2.11.0+cpu and NumPy 2.4.4. Its measured model-only latency
was p50 0.09645 ms and p95 0.15732 ms; preprocessing and I/O were excluded.
This timing is an observation on a shared x86_64 host, not a device SLA.
The three-candidate training loop took 5.75 seconds, excluding intake.

Decision: retain as a research candidate. The next accuracy experiment needs
domain-generalization work and an independent confirmation set. Do not tune
against this opened final test or replace the deployed eight-feature encoder.

## Inference optimization of the frozen candidate

The statistics model permits an algebraic optimization: channel scaling of
mean, standard deviation and absolute temporal differences can be folded into
the first linear layer. This avoids normalizing every amplitude value before
statistics extraction. No retraining or test-based selection is involved.

```sh
python scripts/train-espfi.py \
  --manifest /private/espfi/prepared/manifest.json \
  --optimize-bundle /private/espfi/run-001 \
  --output /private/espfi/run-001-folded
```

The transformed bundle records parent digests and validation-only parity;
it does not copy the parent's test score as a newly measured result. All 70
validation predictions remained identical, with maximum logit difference
0.000004054 and exact reload parity.

A paired alternating benchmark over 1,000 repetitions measured p50 bundle
prediction latency of 0.171623 ms before and 0.144312 ms after folding, a
15.9% reduction (1.189x speedup). p95 was 0.233717 and 0.204566 ms. This
includes input checks and normalization but excludes temporal pooling and I/O.
An earlier 300-pair measurement also improved p50 by 15.2%, but worsened p95
from 0.316924 to 0.349389 ms. Shared-host timing noise prevents a stable tail
latency claim. Both observations are retained; no hardware SLA is implied.

## Development-only domain generalization

The follow-up search uses a separate loader that does not deserialize test
arrays. The old final test has already been opened and cannot be reused as an
independent acceptance set. The existing validation set is also development
evidence, including when a new candidate outperforms the original model there.

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 python scripts/improve-espfi.py \
  --manifest /private/espfi/prepared/manifest.json \
  --baseline /private/espfi/run-001 \
  --output /private/espfi/generalization-001 --max-seconds 600
```

The frozen grid comprises raw statistics, relative temporal features, relative
spectral features and a combined family, each at ridge alpha 0.1, 1, 10 and 100.
Relative features log amplitudes, remove per-channel temporal means, and
normalize by each recording's motion RMS. Frequency bands are in normalized
sample units; no physical sampling rate is inferred.

Two folds use only original training data. Each fits on participants 1 through
4 in one room and evaluates participants 5 and 6 in the other room, then swaps
room direction. Scalers fit only fold training data. Mean fold macro F1 selects
the family and alpha, with feature count and declaration order breaking ties.
Only then is the model fit to the full original training partition and compared
with the frozen baseline on the existing 70-record development partition.

Every candidate score is retained, including losing candidates. Artifacts
include exact source and model digests, safe reload checks, latency scope and
explicit nonpromotion status. No claim of improved final-test performance
follows from this development search.

### Measured development result

The train-only search selected relative spectral features with ridge alpha 10.
The mean inner-fold macro F1 was 0.297443. Best scores by family were 0.200165
for raw statistics, 0.214085 for relative temporal features and 0.278602 for
the combined family. All 16 configurations are retained in the receipt.

On the existing 70-record development partition, macro F1 increased from
0.327905 to 0.366065, an absolute gain of 0.038160 and relative gain of 11.64%.
Accuracy remained 29/70 (41.43%). The three previously zero-F1 classes gained
nonzero scores, while classes 1, 2 and 3 regressed. This is a balanced-metric
tradeoff, not uniform improvement. No new final-test metric was computed.

The classifier has 3,283 fitted parameters and 936 additional preprocessing
scalars. Its first float64 safetensors artifact is 34,016 bytes, larger than
the original float32 MLP despite fewer model parameters. The 16-configuration
search took 0.81 seconds on this CPU, excluding intake. Feature extraction,
scaling and classification measured p50 0.129795 ms and p95 0.180376 ms over
200 calls; this unpaired timing alone is not proof of a speedup.

### Frozen float32 export

After statistical selection was complete, the fitted tensors and extracted
feature vector were converted to float32. Feature extraction itself retains
float64 arithmetic. The exported file is 17,140 bytes, 49.6% smaller than the
float64 candidate and 19.6% smaller than the original 21,316-byte MLP.
All 70 development predictions remain identical; maximum logit difference is
6.663e-7 and exported reload is exact. These are precision checks, not new
accuracy evidence.

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 python scripts/improve-espfi.py \
  --manifest /private/espfi/prepared/manifest.json \
  --baseline /private/espfi/run-001 \
  --optimized-baseline /private/espfi/run-001-folded \
  --optimize-bundle /private/espfi/generalization-001 \
  --output /private/espfi/generalization-001-float32
```

The paired benchmark rotates call order over 1,000 repetitions. It includes
complete public inference APIs, input/output checks and preprocessing, but
excludes acquisition, temporal pooling and I/O. PyTorch and BLAS use one thread.

| Bundle | p50 ms | p95 ms |
| --- | ---: | ---: |
| Original MLP | 0.196385 | 0.284381 |
| Previously folded MLP | 0.164547 | 0.256346 |
| Spectral ridge float64 | 0.178709 | 0.258502 |
| Spectral ridge float32 | 0.150447 | 0.224474 |

Median latency is 8.57% lower than the strongest previous folded baseline in
this experiment. Shared-host timing is not a device SLA. Load the candidate
with `csi_training.espfi_generalize.GeneralizationBundle`, which includes
the required feature transform, scaler and classifier. Never feed it the
old encoder's eight engineered features.

Acceptance remains a new untouched room and participant, resolved semantic
labels, and target-device measurement before any production promotion.

## Nonlinear development search

The next frozen grid contains 56 candidates on the same two training-only
folds. Relative spectral and combined features each use random Fourier maps
of 128 or 512 dimensions, gamma multipliers 0.1, 1 or 10 divided by the input
feature count, and ridge alpha 0.01, 0.1, 1 or 10. Projection seed is 42.
Six linear controls use alpha 1, 10 or 100; two nearest-centroid controls
complete the grid. Fold scalers see only their training records. Mean fold
macro F1 selects the candidate before development evaluation.

```sh
python scripts/improve-espfi-kernel.py \
  --manifest /private/espfi/prepared/manifest.json \
  --baseline /private/espfi/generalization-001-float32 \
  --output /private/espfi/kernel-001 --max-seconds 600
```

The CLI fixes BLAS and OpenMP to one thread before imports. The output records
the frozen search contract before fitting and the complete candidate selection
before external development inference. Load completed candidates using
`csi_training.espfi_kernel.KernelBundle` and the same pooled amplitude contract.

Random Fourier maps permit nonlinear decision boundaries without including
training support vectors in the exported model. Account for their stored
projection matrices separately from fitted classifier parameters. Compare
the selected candidate with the frozen float32 spectral ridge using paired
full-API latency and artifact bytes as well as development macro F1.

The same development partition has now informed multiple research rounds.
It cannot establish independent generalization, and the previously opened
test cannot recover that role by being renamed. See the primary-source
[benchmark evidence](espfi-sota-evidence.md) for the distinct source-only,
calibration and deployment protocols and the gate for a SOTA claim.

### Measured nonlinear result

Training-only selection chose combined features, 128 Fourier dimensions,
gamma multiplier 0.1 and ridge alpha 0.01. Mean inner-fold macro F1 was
0.306028, versus 0.297443 for the spectral ridge control. Development macro
F1 reached 0.383999, versus 0.366065 for the incumbent. Accuracy rose from
29/70 to 31/70. Activities 3 and 7 regressed to zero F1, so this candidate
does not improve every class.

The float64 export contains 903 classifier scalars, 60,032 fixed random-map
scalars and 1,064 preprocessing scalars. Its 496,488-byte artifact is 28.97
times the incumbent's 17,140 bytes. In the verified 500-pair full-API timing,
candidate p50/p95 were 0.168208/0.312287 ms, versus incumbent
0.134792/0.238876 ms. Median latency was 24.79% higher on this shared CPU.
No hardware or stable tail-latency claim follows.

The first real run was launched before a failed synthetic reload check was
inspected. That check exposed floating-point evaluation differences caused
by transposed, noncontiguous centroid weights. Canonical contiguous fitted
state fixed the software issue. After tests passed, the unchanged frozen
grid was replayed. Selection, model bytes and all 70 development logits
were identical to the original real run. Both source snapshots and receipts
are retained; the replay is verification, not independent accuracy evidence.

Decision: retain the small spectral model and archive the nonlinear model
as an accuracy experiment. No deployed model is replaced, no final-test
arrays were evaluated, and neither candidate establishes SOTA.


## Rejected blend and lossless seed encoding

A frozen five-weight blend combined the preceding spectral ridge and RFF
candidates using raw regression scores. Training-only mean fold macro F1
selected equal weights (0.323484), but development macro F1 fell to 0.348261,
with activities 3 and 7 still at zero F1. Accuracy was 30/70. The reproduced
controls matched the previous development scores exactly: spectral 0.366065
and RFF 0.383999. The blend is rejected; its complete receipt remains available.
No other development weights were evaluated or substituted after selection.

```sh
python scripts/improve-espfi-blend.py \\
  --manifest /private/espfi/prepared/manifest.json \\
  --output /private/espfi/blend-001
```

The separate seed encoding changes only the representation of the fixed RFF
projection. It stores 1,967 fitted and preprocessing scalars, then regenerates
60,032 random-map scalars from seed 42 at load. Both regenerated arrays must
match the exact export hashes before inference. Unexpected generator behavior,
tampered model bytes, invalid shapes and nonfinite state fail closed.

```python
from csi_training.espfi_kernel import KernelBundle
from csi_training.espfi_seeded import export_seeded, SeededKernelBundle
parent = KernelBundle('/private/espfi/kernel-001-verified')
export_seeded(parent.state, '/private/espfi/seeded-001')
model = SeededKernelBundle('/private/espfi/seeded-001')
logits = model.predict(amplitudes)
```

MEASURED on the ruOS CPU executor: weight bytes decreased from 496,488 to
16,088 (96.76%); required metadata adds 913 bytes. All 70 development logits
were exactly identical, maximum absolute error zero. Median loading time over
30 loads was 1.948925 ms. Runtime state remains 61,999 float64 scalars, so this
is a storage and distribution improvement, not RAM or sensing speed reduction.
The separate parity receipt binds parent model digest and dataset provenance.
The RFF candidate still has two zero-F1 activities and is not deployment ready.

This round used a different CPU host. In a paired 500-call comparison on that
host, float64 spectral, RFF and blend p50 latencies were 0.352430, 0.443975 and
0.804585 ms. These timings cannot be compared directly with prior-host timings
or with the prior float32 spectral deployment artifact.

The earlier security workflow fails on preexisting Rust dependency alerts in
unchanged `v2/Cargo.lock`; the Python checks pass. That remains a release gate,
alongside independent room/person confirmation. No SOTA or promotion claim.


## Measured float32 seeded inference

The optional float32 export converts the fitted tensors and regenerated map
at load. The RFF normalization constant now follows the state dtype, avoiding
an unintended NumPy float64 promotion. Existing float64 metadata without a
precision field remains supported. Unsupported precision values are rejected.

```python
from csi_training.espfi_seeded import export_seeded, SeededKernelBundle
baseline = SeededKernelBundle('/private/espfi/seeded-001')
export_seeded(baseline.state, '/private/espfi/seeded-002', precision='float32')
candidate = SeededKernelBundle('/private/espfi/seeded-002')
```

MEASURED: all 70 development class predictions are unchanged. Maximum absolute
logit error is 1.119643e-6, below the predeclared 1e-4 tolerance. Reload is exact.
Weights occupy 8,220 bytes plus 1,069 metadata bytes. Retained model tensors use
247,996 bytes instead of 495,992, a 50% reduction. Feature extraction and the
temporary map generated during loading remain float64, so total process memory
and peak loading memory are not halved. This precision conversion is not
bitwise lossless and establishes no new accuracy or final-test result.

A 1,000-pair benchmark alternated call order and cycled all 70 development
records. Float64 p50/p95 were 0.401590/0.568780 ms; float32 were
0.390610/0.556713 ms. The 2.73% median difference is too small for a reliable
speed claim on a shared host. Float32 median load time across 30 loads was
1.633650 ms. Input checks, feature extraction and classification are included;
acquisition, temporal pooling and I/O are excluded. NumPy 2.4.4, Python 3.12.3,
x86_64 and one BLAS thread were used.

Decision: retain the verified precision option for smaller model state and
storage. Keep float64 for numerical comparison. Generalization weaknesses,
independent room/person evaluation and the existing Rust security gate remain.

## Buffered inference runtime

`csi_training.espfi_runtime.BufferedKernelBundle` uses NumPy and safetensors
only. It does not import PyTorch, SciPy, the trainer, or the dataset loader.
The same bounded model bytes are hashed and deserialized. Legacy seeded
float64 and float32 bundles remain loadable; no new weight artifact is needed.

```sh
python -m pip install -r scripts/requirements-espfi-inference.txt
```

```python
from csi_training.espfi_runtime import BufferedKernelBundle
model = BufferedKernelBundle('/private/espfi/seeded-002')
scores = model.predict(amplitudes)  # finite nonnegative (N, 52, 190)
```

Keep the instance alive across windows. A locked workspace reuses centering,
power, pooling and projection buffers. Returned scores own their storage.
Calls on one instance serialize; use separate instances for concurrent workers.
Batch inputs are bounded to 4096 windows and processed one window at a time.
This targets live stream latency, not optimized large batch throughput.

Feature extraction remains float64, including the amplitude floor operation.
The 190 point FFT, bands and temporal bin lengths 48, 48, 47, 47 are unchanged.
The default folds feature normalization into the projection and output centering
into the classifier bias at load. `fuse=False` supplies the buffered control.
`SeededKernelBundle` retains the original calculation as the rollback reference.

MEASURED on the same ruOS x86_64 CPU, Python 3.12.3, NumPy 2.4.4 and one BLAS
thread: all 70 development predictions remain unchanged. Maximum absolute
logit error is 1.415611e-6, below the predeclared 1e-4 tolerance. Three separate
rounds each rotate call order across 1000 paired calls and cycle development
windows. Values below include validation, feature extraction and classification.

| Round | Reference p50 ms | Fused p50 ms | Reference p95 ms | Fused p95 ms |
| --- | ---: | ---: | ---: | ---: |
| 1 | 0.407790 | 0.330460 | 0.596976 | 0.472106 |
| 2 | 0.402875 | 0.327070 | 0.577802 | 0.470141 |
| 3 | 0.408875 | 0.327970 | 0.574514 | 0.478745 |

Median latency falls 18.82% to 19.79%; p95 falls 16.67% to 20.92%.
Every round passes the declared 10% median improvement and nonregressing p95
gate. This is shared CPU evidence, not a device SLA. Acquisition, temporal
pooling and I/O are excluded. Runtime retains 243,740 model tensor bytes plus
204,560 workspace bytes, excluding FFT temporaries and outputs. Buffer reuse
trades persistent scratch memory for fewer allocations; total process memory
was not measured. The stored 8,220 byte model remains unchanged.

A full float32 feature probe preserved development predictions but failed the
synthetic nearly constant signal test: maximum logit error 1.710676. It is
rejected and is only available inside the diagnostic benchmark. This catches
a cancellation failure that ordinary development parity would have missed.

Reproduce the acceptance test with training dependencies installed for loading
the authorized public bundle and comparing against the reference:

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_runtime_benchmark \
  --manifest /private/espfi/manifest.json \
  --model /private/espfi/seeded-002 \
  --output /private/espfi/runtime-verification.json
```

The output path must be new. Require `status=complete`, both prediction parity
and error tolerance, and `parity.fused.speed_gate_passed=true`. The receipt
binds model, metadata, feature, classifier, runtime and benchmark source hashes
to full intake provenance. Reviewed aggregate results are in
[`validation/espfi-runtime-20261012.json`](validation/espfi-runtime-20261012.json).

Focused tests cover precision and mixed amplitude floors, extreme and nearly
constant inputs, exact window boundaries, strided batches, concurrent calls,
owned outputs, rejected inputs, artifact tampering and absent training imports.
All 64 relevant tests pass on ruOS. Independent review found the amplitude
floor precision issue, verified the fix and returned NO BLOCK. No training,
new final test evaluation, model publication or production promotion occurred.
Development accuracy remains 31/70 with two classes at zero F1; fresh independent
rooms and participants remain necessary before a deployment claim.

## Source informed motion candidate

The [ecosystem and industry audit](espfi-ecosystem-research-20261012.md) records
two frozen selection policies over 24 source only representation candidates.
Mean fold selection chose local spectra and failed retention. A separately
declared adaptive followup selecting the worst training fold chose motion
autocorrelation and differences. Its development accuracy is 39/70 versus
31/70, macro F1 0.499871 versus 0.383999, and CPU median/p95 is
0.430715/0.614615 ms versus 0.313720/0.457686 ms in paired timing.

The new model is retained only for research. It is slower and its current
explicit float64 artifact is larger. One class remains at zero F1 and two
classes regress. The optimized compact incumbent remains available. The audit
contains source revisions, all retention checks, full limitations and commands
to reproduce both policies without opening final test arrays.

## RuField and WorldGraph context experiment

The [field context research report](espfi-field-context-research.md) separates
an actual Rust library integrity probe from a Python temporal model ablation.
The frozen 18-candidate experiment adds ordered means and signed trends within
individual recordings. Its selected candidate improves both training folds but
regresses development accuracy to 34/70 and macro F1 to 0.437458. It is rejected;
the retained motion ACF model stays at 39/70 and 0.499871 on these reused data.

Actual RuField and WorldGraph tests verify source ancestry and conservative
rejection policies. Public unlabeled replay is inspected for format only, with
no assumed empty room, activity labels or fabricated positions. Software
integrity and synthetic timing are not predictive accuracy evidence. The report
and aggregate receipts preserve that distinction and the negative model result.
## Native resolution and label audit, 2026-10-13

The [frozen resolution experiment](espfi-resolution-experiment.md) compares
950 native samples against the existing 190 pooled means. Both native ACF
families lose to the pooled control on minimum and mean training fold macro
F1. Selection returns the exact existing model and weight hash. Development
remains 39/70 correct with macro F1 0.499871; no new candidate is retained.

The [label audit](espfi-label-audit-20261013.md) confirms unresolved source
naming conflicts and a separate benchmark output-index mapping. Consistent
semantic renaming cannot improve numeric class accuracy or macro F1. The
original final test was opened in the first pilot and is not reopened here;
repeated development scores remain adaptive research evidence.

## Cross-carrier ablation and compact ACF runtime, 2026-10-13

The [cross-carrier experiment](espfi-spatial-experiment.md) adds normalized
carrier correlations alone and alongside motion features. Both lose to the
exact retained control under the frozen training-fold selection. No accuracy
improvement or replacement model is claimed.

The [compact runtime protocol](espfi-motion-compact.md) separately removes
reconstructable random-map storage and folds classifier affine operations while
retaining float64 feature extraction. It requires all admitted-record prediction
parity before accepting a storage or execution improvement.
The retained compact weights occupy 16,088 bytes versus 496,488 bytes, with
identical predictions on all 907 admitted records. The small measured latency
difference misses the 10% speed threshold, so only the storage gain is claimed.

The [independent dataset audit](espfi-independent-data-20261013.md) verifies
corrected CSI-Bench metadata and distinguishes restrictive data terms from
repository licenses. Neither CSI-Bench nor WiFiTAD has been ingested or scored
as new confirmation evidence in this round.

## Buffered ACF inference, 2026-10-13

The [standalone buffered runtime](espfi-motion-runtime.md) retains the compact
16,088-byte artifact and all 907 admitted predictions. Initial buffer reuse
alone fails the frozen speed gate. A documented followup replaces six ACF
multiply-and-sum reductions with direct contractions while preserving float64
features and the original logit tolerance. All three paired rounds pass:
median latency falls 13.91% to 14.32%, and p95 falls 11.21% to 14.69%.

The runtime requires only NumPy and safetensors. It retains 163,736 workspace
bytes alongside 487,480 model-array bytes; no total memory reduction is claimed.
All 93 relevant Python tests pass. No training, new accuracy evidence, final
test reopening or deployment occurs. Target device latency remains unmeasured.

## Optional mixed classifier inference, 2026-10-13

The [mixed precision runtime](espfi-motion-mixed.md) preserves float64 feature
extraction and casts only the folded classifier and its immediate workspace.
It reduces retained model plus workspace arrays from 651,216 to 408,808 bytes,
a 37.2239% reduction, while retaining all 907 admitted predicted classes.
Maximum logit error is 1.015411e-6 against both retained references.

All three rounds stay within the declared 5% p95 regression limit. None meets
the separate 10% speed gate, so this is retained only as an explicit memory
option. Float64 remains the default. All 99 relevant Python tests pass.
The unchanged weights, reused data and scoped array counts do not establish
new accuracy, target device speed or total process memory improvement.

## Native centered reductions, 2026-10-13

The [NumPy reduction experiment](espfi-motion-reduced.md) preserves all 907
admitted classes but misses the fixed speed gate in every round. Its source
and negative measurement remain available for reproduction.

The distinct [native followup](espfi-motion-native.md) keeps amplitude floor,
log and centering in NumPy float64, then computes only centered reductions in
an explicit trusted C kernel. All three rounds improve median by 29.01% to
30.57% and p95 by 25.78% to 29.81% versus buffered float64. Maximum logit error
is 4.773960e-15 and every admitted prediction is unchanged.

All 113 relevant Python tests and the ruOS ASan/UBSan probe pass. The optional
kernel adds a 15,736-byte compiled library, requires an explicit trusted hash
and does not change model weights, the default runtime or measured accuracy.
Its source, compiler recipe, library hash and both experimental receipts are
recorded. Combining native execution with mixed precision remains unmeasured.
