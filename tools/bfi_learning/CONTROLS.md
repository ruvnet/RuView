# Temporal and static-feature controls

Protocol declared before running the comparison on September 29, 2026.
This is a diagnostic experiment on the existing NTU-Fi HumanID custom split.
It does not change the 238 train / 56 validation / 546 held-out recordings.

## Question and fixed comparison

The mean-amplitude ridge control already classifies all validation recordings
correctly. Compare four GRU inputs to determine whether temporal order and each
recording's feature means are necessary to retain that validation performance:

| Representation | Temporal order | Diagnostic |
| --- | --- | --- |
| Raw amplitudes | Original | Matched reference |
| Raw amplitudes | Shuffled within each recording | Remove chronology, retain the distribution of feature rows |
| Per-recording mean removed | Original | Remove the constant mean vector |
| Per-recording mean removed | Shuffled within each recording | Apply both controls |

Every condition uses seeds 1729, 1730 and 1731, GRU hidden size 64, window 128,
stride 8, batch size 64, learning rate 0.001 and 50 epochs. Each job keeps the
existing time, step, memory and output limits. Report interrupted runs as
incomplete. Validation cross entropy selects the checkpoint within each run.
The shorter budget is shared by all four conditions; it does not replace the
earlier 200-epoch architecture comparison.

Demeaning uses that complete source recording only. It is an offline diagnostic;
it cannot support a claim of causal streaming inference. Shuffling moves complete
feature rows together using a deterministic seed derived from the experiment
seed and source/session hashes. No label enters the permutation. Administrative
timestamps and sequence coordinates stay fixed and are explicitly marked as
having lost their chronological meaning. Training-only normalization follows
these transformations. Neither transformation mixes recordings or partitions.

Nondefault controls are restricted to the public NTU custom identity protocol.
They cannot be applied to next-observation BFI forecasting. Frozen evaluation
restores the recorded transformation configuration and checks its receipt.

## Evidence and interpretation

Report recording accuracy, macro F1, validation cross entropy, included
recordings, window counts and seed variability. Window accuracy is secondary:
overlapping windows are correlated. These are paired diagnostics on the same
56 validation recordings, not independent participant trials or new sessions.

Equal shuffled and original scores would show that this experiment does not
require ordered motion. A drop after demeaning would show dependence on the
removed mean vector, without identifying whether it represented bodies, rooms,
devices, placement or acquisition context. Good demeaned performance alone
does not prove gait learning: other static distributional cues may survive.

These twelve diagnostic runs are **not eligible for final-model selection**.
Keep the previously completed six raw/original 200-epoch LSTM/GRU candidates
as the eligible set for tonight's single final evaluation. Their frozen hashes
and validation results are in `reports/current-comparison.json` on ruvultra.
Select the lowest validation cross entropy, with earlier completion breaking
ties. Keep the final holdout unscored until the scheduled final wake.

## Reproduction

`run_batch.py` now accepts optional `--representation`, `--temporal-order` and
`--epochs`. Defaults retain the original 200-epoch raw/original experiment.
Launch one condition at a time on Linux; all use the same private run root and
exclusive batch lock. The launcher validates the returned seed, protocol,
configuration, input hashes and explicit absence of final-test metrics.

```bash
python tools/bfi_learning/run_batch.py \
  --source-root /private/frozen-controls-source \
  --dataset /private/dataset-000.npz --metadata /private/dataset-000.json \
  --run-root /private/runs --encoder gru --epochs 50 \
  --representation demean --temporal-order shuffle
```

Exact commands, source/data hashes, four batch states and the matrix controller
receipt remain outside Git. This source change also bounds ZIP central-directory
parsing before dataset or checkpoint loading. Synthetic tests verify malformed
inputs, control isolation and frozen restore; only completed real-data receipts
can establish experimental outcomes.
