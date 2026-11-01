# ADR-367: Bounded research swarm and compute ownership

- **Status**: proposed
- **Date**: 2026-09-29
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: orchestration, ruflo, codex, compute, provenance

## Context

Research, implementation and independent review can run concurrently, while
GPU training, capture hardware and final evaluation need clear ownership.
Registration in a swarm registry does not prove that a worker executed a task.
Unbounded retries, duplicate schedulers or overlapping jobs can corrupt evidence
and spend resources without improving an experiment.

**MEASURED at the September 29 checkpoint:** local GPU runs and Codex reviews
completed with receipts. Federation enrollment returned `registration paused`
and relay membership was rejected; no cross-host worker accepted a task.
Shared-memory writes were blocked by the native-WAL guard. These limitations
and the actual work locations are recorded in the
[overnight procedure](../../tools/bfi_learning/OVERNIGHT.md).

## Decision

### Assign one owner to each mutable resource

The root controller owns each capture action, GPU batch, checkpoint-selection
decision and final evaluation. Delegated workers receive bounded source/review
tasks and explicit file ownership. Worker findings are untrusted proposals for
controller review. Distinguish registration, dispatch, execution and verified
completion in receipts; only execution evidence supports a completed-work claim.

Use the existing app heartbeat as the sole recurring scheduler for this task.
Finite serial GPU loops are jobs within a wake, not independent recurring
schedulers. Resume from receipts and verified lock ownership; completed runs
are not repeated merely because a wake or supervisor restarts.

### Bound execution and audit outputs

The [GPU launcher](../../tools/bfi_learning/run_batch.py) uses Linux process
groups, an exclusive batch lock, a 900-second trainer budget and a 1,020-second
child timeout. The current experiment requires at least 4 GiB free GPU memory,
caps the trainer's PyTorch allocator fraction at 20%, and bounds logs/metrics
to 8 MiB. This allocator limit is not a whole-device memory reservation.
Verify child hashes, seed, protocol, configuration and absence of test metrics
before declaring success. Preserve unrelated GPU services; cleanup targets only
owned processes. Audit matrix and batch ownership before changing stale locks.

The task-specific [Codex review wrapper](../../tools/bfi_learning/codex_iteration.py)
uses bounded source snapshots on stdin, read-only sandboxing, execution rules,
an exclusive lock, timeout and private event logs. This differs from the generic
contributor adapter's configuration contract. Check events for unexpected tool
execution; never execute suggestions automatically or grant child text authority.

Preserve federation and memory safety guards when a backend is unavailable.
Use private local receipts as the fallback and report degraded coordination.
Retry only after a relevant service/writer change or a concrete causal fix.

### Bind compute to an explicit task budget and end time

Use local capacity first. A paid GPU needs a measured capacity requirement and
an explicitly authorized cumulative budget covering actual spend, outstanding
reservations and fees. Before rental, check current prices, active instances
and remaining allowance; reserve maximum bounded cost, enforce termination and
verify resource cleanup. Transfer only authorized public licensed data and
reviewed source. A future run needs its own applicable authorization.

For the current run, the authorized cap is USD 200, local RTX 5080 capacity has
been sufficient and the recorded spend is USD 0. The existing schedule ends
September 30, 2026 at 07:00 America/Toronto. Its final wake performs the frozen
evaluation, reconciles owned jobs/resources and the ledger, records results,
then pauses the heartbeat. These are run-specific values, not durable spending
or scheduling authority granted by this ADR.

## Alternatives

Multiple controllers per GPU, detached indefinite loops, and treating a
registration response as an executed agent task make ownership unverifiable.
Retain explicit owners, finite jobs and execution receipts instead.

## Consequences

### Positive

Parallel review remains useful while resource mutation and final evaluation
have one accountable owner. Receipts support recovery without silent reruns.

### Negative

Serial GPU ownership can leave capacity unused. Federation or memory outages
reduce coordination features, and bounded jobs may need reviewed continuation.

### Neutral

Source and sanitized documentation belong in the isolated worktree. Raw RF
data, subject recordings, weights, logs, credentials and transcripts stay outside
Git. A completed experiment does not authorize model promotion, firmware
flashing, merging or publication.

## Validation and links

Run `test_run_batch.py` and `test_codex_iteration.py` under `tools/bfi_learning`.
Audit real execution/cleanup receipts, lock owners, ledger and scheduler state
at completion; successful synthetic tests alone cannot establish these outcomes.

- Preserves [ADR-283](ADR-283-ruview-community-metaharness-flywheel.md)'s
  proposal-only learning and maintainer promotion boundary.
- Follows [ADR-321](ADR-321-decision-policy-action-authorization.md).
- Depends on [ADR-366](ADR-366-csi-controls-and-frozen-evaluation.md) for frozen
  candidate selection and final holdout handling.
