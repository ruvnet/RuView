# Reduced-pass float64 motion inference

Protocol frozen on 2026-10-13 before real-data evaluation. This experiment
changes execution of the retained seven-class activity classifier. It does not
train a model, alter weights, reopen final test data or authorize deployment.

## Hypothesis

A synthetic diagnostic profile of 1,000 predictions on the ruOS x86_64 host
attributes 0.374 of 0.449 seconds to feature extraction, approximately 83%.
Profiling includes instrumentation and synthetic inputs, so these values are
diagnostic costs rather than deployment latency evidence.

The buffered runtime computes a squared window, reduces it once for global
RMS and again for per-carrier energy, then normalizes the entire centered
window before computing three mean absolute differences. The experiment
reuses the per-carrier energy reduction for global RMS and moves the common
normalization after the three difference means.

The optional `ReducedMotionBundle` inherits validated artifact loading,
bounded inputs, float64 classifier, locking and owned outputs from
`BufferedMotionBundle`. Override only feature extraction:

1. Preserve explicit float64 amplitude flooring, log and per-carrier centering.
2. Compute each carrier's squared energy with a direct dot reduction.
3. Derive global RMS from the sum of those energies before applying the energy
   floor. Preserve the existing RMS and energy floors.
4. Keep the six centered autocorrelation contractions and their normalization.
5. Compute absolute differences on unnormalized centered values, reduce each
   lag to its mean, then divide those means by the common global RMS.

These reorderings are algebraically equivalent, but floating point results
need not be bit identical. Preserve the existing absolute logit tolerance of
1e-10. Never substitute a lower precision contract for this experiment.
No native compiler or new runtime dependency is required.

## Frozen gates

Require identical predicted classes and at most 1e-10 absolute logit error
against both the retained buffered float64 and original explicit references.
Exercise all 837 admitted training and 70 reused development recordings at
batch sizes one and 32. The original final test was opened once in the first
pilot and is not reopened. This is numerical equivalence evidence on reused
data, not independent accuracy confirmation.

Measure three rounds of 1,000 rotating and alternating paired full prediction
calls, with 100 warmups for each runtime in each round. Use seeds 42, 43 and
44. The primary timing reference is `BufferedMotionBundle`. Both median and
p95 must improve by at least 10% in every round before retaining a speed
improvement. Include input checks, features and classifier; exclude loading,
acquisition, pooling and I/O. Do not repeat unchanged experiments to search
for favorable timing.

Freeze source and artifact hashes, reserve a new receipt exclusively, record
explicit failure, and reject any source or input change during measurement.
Report retained model and workspace arrays separately from process and peak
memory. Scratch retains transformed values until reuse; it is not erasure.

Test zero and mixed-floor inputs, very large amplitudes, nearly constant
inputs, strided arrays, integer inputs, concurrency, output ownership and
failure recovery. A third-round timing regression must reject the runtime
even when earlier rounds pass. Preserve the previous runtime for rollback.

Decision rule: retain the optional runtime only if numerical and every-round
speed gates pass. Next acceptance is the same comparison on the intended
device and representative admitted inputs. The key risk is rounding near
constant signals, so edge tests and real-record parity are both mandatory.

## Measured outcome

Reject this speed candidate. All 907 predictions match both references at
batch sizes one and 32, with maximum absolute logit error 3.108625e-15.
However, every round fails the frozen 10% median and p95 improvement gate.
Do not switch the default or repeat this unchanged configuration for a more
favorable result.

| Round | Reduced p50 ms | Buffered p50 ms | Reduced p95 ms | Buffered p95 ms |
|---|---:|---:|---:|---:|
| 1 | 0.349145 | 0.363140 | 0.503569 | 0.527381 |
| 2 | 0.363535 | 0.387440 | 0.528979 | 0.553136 |
| 3 | 0.338075 | 0.361850 | 0.500688 | 0.520044 |

Both implementations retain 487,480 model-array bytes and 163,736 workspace
bytes. Six focused tests pass with verified exit zero. This experiment does
not improve model accuracy or establish a retained speed optimization.

The [reviewed aggregate](validation/espfi-motion-reduced-20261013.json) records
all rounds and source/input hashes. Full receipt SHA-256:
`e6d80614b66a599093e1e676f3094dca39a06d4d52a22a609c596c41b29fcf9b`.
The full receipt and capture provenance remain outside Git.

Reproduce on admitted local artifacts with a new output path:

```bash
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
  python3 -m csi_training.espfi_motion_reduced_benchmark \
  --manifest /path/to/espfi-prepared/manifest.json \
  --model /path/to/espfi-motion-compact-001 \
  --baseline /path/to/espfi-motion-002-worst \
  --output /path/to/new-reduced-receipt.json
```

A distinct native followup keeps NumPy floor/log/centering unchanged and
moves only the remaining reductions to a compiled kernel. Its protocol and
result are recorded separately; no threshold is relaxed to accommodate this
failed experiment.
