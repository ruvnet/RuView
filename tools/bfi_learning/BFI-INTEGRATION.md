# Offline BFI representation and integration boundary

Status: **PROPOSED**, source review only. This document does not add a model,
change radio settings, start training, or evaluate the held-out identity set.

## What is already measured

The local capture path has produced real HE feedback from the BE550/client
link. The recorded optimization runs contained 66 and 96 independently checked
reports in roughly 45 seconds; the largest gaps were 23.291 and 16.393 seconds.
A fresh lower-traffic control contained one report. These are sparse, uneven
observations, not a demonstrated continuous motion signal. The measurements,
formats and reproducer are in [BFI optimization evidence](../bfi/OPTIMIZATION.md)
and [initial validation](../bfi/VALIDATION.md).

[The decoder](../bfi/bfi_decode.py) accepts complete VHT SU 2x1/2x2 reports at
20/40/80 MHz and complete HE SU reports of those dimensions at Ng4, with full
RU spans 0..8, 0..17 and 0..36. The corresponding HE tone counts are 64, 122 and
250. It rejects MU/CQI, segmentation, partial HE allocations, Ng16 and EHT.
It reconstructs quantized steering matrices, not full complex channel CSI.

The current real-data claim is bounded by the stored captures and independent
[tshark comparison](../bfi/compare_tshark.py). A matrix orthogonality check alone
cannot validate packet interpretation: an incorrect angle can still produce
an orthogonal matrix.

## Smallest next feature

Add one offline BFI importer to the existing preparation tools. Its output is
an **unlabeled representation and continuity receipt**, not an identity model.
Do not port the parser into Rust or create a new live radio service in this step.

1. Require an existing classic radiotap PCAP, its matching capture manifest,
   and an explicitly selected AP/client direction. Verify the PCAP hash and
   full observation window with [quality_report.py](../bfi/quality_report.py).
   Keep raw files and source addresses outside Git.
2. Refactor a bounded decoded-report iterator out of `bfi_decode.analyze` so
   the importer and aggregate analysis share frame validation, direction
   checks and retry deduplication. Preserve existing `analyze` results exactly.
   Reuse `pcap_packets`, `strip_radiotap` and `decode_report`; do not create a
   second interpretation of HE controls.
3. Partition by capture, selected link, standard, bandwidth, Nr/Nc, grouping,
   codebook and exact tone grid. Keep 20/40/80 MHz outputs separate. Report
   format changes and unsupported variants explicitly; never interpolate
   across grids or silently treat unsupported packets as valid samples.
4. Per tone, encode `[cos(phi), sin(phi), 2*psi/pi]` in subcarrier order.
   This gives 192, 366 or 750 float32 features for the supported HE grids,
   under the trainer's 1,024-feature cap. The circular phi representation
   avoids an artificial discontinuity at 2*pi. Retain the exact integer angle
   codes and codebook in a private audit artifact. Keep average stream SNR,
   token, sequence control and arrival gaps as diagnostics, outside model
   features for the first experiment.
5. Use the existing numeric NPZ contract: `frames` float32, `timestamps`
   float64 seconds from PCAP, and `sequence` uint32 containing the observed
   12-bit management sequence number. Metadata must declare
   `source_kind=bfi`, `label_status=unlabeled`, the exact feature encoding,
   source/decoder hashes, format, and the capture session. A link identifier
   identifies a radio link; it must never become a human identity label.
