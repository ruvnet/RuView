# Evidence Matrix — What the README Claims, and What Backs It

The README states its sensing capabilities in plain, product language. The
implementation notes, the ADRs, the benchmark logs and the published model
cards describe the same capabilities at very different levels of maturity. This
document maps each headline claim to the evidence that exists in this
repository, so a reader can tell which statements rest on a measurement and
which rest on a design intent, a heuristic, or a simulator.

It is the reference for the claim "it is all fake or misleading", and for the
converse: it names the claims that are genuinely measured.

## Contents

- [How to read this matrix](#how-to-read-this-matrix)
- [The matrix](#the-matrix)
- [Claim notes](#claim-notes)
- [Two discrepancies the matrix exposes](#two-discrepancies-the-matrix-exposes)
- [What would move a claim to MEASURED](#what-would-move-a-claim-to-measured)
- [Numbers that are safe to quote](#numbers-that-are-safe-to-quote)
- [References](#references)
- [See also](#see-also)

## How to read this matrix

### Status

| Status | Meaning |
|---|---|
| **MEASURED** | A number produced by a run on real hardware or a named public benchmark, with a reproducer or a witness artifact in this repository. |
| **MIXED** | The claim bundles parts with different statuses. The parts are listed in the note. |
| **CLAIMED** | Asserted, with no measurement in this repository that supports the assertion as stated. |
| **SYNTHETIC** | Measured against a simulator's own ground truth. Proves the pipeline recovers known truth, not field accuracy. |
| **STUB** | The code path exists and is documented as a no-op, placeholder or not-yet-wired. |
| **UNVERIFIED** | No supporting evidence found in this repository. |

### Evidence kind

The kind of evidence matters as much as its presence, and the two are never
substituted for one another:

- **HARDWARE** — measured on physical silicon.
- **BENCHMARK** — measured on a public dataset such as MM-Fi.
- **SOFTWARE** — a gate that passed: a test suite, a build, a document that a
  reader can check against the source.
- **SYNTHETIC** — scored against generated data.
- **DESIGN** — a configured or intended value, never measured.

A number measured on a public benchmark is not a number measured in a room. A
test suite that passes is not evidence that the model is accurate.

## The matrix

| # | Claim (README) | Status | Kind | Evidence |
|---|---|---|---|---|
| 1 | "Detect people, measure breathing and heart rate, track movement, and monitor rooms — through walls, in the dark, with no cameras or wearables" | MIXED | mixed | Composed of claims 3–8 below, which resolve individually |
| 2 | "A $9 ESP32 board … tells you who's there, how they're breathing, and how their heart rate is trending" | MIXED | DESIGN (cost) + UNVERIFIED (accuracy) | `README.md:139`; unit cost is **DESIGN** per [ADR-028](adr/ADR-028-esp32-capability-audit.md); the accuracy half is unresolved |
| 3 | Presence and occupancy — detect people through walls, count them, track entries and exits | MIXED | HARDWARE (invariant) + CLAIMED (accuracy) | [occupancy integrity](validation/2026-08-31-esp32-c6-occupancy-integrity.md) (HARDWARE, invariant only); the accuracy figure was retracted — see note 3 |
| 4 | Vital signs — breathing rate and heart rate, contactless, while sleeping or sitting | CLAIMED | — | `README.md:157-158`; the algorithms are specified and implemented, the accuracy is not measured in this repository |
| 5 | Activity recognition — walking, sitting, gestures, falls — from temporal CSI patterns | MIXED | BENCHMARK | [MM-Fi study](benchmarks/mmfi-wifi-sensing-study.md) §2: in-domain 88.1 %, cross-subject zero-shot 10.0 % |
| 6 | Environment mapping — RF fingerprinting identifies rooms, detects moved furniture, spots new objects | UNVERIFIED | — | No measurement, benchmark or validation report found |
| 7 | Sleep quality — overnight monitoring with sleep stage classification and apnea screening | UNVERIFIED | — | No measurement, benchmark or validation report found |
| 8 | Camera-free pose — estimate 17 body keypoints from WiFi CSI | MIXED | BENCHMARK (research) + STUB (shipped) | 82.69 % torso-PCK@20 in-domain on MM-Fi ([study](benchmarks/mmfi-wifi-sensing-study.md) §1.1); the committed on-device model is PCK@20 3.0 % with a `confidence=0` runtime stub ([`README.md:339`](../README.md)) |
| 9 | Built-in model workflow — record CSI, train models, load RVF files, switch LoRA profiles | MEASURED | SOFTWARE | Crates, CLI and tests exist and pass; the deterministic reference proof is the reproducer |
| 10 | Unified RF world model — WiFi CSI, radar, UWB and cellular in one privacy-bounded scene model | SYNTHETIC | SYNTHETIC | [ADR-260](adr/ADR-260-rufield-mfs.md); the README itself says "accuracy is still synthetic until real-data validation" |
| 11 | "Ships 21 entities per node (11 raw signals + 10 inferred semantic states) plus 3 starter HA Blueprints" | MIXED | SOFTWARE | The 3 blueprints are confirmed ([integration doc](integrations/home-assistant.md):181); the entity count is not — see note 11 and [discrepancy 1](#two-discrepancies-the-matrix-exposes) |

## Claim notes

### 1 — The hero sentence

The sentence is a composition. Each verb resolves separately: "detect people"
to note 3, "measure breathing and heart rate" to note 4, "track movement" to
note 5, "monitor rooms" to note 6, and "17 body keypoints" to note 8. "Through
walls, in the dark" is a statement about the radio, not about a measurement: the
MM-Fi benchmark is line-of-sight across four environments, and no through-wall
live-hardware run is recorded in this repository.

### 2 — The $9 board

The unit cost is **DESIGN**: [ADR-028](adr/ADR-028-esp32-capability-audit.md)
records "~$8–12/node, ~$54 for a 3-node kit" as a designed figure, not a
purchase. "Who's there" is the presence heuristic of note 3, and "how their
heart rate is trending" is note 4.

### 3 — Presence and occupancy

Presence detection in the shipped firmware is a variance-threshold heuristic
with false positives on RF interference, and person count is a slot-capacity
heuristic rather than a learned classifier. The "100 % presence accuracy"
figure was measured on a single-class recording — 6,062 of 6,063 frames labelled
present — where a constant "yes" predictor scores 99.98 %. It was retracted and
superseded by an 82.3 % held-out temporal-triplet metric, which measures
embedding quality and not presence accuracy.

The strongest live-hardware evidence is the
[occupancy integrity run](validation/2026-08-31-esp32-c6-occupancy-integrity.md):
300 seconds of live sensing from a C6 node, 242 edge packets, 61 absent
packets, and 0 absent packets carrying a nonzero count. The report states the
limit itself: "This is not a person count accuracy result", and "The current run
had no labelled ground truth, so multi person fidelity and adjacent room
rejection remain unmeasured."

The learned count head, `cog-person-count` v0.0.2, is measured on 1,077
samples: 62.3 % overall accuracy, 34.3 % class-1 accuracy under a random
80/20 split, with a 5-fold cross-validation reference of 62.2 % ± 1.9 % and
57.1 % class 1 ([benchmark log](benchmarks/person-count-cog.md)). Those are
recorded-room samples, not live ESP32 measurements.

### 4 — Vital signs

The README specifies the algorithms and their operating ranges: breathing by
bandpass 0.1–0.5 Hz on wrapped phase with circular variance and zero-crossing
BPM, 6–30 BPM; heart rate by bandpass 0.8–2.0 Hz with zero-crossing BPM,
40–120 BPM; both "real-time". "Real-time" is a latency statement. No
accuracy figure for either estimator against labelled ground truth exists in
this repository, on live hardware or otherwise.

### 5 — Activity recognition

The [MM-Fi study](benchmarks/mmfi-wifi-sensing-study.md) benchmarks 27
action classes. In-domain accuracy is 88.1 %, and the study records that this
figure is leakage-inflated: the honest cross-subject zero-shot number is 10.0 %,
near chance. The same study's headline conclusion is that WiFi sensing does not
generalise zero-shot to new people or rooms, and that a roughly 30-second
in-room calibration closes the gap for both pose and action.

### 6 — Environment mapping

No measurement, benchmark or validation report for room identification, moved
furniture or new-object detection was found in this repository. The capability
appears in design material and in the edge-module catalogue, not in evidence.

### 7 — Sleep quality

No measurement, benchmark or validation report for sleep-stage classification or
apnea screening was found in this repository. The edge-module catalogue lists
signed modules for sleep and health monitoring, which is a distribution fact
about the module catalogue and not evidence of sleep-stage accuracy.

### 8 — Camera-free pose

Two different numbers are in play, and the README says so. The research
benchmark, published as `ruvnet/wifi-densepose-mmfi-pose`, reaches 82.69 %
torso-PCK@20 in-domain on the MM-Fi `random_split` protocol, against
MultiFormer's 72.25 % and CSI2Pose's 68.41 %; the study records that the
headline was self-corrected down from an inflated 91.86 % under a looser
normalisation before publishing. The committed on-device model
`cog-pose-estimation` v0.0.1 reaches PCK@20 3.0 % and PCK@50 18.5 % on a
held-out time window of the 1,077-sample paired recording — an 80/20
stratified split on `ts_start`, so the evaluation window is held out in time
rather than at random (`README.md` describes the same run as a 217-sample
holdout) — below the ADR-079 target of ≥ 35 % — and its runtime path
is still a centred-skeleton stub returning `confidence=0`, with the weights not
yet wired in. The README's own guidance is explicit: the shippable pose
accuracy the project can stand behind is the MM-Fi benchmark number, not a live
single-ESP32 number.

### 9 — The model workflow

This is the claim with the strongest software evidence: the crates exist, the
CLI is documented, the tests pass, and the deterministic reference proof runs
from a clean checkout. Reproducer: `python archive/v1/data/proof/verify.py`,
which prints `VERDICT: PASS`.

The workflow also carries the project's most instructive correction: the v1
contrastive encoder shipped with a flat training loss and a "100 % presence"
figure that did not survive scrutiny, and the v2 re-benchmark plus the
2026-09-09 fix to the exported training metrics are both documented in the
[model card](huggingface/MODEL_CARD.md).

### 10 — The unified RF world model

[ADR-260](adr/ADR-260-rufield-mfs.md) publishes the open specification and a
reference stack whose benchmark metrics are all **SYNTHETIC**: scored against
the simulator's own ground truth, proving the pipeline recovers known truth and
not field accuracy. Real hardware adapters — ESP32 CSI, mmWave, thermal IR — are
a documented roadmap item, none validated in v0.1. The README carries the same
caveat, which makes this the one headline claim that is already correctly
labelled.

### 11 — The smart-home surface

The three starter Home Assistant Blueprints are real and documented. The entity
count is not 21 today. See [discrepancy 1](#two-discrepancies-the-matrix-exposes).

## Two discrepancies the matrix exposes

### 1 — The README says 21 entities; the integration doc the README links to says 15

`README.md:39` states "Ships 21 entities per node (11 raw signals + 10 inferred
semantic states …)". The document it links to,
[`docs/integrations/home-assistant.md`](integrations/home-assistant.md):39,
states "**15 entities** per device (13 with `--privacy-mode`): the raw signals
and **six** semantic primitives", and line 406 repeats "The 15 entities per
node (9 raw signals and 6 semantic states)".

The reconciliation is visible in the same document: "Some uses below rely on
entities that are not announced yet (zones, bathroom, bed exit, meeting); those
wait on a zone source or a finer motion level." The 21-entity figure is the
ADR-115 design target, carried by
[ADR-116](adr/ADR-116-cog-ha-matter-seed.md):43,
[ADR-122](adr/ADR-122-bfld-ruview-ha-matter-exposure.md):17 and the
[user guide](user-guide.md):902. What ships is 15, or 13 under
`--privacy-mode` — which is exactly the figure
[ADR-115](adr/ADR-115-home-assistant-integration.md):710 measured.

### 2 — The README's "Real & validated" table labels presence with a metric that does not measure presence

`README.md:338` lists `ruvnet/wifi-densepose-pretrained` under "Real &
validated" with "Presence = 82.3 % held-out temporal-triplet accuracy". The
same repository's [model card](huggingface/MODEL_CARD.md) states the
opposite about that metric: "The triplet metric measures embedding quality,
**not** downstream presence/vitals accuracy (which needs multi-class,
multi-room labelled data we don't yet have for this 2.4 GHz feature)."

Both statements are true about the same number. One of them is wrong about
what the number means, and it is the README.

## What would move a claim to MEASURED

| Claim | Artifact that would settle it |
|---|---|
| Presence accuracy | A held-out, labelled live-hardware capture with a person present and absent, with the person count scored against that ground truth. The occupancy-integrity invariant is not a substitute. |
| Person count accuracy | The same, for the 0/1/2/3+ case, with the count estimator fixed before the run so the number is not post-hoc. |
| Vital signs accuracy | A labelled recording with a reference respiratory or cardiac measurement, scored for both estimators, with the operating range validated at the edges rather than in the middle. |
| Activity recognition | A cross-subject, cross-room holdout scored on the deployed model, with the leakage-free protocol the MM-Fi study uses for pose. |
| Environment mapping | A controlled experiment: a room fingerprinted, then re-measured after a known furniture change, with the detection threshold fixed in advance. |
| Sleep quality | Stage-labelled overnight data with an apnea reference. No stage-labelled dataset appears in this repository today. |
| Through-wall operation | A capture with the radio and the person separated by a specified wall construction, reported as a detection rate against labelled ground truth. |
| ESP32-C6 CSI rate | The same repeatable benchmark used for the RTL8721Dx, run against C6 and S3 hardware, so the "which is faster" comparison rests on one protocol. |
| Entity count | An announced-entity count read from a running Home Assistant instance, rather than a design target. |

## Numbers that are safe to quote

**HARDWARE**

- ESP32-C6 cross-board clock sync: raw offset stdev 411.5 µs → 104.1 µs after
  EMA smoothing, 99.56 % cross-board match (2,689 / 2,701 beacons), 0 transmit
  failures over a 5-minute soak
  ([ADR-110](adr/ADR-110-esp32-c6-firmware-extension.md) §4.2).
- ESP32-C6 firmware: 1003 KB image, 46 % flash slack, 310 KiB free heap, boot
  353 ms, time-sync init 413 ms
  ([ADR-110](adr/ADR-110-esp32-c6-firmware-extension.md) §4.1, P7).
- Occupancy invariant on a live C6 node: 242 edge packets, 61 absent packets,
  0 absent packets with a nonzero count
  ([validation](validation/2026-08-31-esp32-c6-occupancy-integrity.md)).
- RTL8721Dx CSI rate: 14.67 fps baseline → 66.40 fps tuned, 2,302.7 → 10,424.8
  bytes/s, ~3.8 % sequence gaps at the tuned rate
  ([ADR-323](adr/ADR-323-rtl8721dx-ameba-csi-wire-protocol.md)).
- ESP32-S3 deep-sleep floor: 10 µA
  ([ADR-028](adr/ADR-028-esp32-capability-audit.md)).

**BENCHMARK**

- MM-Fi pose, in-domain, torso-PCK@20: 82.69 % single model, 83.59 % with
  graph, 3-ensemble and TTA, against MultiFormer 72.25 % and CSI2Pose 68.41 %
  ([study](benchmarks/mmfi-wifi-sensing-study.md) §1.1).
- MM-Fi action, 27 classes: 88.1 % in-domain (leakage-inflated), 10.0 %
  cross-subject zero-shot ([study](benchmarks/mmfi-wifi-sensing-study.md) §2).
- Deployed pose cog v0.0.1: PCK@20 3.0 %, PCK@50 18.5 % on a held-out time
  window of the 1,077-sample paired recording
  ([benchmark log](benchmarks/pose-estimation-cog.md)).
- Deployed count cog v0.0.2: 62.3 % overall, 34.3 % class 1, on 1,077
  samples; 5-fold CV reference 62.2 % ± 1.9 % and 57.1 % class 1
  ([benchmark log](benchmarks/person-count-cog.md)).
- Held-out temporal-triplet accuracy of the published encoder: 82.3 %, against a
  66.4 % raw-feature baseline
  ([model card](huggingface/MODEL_CARD.md)).

**SYNTHETIC**

- ADR-260 reference stack: presence, breathing, bed exit and room transition
  F1 = 1.000, nocturnal scratch 0.923, p95 latency ~0.01 ms, provenance
  coverage 100 %, 0 privacy violations — all against a simulator's own ground
  truth.

**DESIGN — not measurements, do not quote as performance**

- ESP32-S3 CSI sampling rate: 20–100 Hz configurable
  ([ADR-028](adr/ADR-028-esp32-capability-audit.md)). The "54,000 fps"
  figure elsewhere in the documentation is explicitly flagged by the same audit
  as a benchmark claim not measured at audit time.
- Node unit cost: ~$8–12 per node, ~$54 for a 3-node kit.

## References

- [Proof of capabilities](proof-of-capabilities.md) — the existing honest accounting, and its own list of limitations.
- [WiFi-CSI sensing on MM-Fi](benchmarks/mmfi-wifi-sensing-study.md) — the pose and action benchmark, and the generalisation result.
- [Pose estimation cog benchmark log](benchmarks/pose-estimation-cog.md) — every published run of the deployed pose model.
- [Person count cog benchmark log](benchmarks/person-count-cog.md) — every published run of the deployed count model.
- [Model card](huggingface/MODEL_CARD.md) — the published card, including the retraction of the "100 % presence" figure.
- [ADR-028 — ESP32 capability audit](adr/ADR-028-esp32-capability-audit.md) — hardware capability record and the audit of the throughput claims.
- [ADR-079 — camera ground-truth training](adr/ADR-079-camera-ground-truth-training.md) — the pose accuracy target and the current gap.
- [ADR-260 — RuField MFS](adr/ADR-260-rufield-mfs.md) — the unified field-sensing specification and its synthetic-only status.
- [ADR-115 — Home Assistant integration](adr/ADR-115-home-assistant-integration.md) — the 21-entity design target and the measured 13-entity privacy-mode count.
- [Home Assistant integration](integrations/home-assistant.md) — the shipped entity set, the blueprints, and the not-yet-announced entities.
- [Occupancy integrity validation](validation/2026-08-31-esp32-c6-occupancy-integrity.md) — the live-hardware occupancy invariant.
- [RTL8721Dx vs ESP32 comparison](benchmarks/rtl8721dx-vs-esp32-csi-comparison.md) — the radio-level comparison behind the hardware numbers.

## See also

- [CHANGELOG](../CHANGELOG.md) — the public retraction record, kept in place.
- [ADR-187 — archive/v1 deprecation and honest labelling](adr/ADR-187-archive-v1-deprecation-honest-labeling.md) — the rule that a maturity label travels with a claim.
