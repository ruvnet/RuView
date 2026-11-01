# ADR-366: CSI controls and frozen evaluation

- **Status**: proposed
- **Date**: 2026-09-29
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: csi, evaluation, controls, holdout, evidence

## Context

The public NTU-Fi HumanID experiment uses 14 classes and a custom directory
split of 238 training, 56 validation and 546 held-out recordings. It differs
from the author's folder mapping. Physical acquisition-session independence
is unknown; absence of exact duplicates cannot establish it.

**MEASURED:** each of six LSTM/GRU runs classified 56/56 validation recordings
correctly. A fixed-alpha ridge model using only mean amplitudes also scored
56/56. In a matched three-seed GRU diagnostic, raw/original, raw/shuffled and
demeaned/original inputs each scored 56/56 for every seed; demeaned/shuffled
scored 52/56, 55/56 and 56/56. Chronological motion is unnecessary for perfect
accuracy on this validation split. The physical source of the predictive signal
remains unresolved. [Measured results and reproduction receipts](../../tools/bfi_learning/RESULTS.md)

## Decision

### Separate diagnostics from claims of generalization

Use static and temporal controls before interpreting a sequence-model score.
Keep input files, optimizer budgets and seeds matched across raw/original,
raw/shuffled, demeaned/original and demeaned/shuffled conditions. Permute whole
feature rows within one recording, using source/seed information without labels.
Never mix partitions. Mark shuffled time coordinates as administrative.

Whole-recording demeaning is an offline diagnostic that sees the complete
recording. It cannot substantiate causal streaming inference. These nondefault
controls are restricted to the custom public identity protocol and cannot alter
next-observation BFI forecasting. Fit normalization on training inputs only.

Report recording-level accuracy, macro-F1, included recordings and seed variation
alongside validation cross entropy averaged over selected windows. Recording
predictions aggregate softmax probabilities across their selected windows;
the current loss is not computed from those recording aggregates. Treat window
accuracy as secondary because overlapping windows and repeated seeds on the same
recordings are not independent participant trials. Retain session-independence
limitations beside every custom-split result.

### Freeze selection before final evaluation

Keep the [predeclared control protocol](../../tools/bfi_learning/CONTROLS.md):
the twelve 50-epoch diagnostic runs are excluded from final-model selection.
Only the six original raw/original 200-epoch LSTM/GRU runs are eligible. Select
lowest window-averaged validation cross entropy, breaking ties by earlier
completion. Do not substitute recording-aggregated loss or change this rule in
response to a held-out outcome.

Freeze source, dataset, metadata, configuration and checkpoint hashes before
the scheduled final evaluation. Load frozen weights without retraining. The
trainer creates an exclusive one-use claim before holdout scoring and retains
it even if scoring fails; a failure requires explicit review, not an automatic
retry. This claim is scoped to the selected frozen-run directory; the controller
enforces one final evaluation across the task's candidate directories. Stop
tuning against that holdout after its result is inspected. At this decision's
evidence checkpoint, final holdout metrics remain absent.

### Preserve executable provenance

Use bounded numeric NPZ inputs/checkpoints with pickle disabled and validate ZIP
central-directory bounds before allocation. Verify hashes and restore recorded
transformations during frozen evaluation. The launcher checks returned seed,
protocol, configuration and `test: null` during tuning. Save incomplete runs
and denominator changes explicitly rather than substituting successful runs.

Current Python tools are bounded offline reference experiments. Production
benchmark integration continues through the Rust protocols and per-domain
scorecards of ADR-291/317. A fair state-of-the-art comparison needs matched data,
task, representation, partition, adaptation access and evaluation; this custom
CSI result establishes neither BFI identity performance nor paper reproduction.

## Alternatives

Validation accuracy alone cannot distinguish temporal learning from static
cues. Random window splits would leak recordings across partitions. Expanding
the eligible model set after diagnostics would change the predeclared selection
procedure. Retain the frozen recording split and candidate set.

## Consequences

### Positive

Controls expose when model complexity or temporal claims lack experimental
support. The final evaluation has a traceable selection rule and one-use record.

### Negative

A saturated validation set limits further optimization, and strict one-use
evaluation makes interrupted scoring an explicit recovery decision. New-session
evidence requires new data rather than additional fitting to this split.

### Neutral

The GRU has 24.8% fewer parameters than the tested LSTM, but measured peak GPU
allocation was higher and elapsed times do not isolate an architecture speedup.
Preserve these separate resource findings when comparing models.

## Validation and links

Run the focused `test_prepare.py`, `test_train.py`, `test_static_baseline.py`
and `test_run_batch.py` suites under `tools/bfi_learning`. Synthetic isolation
and restore tests complement the retained real-data receipts; neither establishes
new-session generalization.

- Extends [ADR-291](ADR-291-public-benchmark-evaluation-harness.md) and
  [ADR-317](ADR-317-benchmark-multi-domain-scorecard.md) without superseding them.
- [Comparison contract](../../tools/bfi_learning/RESEARCH.md),
  [trainer](../../tools/bfi_learning/train.py) and
  [overnight frozen-selection procedure](../../tools/bfi_learning/OVERNIGHT.md).
