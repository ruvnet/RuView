# Measured research checkpoint — September 29, 2026

The pipeline now imports public CSI and real captured BFI, trains bounded LSTM
and GRU baselines, preserves a final holdout, and records reproducible numeric
checkpoints. This is research tooling. KIT-style BFI person identification and
state-of-the-art performance have not been validated.

## Public CSI experiment

NTU-Fi HumanID, 14 published subject classes; custom split of 238 training,
56 validation and 546 sealed holdout recordings. Physical acquisition-session
separation is unknown. This differs from the author's folder mapping; see
[the comparison contract](RESEARCH.md). Each of the six architecture-comparison
runs completed 200 epochs
on the RTX 5080, using seeds 1729, 1730 and 1731. Validation loss selected each
checkpoint. The final holdout has not been scored.

| Method | Parameters | Validation recording accuracy | Interpretation |
| --- | ---: | --- | --- |
| LSTM, hidden 64, window 128 | 105,358 | 56/56 for each seed | Sequence baseline |
| GRU, hidden 64, window 128 | 79,246 | 56/56 for each seed | 24.8% fewer parameters; same recording accuracy on this validation split |
| Nearest centroid on mean amplitudes | — | 46/56 | No temporal order |
| Fixed-alpha ridge on mean amplitudes | — | 56/56 | No temporal order |

**The static ridge control matches the sequence models.** These results do not
establish that the sequence models learned gait or motion. The cause could be
body information, placement, room, device or acquisition context. Separate
sessions and deployments are needed to distinguish these explanations.

Peak PyTorch allocation was 192,955,392 bytes for LSTM and 198,683,136 bytes
for GRU. Fewer GRU parameters did not reduce measured peak allocation. GPU
context and other processes are excluded. Trainer receipt formats changed
between the LSTM and GRU runs, so their elapsed times do not isolate an
architecture speedup. No paid GPU was needed; task Vast.ai spend remains USD 0
against the authorized cumulative USD 200 cap.

### Temporal controls: completed matched comparison

Twelve further GRU runs completed the [predeclared control protocol](CONTROLS.md):
four conditions, three seeds, 50 epochs each, with the same files and model
settings. These diagnostic runs are excluded from final-model selection.

| Input | Time order | Correct validation recordings, three seeds | Mean window accuracy | Mean validation cross entropy |
| --- | --- | --- | ---: | ---: |
| Raw | Original | 56, 56, 56 out of 56 | 99.531% | 0.01855 |
| Raw | Shuffled | 56, 56, 56 out of 56 | 99.671% | 0.01028 |
| Recording mean removed | Original | 56, 56, 56 out of 56 | 96.099% | 0.14544 |
| Recording mean removed | Shuffled | 52, 55, 56 out of 56 | 96.112% | 0.12256 |

**Chronological motion is unnecessary for perfect recording accuracy on this
validation split.** Removing each recording's mean also preserves high accuracy;
other distributional information remains predictive. These controls cannot
identify its physical cause or establish generalization to new sessions.
Whole-recording demeaning is an offline operation. Windows overlap, and all
seeds share the same validation recordings.

The updated trainer's raw/original seed-1729 checkpoint tensors exactly match
the prior GRU checkpoint selected at the same epoch. This real-data parity check
supports unchanged default numerics after the input-validation changes.
Every diagnostic receipt has `test: null`; the final identity holdout is sealed.
Exact results and the plot are `reports/controls-comparison-v1.json` and `.png`
on ruvultra. `src/controls-v2/summarize_controls.py` verifies receipts and
checkpoint hashes and reproduces the summary.

## Real BFI import and prediction

The offline importer reproduced 66 and 96 unique reports from the existing
captures, separating 20/40/80 MHz feedback into 192/366/750 circular-angle
features. Report counts match the independently checked decoder results.
The full observation intervals retain 23.291- and 16.393-second quiet periods.
Relative timestamps preserve nanosecond ordering and retain the integer origin;
real feature arrays and sequence values were unchanged by that timestamp fix.

A small CPU prediction pilot used one 65-frame, 80 MHz segment, with purged
chronological partitions and four-frame input windows. It completed 20 epochs;
five validation windows remained. Raw feature MSE was **0.004081**, versus
**0.002811** for repeating the last observation. This pilot **did not beat
persistence**. Its test partition remains unscored. It proves the offline
capture-to-training path executes, not movement, identity or generalization.

### Active-traffic continuity

