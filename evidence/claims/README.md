# Sensing claim manifests

Any new or modified production or safety-facing RuView sensing claim must have
a strict JSON manifest in this directory and pass the ADR-328 gate against the
committed policy. This is a forward-only control: existing unmodified README
and documentation claims are not grandfathered as valid evidence and have not
passed this gate; they require a separate evidence audit. Claim authors provide
observations and artifact digests. They cannot provide or weaken their own
thresholds.

Place manifests under `research/`, `production/`, or `safety_critical/`. CI
derives the required class from that directory and rejects a mismatched
self-declared class. Changes in the dedicated benchmark, release, and model-card
directories, plus claim-like additions detected in `README.md`, must add or
update a manifest that names the exact surface path in the same pull request.
CODEOWNERS also covers these surfaces because text classification is a
conservative lint heuristic, not semantic proof that every prose claim was
detected.

Run the gate from `v2/`:

```bash
mkdir -p ../evidence-receipts
cargo run -p wifi-densepose-train --no-default-features \
  --bin sensing-claim-gate -- \
  --manifest ../evidence/claims/<class>/<claim>.json \
  --policy ../evidence/policies/sensing-claim-policy-v1.json \
  --required-class <research|production|safety-critical> \
  --receipt ../evidence-receipts/<claim>.receipt.json
```

The process exits with code `0` only for a policy-conformant research statement.
The committed v1 policy disables production and safety claims until a real
presence reproducer, authenticated evaluator signature, and artifact retrieval
and hashing are integrated. A later reviewed policy may let structurally valid
production metadata reach `metadata_attested`, but it remains
`claim_releasable: false` and exits with code `2`. A denied claim also writes
its receipt and exits with code `2`.

## Required evidence

A policy that enables production requires all of the following:

1. `recorded_hardware` or `live_hardware` source provenance.
2. Device family, stable device identifier, firmware version, capture SHA-256,
   and independently recorded ground-truth SHA-256. Artifact roles must have
   distinct digests.
3. A held-out-environment split with disjoint train and test room identifiers,
   plus per-room sample accounting and confidence-bound metrics.
4. At least 300 test samples across at least two unseen rooms, with at least 100
   samples, 20 positive cases, and 20 negative cases in each room; per-room
   counts must sum exactly to the aggregate.
5. Exact source commit, model, split, and evaluation-report digests plus an
   exact policy-allowlisted reproducer argv and registered confidence method.
6. Capability-specific metrics whose confidence bounds, not only point
   estimates, clear the repository-owned thresholds.
7. A registered evaluator identifier and a distinct, allowlisted independent
   reviewer with a content-addressed report. These identifiers are allowlist
   checks, not proof of identity; production stays blocked until they are
   signed.
8. An exact manifest SHA-256 registered in the reviewed policy and at least one
   protected public claim-surface path. Any change to the statement, metrics,
   counts, evaluator, or artifact bindings invalidates that registration.

The v1 policy defines dormant production thresholds only for presence and
disables production and safety-critical claims entirely. Pose, vitals, and
other production claims also lack a capability policy. Reviewers must add a
real evaluator, authenticated artifact verification, and a versioned policy
before any production class can be enabled.
This gate is evidence governance, not medical, product-safety, or regulatory
certification.

Research manifests may use synthetic or simulator evidence, but a passing
receipt is marked `research_only` and cannot be promoted to production. CI
binds each manifest to the release class selected by its protected directory or
workflow argument, so a caller-classified production surface cannot self-label
its manifest as research.

Do not commit raw CSI, video, personal data, credentials, or private subject
identifiers here. Store only pseudonymous metadata and content digests.
