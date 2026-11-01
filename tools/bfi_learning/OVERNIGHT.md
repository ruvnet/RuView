# Overnight training checkpoint: September 29–30, 2026

## Latest completed iteration

The temporal-control matrix is complete: `runs/controls-matrix-v1-state.json`,
source `src/controls-v2`, summary `reports/controls-comparison-v1.json`.
Do not rerun these twelve jobs. Raw/original, raw/shuffled and demeaned/original
each scored 56/56 validation recordings for all seeds; demeaned/shuffled scored
52/56, 55/56 and 56/56. Chronology is unnecessary for the perfect validation
score. The final holdout remains unscored. These diagnostic runs are **excluded**
from tonight's final-model selection; the eligible set is the six original
200-epoch LSTM/GRU runs below. See [CONTROLS.md](CONTROLS.md).

The public BFI downloader is `src/public-bfi-v1/download_public_bfi.py`, with
its pinned manifest beside it. Default scope is train/validation only, excluding
both held-out day-5 views. Download output is `data/public-bfi-c3013b3`;
its receipt is **COMPLETED**, with 120 traces plus the card verified, no retained
partials or lock, and no held-out directory. Do not repeat the download. Preserve
`runs/public-bfi-download-v1-launch.json` and the dataset receipt. The new body-only MU 3x1
decoder matched all 13,104 sample angles against tshark in independent runs.
Do not confuse that with packet admission or activity training. Read
[RESEARCH-NEXT.md](RESEARCH-NEXT.md) and [PUBLIC-BFI-DECODE.md](PUBLIC-BFI-DECODE.md)
before extending the parser. Preserve header/FCS checks and perform the
predeclared quality census before any public BFI model scoring.

Traffic-period analysis of the two existing captures still fails continuity:
2.2/3.2 reports per second, with maximum silence 8.392/6.437 seconds. The original
full-capture rates are unchanged. Receipts are
`reports/ruview-bfi-opt-1000-*-traffic-coverage-v2.json`.

Read [RESULTS.md](RESULTS.md) first. All three LSTM and three GRU comparison
runs completed; no task GPU process remained after verification. The GRU batch
is `runs/batch-026636162ce9447aa29a980dbabdf234`; its source is `src/gru-v1`.
Do not repeat these completed runs. All identity holdout metrics remain absent.
The static mean-amplitude ridge control also scored 56/56 validation recordings,
so temporal-order or gait-learning claims are unsupported on this split.

Both existing BFI captures were converted to unlabeled numeric data. A tiny
65-frame next-observation pilot did not beat persistence. The latest importer
source is `src/bfi-offline-v2/tools/bfi_learning/prepare_bfi.py`; relative-time
real-data validation is in `data/bfi-1000-1-relative-time`. Preserve old datasets
and source snapshots. Prioritize continuity and independent-session evidence
over further fitting to the saturated identity validation set.

The user authorized implementation, public-data training on ruvultra, hourly
Codex iterations tonight, and Vast.ai only if more GPU capacity is needed,
with a cumulative USD 200 ceiling. The existing app heartbeat owns scheduling.
Its last wake is September 30 at 07:00 America/Toronto.

## Dataset and interpretation

Use NTU-Fi HumanID, with its published subject labels and CC BY 4.0 attribution.
The local experiment uses an explicit **custom archive-directory split**:
238 training files, 56 validation files, 546 held-out files, 14 classes.
Physical acquisition sessions are unknown. See [RESEARCH.md](RESEARCH.md) for
the author's reversed directory mapping and the requirements for a fair paper
comparison. Do not change the partition after inspecting results.

The first five-epoch CUDA pilot completed 1,100 optimizer steps. Its selected
epoch had 95.702% validation window accuracy against a 7.143% majority baseline.
These are MEASURED pilot results on 3,304 correlated windows from 56 files.
The test metric was absent. The pilot predates the corrected split description;
retain its original receipt, and use the corrected metadata for subsequent runs.
It is neither a paper reproduction nor BFI person identification validation.

A second five-epoch CUDA pilot used the corrected custom metadata, 128-sample
windows and a 64-unit LSTM. Its selected epoch measured 98.252% validation
window accuracy and 56/56 recording accuracy, with every validation recording
included. Peak PyTorch allocated memory was 192,955,392 bytes; reserved memory
was 213,909,504 bytes. These exclude CUDA context and other processes. Its
receipt is `runs/pilot-custom-v1/metrics.json`; the holdout remains unscored.

Archive SHA256:
`1fef4e66b088b94160b14f8b950538a5455f5a62514146e17e53d19f3732a094`.
Numeric tensor SHA256:
`f623aa62b265374171ab5b9175f0b6eff08ba4b701cbd4bd761aaaefbbca124c`.
The importer separately hashes every original source file. The independent
audit found no exact duplicate raw or decimated arrays across 840 recordings.
This does not establish independence of acquisition sessions or original crops.

## Work locations

