# ADR-349: Social analytics and proposal only optimization

## Status

Proposed. Phase 1 implements strict local metric normalization and untrusted
optimization screening. It makes no live analytics request, connects no
account, dispatches no action, deploys no service, establishes no review
eligibility, and promotes no candidate.

ADR 352 governs the implemented bounded proposal only autopilot around this
evaluator. It remains proposal only and cannot establish gate authority, review
eligibility, promotion, or any external action.

## Context

Social platforms expose metrics with different names, denominators, windows,
filters, and counting rules. A view on one platform is not necessarily the
same event as a view on another. A follower, star, package download, repository
clone, or page visit is not a verified human user. Ranking raw counters across
platforms would reward incompatible measurements and invite misleading claims.

The portfolio needs analytics that support bounded learning without allowing
an optimizer to redefine a metric, select only favorable data, attest its own
gate authority, or promote its own candidate. Twenty pairs and a 5 percent
directional lift are useful Phase 1 screening floors. They are not statistical
significance or causal evidence.

## Phase 1 invariant

| Capability | Phase 1 budget |
|---|---:|
| Stored platform credentials | 0 |
| Live account connections | 0 |
| Live analytics API calls | 0 |
| Live publications or messages | 0 |
| Advertising spend | 0 USD |
| Hosted deployments | 0 |
| Established review eligibility | 0 |
| Automatic or human promotions | 0 |

Phase 1 may create normalized local records, untrusted screening evidence, and
proposal records only.

## Inputs

The implemented analytics boundary accepts:

1. A bounded metric input for one registered platform, account, and identity
   scope.

2. One explicit collection mode, connector definition version, content
   identifier and digest, source digest, provenance digest, evidence label,
   attribution window, and collection time.

3. Enumerated nonnegative counters, a complete definition for every counter,
   and one or more enumerated rate definitions.

4. `ExperimentPlanV1`, paired baseline and variant arrays, one exact
   `MetricObservationPairV1` binding per pair, the recomputed dataset digest,
   and four exact `SocialEvaluationGateV1` records.

5. Local redacted or synthetic evidence that has already passed its own
   collection, consent, retention, and identity review.

Public metrics, exports, platform labels, comments, model summaries, retrieved
memory, Ruflo outputs, gate receipts, and issuer claims are untrusted evidence.
They cannot set policy, expand identity authority, alter a frozen plan, or
authorize promotion.

## Outputs

Phase 1 produces:

1. `NormalizedMetricsV1` with a canonical `snapshotDigest` and derived rate
   values and semantics digests.

2. `FlywheelEvaluationV1` with sample statistics, paired statistics, the
   frozen metric semantics, plan, snapshot pair, and dataset bindings,
   declared gate results, and a canonical `evaluationDigest`.

3. Either `REJECT_OR_COLLECT_MORE_EVIDENCE` or
   `SCREENING_PASSED_REQUIRES_INDEPENDENT_GATE_VERIFICATION`.

Every evaluation reports `evidenceClass: UNTRUSTED_SCREENING_ONLY`,
`gateAuthorityVerified: false`, `reviewEligibilityEstablished: false`,
`promotionAuthorized: false`, and `causalClaimAllowed: false`.

No output is a live schedule, connector command, platform credential,
publication, message, spend request, deployment, approval, promotion receipt,
or statistical significance claim.

## NormalizedMetricsV1 contract

Every normalized snapshot contains these exact bindings:

