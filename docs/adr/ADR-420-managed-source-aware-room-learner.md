# ADR-420: Managed source-aware room learning and durable candidate evidence

**Status:** Proposed
**Date:** 2026-11-01
**Deciders:** RuView maintainers and app delivery owners; privacy and reference-sensor owners for teacher admission

## Context

The managed application has an installation-bound selected-source background
detector. Its live feature path also receives multiple nodes. Receiving those
nodes does not establish that a room model was trained on each source.

Existing implementations supply substantial building blocks:

- [ADR-024](ADR-024-contrastive-csi-embedding-model.md) describes contrastive CSI
  representations; AETHER supplies the Rust transformer and projection stack.
- [ADR-027](ADR-027-cross-environment-domain-generalization.md) covers domain
  generalization. Geometry/domain adaptation requires measured inputs and a
  composed training path.
- [ADR-151](ADR-151-room-calibration-specialist-training.md) defines room
  specialists and statistical multi-node late fusion. It remains a useful
  comparative baseline with its own enrollment contract.
- [ADR-008](ADR-008-distributed-consensus-multi-ap.md) concerns distributed
  AP/server coordination. Several receivers observing one AP do not establish
  that distributed consensus has occurred.
- `ruview-unified` provides native RF records, masks and provenance;
  `ruview-groundtruth` and `ruview-fusion` provide distinct reference and claim
  fusion mechanisms. Their presence does not establish managed-server callers.

The missing seam is a managed source-preserving capture, actual learning,
account-owned persistence and evaluation lifecycle. The first target is
room-selective binary presence: empty, occupied-static, occupied-moving and
adjacent-only controls. Pose, person count and numeric Vitals require separate
acceptance.

