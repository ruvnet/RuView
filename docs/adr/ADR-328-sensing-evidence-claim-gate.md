# ADR-328: Sensing evidence and claim gate

- **Status**: Proposed, implementation in this PR
- **Date**: 2026-08-21
- **Deciders**: ruv
- **Tags**: evidence, claims, provenance, release-gate, safety, benchmark, receipt

## Context

RuView has several strong evidence primitives but no single enforceable release
boundary. ADR-291 supplies leak-free evaluation reports, ADR-295 keeps synthetic
sources from presenting as live, ADR-298 rejects degenerate model heads, and
`wifi-densepose-train::occupancy_bench` withholds claims from synthetic or mock
data. ADR-304, ADR-317, ADR-318, and ADR-319 describe the longer-term evidence
ledger, scorecard, capability certificate, and witness chain.

The immediate gap is narrower and operationally important: a production-facing
claim can still be written without submitting metadata that documents physical
hardware provenance, held-out room coverage, minimum sample size, a confidence
bound, or a pre-registered threshold. Existing checks are library APIs. They do
not produce a content-addressed decision that CI can retain.

## Decision

Add a strict, dependency-light claim gate to `wifi-densepose-train` and run it
in `.github/workflows/model-release-gate.yml`.

### Separate observations from policy

The claim author submits a JSON manifest containing:

- Claim id, capability, statement, and release class.
- Source kind: synthetic, simulator, mock, recorded hardware, or live hardware.
- For physical sources: device family, pseudonymous device id, firmware build,
  capture digest, and independent ground-truth digest.
- Evaluation protocol, train and test environment identifiers, subject
  identifiers where applicable, aggregate sample counts, and per-environment
  sample accounting.
- Aggregate and per-environment metric point estimates and confidence bounds.
- Exact source commit plus model, split-manifest, and evaluation-report digests.
- A policy-allowlisted evaluator id, registered confidence method, exact
  allowlisted reproducer argv, and optional independent-validation report
  digest.
- Exact repository paths where the statement is presented publicly.

The manifest contains no acceptance thresholds. Thresholds come from the
separate, versioned repository policy in
`evidence/policies/sensing-claim-policy-v1.json`. This prevents a claim author
from choosing a weaker rule in the same artifact being evaluated.

For production and safety classes, the policy also registers the SHA-256 of the
exact manifest bytes by claim id. This protects the statement and every bound
commit, sample count, metric, artifact, evaluator, reviewer, reproducer, and
surface path as one reviewed unit. Changing any field without updating the
reviewed registry fails `registered_manifest`.

Both inputs use strict Serde structures with unknown fields rejected and a 1 MiB
metadata-only size limit. Raw CSI, video, personal data, and credentials are not
accepted as claim metadata.

### Release classes and invariants

The gate evaluates all applicable rules and fails closed.

1. Research claims may use synthetic evidence, but a passing decision is
   `research_only`, never production.
2. The implementation can evaluate production metadata only when a reviewed
   policy enables it. Production claims require physical hardware metadata, a
   held-out-environment protocol, disjoint train and test room identifiers, at
   least 300 samples in at least two unseen rooms with 100 samples, 20 positive
   cases, and 20 negative cases per room, aggregate and per-room
   confidence-bound metrics, exact evaluation bindings, and capability-specific
   thresholds. Identifiers are canonical ASCII, trimmed, and case-folded for
   leakage comparison; whitespace, case, and Unicode aliases cannot manufacture
   a held-out room.
3. Metric thresholds are compared against the conservative confidence bound.
   A production or safety rule cannot use only a point estimate.
4. The v1 production policy covers presence only. Pose, vitals, and every other
   production capability fail closed until a reviewed policy is added.
5. Production metadata that passes every enabled rule is `metadata_attested`,
   not release-authorized. It remains `claim_releasable: false` because v1 does not
   retrieve and hash private artifacts or verify a trusted evaluator signature.
6. Production and safety-critical claims are disabled in the committed v1
   policy. Production cannot be enabled until a real presence reproducer and
   authenticated artifact-verification path exist. Enabling safety also requires
   a new reviewed policy with capability-specific thresholds, at least three
   unseen environments, at least 1,000 samples, subject-disjoint evaluation,
   real hardware evidence, and independent validation. Passing this software
   gate would still not constitute medical, functional-safety, or regulatory
   certification.

