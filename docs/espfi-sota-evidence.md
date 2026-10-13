# ESP-Fi HAR benchmark evidence

Review date: 2026-10-12. External results below are **CLAIMED**, not reproduced by RuView. A larger score on a different split is not evidence of superiority. No SOTA claim is authorized by this review.

## Current reference targets

| Reference | Reported result | Protocol and deployment | Comparability |
| --- | --- | --- | --- |
| TriGFormer, SSRN preprint posted 2026-09-04 | Macro F1 0.8014 for unseen environments; 0.7234 for simultaneous environment and participant shift | Source data only; no target examples or target statistics during training. Reported 333,000 parameters and 10.577 GFLOP inference assigned to a gateway | Closest research reference, but exact folds and preprocessing remain unverified |
| CSI SelfAdapt ESP32, public commit 2026-06-05, manuscript marked submitted | Accuracy 0.2512 across four environment folds; 0.3335 across eight participant folds; 0.6715 within domain | CompactCSINet, 384 frame windows, stride 96, 40 epochs, host MPS evaluation | Different metric, windows, split and averaging from RuView |
| CSI SelfAdapt personalization | Accuracy 0.5828 with 10 labeled windows per class; 0.7386 with 50 | Last block adaptation using labeled data from the target participant, averaged over eight participants and three adaptation seeds | Not source only generalization; compare in a separate calibration track |

TriGFormer separates temporal, subcarrier and local dynamics, combines amplitude with temporal differences, and uses supervised contrastive learning across source environments. Its abstract identifies local processing and temporal differences as useful contributors. These are hypotheses for controlled ablations, not a verified reproduction recipe. The SSRN page lists 20 pages. Direct access to the page and PDF returned access errors during this review; primary indexed abstract text was available. No author code repository was located. Fold membership, seeds, confidence intervals, duplicate handling and exact joint shift protocol could not be checked. [Primary abstract](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=7411437), [DOI](https://doi.org/10.2139/ssrn.7411437).

## Verified source details

The official ESP-Fi publication is Wen et al., *Ad Hoc Networks* 186, article 104192, issue date 2026-05-01. The publisher abstract establishes four environments and seven activities but does not expose a comparable numerical result. Full paper access failed; do not infer a benchmark score from its abstract. [Publisher](https://www.sciencedirect.com/science/article/abs/pii/S1570870526000582), [DOI](https://doi.org/10.1016/j.adhoc.2026.104192).

The official repository at revision `c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4` provides seven model families and sample normalization. Its README describes checkpoint selection using training accuracy and supplies no validation split. This differs from RuView's development procedure. The public archive contains 2,240 recordings; RuView verified the pinned LFS digest and quarantined two duplicate signal groups involving four recordings. The README activity number mapping conflicts with benchmark directory names, so the RuView adapter retains numeric activities. [Pinned official README](https://github.com/AutoSmartGroup/ESP-Fi-HAR/blob/c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4/README.md).

The SelfAdapt repository was inspected at revision `f9b9ac0faf7d0b033ce5b5041d6e40453fb56054`. Its source evaluation computes training normalization before carving trial 10 into validation. The held environment or participant remains excluded from this normalization, but validation statistics enter preprocessing. Its parser does not perform RuView's duplicate quarantine. Consequently even reproducing its scripts is not identical to RuView's audited intake.

SelfAdapt personalization uses target participant trials 1 through 8 for calibration and target normalization, then trials 9 and 10 for evaluation. Its prominent 0.806 int8 accuracy and 151 ms device inference result concern the three class Wallhack task, not seven class ESP-Fi. The ESP-Fi result JSON reports host MPS evaluation. Hardware claims from that paper cannot be transferred to RuView.

Pinned primary evidence:

1. [ESP-Fi source evaluation](https://github.com/Rehan000/CSI-SelfAdapt-ESP32/blob/f9b9ac0faf7d0b033ce5b5041d6e40453fb56054/scripts/eval_espfi_shift.py).
2. [Source result JSON](https://github.com/Rehan000/CSI-SelfAdapt-ESP32/blob/f9b9ac0faf7d0b033ce5b5041d6e40453fb56054/results/e3b_espfi_shift.json).
3. [Personalization code](https://github.com/Rehan000/CSI-SelfAdapt-ESP32/blob/f9b9ac0faf7d0b033ce5b5041d6e40453fb56054/scripts/adapt_espfi_person.py).
4. [Personalization result JSON](https://github.com/Rehan000/CSI-SelfAdapt-ESP32/blob/f9b9ac0faf7d0b033ce5b5041d6e40453fb56054/results/e5b_espfi_personalize.json).
5. [Full manuscript source](https://github.com/Rehan000/CSI-SelfAdapt-ESP32/blob/f9b9ac0faf7d0b033ce5b5041d6e40453fb56054/paper/main.tex).

## Decision and acceptance

Use 0.8014 and 0.7234 macro F1 as provisional external research references only. The current fixed RuView split, with one room and participant combination in the test partition, is not their benchmark. Once test feedback influences later development, that test is no longer untouched confirmation evidence.

Next action: obtain the TriGFormer protocol and freeze a comparable evaluation contract before attempting a superiority claim. Preserve a separate source only track and a labeled calibration track. Record signal digests, complete fold memberships, quarantine policy, training statistics, seed list, parameter count, latency device, memory and metrics.

Acceptance test: an independent runner reproduces both baseline and candidate under identical frozen folds and compute budgets, with test arrays inaccessible during selection, then reports paired per fold macro F1 and uncertainty. Until that passes, describe improvements only relative to the locally reproduced baseline and its stated protocol.
