# Native resolution experiment

Protocol date: 2026-10-13. Research only. The existing ACF model remains the
control. This experiment measures seven numeric activity classes, not pose.

## Specification

Question: does retaining native sample resolution improve the activity
representation under the existing joint room and participant transfer protocol?
The source has 950 samples per carrier, while the retained model receives 190
means of five consecutive samples. Averaging could remove useful variation;
that is a hypothesis, not an established explanation for any class error.

The [label audit](espfi-label-audit-20261013.md) treats semantic naming as a
separate provenance problem. A consistent renaming cannot change accuracy or
macro F1. No named activity or fall detection claim follows from numeric scores.

Inputs are the exact pinned ESP-Fi archive, the original prepared manifest and
the retained ACF artifact. Outputs are a reproducible native intake, an explicit
model contract, source and model hashes, training fold diagnostics, one adaptive
development comparison, and a retain or reject research decision.

## Frozen architecture and selection

The complete grid is fixed before reading candidate development results:

| Family | Input samples | Autocorrelation lags | Absolute difference lags |
| --- | ---: | --- | --- |
| Pooled control | 190 | 1, 2, 4, 8, 16, 32 | 1, 4, 16 |
| Native, matched spans | 950 | 5, 10, 20, 40, 80, 160 | 5, 20, 80 |
| Native, short spans | 950 | 1, 2, 4, 8, 16, 32 | 1, 4, 16 |

All families have 468 features. Each uses gamma multipliers 0.1 and 1, ridge
penalties 0.01, 0.1 and 1, 128 random Fourier dimensions and seed 42. These 18
configurations use the original two training folds. Select highest minimum fold
macro F1, then mean fold F1, then declaration order. No new grid or followup
selection rule is introduced after results are observed.

The matched lags preserve spans in original sample indices. Source timestamps
are unavailable, so neither lag family implies calibrated seconds or Hz.
Log amplitude centering and global per-record RMS remain float64, with log and
RMS floors of 1e-6. The energy floor is sample count times 1e-12.

## Intake and trust boundary

Verify the complete archive size and digest before processing. Validate the
original manifest population, quarantine and split policy. Decode only its
admitted train and development MAT records; skip test and excluded payloads
before requesting their blocks or calling the MATLAB decoder. Require every
selected member digest to match the original manifest.

Retain native values losslessly in float64. Produce the control directly from
the original source dtype and averaging order, with its original final float32
cast. Verify exact pooled tensor equality against the existing prepared data.
Preserving raw float64 input uses ten times the tensor bytes of the float32
190-sample control; the temporal sample count itself grows fivefold.

The new bundle stores train and development only, plus the original parent
manifest for provenance. The loader verifies completion status, the parent
manifest hash and quarantine, fixed array headers and sizes, bounded hashed
bundle bytes, raw and pooled signal digests and exact raw-to-pooled conversion.
Raw samples, individual manifests and model weights remain outside Git.

## Execution and failure paths

1. Prepare the native bundle without changing original split membership.
2. Compute training features and fit each fold on its training rows only.
3. Write the selected configuration and all fold results before computing any
   development features.
4. Refit the pooled control and require exact equality with the retained model
   state. Intake separately verifies the pooled tensors against the parent data.
5. Fit the selected configuration on all admitted training rows, export it,
   evaluate development once, and require exact reload predictions.
6. Run 1,000 alternating paired batch-one calls after 20 warmups, recording
   median and p95. Each path receives its own already prepared input shape.
7. Mark the artifact complete only after all checks finish. Exceptions mark it
   failed; failed artifacts cannot be loaded and output directories cannot be
   reused silently.

The timings include validation, feature extraction and classification. They
exclude acquisition, source decoding, pooling and I/O. They are shared x86 CPU
measurements, not target device or sensor-to-decision guarantees. No overlapping
build or separate benchmark is launched by this experiment.

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_resolution_data \
  --archive /private/espfi/source.rar \
  --parent-manifest /private/espfi/prepared/manifest.json \
  --output /private/espfi/native-prepared

OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_resolution \
  --manifest /private/espfi/native-prepared/manifest.json \
  --baseline /private/espfi/motion-002-worst \
  --output /private/espfi/resolution-001
```

## Acceptance and confirmation limits

The research retention gate is development macro F1 at least 0.03 above the
retained ACF model, no accuracy regression, no newly zero class F1, and measured
p95 below 1 ms. A passed gate would retain a research candidate only.
`promotionAuthorized` remains false in every outcome.

The original final test was opened once in the first pilot. This round does
not reopen it or call it fresh. The 70 development recordings have been reused
throughout optimization; training-only selection within a round does not erase
adaptation between rounds. No available partition is silently reassigned as a
new confirmation set. Independent deployment acceptance still needs fresh
annotated recordings and target hardware latency. Those inputs are not supplied
by a source resolution change.

## Measured result

The pinned intake admits exactly 837 training and 70 development recordings.
All admitted source matrices are float64. Original pooled samples and the
refitted control state reproduce exactly. Intake took 31.518 seconds; the
experiment process took 12.019 seconds including startup and loading, with
6.865 seconds reported inside the model experiment.

| Best configuration per family | Weaker fold macro F1 | Mean fold macro F1 | Gamma | Ridge |
| --- | ---: | ---: | ---: | ---: |
| Pooled190 control | **0.332115** | **0.334030** | 0.1 | 0.1 |
| Raw950 matched spans | 0.300862 | 0.321984 | 0.1 | 0.1 |
| Raw950 short spans | 0.299106 | 0.304703 | 1 | 0.1 |

The declared selection chooses the pooled control. Both raw families lose on
the weaker fold and the mean fold score. Only the selected control is evaluated
on development; this round does not report development accuracy for rejected
raw configurations. No additional search follows the result.

Development remains 39/70 correct, accuracy 55.71%, macro F1 0.499871. Exported
weights are byte identical to the retained artifact, SHA256
`8d3c8b144663808159159f7af9d14b09f54ee78dc1f852c643fbd001a23d2fc5`,
with 496,488 bytes. The required 0.03 macro F1 gain is absent, so no new candidate
is retained. This result rejects these specific raw ACF representations; it
does not establish that native resolution cannot help another architecture.

Numeric activity 3 is already weak in the pooled control's two training folds:
F1 0.08 and 0.20, with 1/20 and 3/20 correct. The matched native family improves
those values to 0.090909 and 0.307692, but loses on overall transfer. This is a
diagnostic tradeoff, not a reason to override the frozen selection. The reused
development class still has zero F1; semantic renaming cannot change that.

Paired selected-wrapper median/p95 is 0.492675/0.701795 ms, versus the incumbent
wrapper's 0.480525/0.686287 ms. Both execute the same pooled model state. The
small difference reflects wrapper overhead and shared-host timing variation;
it is not a new model speed improvement. The raw candidates were not timed as
retained deployment candidates.

The [sanitized aggregate receipt](validation/espfi-resolution-20261013.json)
contains source, parent manifest, native manifest, model and full receipt
digests, class diagnostics and explicit test-access limits. Full capture
manifests and model artifacts remain outside the repository.

Validation on ruOS passes all 52 public pipeline tests and 28 original encoder
tests. The eight new focused tests cover exact pooling and control state,
source precision, admission before payload decoding, failure lifecycle,
artifact tampering, development isolation and exact chunked arithmetic.
Independent source review found no blocking issue.

Decision: keep the existing pooled ACF model. The immediate proposed acceptance
test is a frozen comparison on independently annotated new sessions, with at
least 0.03 absolute macro F1 gain, no newly zero F1 class and target device p95
within the agreed budget. This round supplies neither fresh confirmation nor
target hardware evidence and authorizes no deployment.