- Trusted checkout: `C:\Users\ruv\.codex\worktrees\bfi-capture-validation\wifi-densepose`.
- Local private receipts: `C:\Users\ruv\.codex\hardware-evidence\ruview-overnight-20260929`.
- SSH host: `ruvultra`.
- Remote task root: `/data/scratch/ruview-overnight-20260929`.
- Corrected input: `data/prepared-humanid-custom-v1/dataset-000.npz` and `.json`.
- Frozen baseline source copies: `src/baseline-v1/`; bounded experiments: `runs/`.

The initial three-seed batch launch receipt is `runs/baseline-v1-launch.json`;
its state is `runs/batch-daeea1d396ed4bac9b27f59ba55e4dc8/batch-state.json`.
Recheck its state and lock before subsequent work. The trainer source digest is
`0b94c5f05b7004fcf54de0888ae1072af6148932e3198bbffa0188e0d16eca0e`.

Seed 1729 completed 200 epochs and 35,000 optimizer steps. Its batch supervisor
then rejected the 1.6 MB receipt against a 1 MB limit. The model receipt remains
valid; the supervisor's incomplete state is retained. A bounded 8 MB reader was
tested and deployed as `src/batch-v2/run_batch.py`. It resumes only seeds 1730
and 1731 against the same frozen trainer and data. Its launch receipt is
`runs/baseline-v2-continuation.json`. Do not retrain seed 1729 to repair this
supervision issue.

The original checkout has unrelated changes. Raw RF data, public subject data,
weights, transcripts, logs, and private receipts stay outside every Git checkout.

## Initial experiment

Hypothesis: longer windows and a 64-unit LSTM improve validation cross entropy
over the short-window pilot. Run three fixed seeds to inspect sensitivity.
`run_batch.py` executes seeds 1729, 1730, and 1731 serially, 128-sample windows,
stride 8, batch 64, up to 200 epochs, and a 900-second trainer deadline per seed.
It checks at least 4 GiB free on GPU 0 and applies a 1,020-second child timeout.
The exclusive batch lock and receipts prevent overlapping launches.

Acceptance: complete all three jobs with finite losses and numeric checkpoints;
report validation loss, window and recording metrics, full recording coverage,
seed variability, elapsed time and measured GPU allocation. All final-test
metrics remain absent during tuning. Record regressions as well as gains.

Before launching, verify no active task batch owns `runs/.batch.lock`. Inspect
its owner PID and state before changing a stale lock. Preserve existing GPU
services. Use an outer `timeout` for a detached batch and retain its launcher log.

## Hourly implementation and review

RuFlo swarm `swarm-1790734558308-txiuvo` tracks three roles:
`ruview-sota-research-20260929`, `ruview-model-review-20260929`, and
`ruview-training-20260929`. The existing Codex collaboration agents execute the
work; Ruflo agent registration alone does not launch an inference worker.
The research and integration notes are `SOTA-BFI.md` and `BFI-INTEGRATION.md`.

Federation checks on this run: registry access succeeded, but the app connector
returned internal errors. The direct client could not enroll because registration
returned HTTP 503, `registration paused`; authenticated channel access returned
`not a relay member`. No cross-host worker accepted a task. Ruflo memory storage
also refused a write because its SQL.js backend found active native WAL files.
Preserve that guard; private receipts and these files remain the working state.
Retry only after evidence of a changed service or writer state.

Each app wake resumes current receipts, selects one justified code or model
experiment, tests it, and saves a checkpoint. Use `codex_iteration.py` to supply
reviewed source snapshots to one bounded read-only `codex exec` process. Its
source and output bounds, event audit, timeout, private logs, and exclusive lock
are part of the review contract. The controller independently evaluates and
implements suggestions; child text is never executed automatically.

The local executable is
`C:\Users\ruv\AppData\Local\Programs\OpenAI\Codex\bin\codex.exe`.
Keep execution rules and read-only sandboxing active. The initial child was
unable to read files through shell policy, so source contents are passed on stdin.
If a review returns errors, save them and fix a concrete cause before retrying.
Use agent review and focused tests as independent evidence.

Keep the holdout sealed across hourly model choices. Select the final model by
lowest validation cross entropy, with earlier completed run as the tie break.
Freeze the dataset, configuration and checkpoint hashes before the final wake's
single `--evaluate-checkpoint` evaluation. That option loads the frozen weights
without retraining and leaves a one-use holdout receipt. Report recording-level
accuracy and coverage; window counts are not independent participant trials.
After inspecting the holdout, stop tuning against this holdout.

## Compute budget and completion

The RTX 5080 was sufficient for the pilot; initial Vast.ai spending is USD 0.
The private `cloud-budget.json` is the cumulative ledger. Before renting, verify
the current provider price, fees, active instances and remaining budget. Reserve
the maximum bounded cost, enforce automatic termination, and verify deletion of
the instance afterward. A rental is justified only by measured local capacity
limits. Only public licensed data and reviewed source may be sent to it.

At the final 07:00 wake, collect the frozen evaluation and morning summary,
terminate any owned finite jobs still running, verify paid resources stopped,
record the ledger and pause the existing heartbeat. The scheduled work ends
that morning. No claim of state of the art is supported without a matched task,
protocol, baseline and independent evaluation.
