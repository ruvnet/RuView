# Buffered ACF inference

Protocol frozen on 2026-10-13. Optimize execution of the retained compact
activity classifier without changing its weights, features, training or labels.
The compact reference and original explicit model remain rollback paths.

## Evidence and hypothesis

A diagnostic cProfile run over 1,000 real development windows attributed
0.490 of 0.533 seconds inside prediction to motion feature extraction, about
92%. Profiling adds overhead and those numbers are not latency benchmarks.
The classifier is a small part of this path. Reusing feature buffers and
avoiding temporary arrays is therefore the selected implementation experiment.

The existing compact export already removes reconstructable map storage.
Further file compression is not the objective here. Float64 feature arithmetic
remains mandatory because earlier float32 feature experiments failed nearly
constant signal tests despite matching ordinary development predictions.

## Architecture and boundaries

Use a standalone NumPy and safetensors runtime that loads the existing compact
artifact. Verify bounded status, metadata and model bytes; hash the same bytes
that are deserialized. Require the frozen candidate and exact random map recipe,
float64 tensor shapes, finite state and positive feature scales. Regenerated
map hashes must match before affine folding. Do not rewrite the artifact.

Reuse a fixed per-instance workspace for 52 by 190 amplitude windows, centered
log samples, products, carrier means and energies, 468 features and 128 hidden
values. Preserve the six autocorrelation and three absolute difference lags.
Compute the amplitude floor in float64 explicitly, including float32 inputs.
Keep the original global RMS floor 1e-6 and carrier energy floor 190e-12.

One lock protects each instance's workspace. Returned logits own their memory.
Use a separate instance per concurrent stream when parallel execution is needed.
Bound input batches at 4,096 windows and validate shape, dtype, nonnegativity and
finiteness. The inference import path must not load Torch, SciPy or training code.

## Frozen acceptance

1. Compare all 837 training and 70 reused development recordings against both
   the compact folded reference and the original explicit model. Require exact
   predicted classes and maximum absolute logit error at most 1e-10.
2. Verify mathematical edge cases, mixed floor precision, noncontiguous inputs,
   batch boundaries, output ownership, concurrent calls, corrupted artifacts
   and failed status rejection. Do not open final test arrays.
3. Run three rounds of 1,000 rotating alternating paired batch-one predictions
   after 100 warmups. Compare the actual public prediction APIs against the
   compact folded reference on identical inputs and one CPU thread.
4. Retain the runtime only if median and p95 are each at least 10% lower in
   every round and all correctness checks pass. Preserve all measured rounds,
   including failures; no timing cherry picking or lower post hoc threshold.

Timing includes input validation, feature extraction and classification. It
excludes loading, map regeneration, acquisition, pooling and I/O. Report retained
model and workspace bytes separately from total process memory. The original
test was opened once during the first pilot and is not reopened here. Existing
development data establishes numerical parity, not new accuracy evidence.

The biggest remaining deployment uncertainty is latency on the intended device.
Acceptance there requires the same prediction checks and measured device p50/p95;
shared x86 timings cannot establish ESP32, ARM or sensor-to-decision latency.

## Initial buffered result and causal followup

The first implementation preserves feature reduction order but fails the speed
gate in all three rounds. Median latency is 0.427865, 0.421035 and 0.439405 ms,
versus reference 0.434730, 0.417775 and 0.440610 ms. All 907 predictions match;
maximum logit error against either reference is 3.165e-15. Buffer reuse alone
does not establish a speed benefit. The [initial receipt](validation/espfi-motion-runtime-initial-20261013.json)
and source commit `46895cc603d500ba510356565338420159c2b2e6` preserve this result.

A subsequent isolated diagnostic compares one autocorrelation reduction over
5,000 calls on an admitted window. Multiplication followed by sum takes
0.021726 ms per call; direct `einsum` contraction takes 0.008122 ms. The raw
dot-product maximum difference is 1.11e-16. This is a kernel diagnostic on one
window, not a complete runtime claim.

The second implementation changes only the six autocorrelation reductions to
direct contractions. Feature reduction order can differ, so synthetic feature
parity uses absolute tolerance 1e-12. The original logit tolerance 1e-10, exact
predicted classes, and every-round 10% speed gates remain unchanged. Global RMS,
energy normalization, floors, differences, weights and model artifacts stay
fixed. This is a documented causal followup, not an unchanged timing retry.

## Measured retained result

The direct-contraction implementation passes all three frozen speed gates.
All 907 admitted recordings have identical class predictions against both the
compact folded and original explicit reference, tested at batch sizes one and
32. Maximum absolute logit error is 3.442e-15, below the fixed 1e-10 limit.
Weights remain byte unchanged at 16,088 bytes. No retraining or accuracy gain
is involved; development remains 39/70 correct with macro F1 0.499871.

| Round | Reference median, ms | Buffered median, ms | Median reduction | Reference p95, ms | Buffered p95, ms | p95 reduction |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 0.552715 | 0.475830 | 13.91% | 0.734181 | 0.638555 | 13.02% |
| 2 | 0.496870 | 0.427045 | 14.05% | 0.707697 | 0.603767 | 14.69% |
| 3 | 0.483825 | 0.414530 | 14.32% | 0.684120 | 0.607423 | 11.21% |

These are paired full prediction API measurements on a shared x86 CPU with
NumPy 2.4.4 and one configured BLAS thread. Absolute host latency varies between
experiments, so compare the paired columns within each round, not an older
unpaired baseline. No separate build or benchmark was launched concurrently
by this experiment.

Both runtimes retain 487,480 bytes of folded model arrays. The buffered runtime
adds 163,736 bytes of fixed workspace, totaling 651,216 bytes of those retained
arrays. This excludes caller input, owned output, transient allocations,
interpreter and dependency memory. Buffer reuse trades retained workspace for
reduced temporary work; no total RAM reduction is claimed.

All 65 public pipeline and 28 original encoder tests pass on ruOS. The six
new tests cover feature and logit parity, mixed float32 floor neighbors,
extreme and nearly constant signals, strides, ownership, concurrent calls,
invalid inputs, bounded corrupt artifacts, dependency isolation, provenance,
exclusive receipts and an intentionally failing third timing round.
The independent source review found no blocking issue.

The [final aggregate receipt](validation/espfi-motion-runtime-20261013.json)
binds input and implementation digests to all measured rounds. The benchmark
verifies source and input hashes before and after execution. It creates a new
receipt exclusively and records failure without overwriting prior results.

## Use and reproduce

The inference module uses the existing compact artifact and the pinned
`requirements-espfi-inference.txt` dependencies. No Torch, SciPy or training
module is imported by this path, as verified in a fresh subprocess.

```python
from csi_training.espfi_motion_runtime import BufferedMotionBundle

model = BufferedMotionBundle("/private/espfi/motion-compact-001")
# amplitudes has shape (batch, 52, 190), finite and nonnegative.
scores = model.predict(amplitudes)
predicted_numeric_activity = scores.argmax(axis=1) + 1
```

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_motion_runtime_benchmark \
  --manifest /private/espfi/prepared/manifest.json \
  --model /private/espfi/motion-compact-001 \
  --baseline /private/espfi/motion-002-worst \
  --output /private/espfi/motion-runtime-verification.json
```

Decision: retain the faster research runtime with the unchanged model and
explicit reference fallback. Next action is the same benchmark on the intended
target. Acceptance requires identical predictions, logit error at most 1e-10,
and at least 10% lower median and p95 in every round. Production promotion
remains unauthorized, and the existing zero-F1 activity class remains unresolved.
