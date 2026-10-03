# ADR-348: Voice and direction control

## Status

Proposed. Phase 1 validates text fixtures that represent transcripts. It does
not request microphone access, record audio, connect an account, store a
credential, dispatch an action, synthesize a custom voice, promote a learning
candidate, or deploy a service.

## Context

Voice can reduce drafting and navigation time, especially when the operator is
reviewing a portfolio across several channels. It is also an unreliable and
replayable input. Speech recognition can change names, quantities, targets,
negation, and channel selection. Recorded speech can be replayed, synthesized,
or captured from the wrong person. A spoken phrase therefore cannot prove
identity, intent, consent, or approval.

The social metaharness needs a voice and direction system that preserves the
identity, evidence, policy, approval, and receipt boundaries in ADR 345 and ADR
346. Voice is an input convenience only. It is never authority.

## Phase 1 invariant

Phase 1 has the following exact budgets:

| Capability | Budget |
|---|---:|
| Microphone permissions | 0 |
| Audio recordings | 0 |
| Persisted audio bytes | 0 |
| Custom voice samples | 0 |
| Platform credentials | 0 |
| Live account connections | 0 |
| Live dispatches | 0 |
| Learning promotions | 0 |
| Hosted deployments | 0 |

Any nonzero value fails Phase 1.

## Inputs

Phase 1 accepts only bounded local test inputs:

1. Synthetic transcript fixtures labeled `SYNTHETIC`.

2. Human typed direction fixtures that contain no credential, session, private
   message, contact graph, or private group data.

3. Tenant scoped identity and channel records accepted under ADR 345.

4. Channel specific `VoiceProfileV1` records that define tone, structure,
   evidence expectations, and prohibited behaviors for one identity and one
   channel.

5. Evidence receipts for every quantitative claim.

6. Deterministic policy versions, approval policy fixtures, adapter fixtures,
   and receipt fixtures.

A future speech input may enter only after a separate implementation gate
accepts microphone permission, transcription provider, consent, privacy,
retention, and local failure behavior. Speech, transcripts, model output,
retrieved content, platform content, MCP output, and adapter responses are
untrusted data.

## Outputs

Phase 1 may produce only local records:

1. Content lint results for bounded synthetic transcript text, including a
   content digest, warnings, and evidence errors.

2. `DirectionV1`, containing the validated intent, exact registered account
   and identity, target, policy and evidence digests, schedule, expiry, and no
   authority.

3. A deterministic platform policy decision that records the requested route,
   result, reasons, and confirms no network attempt or execution authority.

4. `ApprovalChallengeV1`, containing the exact action bindings and the future
   required factor. It always reports `voiceAcceptedAsApproval: false` and
   `authorizationCreated: false`.

5. Local audit and test fixtures. Phase 1 does not synthesize or play audio.

No output is a credential, account connection, live platform request, audio
recording, custom voice model, or evidence of real human approval.

## Assumptions

1. Transcription confidence does not establish semantic correctness.

2. A familiar voice does not establish speaker identity because replay and
   synthesis are practical.

3. Device possession alone is insufficient. Device bound approval also needs
   authenticated human presence and an exact digest preview.

4. Channel audiences expect different formats. One generic rUv voice would
   create weak content and increase identity confusion.

5. Quantitative claims have higher reputation and compliance risk than
   qualitative drafting choices.

6. A spoken confirmation can improve operator awareness but cannot replace a
   machine verifiable execution receipt.

## DirectionV1 contract

`DirectionV1` is a data record, not an approval. It contains:

| Field | Requirement |
|---|---|
| `schema` | Exact `DirectionV1` value |
| `principal` | Claimed operator identity, reported as unverified in Phase 1 |
| `source` | `text` or `voice` |
| `platform` and `account` | One registered platform and exact account |
| `identityScope` | One of personal rUv, ruvnet, Agentics, or Cognitum |
| `operation` | One exact registered operation |
| `audience` | One bounded declared audience |
| `target` and `targetDigest` | Exact target kind and identifier, including the account or feed container for a new publication |
| `contentDigest` and `claimsDigest` | Digests only; raw content and evidence records are not persisted in the direction |
| `scheduledAt` and `expiresAt` | Optional valid schedule and mandatory future expiry; schedule precedes expiry |
| `approvalRequired` and `voiceIsAuthority` | External effects require approval and voice authority is always false |
| `intentDigest` and `idempotencyKey` | Canonical binding for review and duplicate control |
| `warnings` | Bounded evidence and channel voice findings |

The validator rejects unknown schema versions, unknown fields, unsupported
intents, ambiguous identity names, missing targets for target bound actions,
multiple tenants, expired requests, invalid asset references, hidden control
characters, credential shaped values, and unbounded content.

The transcript is never executed directly. Validation creates a new canonical
record. A correction creates a new `DirectionV1` and digest.

## Deterministic control sequence

The only permitted future sequence is:

1. Speech transcription produces an untrusted transcript.

2. `DirectionV1` validation resolves one tenant, identity, channel, intent,
   target, content, assets, and claim set.

