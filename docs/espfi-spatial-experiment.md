# Cross-carrier correlation experiment

Protocol frozen on 2026-10-13 before execution. Research only, seven numeric
activity classes. This is a new representation experiment at the retained
190-sample temporal resolution, not a location or pose estimator.

## Hypothesis and specification

The retained model summarizes temporal autocorrelation and amplitude changes
independently for each of 52 subcarriers. Their joint fluctuations might add
activity information. This is a hypothesis; correlation can also encode room
conditions and worsen transfer. The experiment therefore retains the original
joint room and participant training folds and exact incumbent control.

Inputs are the existing admitted ESP-Fi training and development arrays and
retained motion artifact. No new capture intake or final test decoding is
needed. Outputs are a frozen selection receipt, bounded loadable research
artifact, source and model digests, paired timing and a retain or reject decision.

## Frozen architecture

| Family | Features | Definition |
| --- | ---: | --- |
| motion_acf | 468 | Exact retained autocorrelation and differences |
| cross_corr | 1,326 | Off-diagonal upper triangle of carrier correlations |
| motion_cross | 1,794 | Concatenation of both feature families |

For correlations, take log amplitude with floor 1e-6 and center each carrier
over the recording. Divide each centered carrier by the square root of its
energy, floored at 190e-12. Compute carrier dot products and retain the upper
triangle excluding the diagonal. Constant carriers have zero correlations.
This is a deterministic per-record transform with no fitted state or labels.
Process at most 32 records per correlation chunk. Carrier order follows the
source; it is not calibrated geometry, distance, position or field history.

Every family uses random Fourier features with 128 dimensions, seed 42,
gamma multipliers 0.1 and 1, and ridge penalties 0.01, 0.1 and 1. These 18
configurations are fixed before results. Fit scalers and classifier weights on
the fitting side of each original training fold only. Select by maximum minimum
fold macro F1, then mean F1, fewer stored scalars and declaration order.
The actual kernel gamma is the multiplier divided by the family's feature width,
after training-only feature standardization.

## Execution and integrity

1. Load only admitted training and development arrays through the existing
   provenance and quarantine checks. Require the incumbent manifest binding.
2. Compute training representations and the frozen fold search.
3. Persist selection before extracting development features. Refit the exact
   incumbent control and require identical learned tensors.
4. Fit the selected model, export bounded safetensors, and evaluate only this
   selection on reused development. Require exact reload predictions.
5. Run 1,000 rotating, alternating paired batch-one calls after 20 warmups.
   Measure both complete feature and classifier paths on the same inputs.
6. Complete the artifact only on success; errors mark it failed, output paths
   cannot be reused, and incomplete or corrupt artifacts cannot be loaded.

Correlation models have larger projections, so their bounded artifact reader
allows up to 4 MiB. It verifies the digest of the same bytes deserialized and
checks tensor names, shapes, float64 dtype, finiteness and positive scales.
No signal arrays, full capture manifests or weights belong in Git.

## Acceptance and limits

Retain a research candidate only with development macro F1 gain at least 0.03,
nonregressing accuracy, no newly zero class F1 and candidate p95 below 1 ms.
Promotion remains unauthorized. Shared CPU timings exclude acquisition,
decoding, temporal pooling and I/O, and do not establish target device latency.

The original final test was opened once in the first pilot and is not reopened
here. Development has been reused across rounds; within-round selection does
not make it fresh independent evidence. Label semantics remain unresolved.
This experiment cannot establish SOTA, named activity performance, pose accuracy,
field generalization or benefit from calibrated spatial context.

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_spatial \
  --manifest /private/espfi/prepared/manifest.json \
  --baseline /private/espfi/motion-002-worst \
  --output /private/espfi/spatial-001
```

The acceptance test for deployment remains a frozen comparison on independent
annotated sessions with the same accuracy gates and measured target latency.

## Measured result

The complete 18-configuration experiment finished successfully on existing
ruOS CPU compute. The unchanged control reproduced its learned tensors exactly.

| Best configuration per family | Weaker fold macro F1 | Mean fold macro F1 | Gamma multiplier | Ridge |
| --- | ---: | ---: | ---: | ---: |
| motion_acf | **0.332115** | **0.334030** | 0.1 | 0.1 |
| cross_corr | 0.115515 | 0.200194 | 1 | 0.01 |
| motion_cross | 0.239524 | 0.315203 | 0.1 | 0.01 |

The frozen selection returns the exact retained model. Development therefore
remains 39/70 correct, accuracy 55.71%, macro F1 0.499871. No development score
is computed for the rejected correlation configurations. No new model is
retained. These results reject this specific correlation representation and
grid; they do not prove that all joint-carrier methods are ineffective.

The selected artifact is 496,488 bytes with the same SHA256 as the incumbent:
`8d3c8b144663808159159f7af9d14b09f54ee78dc1f852c643fbd001a23d2fc5`.
Internal experiment duration is 3.210 seconds. Selected-wrapper median/p95 is
0.449270/0.623414 ms versus incumbent-wrapper 0.435755/0.605297 ms. Identical
weights and small wrapper differences do not support a model speed improvement.

The [sanitized receipt](validation/espfi-spatial-20261013.json) binds the full
measurement digest and source hashes. Four focused tests pass, covering the
mathematical reference, constant and anticorrelated carriers, batch independence,
development isolation, exact control, corrupt artifacts and failure lifecycle.
Independent source review found no blocking issue.
The integrated regression run passes all 59 public pipeline and 28 original
encoder tests on ruOS with verified exit code zero.

Decision: retain the existing ACF model and preserve this negative result.
The [independent data audit](espfi-independent-data-20261013.md) records the
access, label and license requirements for the next external benchmark.