Implementation is reviewed in [#2423](https://github.com/ruvnet/RuView/pull/2423),
with the client in
[mobile #122](https://github.com/cognitum-one/ruview-mobile/pull/122).
[Tracker #2418](https://github.com/ruvnet/RuView/issues/2418) separates source,
artifact, device and physical acceptance. This ADR and those draft PRs do not
constitute architectural acceptance or measured physical accuracy.

## Decision

### Preserve the admitted RF view

Use native `RfFrameV2` records with original IQ, validity masks and explicit
provenance. The initial learner consumes derived magnitude features only.
Preserve gain and DC information needed by the task; avoid per-window centering
that removes the evidence the model is expected to learn.

Use an opt-in convex magnitude projection. The existing cubic control path can
undershoot below zero at valid spectral nulls. Keep that control path unchanged
for comparison; bind the new projection to `usable-drop2-linear-v2` so previous
candidate preprocessing cannot be silently relabelled.

Key readiness by the reported receiver, antenna count, native bin grid, PPDU,
frequency and bandwidth. A stable arrival stream for one class cannot supply
cadence for another class later rejected by the adapter. Require both the
bounded stability window and the fresh packet window consumed by the learner.
Ignore other PHY classes from the same receiver without admitting them into the
selected view. Missing selected inputs continue to withdraw inference coverage.

The first adapter supports its explicitly recognized narrow-band layouts.
Primary-channel metadata must not be silently treated as the centre of a bonded
wide channel. Receiver timing remains receiver timing; it does not establish a
measurement clock, coherent phase, authenticated transmitter or calibrated
signal quality.

### Train a real source-conditioned candidate

Reuse the AETHER transformer, with context-ordered source conditioning,
source-routed projection heads and a room-presence head. Optimize the actual
symmetric InfoNCE and supervised BCE objectives. Require intended backbone and
projection updates; a task-head improvement or a logged loss alone is
insufficient evidence of representation learning.

Treat aligned views of the same observation as positives. Exclude sibling trials,
shared frame identities and overlapping windows from contrastive negatives.
Freeze train, validation and held-out roles before fitting. Fit normalization
profiles from Train only. Several receivers on the same AP are correlated views;
their count is not an independent-confidence multiplier.

The initial bounded optimizer/model is a vertical integration candidate. Physical
comparison determines whether it has adequate capacity, robustness and latency.
This decision does not establish equivalence to a pretrained pose model or
justify scaling capacity without new evidence.

### Keep lifecycle and authority distinct

Expose explicit configure, prepare/start, complete/cancel, train, evaluate and
resume operations through the existing protected API. Feature support in a
verified helper manifest permits the runtime flag; it grants no model authority.

Every mutation carries the observed runtime epoch, context, generation, workflow
revision and active phase identity. Compare and mutate under the same lock.
Return prediction identity and its score atomically. Client read polling and
mutation ownership must be separate so a queued poll cannot abort an action or
leave the UI busy.

Preparation is excluded from labelled collection. Completion requires the
operator's conditions-held acknowledgement. A completed recording interval can
remain provisional while that acknowledgement is pending. Untrained observations
stay in bounded memory under the current retention policy and can be lost if the
helper or account runtime closes; the UI must disclose that limitation.

Store trained candidates privately beneath the verified owner/installation
namespace, with immutable model versions, hashes and an exact-context pointer.
Preserve previous rooms/models and original provenance through cold load.
Logout retains owned records; another account cannot load them. Resume must use
the trained view rather than today's preferred, different source class.

Candidate evaluation is diagnostic. Preserve aggregate confusion, abstention and
condition metrics with exact model/context/role/data hashes and original time.
Coverage describes evaluated cohorts, not wall-time availability. Distinct
declared trial IDs do not prove statistical independence. Missing or abstained
positives cannot disappear from unconditional sensitivity.

Historical evaluation must remain historical after restart. Missing retained
Train identities cannot be converted into a newly verified lineage claim.
Corrupt evaluation evidence must be retained/quarantined without discarding a
valid candidate or manufacturing a pass. Evaluation receipts authorize neither
physical qualification nor adoption.

### Admit optional teachers through a validated reference boundary

The initial RF workflow does not activate radar, camera or LiDAR teaching.
Current receipt strings and historical geometry are insufficient admission
proofs. A server-owned registry must resolve authenticated source/session
identity, measured clock mapping, spatial registration, coverage, expiry and
revocation before constructing trusted derived teacher evidence.

Visible-region absence and whole-room absence are different capabilities.
Occlusion, missing data and out-of-view observations are UNKNOWN. Adjacent-only
labels require target-room absence and separate adjacent activity evidence.
CSI/radar-fused outputs cannot serve as independent RF ground truth.

References may teach a frozen RF input model. Phone departure ends future
teaching; RF-only inference must remain usable under its own acceptance gates.
Measured spatial benefit belongs to
[mobile #105](https://github.com/cognitum-one/ruview-mobile/issues/105), including
current anchors/transforms and aligned, whole-sequence comparisons.

### Preserve the control and require explicit adoption

Keep the selected-source detector and source-bank shadow as labelled controls.
The candidate does not replace their decisions automatically. Adopting a model
requires a separately reviewed policy and matched fresh evidence; saving a
candidate, reporting accuracy or restoring an old receipt cannot substitute.
Presence acceptance does not grant pose, count or Vitals authority.

Full local account erasure must stop the helper before deleting its owned tree.
Selective learner erasure needs its own scope, generation/cancellation fence and
writer drain before removing artifacts or RAM. Legacy calibration reset and
phone-local spatial deletion must not imply they remove server learner history.

## Options considered

| Option | Complexity and reuse | Evidence and limitation |
|---|---|---|
| Independent background models | Reuses existing detector; useful shadow control | Does not train a joint representation or establish occupied/adjacent discrimination |
| Managed source-conditioned learner | Reuses native records and real AETHER computation; adds lifecycle/evidence seams | Requires physical qualification, reference admission and explicit adoption |
| Pretrained pose backbone first | Potential transfer benefit after input/task adaptation | Cannot repair source identity, preprocessing, ownership or reference-registration gaps by itself |

Choose the managed composition as the next implementation, retaining the other
options as comparisons or later adapters. Keep new architecture review separate
from source CI, packaged native verification and end-user validation.

## Consequences

- Source admission and training attribution become explicit and testable.
- Candidate/model/evidence histories can survive sessions without renewing trust.
- Real-world preprocessing errors are visible before spending a room-capture window.
- Bounded RAM-only capture creates a deliberate interruption/restart limitation.
- Additional sources or modalities require compatible view and reference contracts.
- Model capacity, generalization and uncertainty remain empirical questions.

## Action items

- [ ] Review this decision and the managed source/client PRs.
- [ ] Qualify the delivered native artifacts and independent physical conditions.
- [ ] Complete durable evaluation integration and historical UI attribution.
- [ ] Implement fenced selective learner erasure with absence/resurrection tests.
- [ ] Agree an adoption/rollback policy before enabling candidate authority.
- [ ] Validate reference clock/coverage/registration and optional derived teachers.
- [ ] Measure spatial, multi-source and reference benefit against frozen controls.