3. Deterministic policy evaluates the canonical direction and records every
   denied capability.

4. Preview displays the exact action and quantitative evidence to the human.

5. Device bound human approval binds the exact preview digest, actor, role,
   device, tenant, target, action, issued time, and expiry.

6. The adapter prepares the exact official API payload and proves that its
   digest matches the approved payload.

7. A future CLI may dispatch only that digest after every ADR 346 gate passes.

8. The adapter returns a redacted result that becomes an immutable digest
   receipt.

9. Spoken confirmation is generated only from the verified receipt and states
   the identity, channel, action, and result without exposing private content.

Phase 1 stops before step 5 real approval and has no path to steps 7 through 9
live behavior. It tests those steps with local fixtures only.

## Voice is never authority

No spoken word or transcript can approve, publish, message, delete, moderate,
follow, connect an account, select a credential, spend, deploy, change policy,
or promote learning. Phrases such as approve, send it, owner requested, urgent,
or ignore the policy remain untrusted content.

In a future phase, an approval may exist only as a device bound receipt created
after the exact preview is displayed. The device must be enrolled to the
tenant, authenticate the human through a platform protected user presence
ceremony, and sign or attest the exact preview digest. The proposed approval
expires after 2 minutes for a voice initiated flow and after any content,
identity, target, action, evidence, adapter, or policy change. Phase 1 creates
only a nonauthorizing challenge and has no device enrollment or receipt
verifier.

A spoken yes, voice match, speaker recognition score, transcript confidence,
or recorded name cannot satisfy this requirement.

## Separate rUv voice by channel

`VoiceProfileV1` belongs to exactly one tenant, identity, and channel. It is a
reviewed writing policy, not a biometric voice model and not permission to
impersonate a person.

| Channel | Default rUv direction profile |
|---|---|
| LinkedIn | Practitioner led, specific business consequence, evidence near the claim, one practical takeaway |
| X | Concise, one claim or insight, direct source when quantitative, no thread expansion without a separate proposal |
| GitHub and Gists | Technical, reproducible, versioned, explicit limitations, linked code or evidence |
| Reddit | Context first, answer the community question, disclose affiliation, avoid promotional repetition |
| Discord and WhatsApp | Community service tone, minimum necessary context, no private data reuse, no broadcast assumption |
| Instagram and Threads | Visual context, accessible description, bounded caption, explicit synthetic media disclosure where applicable |
| Facebook | No profile until ownership is accepted; no direction profile grants authority |

A proposal intended for three channels is rendered and reviewed as three
separate proposals with three digests. Text cannot be copied across channels
without revalidation against the target profile and evidence policy.

## Quantitative claim evidence

Every number that describes performance, adoption, audience, reach, cost,
latency, accuracy, revenue, safety, or comparison must use one label:

| Label | Meaning |
|---|---|
| `MEASURED` | Observed with a dated source, metric definition, denominator where applicable, and reproducer or collection method |
| `CLAIMED` | Published by an identified source but not independently reproduced |
| `SYNTHETIC` | Produced by a simulation, generated fixture, model, or hypothetical example and not observed in production |

The preview places the label, source, observation date, freshness, and material
limitation next to the claim. A `CLAIMED` or `SYNTHETIC` value cannot be
restated as `MEASURED`. Missing evidence blocks approval.

## Recorded voice and custom voice consent

Recording, retaining, cloning, adapting, or synthesizing a recognizable human
voice requires an explicit `VoiceConsentV1` from that person before collection.
Consent must identify the person, recorder, purpose, allowed providers,
training or inference use, permitted identities and channels, sample sources,
retention, geographic processing, expiry, revocation path, and whether public
disclosure is required.

Consent for transcription is not consent for custom voice training. Consent
for one channel is not consent for another. Consent for one provider is not
portable. Employment, account ownership, a public recording, or a prior post
does not imply consent.

Revocation blocks new recording, training, generation, and publication. It
also starts the provider deletion and local deletion workflow. A receipt must
record completion or an unresolved provider obligation.

Phase 1 records no audio, stores no sample, and creates no custom voice.

## Trust boundaries

| Boundary | Trusted responsibility | Untrusted input |
|---|---|---|
| Transcription | Bounded conversion to text | Audio, speaker identity, ambient speech, model confidence |
| Direction validator | Schema, tenant, channel, and target validation | Transcript and inferred intent |
| Policy engine | Deterministic decision under a pinned policy version | Direction content, retrieved instructions, model suggestions |
| Preview | Exact digest display | Summaries that omit material action details |
| Device approval | Human presence and exact digest binding | Spoken approval and transcript identity |
| Adapter | Payload preparation and future official API dispatch | Platform responses and fallback suggestions |
| Receipt ledger | Canonical digest and append only linkage | Claims about success without a matching adapter result |
| Spoken confirmation | Render only a verified receipt | Requested confirmation text from the original transcript |

No boundary may silently fall back to a weaker identity, transport, approval,
or evidence rule.

## Threats and controls