The new analyzer verifies the capture and traffic receipts and retains the
original full-capture metrics. During the actual approximately 30-second sender
intervals, the two captures contain 66 and 96 reports: **2.2 and 3.2 reports/s**.
Weighted occupied one-second bins cover 43.33% and 70.00% of those intervals;
maximum silence remains **8.392 and 6.437 seconds**. No reports occurred in the
following approximately 15 seconds. The continuity target still fails.
These additional rates use a different denominator; they are not an RF
performance gain. See [CAPTURE-CONTINUITY.md](CAPTURE-CONTINUITY.md).

## Public BFI activity benchmark

The [research update](RESEARCH-NEXT.md) pins a licensed public dataset and a
3.365 GB subset across three day/room setups and two device/view labels.
Day and room are confounded, and the selected filenames cover three people.
Default download scope is only 120 training/validation traces (929,362,200 bytes)
plus the dataset license card. Held-out day-5 views require an explicit option.
That default download completed in 92.9 seconds: all 120 traces and the card
passed exact size/hash checks, totaling 929,368,919 bytes. No partial files or
download lock remain, and no day-5 directory was created. The private receipt is
`data/public-bfi-c3013b3/download-receipt.json` on ruvultra.

Implemented a separate VHT MU 3x1 report-body decoder. Two independent executions
matched all **13,104 quantized angles across 14 public sample bodies**, all
control fields and signed SNR codes against tshark 4.2.2. All frame CRCs passed
separate checks. The existing HE/SU capture parser remains unchanged.
Packet admission, nonzero fragment semantics, frequency indices and the full
activity-data importer still require work; body agreement alone is insufficient
for training admission. See [PUBLIC-BFI-DECODE.md](PUBLIC-BFI-DECODE.md).

The downloader pins URLs, revision, lengths, SHA-256 and dataset license;
resumes only after rehashing completed files; retains failed partials; and bounds
transfers and receipts. It does not execute downloaded code. Source and public
packets stay in separate directories outside Git on ruvultra.

## Validation improvements

An independent Codex review prompted two fixes: NPZ central-directory bounds
are now checked before ZIP metadata allocation, and the GPU launcher verifies
the expected seed, protocol, configuration and explicit absence of test metrics.
The launcher requires Linux process groups for descendant cleanup. The learning
suite passes; its Windows symlink-creation test was skipped locally and passed
in the Linux downloader/decoder suite. No validation claim depends on that skip.

## Coordination and next work

RuFlo tracks research, implementation and independent validation roles; existing
Codex collaboration workers execute them. Cross-host federation is blocked by
paused registration and relay membership rejection. Shared-memory writes are
also blocked by the native-WAL guard. No guard was bypassed.

The hourly app automation continues tonight through 07:00 America/Toronto,
then performs the selected frozen evaluation and pauses. The twelve control runs
are complete and should not be repeated. Next priorities are repeatable local
BFI coverage, public packet admission and a complete recording-quality census
before activity modeling. See [SOTA-BFI.md](SOTA-BFI.md)
and [BFI-INTEGRATION.md](BFI-INTEGRATION.md). A fair state-of-the-art comparison
still requires matched data, representation, splits and independent evaluation.

## Reproduce and audit

Source entry points: `prepare.py`, `prepare_bfi.py`, `train.py`, `run_batch.py`,
`static_baseline.py`, and `codex_iteration.py`. Private source snapshots, exact
commands, dataset/metadata hashes and checkpoint hashes are retained on ruvultra
under `/data/scratch/ruview-overnight-20260929`.

- `reports/current-comparison.json`: all six neural runs and controls.
- `reports/lstm-validation.png`: validation curves and selected epochs.
- `runs/seed-1729-recovery-verification.json`: successful model receipt after
  fixing the supervisor's overly small results-file limit; original state kept.
- `runs/static-control-01/dependency-receipt.json`: static-control dependencies.
- `data/bfi-1000-1-relative-time/receipt.json`: updated real BFI import.
- `reports/controls-comparison-v1.json`: completed temporal diagnostics.
- `runs/controls-matrix-v1-state.json`: serial controller completion and batches.
- `reports/ruview-bfi-opt-1000-*-traffic-coverage-v2.json`: traffic intervals.
- Local `public-bfi-oracle-v1/`: frozen decoder, reproducer and oracle receipt.

Focused and existing BFI regression tests pass. Run them with:

```bash
python -B -m unittest discover -s tools/bfi_learning -v
python -B -m unittest discover -s tools/bfi -v
```

Raw samples, public subject data, checkpoints, logs and Codex transcripts remain
outside Git. No hardware settings, existing GPU services or production models
were changed during these offline experiments.