| Field | Requirement |
|---|---|
| `platform`, `account`, `identityScope` | One registered exact identity binding |
| `collectionMode` | `PLATFORM_EXPORT`, `PUBLIC_PAGE`, or `SYNTHETIC_FIXTURE` |
| `connectorDefinitionVersion` | Bounded version of the collection semantics |
| `contentId`, `contentDigest` | Exact evaluated content identity and digest |
| `sourceDigest`, `provenanceDigest` | SHA 256 bindings for source and transform chain |
| `evidenceLabel` | A label compatible with the collection mode |
| `windowStart`, `windowEnd`, `collectedAt` | Normalized timestamps with a closed chronology and no future evidence |
| `qualityFlags` | A nonempty, unique set from `DELAYED`, `ESTIMATED`, `FILTERED`, `NONE`, `ROUNDED`, and `SAMPLED` |
| `counters` | One or more enumerated nonnegative safe integers |
| `definitions` | One bounded definition for every and only present counter |
| `rates` | Derived rate values with exact numerator, denominator, and semantics digest |
| `quality` | Fixed prohibitions on cross platform ranking, user inflation, and causal claims |
| `snapshotDigest` | Canonical digest of all preceding fields |

The accepted counters are clicks, comments, delivered, engagements, failed,
followers, impressions, link clicks, reach, reactions, replies, saves, sent,
shares, and views. Unknown counters and incomplete definitions fail closed.

Evidence labels cannot be laundered across collection modes:

| Collection mode | Permitted evidence label |
|---|---|
| `SYNTHETIC_FIXTURE` | `SYNTHETIC` only |
| `PLATFORM_EXPORT` | `MEASURED` only |
| `PUBLIC_PAGE` | `MEASURED` or `CLAIMED` |

A normalized record is not trusted merely because it has the right schema. The
validator regenerates every derived rate, semantics digest, quality assertion,
and snapshot digest, then requires exact canonical equality. Mutation of a
counter, definition, binding, rate, quality assertion, or digest fails.

## Exact rate semantics

Phase 1 supports only these rate names and denominator rules:

| Rate | Required denominator | Eligible numerators |
|---|---|---|
| `clickThroughRate` | impressions | clicks or link clicks |
| `deliveryRate` | sent | delivered |
| `engagementPerImpression` | impressions | comments, engagements, reactions, replies, saves, or shares |
| `engagementPerReach` | reach | comments, engagements, reactions, replies, saves, or shares |
| `failureRate` | sent | failed |
| `replyRate` | sent | replies |

The implementation derives numerator value, denominator value, and rate value.
A zero denominator produces a null rate, not zero or infinity. Its semantics
digest binds platform, collection mode, connector definition version, metric,
unit, numerator set, denominator, and the relevant counter definitions.

Aggregate and component counters cannot be summed together. Click through rate
requires exactly one of clicks or link clicks. An aggregate engagements counter
must be the only engagement numerator, and comments cannot be combined with a
replies counter that may already be included in comments. These conservative
rules reject some platform specific disjoint cases until the connector
definition proves non overlap.

## Comparison boundary

Both snapshots are fully regenerated and verified before comparison. A
comparison requires the same platform, account, identity scope, collection
mode, evidence label, connector definition version, quality flags, and window
duration. Rate comparisons also require identical semantics digests. Counter
comparisons require identical definitions.

Cross platform, cross account, cross identity, mixed collection mode, mixed
evidence, mixed quality, changed duration, changed denominator, or changed
definition comparisons fail closed. A valid result reports absolute and
relative change with `causalClaimAllowed: false`.

Followers, stars, downloads, clones, views, impressions, reach, visits, and
memberships are platform counters. They are not verified unique people and
must not be labeled users without a cited source definition. Repository clones
are events, package downloads are transfers, stars are reactions or bookmarks,
and followers are account relationships.

## ExperimentPlanV1 contract

The experiment plan has an exact closed schema:

| Field | Requirement |
|---|---|
| `platform`, `identityScope` | One bounded experiment scope |
| `objective` | One bounded objective |
| `metric` | One registered normalized rate: click through, delivery, engagement per impression, engagement per reach, failure, or reply rate |
| `metricSemanticsDigest` | Frozen semantics digest from the normalized metric definition |
| `direction` | `increase` or `decrease` |
| `minimumSamples` | Integer from 20 through 10000 |
| `minimumRelativeLift` | Number from 0.05 through 10 |
| `pairingRuleDigest` | Frozen pairing rule digest |
| `anchorSetDigest` | Frozen anchor set digest |
| `policyDigest` | Frozen policy digest |
| `registeredAt` | Normalized registration time |
| `observationStartsAt` | Normalized later observation start |

