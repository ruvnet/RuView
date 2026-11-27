# RuField and WorldGraph sample data experiment

Date: 2026-10-12. This is a bounded research result, not model promotion.
The existing motion ACF candidate remains the retained model.

## Decision and evidence boundary

RuField supplies typed sensor events, trust verification and temporal fusion.
WorldGraph supplies a typed graph of environmental beliefs and their evidence.
They do not supply missing labels, independent room calibration or physical
track positions for an amplitude activity dataset. There is no measured
classification improvement from attaching their names or APIs to a pipeline.

We therefore separate two executable questions:

1. Can ordered summaries inside each existing recording improve the frozen
   activity model? Measure on the authorized public ESP-Fi bundle.
2. Can actual RuField and WorldGraph libraries maintain source ancestry and
   reject duplicate, stale or circular evidence? Test deterministic synthetic
   events and characterize the real replay sample boundary separately.

The first question is a temporal representation ablation inspired by the
context discussion. It does not use graph features or call the Rust libraries.
The second question calls actual pinned libraries, but cannot establish field
accuracy. These outputs must never be combined into an accuracy claim.

## Sample audit

ESP-Fi contains labeled individual trials. The prepared input is 52 amplitude
channels over 190 pooled samples. It has no validated wall clock sampling rate,
continuous sequence across trials, independently annotated empty room interval,
sensor position or room geometry. Numeric activity names remain unresolved.
The existing 837 training and 70 development recordings are reused; final test
arrays are never opened. Sorting trial filenames is not evidence of chronology.