| Threat | Severity | Control |
|---|---|---|
| Transcription changes a name, quantity, negation, or target | High | Canonical validation and exact visual preview before device approval |
| Replay or synthesized speech issues a command | Critical | Voice has zero authority; require authenticated device bound approval |
| Ambient speech or a media clip triggers a direction | High | Attended activation, visible recording state in future phases, and no direct execution path |
| Prompt injection in a transcript changes policy | Critical | Direction content cannot set policy, identity, approval, credential, or adapter scope |
| One channel voice is applied to another identity or audience | High | Tenant and channel bound profiles plus separate digests |
| A quantitative claim loses its evidence label | High | Structured claim validation and preview rejection |
| Custom voice is trained or used without consent | Critical | Explicit purpose bound consent, provider allow list, expiry, revocation, and deletion receipts |
| Spoken confirmation leaks private content | High | Confirm only minimal receipt fields and suppress recipient or message content by default |
| Device approval is replayed or payload is changed | Critical | Two minute expiry, nonce, exact digest, tenant, target, action, policy, and adapter binding |

## Retention

Phase 1 retains no audio and no biometric voice data.

Future retention defaults are maximums, not minimums:

| Record | Default maximum retention | Rule |
|---|---:|---|
| Raw audio buffer | Memory only, at most 60 seconds | Delete immediately after transcription or cancellation; persistence requires separate explicit recording consent |
| Persisted consented debug audio | 7 days | Encrypted, access logged, purpose bound, and deleted earlier on revocation |
| Unapproved transcript and direction | 24 hours after rejection or expiry | Delete content while retaining a minimal noncontent denial receipt where required |
| Approved direction, policy, approval, and execution receipt | 365 days | Tenant policy may shorten this unless legal or security requirements document a longer period |
| Custom voice training sample | 0 by default | A separate accepted voice model ADR and explicit `VoiceConsentV1` are required |
| Spoken confirmation audio | 0 by default | Generate ephemerally and retain only the text receipt |

Retention jobs must be tenant scoped, fail closed on ambiguous ownership, and
produce deletion receipts. Backups and provider copies need the same expiry and
revocation mapping.

## Consequences

The system adds a preview and device approval step to every future voice
initiated external action. The two minute approval window can add operator
latency and require a repeated review when it expires. That friction limits the
much larger risk of a replayed, mistranscribed, or misdirected instruction.

Phase 1 external platform cost is exactly 0 USD because it records no audio,
uses no transcription provider, connects no account, and dispatches nothing.
Future transcription, synthesis, storage, and operator review cost must be
measured before rollout and cannot be inferred from this ADR.

## Implemented Phase 1 acceptance tests

Phase 1 is accepted only when all of the following pass:

1. The application requests no microphone permission and creates no audio or
   custom voice file.

2. A transcript containing approve, send it, ignore policy, or owner requested
   cannot create an approval, select a credential, connect an account, or reach
   an adapter execution method.

3. Unknown tenants, identities, channels, fields, intents, targets, asset
   references, expired directions, and credential shaped content are rejected.

4. Two identical direction fixtures produce the same digest. A change to one
   byte, identity, platform, target, action, claim, evidence digest, active
   policy, or expiry produces a different digest.

5. Quantitative content without an evidence record is rejected. An unknown
   grade, non HTTPS source, or invalid measurement timestamp is rejected, and a
   `MEASURED` record without a reproducer is warned. Source truth and relabeling
   detection require independent evidence review and are not established by
   the text fixture alone.

6. Approval language, a claimed speaker match, or high transcript confidence
   cannot create authorization. An `ApprovalChallengeV1` always reports
   `authorizationCreated: false`.

7. Expired directions, changed targets, identity mismatch, stale policy
   digests, unverified authority evidence, and unbound context evidence fail
   challenge validation.

8. A LinkedIn proposal cannot reuse a GitHub, Reddit, or community channel
   profile without producing a new target specific proposal and digest.

9. Package and source inspection find no microphone request, audio input,
   recording, transcription provider, custom voice sample, or audio output.

10. Sensitive value scanning rejects credentials and capability links in
    transcript text and direction inputs.

11. No spoken confirmation, device approval, adapter preparation, or dispatch
    function is exposed by CLI or MCP.

12. Phase 1 evidence records zero audio recordings, zero credentials, zero
    account connections, zero dispatches, zero messages, zero spending, and
    zero learning promotions, and zero deployments.

One path from voice, transcript, or model output to authority makes acceptance
`FAIL`.

## Future audio and device gates

The following are design requirements, not implemented Phase 1 evidence:

1. Microphone consent, capture indication, audio retention, transcription
   provider qualification, and local failure behavior.

2. Purpose bound `VoiceConsentV1`, provider deletion, revocation, and custom
   voice disclosure tests.

3. Device enrollment, authenticated human presence, nonce, two minute expiry,
   exact preview binding, replay rejection, and revocation.

4. Spoken confirmation generated only from a verified adapter receipt and
   limited to nonprivate receipt fields.

5. Tenant scoped retention and deletion receipts for transcripts, approvals,
   provider copies, and backups.

No audio, device, consent, custom voice, or live adapter claim is accepted
until a separate implementation validates these gates with real evidence.
