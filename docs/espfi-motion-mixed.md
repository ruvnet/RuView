# Optional mixed classifier inference

Protocol frozen on 2026-10-13. This is an opt-in precision experiment on the
retained activity classifier. The existing float64 runtime and its 1e-10 logit
contract remain unchanged. No model training, feature change, final test
reopening or default replacement is authorized by this experiment.

## Hypothesis and architecture

The buffered runtime retains 487,480 bytes of folded model arrays and 163,736
bytes of workspace. Classifier arrays may tolerate float32 even though the
feature calculations require float64 near constant signals. Earlier feature
precision experiments failed that numerical boundary, so do not cast amplitudes,
logs, centering, energies, normalization or motion descriptors to float32.

The optional class inherits the existing artifact validation and float64
feature extractor. Reconstruct and fold the model in float64, then cast only
projection, phase, weights and bias to float32. Reject nonfinite cast results.
Cast the 468 finished feature values immediately before classifier execution.
Use float32 hidden values, scores and normalization scalar, and return owned
float32 logits. Keep the instance lock around all shared workspace operations.

The model artifact remains unchanged. No additional weight export or dependency
is needed. Constructor temporaries can still be float64; retained array counts
do not establish peak loading memory or total process RAM. Transformed samples
remain in scratch until overwritten, so buffer reuse is not memory erasure.

## Frozen acceptance and failure paths

The separate mixed precision contract permits maximum absolute logit error
1e-4 against both retained buffered float64 and original explicit references.
Require identical predicted classes on all 837 admitted training and 70 reused
development records, tested with batches of one and 32. Report reference class
margins as diagnostics. Data parity is not a guarantee for all future inputs.

Retain the optional memory candidate only if all of these checks pass:

1. All admitted prediction and mixed logit checks pass.
2. Retained model arrays occupy at most 50% of the float64 reference.
3. Model plus workspace arrays occupy at most 65% of the reference total.
4. Candidate p95 is at most 1.05 times reference p95 in every timing round.

Measure three rounds of 1,000 rotating and alternating paired batch-one calls,
with 100 warmups per runtime per round. A separate speed gain flag requires at
least 10% lower median and p95 in every round. A memory result is not a speed
result. Include input validation, feature extraction and classifier execution;
exclude loading, random map generation, acquisition, pooling and I/O.

Validate numerical edge cases, positive and negative float32 cast overflow,
mixed amplitude floors, strides, concurrency and returned array ownership.
Fresh process import must not load Torch, SciPy or training modules. Reuse the
bounded same-byte artifact loader and retain its corruption/status checks.
Reject changed input or source hashes during measurement. Create receipts
exclusively and record failed execution instead of overwriting past results.

The old test was opened once in the first pilot and is not reopened here.
Development has been reused and only supplies numerical equivalence evidence
in this experiment. No activity accuracy, named label, SOTA or target device
claim follows from lower precision.

Decision gate: the float64 runtime remains the default regardless of outcome.
Retain mixed precision only as an explicitly selected research option when the
declared gates pass. Next acceptance is the same comparison on the intended
device and its actual input distribution, with float64 available for rollback.

## Measured outcome, 2026-10-13

Retain `MixedPrecisionMotionBundle` as an optional memory-saving research
runtime. Every memory, numerical and latency-regression gate passes. The
separate 10% speed gate fails in all three rounds, so no new speed gain is
claimed. `BufferedMotionBundle` remains the float64 default.

| Retained arrays | Float64 bytes | Mixed bytes | Reduction |
|---|---:|---:|---:|
| Folded model | 487,480 | 243,740 | 50.00% |
| Workspace | 163,736 | 165,068 | Increases 1,332 bytes |
| Model plus workspace | 651,216 | 408,808 | 37.2239% |

The on-disk compact weights remain exactly 16,088 bytes with SHA-256
`87765a4da415465e3a4c5415b199da4593ef173e8fc0a2398dbe67faa454374d`.
The memory counts include retained NumPy array data only, excluding Python
object overhead, dependencies, input/output arrays and constructor temporaries.

All 837 training and 70 development recordings retain identical predicted
classes against both float64 references at batch sizes one and 32. Maximum
absolute logit error is 1.015411e-6 on training and 7.940612e-7 on development,
below 1e-4. The smallest original half-margin is 0.000145302 on training and
0.002173962 on development. Those margins are diagnostics for these records,
not guarantees for unseen inputs near a decision boundary.

| Round | Mixed p50 ms | Float64 p50 ms | Mixed p95 ms | Float64 p95 ms |
|---|---:|---:|---:|---:|
| 1 | 0.451410 | 0.464445 | 0.607638 | 0.630166 |
| 2 | 0.441040 | 0.440140 | 0.614359 | 0.625946 |
| 3 | 0.486470 | 0.496145 | 0.672452 | 0.669549 |

The third round has a 0.43% p95 regression and the second has a 0.20% median
regression. Preserve both observations. Do not rerun unchanged configurations
until a favorable speed result appears. These are full batch-one predict
calls on the shared ruOS x86_64 host with Python 3.12.3, NumPy 2.4.4,
`OPENBLAS_NUM_THREADS=1` and `OMP_NUM_THREADS=1`.

The [reviewed aggregate receipt](validation/espfi-motion-mixed-20261013.json)
binds input and source hashes, frozen gates, all rounds and test counts.
The full private receipt has SHA-256
`4569955a2090d2e848038f99808328cc0c95bf777d99853983e625a02119aced`.
It remains outside Git with capture provenance. No raw CSI, per-record data
or model weights are included in this change.

All 99 relevant Python tests pass on ruOS: 71 ESPFi tests and 28 original
training tests. The six focused mixed-runtime tests include positive and
negative cast overflow, numerical boundaries, concurrent calls, ownership,
artifact corruption, failed provenance, exclusive receipts and a simulated
third-round regression that must fail the every-round gate. A fresh process
confirms runtime imports do not load Torch, SciPy or training modules.

## Explicit use and reproduction

The caller must explicitly select mixed precision. The compact model directory
is the same validated artifact consumed by the float64 runtime.

```python
from csi_training.espfi_motion_mixed import MixedPrecisionMotionBundle

runtime = MixedPrecisionMotionBundle("/path/to/espfi-motion-compact-001")
logits = runtime.predict(amplitudes)  # [N, 52, 190] input, owned [N, 7] float32 output
```

For admitted local data and artifacts, write a fresh receipt path:

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
  python3 -m csi_training.espfi_motion_mixed_benchmark \
  --manifest /path/to/espfi-prepared/manifest.json \
  --model /path/to/espfi-motion-compact-001 \
  --baseline /path/to/espfi-motion-002-worst \
  --output /path/to/new-mixed-receipt.json

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
  python3 -m unittest discover -s scripts/tests -p 'test_espfi*.py' -q
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
  python3 -m unittest discover -s scripts/tests -p 'test_csi_training*.py' -q
```

Decision: retain the explicit option for its measured array-memory reduction.
Next action: measure on the intended device and representative admitted inputs.
Key risk: float32 rounding may change classes for unseen low-margin inputs.
Acceptance: preserve exact classes, stay below 1e-4 absolute logit error and
within 5% reference p95 in every declared round before adopting it there.
This round does not train, replace the Hugging Face model or improve measured
activity accuracy, which remains 39/70 on reused development data.