At [RuField revision 99556728](https://github.com/ruvnet/rufield/tree/995567283606b86594439d6594387ff551a21ede),
the public replay fixtures have the following structural properties, verified
without exporting signal values:

| Fixture | Complete JSON frames | Time span | Structural status |
| --- | ---: | ---: | --- |
| `real_csi_small.jsonl` | 3 | approximately 0.201 s | increasing timestamps |
| `real_csi_medium.jsonl` | 199 | approximately 13.415 s | increasing complete frames, truncated JSON at line 200 |

Git blob hashes are `fb2b2aa1ccd65306e4eccf5b30309c071b6da3ea` and
`81b8097a54940319ff4b6df290c34628d4418772`, respectively. The source path is
`crates/rufield-adapters/tests/fixtures/`. The [adapter](https://github.com/ruvnet/rufield/blob/995567283606b86594439d6594387ff551a21ede/crates/rufield-adapters/src/csi_replay.rs)
and [replay tests](https://github.com/ruvnet/rufield/blob/995567283606b86594439d6594387ff551a21ede/crates/rufield-adapters/tests/csi_replay_fusion.rs)
describe unlabeled replay. Its first frames are a statistical reference, not
independently verified emptiness. The code license is MIT; no separate dataset
permission or consent record was found in the inspected sources. These samples
are not imported into model training or committed here.

The [RuView BatVu fixture test](https://github.com/ruvnet/RuView/blob/5cfbb8d98d561bddb7c8918ae431407267517016/v2/crates/wifi-densepose-rufield/tests/ultrasonic_gates.rs)
identifies its 72 pings as simulator output. It cannot provide an independent
physical sensing confirmation set. The inspected WorldGraph sources likewise
did not supply independently annotated continuous sensing data for this task.

## Frozen model experiment

The current ACF representation is invariant under reversing a recording. This
is an exact feature property, not proof that direction loss explains a class
failure. Test three families: unchanged ACF with 468 features; ACF plus three
ordered per-carrier means with 624; and those features plus a signed linear
trend with 676. The three temporal bins have 63, 64 and 63 samples. Reversal
swaps the first and last bin and negates the trend. All log amplitude centering
and global RMS arithmetic remain float64.

Each family uses gamma multipliers 0.1 and 1, ridge penalties 0.01, 0.1 and 1,
128 RFF dimensions and seed 42. The 18 candidates were fixed before the run.
Selection maximizes the minimum of the two original training fold macro F1
scores, then mean score, then smaller state, with declaration order for ties.
Scalers and models fit only their respective training rows. No room label is an
input feature. The complete selection is saved before development extraction.

The unchanged ACF control must reproduce every learned tensor exactly. The
retention gate requires development macro F1 at least 0.03 above the retained
ACF candidate, no accuracy regression, no newly zero class F1, and measured p95
under 1 ms. Repeatedly viewed development data remains adaptive research data.

MEASURED: `ordered_trend`, gamma 0.1, alpha 0.1 wins the training selection.

| Metric | Retained ACF | Ordered candidate |
| --- | ---: | ---: |
| Training fold 1 macro F1 | 0.332115 | 0.347404 |
| Training fold 2 macro F1 | 0.335944 | 0.364028 |
| Development correct | 39/70 | 34/70 |
| Development accuracy | 55.71% | 48.57% |
| Development macro F1 | 0.499871 | 0.437458 |
| Explicit float64 artifact bytes | 496488 | 712816 |

The candidate is rejected. Activity 3 still has zero F1; its confusion row is
unchanged. Activity 1 improves only from one to two correct predictions, while
other classes regress. Better internal fold scores did not transfer to this
development room and participant. No second accuracy search follows this result.

Exact baseline reconstruction and exact exported reload pass. The paired
1000-call run observed candidate median/p95 0.782435/4.814087 ms versus
0.512245/0.861634 ms for the ACF control. An independent Rust compilation was
running on the shared machine, so this timing is recorded as contended execution,
not a stable runtime regression estimate. The accuracy failures alone reject
the candidate. Acquisition, pooling and I/O are excluded from these timings.

The [aggregate receipt](validation/espfi-context-20261012.json) binds source,
model, baseline and manifest hashes. Full experiment records remain with the
executor; no individual input recordings or model weights enter the repository.

```sh
OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 PYTHONPATH=scripts \
python -m csi_training.espfi_context \
  --manifest /private/espfi/prepared/manifest.json \
  --baseline /private/espfi/motion-002-worst \
  --output /private/espfi/context-001
```

## Actual library integration

[`scripts/field-context-probe`](../scripts/field-context-probe/README.md) is a
standalone Rust executable with exact Git dependency revisions and a lockfile.
It creates signed synthetic RuField events, runs actual RuField trust and
fusion, creates actual WorldGraph semantic beliefs with `DerivedFrom` edges,
and serializes and restores the graph.

The additional bridge admits only fresh unique root evidence. It refuses derived
events as new roots and refuses missing source handles, expired beliefs and
duplicate belief publication. These are deliberately conservative application
rules; they are not native WorldGraph cycle detection. WorldGraph permits a
self edge, which is demonstrated in a separate boundary test. RuField simulation
trust intentionally skips wall clock freshness, whereas its production policy
rejects synthetic evidence. No synthetic key is enrolled for production use.

The bridge never turns a model score into a calibrated probability. No person
track, physical position or room adjacency is fabricated. In this closed
synthetic probe, every declared derived input is rejected as independent root
evidence. The caller supplies that ancestry declaration; it is not a complete
cryptographic proof of independence. A production bridge would need verified
lineage, including correlation between observations from the same original
signal, before claiming to prevent all circular confidence amplification.

The optional replay inspector uses a bounded regular UTF-8 file, a 1 MiB input
limit and at most 10,000 nonempty lines. It validates every required field,
numeric conversion, increasing timestamp and consistent channel width before
calling the native adapter. The parsed frame count must match the admitted line
count. It does not silently recover truncated or schema-invalid tails. This is
a stricter research intake policy than the native replay parser's documented
tail recovery behavior. No input capture is calibrated or inserted into a graph.

The official small sample is accepted as three frames. The medium sample is
rejected at malformed line 200 with exit status 2; the native adapter alone
accepts its complete prefix. Synthetic regressions also cover a valid JSON but
invalid final schema and numeric overflow. These checks establish completeness
and bounded parsing, not labels or physical truth.

The release benchmark compares actual RuField alone against the bridge with
actual WorldGraph in 1,000 alternating pairs after 100 warmups. Both paths create
a fresh isolated engine, parse the rule, verify a signature, ingest and infer.
The bridge adds its guards, graph creation and ancestry publication. The exact
timings and source hashes are in the probe's `example-result.json` and
`verification-receipt.json`. Final added p95 is 0.038340 ms on synthetic
fixtures. They exclude signing, acquisition, calibration,
classification and persistent streaming costs, and do not establish target
device performance. Benchmark improvement is not model improvement.

All 44 public pipeline tests and 28 original encoder tests pass on ruOS. The
first isolated original-suite attempt lacked an unchanged collector dependency;
copying that exact existing script restored all 28 passes. Python source review
found no blocking issue. Rust review exposed tail completeness and numeric
conversion gaps in the new inspector; explicit validation and regression tests
address those gaps. All six Rust tests and the release build pass; an independent
review found no remaining blocker. The primary agent reran the six tests and
verified the tested source hashes against the committed receipt. CI includes both
Python suites and the standalone locked Rust probe.

## Next acceptance

Keep the current ACF model. A useful deployment experiment needs independently
annotated continuous captures with timestamps, permitted data use, verified
empty intervals, sensor placement, room connectivity and activity boundaries.
Compare the same streams with classifier alone, RuField validation, and RuField
plus WorldGraph context. Proposed acceptance is 20% fewer false alerts at the
same recall, plus less than 0.1 ms incremental p95 processing overhead on the
target device. Measure event detection delay separately from compute time.

Those are future acceptance targets, not results of the isolated clips or the
synthetic integrity probe. No production model, published Hugging Face weights,
live sensor configuration or canonical learned memory is promoted by this work.
