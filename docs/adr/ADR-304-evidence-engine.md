# ADR-304: Evidence engine — MLflow for physical sensing

- **Status**: Accepted — ADR-328 claim receipt edge implemented; signed ledger planned
- **Date**: 2026-08-11
- **Deciders**: ruv
- **Tags**: evidence, provenance, ledger, accuracy, drift, benchmark, honesty, substrate

## Context

This ADR is primitive 4 of the perception-substrate program (ADR-300) and a
central pillar of the phase-1 certificate spine. It consumes the domain state
from out-of-distribution detection (ADR-302) and the calibration age from the
calibration certificate (ADR-301), it is the store that capability certificates
(ADR-318) are minted from, and it is the accuracy source the real benchmark
service (ADR-317) reads. In phase 2 it ingests agreement reports from the
ground-truth plane (ADR-303).

The strategic assessment (ADR-300) judged this primitive **more commercially
important than another pose architecture**: what unblocks OEM and integrator
conversations is not a higher headline number but a defensible, auditable record
of how a model actually performs, per room, per device, per subject, over time.
MLflow made ML experiments trackable; physical sensing needs the equivalent for
deployed accuracy, drift, and evidence level — an append-only ledger, not a
dashboard that overwrites yesterday's number.

RuView already has the constituent evidence types; what is missing is the ledger
that unifies them per deployment context:

- RuField provenance/signature types (ADR-260/262/277/279) — the signed,
  provenance-bearing record types to reuse rather than reinvent.
- The AetherArena witness-ledger pattern (ADR-149) — an append-only,
  witness-anchored ledger of scored results, the structural template here.
- `frame::EvidenceLevel` L0–L5 (ADR-282) — the mandatory evidence tag every
  record carries.
- ADR-302 `DomainState`, ADR-137 `QualityScore`, ADR-301 certificate version
  and age — the per-inference signals to accumulate.

## Options considered

1. **Log accuracy to flat files / metrics dashboards.** Rejected: mutable,
   un-signed, un-scoped, and not comparable over time — the exact gap.
2. **Reuse a general experiment tracker (MLflow itself).** Rejected: it is
   experiment-time, not deployment-time; it has no notion of room/device/
   subject context, calibration age, evidence level, or signed provenance, and
   it would add an external service dependency contrary to the substrate's
   edge-first, dependency-light direction.
3. **A native append-only evidence ledger reusing RuField record types and the
   AetherArena ledger pattern.** Chosen.

## Decision

Build an **evidence engine**: a per-`(room, device, subject)` append-only
accuracy ledger that every model automatically writes to.

ADR-328 now supplies the first enforceable edge of this decision: a strict
claim manifest, a separately versioned policy, fail-closed production/safety
rules, and a content-addressed JSON receipt. It deliberately does **not** claim
to be the append-only signed ledger described below. Until the ledger and
ADR-319 witness anchoring land, the receipt carries artifact digests and CI
decisions, production cannot exceed `metadata_attested`, and the committed
policy denies production entirely. It cannot authorize a production claim.

### 1. The evidence record

- An `EvidenceRecord` keyed by context — space id (ADR-306), signed device id
  (ADR-305), and subject id where consented and available — carrying: model
  version; calibration certificate version and **age** (ADR-301); the ADR-302
  `DomainState` (KNOWN/DEGRADED/UNKNOWN) and its four inputs; the ADR-137
  quality signals; predictive uncertainty; and, when a reference is present
  (ADR-303), the agreement result (accuracy, false-positive rate). Each record
  carries exactly one `EvidenceLevel` (L0–L5, ADR-282).
- Records are **append-only** and signed with RuField signature types
  (ADR-260/262/277/279); the ledger is anchored in the witness chain (ADR-319),
  following the AetherArena witness-ledger pattern (ADR-149). No record is ever
  mutated in place — a correction is a new record.

### 2. Per-context accuracy accounting

- The engine maintains, per `(room, device, subject)` context: measured
  accuracy (only where an ADR-303 reference backs it — otherwise the record is
  CLAIMED/SYNTHETIC, never MEASURED), false-positive rate, drift trajectory
  (fingerprint distance over time from ADR-301), the fraction of inferences in
  each domain state, calibration age distribution, and model-version history.
- Aggregation is a pure function over the append-only log at a queried time —
  the ledger is the source of truth; summaries are derived, never authoritative
  (mirroring CLAUDE.md's "source over summaries" rule).

### 3. Honesty enforced in the record

- The engine cannot upgrade an evidence level; a level is set by the record's
  provenance at write time (synthetic input → L0/`Synthetic`; no reference →
  CLAIMED; reference + reproducer → MEASURED), reusing the ADR-282/ADR-291/
  ADR-293 constructor discipline. A benchmark or certificate reading the ledger
  gets the honest level, not an optimistic rollup.
- No benchmark numbers are invented by this ADR; it delivers the ledger and the
  accounting. Empty contexts report "no evidence," which downstream (ADR-318)
  must treat as no capability.

## Consequences

- RuView gains a single auditable answer to "how well does this model actually
  work, here, on this device, for this subject, and how fresh is the
  calibration?" — the artifact OEM/integrator diligence actually asks for.
- ADR-318 capability certificates become derivable (a certificate is a signed
  attestation over a slice of the ledger) and ADR-317 gains a real accuracy
  source per PR instead of self-reported numbers.
- The append-only, signed design has storage and key-management cost; bounded
  by per-context retention policy and by reusing the existing RuField/witness
  infrastructure rather than a new store.
- Some contexts will show sparse or unflattering evidence. Surfacing that is the
  point; the engine must never paper over a thin context with a global average.

## Validation

- ADR-328 gate: simulator evidence cannot release a production claim; held-out
  environment overlap, insufficient samples, missing thresholds, and a failed
  confidence bound each produce a deterministic denial receipt. CI evaluates
  every manifest in `evidence/claims/` against the repository-owned policy.
- `cargo test` on the evidence-engine crate — append-only invariant (no
  in-place mutation; corrections are new records); per-context aggregation math
  against fixtures; evidence-level is set by provenance and cannot be upgraded;
  signature round-trip and tamper rejection; witness anchoring; empty-context
  queries return "no evidence" not a fabricated number.
- Cross-ADR: ingests ADR-302 `DomainState` and (phase 2) ADR-303 agreement
  reports; an ADR-318 test mints a certificate from a ledger slice and an
  ADR-317 test reads accuracy from the ledger.
- Real-deployment evidence (a populated ledger from live ESP32 captures with
  ADR-303 references) is the maturity milestone and requires hardware evidence
  per CLAUDE.md; a synthetic ledger is L0 by construction.