6. Reject nonfinite values, conflicting duplicate records and backward or
   equal retained timestamps. Do not invent time offsets to force ordering.
   Split contiguous runs at a declared gap threshold (initially 1 second,
   matching the trainer's default), and report how many windows remain for
   window lengths 4, 8, 16 and 32. This window census is read-only and precedes
   any decision to train. Keep frame and element budgets explicit.

The first acceptance artifact is a small JSON receipt with source hashes,
per-format counts, occupied bins, gap statistics, rejected variants and usable
window counts. Existing capture sizes may fail minimum train/validation window
requirements; report that outcome without relaxing the split or filling gaps.

### What continuity can and cannot measure

Use the **entire manifest interval**, including leading/trailing silence and
partial final bins. Report unique reports per second, occupied 1-second bins,
maximum silence, median/p95 inter-report gap, retries removed, timestamp
regressions, kernel drops when available, and counts per format/direction.
Use [coverage()](../bfi/quality_report.py) rather than dividing by only the short
span between the first and last report.

Management sequence numbers also advance for other management frames. Sounding
tokens wrap and do not provide a complete capture-side expected-report count.
Therefore sequence jumps and no-report periods do **not** establish packet loss,
AP sounding rate, or missing people. Name them observed sequence discontinuities
and observation gaps. Zero kernel drops only excludes the reported kernel-drop
mechanism; it does not prove complete RF reception.

### Representation evaluation, if enough data exist

Use chronological, purged train/validation intervals within each capture;
windows must not cross capture, format or gap boundaries. Fit normalization on
training frames only. Compare a next-observation predictor with persistence
(last observed representation) and the training mean. Report errors versus
actual elapsed time as well as per-observation errors. Improvement on a single
link/capture only measures temporal predictability of that representation.
It does not establish movement, position or person recognition. A later,
separately recorded session is needed for session transfer evaluation.

## Rust BFLD boundary

[BfldFrameHeader](../../v2/crates/wifi-densepose-bfld/src/frame.rs) is an 86-byte,
little-endian internal envelope with hashed AP/STA identifiers, a session ID,
channel/dimensions, monotonic timestamp and payload CRC. It is not an 802.11 HE
feedback decoder. [BfldPayload](../../v2/crates/wifi-densepose-bfld/src/payload.rs)
parses length-prefixed opaque sections for angles, amplitude/phase proxies,
SNR, optional CSI delta and a vendor extension. Structural parsing does not
validate an HE angle codebook or subcarrier grid.

A future adapter needs a reviewed byte-level contract for those sections before
writing frames: the current opaque byte vectors alone do not define how this
Python angle representation maps to them. It must preserve standard, codebook,
grouping and grid semantics; keep unavailable channel noise or RSSI explicitly
unavailable through an agreed encoding instead of inventing measured values.
The report's Nc/Nr steering dimensions also need an explicit mapping to the
header's n_tx/n_rx semantics. PCAP wall-clock time needs a declared monotonic
rebasing policy rather than direct assignment to a monotonic header field.

The integration point is
[`BfldPipeline::process_to_frame`](../../v2/crates/wifi-densepose-bfld/src/pipeline.rs),
which takes `SensingInputs`, a header template, a typed payload and an optional
identity embedding. An offline unlabeled import supplies **no identity
embedding** and must not manufacture coherence/presence evidence to pass this
pipeline. The pipeline stamps its own privacy class/timestamp and demotes
payload contents. `Anonymous` strips angles and CSI delta; `Restricted` also
strips amplitude/phase proxies. Preserve those gates and the existing sink
constraints. The proposed first step stops before this integration boundary.

Relevant Rust regression contracts are
[frame_payload_integration.rs](../../v2/crates/wifi-densepose-bfld/tests/frame_payload_integration.rs),
[payload_sections.rs](../../v2/crates/wifi-densepose-bfld/tests/payload_sections.rs),
[pipeline_to_frame.rs](../../v2/crates/wifi-densepose-bfld/tests/pipeline_to_frame.rs),
[privacy_gate_demote.rs](../../v2/crates/wifi-densepose-bfld/tests/privacy_gate_demote.rs)
and [pipeline_i3_isolation.rs](../../v2/crates/wifi-densepose-bfld/tests/pipeline_i3_isolation.rs).

## Public CSI classifier: controls before stronger claims

Yes, the current classifier can learn static room, antenna, device, or recording
cues when they correlate with identity. This is a risk inferred from its inputs
and protocol, not a finding that any specific confound has been measured.
[prepare.py](prepare.py) retains absolute CSI amplitudes; [train.py](train.py)
uses train-fitted feature normalization and an LSTM whose final hidden state
feeds a linear identity head. Normalization does not remove persistent
per-recording offsets. The custom partition is file-disjoint, but physical
session independence and room/device metadata are unknown.

Freeze the current custom split and leave its held-out test files unopened for
these controls. Use only training and validation recordings, with the same
recording-level metric, seeds, normalization policy and trial budget:

| Control | Concrete experiment | Interpretation |
|---|---|---|
| Static summaries | Train a small classifier on each recording's feature-wise temporal mean and standard deviation, using train-fitted scaling | Strong validation performance establishes that static summaries contain label information; it does not identify which nuisance caused it |
| Frozen temporal content | Replace each window with its repeated temporal mean; retrain with identical split and model budget | Performance retained means within-window dynamics were unnecessary for that result |
| Time permutation | Permute frame order independently within each recording, reproducibly, for both train and validation; retrain | Similar performance weakens a claim that temporal order is essential |
| Remove static offsets | Subtract each input window's own mean; compare first differences in a separate fixed run | A collapse is consistent with dependence on static content, though true identity information may also be removed |
| Label null | Permute identity labels among complete training recordings with class counts preserved; leave validation labels unchanged | Above-chance validation requires investigating leakage or an implementation error; no windows may cross recordings |
| Duplicate sensitivity | Audit normalized/cropped and near-duplicate recording fingerprints across train/validation only | Finds related samples missed by exact raw or float32 hash checks; report matches, do not silently rewrite the frozen split |

Keep label permutation at recording level, not independently per overlapping
window. Aggregate window probabilities to one decision per recording as in the
trainer. Report mean and spread across seeds, confusion matrices, and a
recording-level bootstrap interval; overlapping windows are not independent
samples. The balanced 14-class chance accuracy is 1/14, but also report the
observed class-majority baseline.

These controls can reveal weak evidence for dynamics; none proves room/device
invariance. That claim needs verified nuisance metadata or a new collection
with the same identities across rooms/devices and whole-session holdouts.
Public amplitude classification cannot be transferred directly to the local
quantized BFI angle features without a separately validated representation and
new labeled evidence.

## Required tests for the proposed importer

Extend [test_bfi_decode.py](../bfi/test_bfi_decode.py) to ensure a shared iterator
preserves summaries, rejection outcomes, cache isolation and exact angles.
Use [test_compare_tshark.py](../bfi/test_compare_tshark.py) for independent
literal-packet controls; keep real captures private.

Add synthetic tests for phi wrap continuity, exact per-grid feature order,
retry deduplication, mixed-width partitioning, repeated/backward timestamps,
gap segmentation, empty captures, unsupported variants and numeric byte bounds.
Prove that sequence/token jumps never become a loss percentage or identity label.
Reuse [test_quality_report.py](../bfi/test_quality_report.py) cases for full-window
edges, zero-report blackout, wrong direction, hash mismatch and partial bins.
Add importer-to-[train.py](train.py) contract checks for unlabeled status,
train-only normalization and purged windows; reject insufficient data rather
than weakening the holdout. These proposed tests have not been implemented or
run as part of this document-only review.
