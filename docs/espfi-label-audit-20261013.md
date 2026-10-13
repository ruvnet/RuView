# ESP-Fi label and confirmation audit

Audit date: 2026-10-13. Status: semantic mapping unresolved. No labels, recordings or model parameters changed.

The pinned source is `AutoSmartGroup/ESP-Fi-HAR` revision `c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4`. A direct GitHub API check of `commits/main` on the audit date returned this same revision, dated 2026-03-04T04:49:57Z. There is no newer main revision resolving this conflict in the inspected source.

## Observed conflict

Both source README files describe the third filename component as the activity ID. Their semantic mapping disagrees with the supplied benchmark directories:

| Filename activity ID | README meaning | Benchmark directory meaning |
| --- | --- | --- |
| 1 | run | run |
| 2 | fall | walk |
| 3 | walk | jump |
| 4 | turn | squat |
| 5 | jump | arm wave |
| 6 | squat | turn |
| 7 | arm wave | fall |

The pinned recursive Git tree contains 560 benchmark MAT paths, with 80 filenames per activity ID. Each ID appears in exactly one semantic directory. For example, `Model Code/Data/test_amp/jump/3-1-3-1.mat` assigns filename ID 3 to the jump directory. This is direct path metadata evidence, not an interpretation of signal shape. The consistency of this subset does not establish which naming convention is correct for all 2,240 archive recordings.

The source `dataset.py` assigns output indices by enumerating directories in this order: `arm_wave`, `fall`, `jump`, `run`, `squat`, `turn`, `walk`. It does not parse activity numbers from filenames. Its classifier indices therefore differ from RuView's numeric IDs even independently of the README conflict. A comparison of score columns must use an explicit mapping, never positional coincidence.

Primary pinned sources and digests:

| Source | Git blob SHA1 | Content SHA256 |
| --- | --- | --- |
| [Root README](https://github.com/AutoSmartGroup/ESP-Fi-HAR/blob/c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4/README.md) | `fe6e213a5f849b4ac62049f3e5a1d75a24890d27` | `b2b3f1f9296c848c9c4bf73bf760a719a907cd5a4acd2dd9325ca7a5b7792099` |
| [Benchmark README](https://github.com/AutoSmartGroup/ESP-Fi-HAR/blob/c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4/Model%20Code/README.md) | `74cd498c9a19859ec583ba4f1832f9b4038bb4dd` | `31f5b3ffc29ea1b0fd665f63f985620295eb780e716473070f4ee17cae333a2f` |
| [Benchmark loader](https://github.com/AutoSmartGroup/ESP-Fi-HAR/blob/c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4/Model%20Code/dataset.py) | `7379ded7c503b757c4490ee0fe88dfbec802d7a1` | `859cdf3150555002bc096258eb1473cef18f39d35dce559f78467ab6cccac977` |

[Recursive tree metadata](https://api.github.com/repos/AutoSmartGroup/ESP-Fi-HAR/git/trees/c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4?recursive=1) provides the directory membership evidence. The current all states issue listing returned only [issue 1](https://github.com/AutoSmartGroup/ESP-Fi-HAR/issues/1), asking about sampling rate, with zero comments. No author correction of label semantics was found there. The publication remains [Wen et al., 2026](https://doi.org/10.1016/j.adhoc.2026.104192); no independently inspected authoritative annotation table resolves the discrepancy. Absence of a located correction is not proof that no correction exists elsewhere.

## What renaming can and cannot fix

RuView retains `activity_1` through `activity_7`, with target indices 0 through 6. Explicitly distinguish filename activity 3 from zero based classifier column 3, which represents filename activity 4.

A single bijection applied consistently to predictions and ground truth leaves every equality comparison unchanged, hence accuracy is invariant. It permutes the confusion matrix rows and columns together, so the seven per class F1 values are reordered and their macro average is unchanged. Thus semantic renaming cannot repair poor numeric class 3 recall or improve macro F1. It can correct descriptions and names only.

This invariance does not cover inconsistent relabeling, incorrect score column alignment, or annotation mistakes affecting individual recordings. The inspected benchmark subset supports a consistent naming permutation, but does not prove that every archive recording has a correct activity annotation. RuView must not promote a named fall detector from this evidence.

## Confirmation status

The first pilot opened the original 70 recording test partition once and reported 22/70 correct, macro F1 0.288679. Subsequent model development rounds did not reopen those arrays, according to the recorded experiment history and development loader boundary. This audit performed no dataset or test tensor access and is not an independent access log attestation.

Keeping the original test closed in later runs avoids additional direct test queries; it does not make previously disclosed test results fresh confirmation evidence again. The repeatedly evaluated 70 recording development partition is adaptive research data. Frozen candidate selection within individual rounds does not erase adaptation between rounds. Its gains remain development results.

The current split tests one particular room and participant combination. Neither a rename nor another evaluation on the same development set establishes performance across new homes, devices or users.

## Decision and acceptance

Keep numeric labels until an author supplied correction or independently verified annotation record identifies the mapping and its scope across the full archive. Preserve the old source revision and explicit mapping version when that happens. Do not contact upstream or alter labels as part of this audit.

Fresh confirmation requires independently labeled, consented captures or a genuinely untouched authorized dataset under a protocol frozen before inspecting candidate results. Include new sessions, multiple participants and environments, calibrated empty periods when evaluating presence, and hashes binding annotations to recordings.

Acceptance test: consistent permutation of a saved prediction vector and its targets must leave accuracy and macro F1 unchanged. Separately, a fixed model and baseline must pass the declared comparison on fresh confirmation data. Neither condition substitutes for the other.