The supplied plan digest must match the canonical plan. Registration must
precede observation. Changing a threshold, direction, scope, metric, pairing
rule, anchors, policy, or time creates a different plan digest.

Baseline and variant arrays must have equal length and finite numeric pairs.
Each pair binds a unique pairing key, unique baseline and variant
`NormalizedMetricsV1` snapshot digests, and baseline and variant metric
semantics digests that exactly match the frozen plan. `datasetDigest` is
recomputed from both complete arrays and all complete observation bindings. A
caller cannot repeat one snapshot under new pairing keys or supply a favorable
digest for altered or partial observations. Snapshot bindings establish
integrity, not source authority; provenance remains an independent gate.

## SocialEvaluationGateV1 contract

Exactly four gate records are required: anchor, provenance, security, and
blocked actions. Every record binds the exact experiment plan and dataset,
contains one through 32 unique evidence digests, an issuer evidence digest,
`PASS` or `FAIL`, issue and future expiry times, and a recomputed receipt
digest. The blocked action record also contains a nonnegative blocked action
count.

Gate issue time cannot precede observation or be in the future. Expired gate
records fail, and one gate receipt cannot remain valid for more than 30 days.
A passing blocked action gate also requires a count of zero.

Receipt structure, digest, timing, and binding are verified. Issuer authority
is not. This is the central Phase 1 limitation: a syntactically valid record
cannot prove that an independent qualified reviewer produced the evidence.

## Screening calculation

The evaluator computes mean, median, and interquartile range for baseline,
variant, and paired differences. It also reports absolute mean change, relative
lift, paired wins, and paired win rate.

For an increase plan, relative lift must be at least the frozen threshold. For
a decrease plan, relative lift must be no greater than the negative threshold.
A zero mean baseline cannot pass the relative lift test. The sample floor and
all four declared gates must pass.

Even then, the result is only an untrusted signal awaiting independent gate
verification. Twenty pairs and 5 percent lift do not prove randomization,
identification, statistical significance, generality, business value, or
future performance. A causal or significance claim requires a separate
reviewed analysis plan and evidence not implemented here.

## Proposal only Ruflo and autopilot boundary

Ruflo may propose one bounded change to voice rules, content structure, timing
hypotheses, or evidence presentation. It cannot select credentials, call
connectors, change measurements, alter a plan, exclude failures, relax a gate,
rewrite policy, approve, promote, deploy, dispatch, or update canonical
learning.

ADR 352 governs a bounded and restartable proposal only autopilot. Its inputs
are 1 through 100 digest bound optimization proposals, their evaluation inputs,
a 1 through 100 cycle cap, and an optional checkpoint. The aggregate input is
bounded to 20,000 observations. Its outputs are dispositions, checkpoint state,
and a digest queue for independent verification.

A checkpoint is accepted only when its digest and run, batch, identity
registry, cursor, and processed prefix bindings verify. This structural check
does not establish checkpoint issuer authority. Every run reports
`checkpointAuthorityVerified: false`, `networkAttempted: false`,
`executionAuthorized: false`, `reviewEligibilityEstablished: false`,
`promotionAuthorized: false`, and `selfMutationAuthorized: false`.

No autopilot result can establish review eligibility or promotion, even when
the screening signal passes.

## No Phase 1 promotion state

Phase 1 has no `REVIEWED_INACTIVE` or other promotion transition. It creates no
human approval receipt and does not verify reviewer identity. A future
promotion design requires a separate accepted decision, authenticated distinct
reviewers, exact candidate and evidence bindings, expiry, role separation,
rollback, canonical learning governance, and a live rollout gate.

