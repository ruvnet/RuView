# ADR-419: Reproducible CSI encoder training from authorized capture manifests

Status: Proposed, implementation supplied for review

Date: 2026-10-12

Related: ADR-024, ADR-299, ADR-371, ADR-414, ADR-415

## Problem

The published `ruvnet/wifi-densepose-pretrained` v2 encoder points to a trainer
that is absent from the current repository. The existing JavaScript trainer
produces a separate artifact family. Neither an upload date nor a model card
score establishes that a newer capture is available or suitable for training.
The published metric describes temporal embeddings, not presence or pose.

## Decision

Add an isolated Python CLI at `scripts/train-csi-encoder.py`. It does not change
the npm operator surface, production sensing, or a deployed model.

1. Admit local feature captures using a versioned manifest with complete file
   SHA256, declared UTC capture date, room, session, explicit partition,
   real/synthetic evidence class, and affirmative permission to use the data.
   Permission is an operator assertion, not independently verified consent.
2. Require disjoint rooms by default, or explicitly choose disjoint sessions.
   Session separation alone is not evidence of generalization across rooms.
   Reject duplicate files, copied observations across partitions, nonfinite
   features, malformed records, and oversized input. Raw CSI requires a
   separately reviewed feature adapter; do not fabricate features or labels.
3. Fit all standardization statistics on training observations only. Train an
   8 to 64 to 128 encoder with temporal contrastive pairs. Positives are within
   two seconds of the same session and node; negatives are more than thirty
   seconds away. No arbitrary batch row becomes a negative.
4. Select the checkpoint using validation only. Evaluate the final frozen
   checkpoint and raw, standardized, and random encoder controls using the same
   test triplets. Reusing test results for another experiment invalidates an
   untouched acceptance claim and requires an external holdout policy.
5. Export safetensors, preprocessing, file hashes, actual dependency versions,
   configuration, data manifest digest, and honest metrics. Refuse output
   directory reuse. Failed or interrupted work cannot represent completion.
6. All runs retain `promotionAuthorized=false`. Temporal proxy improvements
   never automatically qualify occupancy, vitals, pose, or an edge deployment.

## Execution and boundaries

ruOS can validate manifests and execute the CPU trainer. A separately
authorized Vast machine can execute the same CLI with CUDA and a protected
local dataset mount. The harness neither rents machines nor transfers data,
reads vault credentials, publishes artifacts, or promotes candidates. A
cooperative runtime limit is not a provider billing cap or instance teardown;
operators must separately bound and end any GPU rental.

Slack can carry private handoff metadata. Public federation messages must be
limited to code coordination and public evidence. A failed coordination tool
is not a successful claim, agent execution, or training result.

## Alternatives

Extending the existing pose gate would conflate pose and embedding metrics.
Recovering deleted CSI from Git history conflicts with ADR-299's incident
boundary. An unrestricted GPU sweep before resolving provenance would spend
money without producing a defensible comparison. These options are rejected.

## Validation and acceptance

The focused unittest suite exercises actual CPU gradients, deterministic replay,
artifact loading, current collector compatibility, partition leakage, malformed
input and failure paths. Fixtures are generated temporarily and explicitly
synthetic; they establish software behavior only. No raw captures enter Git or CI.

Real model improvement remains unverified until an authorized manifest and
adequate independent capture groups are available. A separate deployment gate
must compare downstream task performance on untouched rooms, quantization
parity, and measured device RAM and latency. A 4-bit file size alone proves
none of those properties. Keep the existing deployed model for rollback.
