# CSI optimization source audit

Review date: 2026-10-12. Source facts, author claims and RuView measurements
are distinct. The task is seven class amplitude activity recognition, not
dense pose, vital signs or identity. No external score establishes a comparable
SOTA result. Existing confirmation data has already informed development.

## Mechanisms and decisions

| Mechanism | Verified source | Decision and rationale |
| --- | --- | --- |
| Motion autocorrelation | [AM-FM v1](https://arxiv.org/html/2602.11200v1), Origin Research, 2026-02-04 | Test a small independent feature ablation. The paper predicts normalized, uncentered autocorrelation of mean CSI power. Our centered log amplitude correlations and temporal differences are different operators, not an AM-FM reproduction. |
| Local temporal spectra | [RuView STFT](https://github.com/ruvnet/RuView/blob/5cfbb8d98d561bddb7c8918ae431407267517016/v2/crates/wifi-densepose-signal/src/spectrogram.rs) | Adapt the existing temporal spectrum mechanism to 32 sample Hann windows, fixed bands and temporal pooling. Native FFT plan reuse is implemented in RuView; its earlier timing cannot be transferred to this Python runtime. |
| Parameter free local fluctuation | [WiFiTAD paper](https://arxiv.org/html/2412.16233v2), 2025-01-26 revision; [AAAI publication](https://ojs.aaai.org/index.php/AAAI/article/view/32035), 2025-04-11; [Apache 2.0 code](https://github.com/AVC2-UESTC/WiFiTAD) | A later ablation can measure local maximum minus minimum over temporal scales. Temporal detection on another dataset is not this classification benchmark. Not implemented in this round. |
| Motion preserving pretraining | [WiSwiss](https://sites.pitt.edu/~weigao/publications/sensys26.pdf), SenSys 2026 | Defer external checkpoint integration. The inspected linked repository supplies no verified usable checkpoint and model license. A future source only encoder objective could preserve amplitude spectra, without calling them calibrated Doppler. |
| Native ESP32 capture | [rvcsi adapter](https://github.com/ruvnet/rvcsi/blob/3e1bdbdb3a6e2a35c3e5019e083135f7abea7188/crates/rvcsi-adapter-esp32/src/lib.rs), landed 2026-10-12 | Useful path to new labeled field recordings. The adapter handles files, serial and UDP, validates bounded I/Q records and preserves timestamps. Its presence does not validate a sensor, dataset or model. |
| Learned sparse projection | [RuVector predictor](https://github.com/ruvnet/RuVector/blob/31549f798997cbb16b1ec872ca30b9e690bbd3ff/crates/ruvector-sparse-inference/src/predictor/lowrank.rs) | Do not use its calibration as learned compression. The inspected method computes covariance but does not update P or Q before setting its calibration flag. |
| Empty room subtraction | [RuField replay](https://github.com/ruvnet/rufield/blob/b74bc035e4a3d4023665aa5479b41e938facd113/crates/rufield-adapters/src/csi_replay.rs) | Do not assume the beginning of an activity recording is empty. Explicit empty room captures are needed for that calibration contract. |
| Default per frame filtering | [rvcsi pipeline](https://github.com/ruvnet/rvcsi/blob/3e1bdbdb3a6e2a35c3e5019e083135f7abea7188/crates/rvcsi-dsp/src/pipeline.rs) | Its per frame subcarrier filtering is not temporal filtering. Do not silently swap axes when adapting signal operators. |

AM-FM author weights and commercially licensed implementation were not located.
The paper is CC BY NC ND 4.0; a paper license is not a model license. No third
party model, training corpus or paper implementation is imported here. The
experiment uses independently written mathematical feature operators on the
previously authorized, pinned CC BY 4.0 ESP-Fi source data.

RuFlo coordinates source review, implementation, independent review and evidence.
It is not an accuracy mechanism. New dependencies, retrieval stores and adaptive
state do not enter the inference path without a measured benefit.

## Frozen experiment

The candidate grid is declared before fitting: four representations, two RFF
gamma multipliers, three ridge penalties, 128 RFF dimensions and seed 42.

1. Existing combined temporal and whole recording spectral control, 468 values.
2. Six per carrier log amplitude correlation lags and three absolute temporal
   difference lags, 468 values. ACF normalization has an explicit energy floor.
3. Fixed carrier quantiles of those nine descriptors, 45 values. This discards
   carrier ordering deliberately and is not a learned channel selector.
4. Local spectra with four temporal groups and two frequency bands per carrier,
   plus carrier RMS, 468 values. Window starts and the final tail window are fixed.

Each candidate fits scalers and classifier only on its training fold. The folds
remain joint room and participant shifts within original training data. The
winner is persisted before accessing external development features or scores.
No final test arrays enter this API. All dimensions, lags, numerical floors and
selection policies are serialized in the experiment contract.

The retention gate is a development macro F1 gain of at least 0.03, no accuracy
regression, no new zero F1 classes and measured CPU p95 below 1 ms. It retains
only a research candidate; it cannot promote a production model.

## First result: mean fold selection

MEASURED: all three new representation families improve their best mean internal
CV score over the unchanged control. This is search evidence, not confirmation.

| Representation | Best mean CV macro F1 | Fold 1 | Fold 2 |
| --- | ---: | ---: | ---: |
| Existing combined control | 0.306028 | 0.304146 | 0.307910 |
| Motion ACF | 0.334030 | 0.332115 | 0.335944 |
| Carrier invariant motion | 0.354175 | 0.312178 | 0.396172 |
| Local spectra | 0.355599 | 0.266002 | 0.445197 |

Mean selection chooses local spectra, gamma multiplier 0.1 and alpha 0.01.
Development macro F1 changes from 0.383999 to 0.397205, but accuracy falls from
31/70 to 30/70. All seven class F1 values become nonzero. Paired median/p95
latency is 0.531470/0.737181 ms versus the incumbent 0.315420/0.456288 ms.
The proposed model fails both the declared F1 gain and accuracy gates. It is
rejected as a replacement. Its explicit float64 artifact is 496,488 bytes;
that storage comparison also includes a different encoding from the incumbent.

The average hides a worse first room fold. After observing this result, one
separate, explicitly adaptive followup was declared: maximize the minimum
training fold macro F1, then mean score, then smaller state. The same 24
candidates and retention thresholds are preserved. This is not an independent
confirmation run, and the first result remains in the evidence history.

## Adaptive followup: worst fold selection

MEASURED: the second policy selects motion ACF, gamma multiplier 0.1 and alpha
0.1. Both training fold macro F1 values improve, to 0.332115 and 0.335944 from
the fixed control's 0.304146 and 0.307910. This policy improves the weaker room
instead of accepting an average dominated by the stronger room.

Development accuracy is 39/70, or 55.71%, versus the incumbent's 31/70, or
44.29%. Macro F1 is 0.499871 versus 0.383999, a 30.18% relative increase and
0.115872 absolute gain. All four predeclared retention checks pass. Exact export
reload passes. This is retained as a research candidate alongside the incumbent;
no deployed sensing or Hugging Face model is replaced.

| Numeric class | Incumbent F1 | Motion ACF F1 |
| --- | ---: | ---: |
| activity_1 | 0.352941 | 0.166667 |
| activity_2 | 0.615385 | 0.645161 |
| activity_3 | 0 | 0 |
| activity_4 | 0.588235 | 0.842105 |
| activity_5 | 0.560000 | 0.533333 |
| activity_6 | 0.571429 | 0.645161 |
| activity_7 | 0 | 0.666667 |

These are not universal class improvements: activity_1 and activity_5 regress,
and activity_3 remains at zero. Label names remain unresolved upstream.

Across 1000 alternating paired calls, candidate median/p95 latency is
0.430715/0.614615 ms versus incumbent 0.313720/0.457686 ms. Median cost rises
37.29% for the development accuracy improvement. Acquisition and pooling are
excluded; the shared CPU is not target edge hardware. The candidate stores
61,999 float64 scalars in a 496,488 byte safetensors artifact, including its
explicit random map. The incumbent remains preferable when minimum latency
or compact storage is the main constraint. Compression of this new candidate
has not been measured.

Both policies use the same candidate grid; the followup is adaptive because
the first development outcome had already been observed. The development set
has been reused across earlier rounds as well. Neither result is an untouched
generalization estimate, and no external paper comparison is claimed.

All 40 public pipeline tests and 28 original encoder tests pass on ruOS.
Reviewed aggregate receipts are
[`espfi-motion-mean-20261012.json`](validation/espfi-motion-mean-20261012.json)
and [`espfi-motion-worst-20261012.json`](validation/espfi-motion-worst-20261012.json).
The first implementation is preserved at commit
`b7b76de1f3fdec309f69efb9c900829bdb6ba8bc`, matching its receipt source digest.
The second receipt binds the updated policy implementation and candidate bytes.

## Reproduce

Use the pinned training requirements and the existing prepared public bundle.
The baseline receipt must bind the same manifest and baseline weight digest.

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_motion \
  --manifest /private/espfi/manifest.json \
  --baseline /private/espfi/seeded-002 \
  --baseline-receipt /private/espfi/seeded-002/verification.json \
  --output /private/espfi/motion-mean --selection mean
```

Use a new output directory and `--selection worst` for the separately declared
robust comparison. Existing output directories cannot be overwritten. Partial
or failed runs cannot load as completed models. Model bytes are bounded, hashed
and deserialized from the same buffer. Tests verify feature mathematics, gain
and carrier invariants, provenance, exact export reload and that changing
development data cannot change the mean policy winner or model bytes.

Acceptance for deployment remains separate: fresh room and participant captures,
confirmed numeric label semantics, matched target hardware latency and explicit
promotion approval. No amount of optimizing these 70 repeatedly observed
development records establishes that result.