Independent verification is therefore a new evidence collection task, not an
approval or promotion. Its result must return through a future reviewed input
boundary before any later state transition is considered.

## Risks and controls

| Risk | Severity | Control |
|---|---|---|
| Incompatible platform counters are ranked as one score | High | Permit only fully compatible within platform comparisons |
| Followers, stars, downloads, or clones are presented as users | High | Fixed quality assertions and explicit definitions |
| Five percent lift with 20 pairs is called significant | High | Label every result untrusted screening only and forbid causal claims |
| A normalized record is mutated after digest creation | Critical | Regenerate the full record and require canonical equality |
| Optimizer changes denominators or collections | Critical | Enumerate rate semantics and bind connector version, collection mode, and definitions |
| Gate receipt structure is mistaken for issuer authority | Critical | Report `gateAuthorityVerified: false` and require independent verification |
| Unsafe actions are hidden behind a passing effect | Critical | Require all four bound gates and zero declared blocked actions |
| Ruflo or autopilot promotes its own proposal | Critical | Expose no promotion state and force all authority flags false |
| Analytics crosses identity boundaries | Critical | Resolve exact account bindings and reject cross identity comparisons |

The biggest uncertainty is gate provenance. Phase 1 can prove that a gate
record is structurally intact and bound to one plan and dataset. It cannot
prove who produced it or whether the underlying evidence was competently
reviewed. The fix path is an independently authenticated gate issuer and
evidence store under a separate accepted ADR.

## Consequences

The strict contracts reject some potentially useful comparisons when a
platform definition, quality flag, or collection mode changes. That is a
deliberate false negative. It is cheaper than converting incomparable counters
or unverified gate claims into a self reinforcing content policy.

Phase 1 external API, advertising, and deployment cost is exactly 0 USD by
design. Local compute and review time are not claimed as zero. They must be
measured before a production business case is accepted.

## Acceptance tests

Phase 1 is accepted only when all of the following pass:

1. Unknown fields, identity mismatches, forbidden sensitive material, invalid
   collection and evidence combinations, future evidence, and incomplete
   counter definitions are rejected.

2. Unsupported counters or rates, duplicate numerators, wrong denominators,
   absent counters, and mixed `NONE` quality flags are rejected.

3. Regeneration detects a changed counter, rate value, rate semantics digest,
   quality assertion, or snapshot digest.

4. Cross platform, cross account, cross identity, mixed collection mode,
   changed connector definition, mixed evidence, mixed quality, changed window
   duration, and changed metric semantics comparisons are rejected.

5. A missing or zero denominator cannot be silently converted to another
   denominator or a zero rate.

6. The experiment plan is registered before observation, meets the 20 pair and
   5 percent Phase 1 floors, and matches its supplied digest.

7. The paired arrays match in length, contain only finite values, have one
   unique snapshot and pairing binding per pair, match the frozen registered
   metric semantics, and match the recomputed dataset digest.

8. All four exact gate records are present, current, valid for no more than 30
   days, digest valid, and bound to the same plan and dataset. Any declared
   gate failure or nonzero blocked action count fails the screen.

9. Nineteen pairs fail a default plan. Twenty pairs below the exact frozen
   directional threshold fail.

10. A passing screen reports independent verification required, untrusted
    evidence, no causal claim, no established review eligibility, and no
    promotion authority.

11. Ruflo output cannot alter policy, credentials, connectors, canonical
    learning, schedules, deployment, review eligibility, or promotion state.

12. Phase 1 evidence records zero credentials, account connections, live
    analytics requests, publications, messages, spending, deployments, review
    eligibility transitions, and promotions.

One raw cross platform ranking, user inflation, denominator substitution,
tampered record, plan mutation, gate provenance overclaim, or promotion path
makes acceptance `FAIL`.
