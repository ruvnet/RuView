# Next benchmark: generalization and temporal controls

Research snapshot: September 29, 2026. **Recommendation:** develop the public
BFI activity benchmark below, with explicit recording quality and held-out
acquisition domains. Keep the current NTU identity results as a separate
experiment. Its static ridge control already matched the sequence models at
56/56 validation recordings; temporal order was unnecessary for that split.
This does not identify the physical source of the predictive signal.
[Measured local results](RESULTS.md)

## What recent research changes

- **Time matters (Internet of Things, 2025)** evaluates changes across recording
  times and distinguishes static channel structure from dynamic variation.
  Same-session accuracy is insufficient evidence of later-day performance.
  Its author release includes two-day positioning and a longer signal-stability
  study. [Paper](https://air.uniud.it/retrieve/50c9dc71-a9f0-4f17-81d2-44512a662566/IOT2025.pdf),
  [author dataset](https://zenodo.org/records/14212401).
- **WiTTA-Bench (CVPR, June 2026)** compares 20 adaptation methods under online
  and offline protocols, with environment, subject and heterogeneous-device
  shifts. Its conclusions are protocol-specific; source-only inference and
  adaptation using unlabeled target observations must receive separate scores.
  Start with its CNN/source-only and TENT references before an expensive model
  search. These are relevant comparators, not proven winners on BFI.
  [Primary paper](https://openaccess.thecvf.com/content/CVPR2026/papers/Li_WiTTA-Bench_Benchmarking_Test-Time_Adaptation_for_WiFi_Sensing_CVPR_2026_paper.pdf),
  [author code](https://github.com/BdLI-group/WiTTA-Bench).
- **DGSense (February 2025 preprint)** proposes training-domain diversification
  and episodic learning without target-domain data. That is a distinct claim
  from test-time adaptation. Our first one-environment training subset cannot
  reproduce its full evaluation. [Primary paper](https://arxiv.org/abs/2502.08155).

These papers motivate the evaluation design. No accuracy from them has been
reproduced here. A sequence permutation control is our proposed diagnostic,
not a published result attributed to those authors.

## Exact public BFI subset

**MEASURED metadata:** the author's Hugging Face dataset declares `gpl-3.0`
and permits ungated downloads. Individual captures avoid the large multipart
BeamSense archive. [Pinned dataset](https://huggingface.co/datasets/foysalhaque/CSI-BFI-HAR-Dataset/tree/c3013b3edc5a563a9cf6759f5d464a9d9b6f456a),
[pinned metadata API](https://huggingface.co/api/datasets/foysalhaque/CSI-BFI-HAR-Dataset/revision/c3013b3edc5a563a9cf6759f5d464a9d9b6f456a?blobs=true).

The [non-data manifest](manifests/csi-bfi-har-c3013b3-subset.json) records every
source path, byte size, SHA-256, immutable URL, proposed split and label fields.
Large-file SHA-256 values are Hub LFS references, not independent downloads.
Twenty small files totaling 595,144 bytes were fetched into memory and verified
against their Git blob identifiers; their SHA-256 values are also included.
No full subset download or model scoring was performed by this research task.

| Proposed partition | Author directory | Day index | Files | Bytes |
|---|---|---:|---:|---:|
| Train | `HAR-1/BFI/M1` | 1 | 60 | 459,096,936 |
| Validation | `HAR-3/BFI/M1` | 3 | 60 | 470,265,264 |
| Held out, same device label | `HAR-5/BFI/M1` | 5 | 60 | 1,195,844,496 |
| Held out, other device/view | `HAR-5/BFI/M2` | 5 | 60 | 1,239,776,712 |
| Total | | | 240 | **3,364,983,408** |

All partitions contain activity codes A–T and participant codes P1–P3 in their
filenames. The selected files do not establish the README's broader P1–P6
coverage. The author maps these setups to kitchen, classroom and living room:
**day and environment are confounded**. M1/M2 establish device/view labels, not
heterogeneous-chipset transfer. Report the combined domain shift honestly.
Keep all views of a day/activity/person event in the same partition; M2 must not
become training data while its M1 counterpart is held out.
[Author label/setup definitions](https://github.com/kfoysalhaque/CSI-BFI-HAR).

### License and integrity receipt

- Revision: `c3013b3edc5a563a9cf6759f5d464a9d9b6f456a`.
- Manifest SHA-256: `c2db11f71fb7cef87ff9853bdf7571816347824445855c128caeb4053eef288a`.
- Dataset license declaration: `license: gpl-3.0` in the author's **dataset**
  card, independently checked in API metadata.
- [Pinned card snapshot](https://huggingface.co/datasets/foysalhaque/CSI-BFI-HAR-Dataset/resolve/c3013b3edc5a563a9cf6759f5d464a9d9b6f456a/README.md):
  6,719 bytes, SHA-256 `0870600f7e9e5f232f515876fadf76b9a4cafee324a5066d7aaa65a262541d0e`.

### Download recipe for a later authorized run

Use the manifest URLs with a downloader that enforces a 4,000,000,000-byte total
budget, a 64 MiB per-file cap, exact lengths and SHA-256 checks before marking
files complete. Store the pinned card beside the data and retain attribution.
The following CLI selection describes the same immutable files; it does not
replace the downloader's byte and integrity checks:

```bash
hf download foysalhaque/CSI-BFI-HAR-Dataset --repo-type dataset \
  --revision c3013b3edc5a563a9cf6759f5d464a9d9b6f456a \
  --include 'HAR-1/BFI/M1/*.pcapng' 'HAR-3/BFI/M1/*.pcapng' \
            'HAR-5/BFI/M1/*.pcapng' 'HAR-5/BFI/M2/*.pcapng' 'README.md' \
  --local-dir /private/ruview-public-bfi-c3013b3
```

Require exactly the manifest's 240 trace files plus the card; do not expand the
selection to CSI, CSV, other participants or all dataset files. Keep packets
outside Git. Record successful and incomplete downloads explicitly.

## Decoder and quality gates before training

**MEASURED small-sample inspection:** `HAR-3/BFI/M1/L_3_M1_P3.pcapng` is 16,052
bytes, SHA-256 `ff5f30d4abc4be9796a24eced82fb29be3a3978a4388ca1dcb8a1a957b6cd5f6`.
It contains 14 packet blocks across 196.223407343 seconds, with radiotap link type
127 and nanosecond PCAPNG timestamp resolution. All 14 MAC FCS values matched an
independent CRC32 calculation. Local tshark 4.2.2 reported no malformed packets.

The observed feedback is **VHT MU 3×1, 80 MHz, Ng=1, codebook 1**, Action No Ack,
category 21/action 0. First-segment is set, remaining segments are zero. The
first report has 234 tone groups and phi11, phi21, psi21, psi31 angles. Sequence
Control's low bits are nonzero in many packets; their subtype-specific meaning
needs an explicit protocol decision. This does not justify weakening fragment
checks on other management frames. The current HE SU decoder cannot admit this
dataset unchanged. Independently validate all angle integers, controls, report
lengths, MU-exclusive fields and FCS before import.

**Predeclared quality census, before training or viewing model scores:**

1. Account for every manifest file: integrity, malformed/unsupported counts,
   format changes, timestamps, valid reports, retries, duplicate payloads and
   coverage. Retain the sparse recordings in the census.
2. Proposed initial input unit: a five-second observed interval with at least
   20 valid unique reports and no inter-report gap above one second. Do not
   interpolate across missing observations. This is a project admission rule,
   not a requirement inferred from the papers.
3. Require at least two independently recorded files per activity in each of
   train, validation and each held-out device/view group to supply at least one
   eligible interval. If any of A–T lacks coverage, report benchmark admission
   failure; do not silently shrink the label set. Record rejected-file reasons
   and abstention coverage separately from conditional prediction accuracy.
4. Freeze the resulting file groups and quality rule before model selection.
   Split recordings/events first, then generate windows. Determine per-file
   time coverage from observed packets; these public traces have no trusted
   external capture-start/end manifests, so unobserved leading/trailing time
   cannot be claimed as measured.

## Comparisons that would establish progress

First compare constant-class, timing-only and static circular-angle statistics
with the existing small GRU adapted to BFI. Add a small CNN reference, then
source-only versus separately labeled TENT adaptation if justified. Freeze the
representation, budgets and three-seed selection rule before opening the two
day-5 groups. Report recording-level macro-F1/balanced accuracy, confusion
counts, eligible recording/time coverage, latency and memory; paired M1/M2
observations are not independent trials.

Use four controlled inputs with identical partitions and optimizer budgets:
raw/original, raw/shuffled, demeaned/original and demeaned/shuffled. Retrain each
control; shuffling only at evaluation also creates a distribution shift. For BFI,
apply circular handling to phi rather than subtracting wrapped angle integers.
Full-recording centering is an offline protocol; a streaming claim needs a
causal centering rule. Keep time coordinates administrative after shuffling and
label the destroyed chronology explicitly. Success would be a repeatable lift
over static/timing controls on held-out acquisition domains, with unchanged
coverage. A gain on random windows from the same recording is insufficient.

For a later **pure temporal** CSI control, 3DO supplies fixed-layout days 1–2
and a changed-layout day 3, in a 435,217,412-byte archive. Its Zenodo API says
CC BY 4.0 while its description says noncommercial research only; resolve that
conflict before reuse. WiTTA's WiHAR-Dual supplies a genuine heterogeneous-device
reference, but its dataset license was not independently established here.
Neither is silently substituted for the licensed BFI subset.
[3DO release](https://zenodo.org/records/10925351),
[3DO author implementation](https://github.com/StrohmayerJ/3DO),
[WiTTA author release](https://github.com/BdLI-group/WiTTA-Bench).
