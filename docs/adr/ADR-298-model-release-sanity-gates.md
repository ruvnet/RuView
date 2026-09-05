# ADR-298: Model release sanity gates — block degenerate and mislabeled model artifacts

- **Status**: Accepted — initial implementation (this PR)
- **Date**: 2026-08-11
- **Deciders**: ruv
- **Tags**: models, evaluation, release-gate, honesty, presence

## Context

The external review (corroborating issue 1521) showed the published presence
head is mathematically degenerate: with L2-normalized embeddings, a weight
norm ≈ 3.67 against a bias ≈ 8.19 makes the smallest possible logit positive,
so predicted presence probability is ≥ ~0.989 for every valid input — the
decision boundary is unreachable and the head is effectively constant. The
README then labeled a temporal-triplet accuracy (a representation-ordering
metric) as "presence accuracy" — a category error.

Nothing in the release path catches a constant classifier, an unreachable
boundary, or a metric-name mismatch. A machine check would have.

## Decision

Add a `model_gates` module (in `wifi-densepose-train`) plus a CI gate that,
for any classifier artifact proposed for release, fails on:

- **Constant output** — output variance below a threshold across a diverse
  probe set (including the degenerate-embedding probe from issue 1521).
- **Unreachable decision boundary** — for a normalized-embedding linear head,
  check whether `bias` sign dominates `‖weight‖` so the logit cannot change
  sign; fail if the boundary is analytically unreachable.
- **Degenerate class balance** — predicted-positive rate at/above a ceiling
  (e.g. > 99%) on a balanced probe set.
- **Missing/blank baseline** — a report without a paired mean-pose/majority
  baseline (ties into ADR-291 `EvaluationReport`).
- **Metric-name provenance** — a metric may not be surfaced under a task name
  that does not match its computed kind (temporal-triplet ≠ presence);
  enforced by making the metric carry its kind and the label derive from it.

Each gate emits a structured, human-readable failure explaining the defect and
the offending numbers.

ADR-328 adds the complementary evidence-release boundary. ADR-298 answers
"is this model artifact structurally degenerate?" ADR-328 answers "does the
submitted physical-sensing evidence support this class of public claim under a
pre-registered policy?" A model must pass both checks. Structural model health
cannot substitute for real-hardware held-out evidence, and strong benchmark
evidence cannot excuse a degenerate classifier head.

## Consequences

- The specific degenerate presence head cannot ship again, and the
  temporal-triplet-as-presence mislabel is structurally prevented.
- Some existing artifacts will fail the gate on introduction — intended; they
  should fail.
- The gate is heuristic, not a correctness proof; it catches the known
  failure shapes, not all bad models.

## Validation

- Unit tests: the issue-1521 weights fail the unreachable-boundary and
  constant-output gates; a healthy synthetic head passes; a temporal-triplet
  metric cannot be constructed with a presence label.
- `cargo test -p wifi-densepose-train`; the CI gate runs in the model-check
  workflow.
- `cargo test -p wifi-densepose-train --no-default-features sensing_claim_gate`;
  `.github/workflows/model-release-gate.yml` also evaluates committed claim
  manifests and retains machine-readable receipts.
- This ADR does **not** withdraw the already-published artifact (an
  outward-facing action requiring maintainer sign-off) — it prevents
  recurrence and documents the model-card correction.