The dormant initial presence thresholds require confidence-bound ROC AUC,
sensitivity, and specificity of at least 0.90, an upper-confidence-bound
false-positive rate of at most 0.10, and at least 60 positive and 60 negative
samples. These are
minimum evidence-governance thresholds, not a claim that the current RuView
model meets them.

### Content-addressed receipt

Every well-formed evaluation emits
`ruview.sensing-claim-receipt/v1` JSON containing:

- SHA-256 of the exact manifest and policy bytes.
- Claim, capability, class, and source kind.
- One stable rule result with observed and required values per invariant.
- `metadata_attested`, `research_only`, or `denied` decision semantics plus
  separate `metadata_gate_passed` and `claim_releasable` booleans.
- SHA-256 of the canonical compact JSON receipt.

Denied evidence and production metadata both produce a receipt and exit with
code 2. Only policy-conformant research evidence exits 0. Malformed input exits
1. This gives CI and reviewers a durable, machine-readable answer without
pretending that the receipt is already a signed ADR-319 witness.

## Consequences

- Simulation can no longer release a production or safety claim, even with
  perfect submitted metrics.
- A strong point estimate with a weak lower confidence bound is blocked.
- Missing policies and missing capability thresholds are denials, not implicit
  passes.
- Once a protected caller selects production, a manifest cannot downgrade that
  decision to research: the CLI requires an exact class match.
- Production and safety manifests must list a protected public surface, and CI
  requires a changed surface to be named by a changed class-bound manifest.
- Capture and ground-truth artifacts, model/split/evaluation artifacts, and the
  independent review report must use distinct content digests for distinct
  evidence roles.
- Repository CI is evidence lint, not the external HuggingFace publication
  authority. Dedicated benchmark, release, and model-card paths require a
  class-bound manifest; claim-like README additions use a conservative keyword
  lint. CODEOWNERS covers all of those surfaces because the keyword lint is not
  semantic proof that every possible prose claim was detected. The policy
  digest is also pinned, but branch protection must require Code Owner review
  for either control to bind.
- The gate adds negligible evaluation cost for sub-1-MiB metadata. Rust compile
  time, approximately minutes on a cold runner, dominates the workflow.
- The default presence requirements will block current weak or incomplete
  evidence once production is enabled. This is intended.
- This change is prospective. It does not attest the existing numeric and
  capability statements already present in README or documentation. Those
  statements remain an explicit evidence-audit backlog; only new or modified
  claim surfaces are forced through the CI association check.

The largest residual risk is artifact authenticity. A syntactically valid
SHA-256 reference proves content identity only after the referenced artifact is
retrieved and hashed; it does not prove who captured it or whether the
ground-truth process was independent. ADR-304 and ADR-319 remain responsible
for append-only storage, signatures, authenticated identity, witness anchoring,
and offline chain verification. Until that integration lands, the gate cannot
accept a production claim; it can only issue a manual-review-required metadata
receipt.

## Validation

- `cargo test -p wifi-densepose-train --no-default-features sensing_claim_gate`
- The test suite proves: simulator production denial; research-only synthetic
  handling; environment leakage denial; minimum-sample denial; confidence-bound
  gating; missing-threshold denial; default safety denial; strict JSON; and
  deterministic receipt hashing.
- CI evaluates the committed policy against a synthetic research fixture, then
  fails an empty or improperly nested claim inventory, evaluates every JSON
  manifest in `evidence/claims/`, and uploads receipts.
- Acceptance test: a simulator manifest with perfect values exits 2 with failed
  `source_class` and `hardware_provenance` rules. Under the unit-test policy
  that enables production, real-hardware presence metadata that clears every
  registered confidence-bound rule still exits 2 as `metadata_attested` with
  `claim_releasable: false`; the committed policy denies production earlier.

## References

- ADR-291: Public benchmark evaluation harness
- ADR-295: Source provenance state machine
- ADR-298: Model release sanity gates
- ADR-304: Evidence engine
- ADR-317: Multi-domain scorecard
- ADR-318: Capability certificates
- ADR-319: Witness chain
