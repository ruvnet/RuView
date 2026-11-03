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
remain provisional while that acknowledgement is pending. By default, untrained
observations stay in bounded memory and can be lost if the helper or account
runtime closes. The proposed explicit-consent recovery extension below is a
narrow local-storage exception; it is not enabled by an app update or helper flag.

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

### Proposed November 3 extension: local qualification, explicit selection and Room Save

The current app's legacy Room Save requires a background-model receipt and its
empty check. A separately trained learner cannot satisfy that contract merely
by completing capture or evaluation. Do not manufacture a legacy receipt, or
make the user repeat both workflows without explaining the missing seam.

Introduce a distinct, versioned **local operational qualification**. The server
computes it from the immutable candidate, exact retained Train lineage,
validated role-disjoint HeldOut data, managed sealed-phase history, and the
existing immutable evaluation artifact. The policy is fixed before evaluation;
clients cannot submit scores, thresholds or a claim that qualification passed.
Reuse the targets proposed in issue #2418: at least four Train and four HeldOut
cohorts for each of Empty, OccupiedStatic, OccupiedMoving and AdjacentOnly;
unconditional sensitivity at least 90% for each occupied condition; false
positive rate at most 5% for each negative condition; evaluated cohort coverage
at least 90% in every condition. Missing results, abstentions and unavailable
rates cannot be counted as correct. At the minimum cohort count these gates
require all four observations in every condition to be correctly scored.

These are operator-labelled representative-cohort checks for one installation.
They do not measure wall-time coverage, independent trial statistics, p95
decision latency, coherent source clocks or general physical accuracy. Preserve
the original diagnostic receipts' false `physically_validated`,
`independence_verified` and `adoption_authorized` fields. The separate receipt
names its narrower authority, policy hash, exact model/evaluation/data/context
hashes, label provenance, original observation bounds and evaluation time.
Older trusted managed sealed history may establish that the existing
conditions-held gate was passed; it cannot acquire invented historical
confirmation times or full-duration coverage.

Qualification never selects a model automatically. Explicit guarded selection
binds the verified owner, installation, room revision, ordered source/PHY
context, immutable model and qualification hashes. Candidate discovery,
training, evaluation, a new latest-model pointer, or consent completion cannot
switch the selected model. Keep previous selection history for explicit
rollback. A newer completed check that fails for the selected model withdraws
current authority without rewriting its historical passing receipt or silently
falling back to another detector.

Persist model and selection independently of optional capture recovery. The
24-hour reduced-sample recovery deadline does not expire a trained model or
renew its evidence. After restart/logout the original selection and evidence
times remain historical. Current use additionally requires the actual published
room binding, exact artifact/policy verification, complete fresh source windows,
and the model's existing out-of-distribution checks. Missing sources, changed
room/PHY context, corruption, failed checks or unknown model state produce an
explicit unavailable/abstaining result. They must not be rendered as an empty
room or trigger an unlabelled heuristic fallback.

Publish the selected result through a versioned binary-only evidence surface.
It grants no person count, movement class, location, pose, numeric Vitals,
HomeCore or cloud authority. Keep legacy output visibly attributable as a
comparison where compatibility requires it; selected-model clients exclusively
use the selected proof and its withdrawal state. Withhold downstream projections
whose contracts cannot preserve this binary evidence boundary.

The app stores a discriminated learner installation receipt and its own saved
timestamp/history. It does not reuse the legacy baseline's receipt or timestamp.
Room Save validates a fresh exact selection proof around durable persistence,
and reports saved progress separately from qualified/selected/current state.
Attaching a model must not republish geometry: the existing companion publish
operation increments its revision and would invalidate the candidate's binding.
Paired clients receive only the bounded read-scoped proof; qualification and
selection mutations remain with the managed owner. Any final native delivery
still requires composed tests and actual device qualification.

### Proposed November 2 extension: opt-in provisional capture recovery

The operator can explicitly choose encrypted local recovery when configuring a
run. This retains bounded **reduced magnitude training samples** in selected
representative/sealed cohorts, original source/context/role/time identities and
sequence watermarks. It excludes raw IQ/CSI packet files, camera/depth images,
teacher frames, live receiver windows and pending packet buffers. It is not a
blanket relaxation of the default no-observation-retention policy. API metadata
must disclose when derived samples persist rather than keeping a false raw-data
absence flag.

Use the existing per-owner persistent Keychain signing seed only as input to a
domain-separated HKDF purpose key bound to owner and installation. AES-256-GCM
uses a fresh nonce and authenticates the descriptor. No key enters JavaScript,
HTTP responses or diagnostics; the existing native-to-helper secret mechanism
is unchanged. A nonsecret stability marker is true only for actual persistent
Keychain sources. Missing, ephemeral or unavailable keys cannot silently fall
back to plaintext or claim recoverability. Other platforms without a qualified
private-file/key path keep the RAM-only workflow.

One owner-bound recovery slot is limited to64MiB of plaintext plus bounded
framing,0700 directories and0600 regular files, with authenticated atomic
write/fsync/rename/directory barriers. Symlinks, unsafe ownership/permissions,
wrong keys, identity/context mismatches and unknown/corrupt formats fail closed.
The original24-hour recovery-availability deadline never renews on save/read/
resume. Expiry withdraws recovery; it does **not** promise that a stopped app
can automatically erase ciphertext. The UI states that an expired encrypted
copy may remain until explicitly discarded. Authenticated expired records can
be explicitly deleted by their guarded identity; unreadable unknown files remain
blocked rather than being overwritten or silently erased.

After restart, recovery is explicit and requires the exact current room,
revision, source/PHY contract and verified owner/installation. Restore preserves
original evidence time and role isolation. A completed phase remains pending
operator conditions-held confirmation. An incomplete interrupted phase fails
and must restart; waiting through downtime never creates measured windows.
Restored live/pending buffers are empty, so new measured packets must establish
current freshness before inference. Interrupted fitting becomes an explicit
retry state, never an automatic job. A saved model is restored by its exact
immutable hash, not today's pointer. Neither recovery nor retention grants
physical qualification, statistical independence or adoption.

Serialize checkpoint writes and disable/delete under the same runtime lock,
with generation/phase guards and bounded failed-write retry. Cancellation or
discard never acknowledges deletion after I/O failure. A late writer cannot
recreate a successfully discarded copy. Saved state is shown only after durable
success; unavailable/unverifiable retained contents report `null` (unknown), not
`false` (absence), in persistence metadata. Confirmed empty/deleted state may
report false. Full account erasure still drains/stops the helper before removing
its tree; narrower checkpoint discard preserves room scans, candidates and
control/evaluation artifacts.

Software fixtures are not crash/power-loss or signed-device acceptance. The
extension remains Proposed pending privacy/maintainer review and actual Mac/
iPhone interruption, owner-isolation, expiry and retention verification.

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
- Default RAM capture has an interruption limitation; explicit recovery adds encrypted sample-retention consent and cold-restore obligations.
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
