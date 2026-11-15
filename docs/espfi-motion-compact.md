# Compact retained ACF model

Protocol frozen before execution, 2026-10-13. This changes the representation
and execution of the retained classifier, not its learned task or training data.
The exact original artifact remains the rollback model.

## Specification and architecture

The retained classifier stores a reproducible random Fourier projection and
phase alongside fitted weights. Reconstruct the projection from its exact
recipe instead of storing it: NumPy `default_rng(42)`, a float64 normal draw
of shape 468 by 128 with standard deviation sqrt(2 times 0.1 / 468), followed
by 128 uniform phases between zero and two pi. Export is allowed only when
both arrays exactly match the original model.

Store the 1,967 fitted float64 scalars: feature mean and scale, Fourier output
mean, classifier weights and bias. This removes reconstructable storage; it
does not reduce the runtime projection matrix or feature extraction memory.
Keep the existing float64 motion feature transform and its floors unchanged.

An unfused reconstructed runtime must reproduce original logits exactly.
The folded runtime computes the following once at load time:

* Projection divided by feature scale.
* Phase minus feature mean multiplied by the folded projection.
* Classifier bias minus output mean multiplied by classifier weights.

Folding changes floating point operation order, so it is judged separately
from exact projection reconstruction. The complete batch-one inference path
includes the original motion transform, validation and classification.

## Acceptance

Check all 837 admitted training and 70 development recordings against the
original artifact. Require exact predicted classes, exact unfused logits, and
folded maximum absolute logit error at most 1e-10. This is numerical equivalence
validation, not another accuracy search or independent confirmation result.
Do not deserialize the final test.

Run 1,000 rotating, alternating paired calls after 20 warmups. Both paths
exclude loading, random map generation, acquisition, pooling and I/O. Retain
the compact research artifact if its model file is below 10% of the original,
all parity checks pass, and measured p95 is at most 1.05 times the incumbent.
Call it a speed improvement only if median and p95 both improve by at least
10%. Storage improvement alone does not establish runtime memory or latency
improvement. No retraining, threshold search or development-based tuning occurs.

Verify bounded status, metadata and model bytes, model hashes, exact tensor
shape and dtype contracts, positive scales and known recipe. Reject failed
or corrupt artifacts and preserve failure status if export or verification
fails. Promotion remains unauthorized. Independent annotated data and target
hardware remain necessary for deployment claims.

## Measured result

The compact research artifact passes the declared retention gate. Its weights
file is 16,088 bytes versus 496,488 bytes, a 96.76% reduction. The three inference
artifact files total 17,245 bytes: weights 16,088, metadata 1,098 and status 59.
This total excludes code, dependencies and the optional measurement receipt.
The runtime reconstructs the large projection; no proportional RAM reduction
is claimed.

| Parity partition | Records | Unfused maximum logit error | Folded maximum logit error | Predictions |
| --- | ---: | ---: | ---: | --- |
| Training | 837 | 0 | 2.984e-15 | Identical |
| Reused development | 70 | 0 | 2.665e-15 | Identical |

Every admitted prediction matches. Development accuracy therefore remains
39/70, 55.71%, with macro F1 0.499871. This is a storage and numerical equivalence
result, not an accuracy improvement or fresh generalization evidence.

Paired median/p95 is 0.422395/0.594396 ms for compact folded inference versus
0.430370/0.623401 ms for the original. The candidate meets the allowed p95 budget
but misses the declared 10% gain threshold on both metrics. The single shared
CPU measurement does not establish a repeatable speed improvement. No further
timing search is used to change this decision.

Compact weights SHA256:
`87765a4da415465e3a4c5415b199da4593ef173e8fc0a2398dbe67faa454374d`.
The [sanitized receipt](validation/espfi-motion-compact-20261013.json) records
the original model, generated-map, implementation and full measurement digests.
Exact map regeneration is verified at load time; an incompatible RNG result
fails instead of silently changing the model. This run used NumPy 2.4.4.

All 59 public pipeline tests and 28 original encoder tests pass on ruOS.
The three new compact tests cover exact reconstruction, folded edge cases,
preserved original weights, artifact corruption, rehashed invalid state,
wrong seed recipe and failed export rejection. Independent source review found
no blocking issue.

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_motion_compact \
  --manifest /private/espfi/prepared/manifest.json \
  --baseline /private/espfi/motion-002-worst \
  --output /private/espfi/motion-compact-001
```

Decision: retain the compact research bundle alongside the exact original.
Next acceptance is integration and latency measurement on the intended target,
with the same parity gate. Accuracy still needs independently annotated data;
this storage optimization does not address the existing zero-F1 class.
