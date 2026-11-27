# ADR-347: Social platform connectors and action authorization

**Status:** Proposed

**Date:** 2026-08-29

**Decision owners:** ruv and RuView maintainers

**Extends:** ADR-321 and ADR-327

**Configuration:** `harness/social-media/config/platforms.json`

## Context

The social metaharness needs a common control plane across WhatsApp, Discord,
LinkedIn, X, Reddit, Facebook, Threads, Instagram, GitHub, and Gist. These
providers do not expose a common capability or policy model. An OAuth scope is
only technical capability. It is not consent to publish, message, moderate,
delete, spend, change permissions, or optimize engagement.

The weak premise is that computer use can fill an API gap. That would turn a
provider denial, missing scope, removed endpoint, or rate limit into an
authorization bypass. LinkedIn and X explicitly restrict website automation.
Discord prohibits self bots. Reddit requires explicit approval before API data
access. Meta limits sanctioned automation to approved business, professional,
Page, and Threads surfaces. The metaharness therefore needs a fail closed route
registry before connector implementation.

## Inputs

1. A typed action intent containing principal, platform, account, operation,
target, content digest, audience, requested schedule, expiry, policy version,
and idempotency key.

2. The reviewed platform route from
`harness/social-media/config/platforms.json`.

3. Current connection state, granted product access, OAuth scopes, account
roles, consent records, provider review state, and any platform specific
preconditions.

4. Current provider quota endpoints, response headers, developer console
state, pricing or credit state, account quality state, and official policy.

5. A verified approval bound to the exact action or a signed campaign envelope
whose account, target class, content set, schedule, audience, volume, expiry,
and rollback limits include the requested action.

## Outputs

1. One route decision: `API_ALLOWED`, `ATTENDED_MANUAL`, or `DENY`.

2. For `API_ALLOWED`, a bounded official API request plan that is still subject
to ADR-327 action authorization and receipt verification.

3. For `ATTENDED_MANUAL`, instructions for the account owner. This output is
not permission for autonomous computer use.

4. For `DENY`, a terminal reason naming the missing authority, prohibited
surface, prerequisite, or policy condition. Denial occurs before browser,
computer use, MCP, API, or other network access.

5. A witnessed decision and execution receipt containing provider request and
resource identifiers without credentials or private response bodies.

## Assumptions

1. Provider policies, API products, scopes, pricing, quotas, metric definitions,
and account eligibility can change without a repository release.

2. Current provider responses and official account state are runtime authority.
Advisory numbers in the registry are never permission to exceed a lower live
limit.

3. The account owner will separately complete provider onboarding, app review,
business verification, OAuth consent, and organization or Page role grants.

4. The first implementation is proposal and read oriented. External writes
remain disabled until the relevant adapter, policy, approval, receipt,
idempotency, redaction, and provider sandbox tests pass.

5. Computer use may assist an attended OAuth or setup flow only when the
provider permits it. The human enters credentials and reviews consent on the
provider domain. The metaharness does not capture credentials.

## Decision

### 1. Three route classes are exhaustive

The registry is `SocialPlatformRegistryV1`. Its top level contains `schema`,
`version`, and a `platforms` object keyed by platform. Every platform declares
`identity_status`, `prerequisites`, `official_sources`, and `operations`. Every
operation declares `route`, `approval_required`, `conditions`, and one
`policy_source` URL so the loader can validate authority before tool selection.

Every registered platform operation has exactly one route.

`API_ALLOWED` means a documented official API path exists. It does not mean the
action may execute autonomously. Preconditions, current limits, deterministic
policy, and approvals still apply.

`ATTENDED_MANUAL` means a human performs or explicitly controls the operation
in the official interface. The metaharness may prepare a checklist or draft but
cannot synthesize credentials or silently operate the interface.

`DENY` stops before any browser, computer use, MCP, API, or other network call.
Unknown platform, unknown operation, missing policy, or registry parse failure
is `DENY`.

There is no automatic browser fallback. A denied, unavailable, rejected,
removed, or rate limited API route cannot be retried through a website, another
account, or a different connector.

### 2. Action authorization is separate from connector capability

Stats reads may execute after connection approval when they remain within the
registered account, scope, retention, and purpose. Publishing, replying,
messaging, moderating, deleting, spending, repository writes, merges, voice
recording, and permission changes require approval bound to the exact action or
to a bounded signed campaign envelope.

The executor verifies the route, intent digest, account, target, approval,
policy version, expiry, idempotency key, current live limit, and provider state
immediately before the call. A changed target, content digest, audience,
schedule, account, or provider precondition invalidates approval. Exact retries
return the prior receipt where idempotency is supported. Changed reuse denies.

OAuth scopes, Page roles, server permissions, organization administration, or
repository installation rights never replace this authorization.

### 3. Explicit high risk denials

1. Personal WhatsApp account or WhatsApp Web automation is denied. Only the
WhatsApp Business Platform is registered. Free form outbound messaging outside
the customer service window and messaging without recipient opt in are denied.

2. Discord self bots, user token automation, unapproved privileged intent data,
and Discord UI automation are denied.

3. LinkedIn browser automation and scraping are denied. Missing Community
Management product access, member scope, organization role, or API version
cannot be replaced with computer use.

4. X website scripting, automated likes, automated reply hiding, artificial
engagement, and dynamic AI generated replies without prior written X approval
are denied. Obtaining written approval requires a reviewed registry and ADR
change before the route can be enabled.

5. Reddit automated reads, stats, posts, comments, messages, and moderation are
denied while the connector status is
`disabled_pending_explicit_reddit_approval`. Devvit is evaluated first. Data
API or commercial use requires the corresponding written Reddit approval.

6. Facebook personal profile writes, removed Facebook Group publishing routes,
unnecessary full Page control, scraping, and UI automation are denied. Only
authorized Page API operations are registered.

7. Consumer Instagram writes and browser automation are denied. Only eligible
Business and Creator accounts use the official API. Business initiated direct
messages without a user initiated conversation are denied.

8. Threads UI automation and live quota bypass are denied.

9. GitHub artificial engagement, spam, branch protection bypass, and browser
fallback are denied. Gist publication of credentials, personal data, raw
transcripts, or private indexes is denied.

### 4. Mutable limits are runtime authority

Each adapter reads the provider quota endpoint or response headers and enforces
the lower of live state and any configured guardrail. Static snapshots exist
only for planning and tests.

WhatsApp uses messaging tier, quality state, WhatsApp Manager, and business use
case headers. Discord uses per route buckets, global state, `retry_after`, and
Gateway close codes. LinkedIn uses current product tier, response state, roles,
and monthly API version. X uses developer console credit state, endpoint
headers, post caps, and current automation policy. Reddit remains disabled
until approval and then uses approval terms plus OAuth headers. Facebook uses
Graph usage headers and Page state. Threads uses
`/me/threads_publishing_limit`. Instagram uses
`/content_publishing_limit`. GitHub and Gist use primary and secondary rate
limit responses plus repository rules.

No adapter rotates accounts, invokes computer use, duplicates actions, or
widens scope to avoid a provider limit.

### 5. Statistics and optimization remain bounded

WhatsApp exposes delivery state and WABA aggregates. Discord metrics are
derived only from events the bot may receive. LinkedIn metrics require approved
member or organization analytics permissions. X private metrics have a limited
age window. Reddit analytics remain unavailable while access is disabled.
Facebook statistics cover authorized Pages. Threads and Instagram insights are
account, media, scope, and metric dependent. GitHub repository traffic covers
only the previous 14 days. Gist has no registered traffic analytics API.

The normalization layer preserves provider, metric name, definition, time
window, account, source endpoint, collection time, and missingness. It never
equates similarly named metrics across providers without an explicit mapping.

Self optimization may collect approved stats, detect anomalies, draft content,
and propose bounded experiments. It cannot publish, reply, message, follow,
react, moderate, spend, change cadence beyond an approved envelope, widen
scopes, modify this route registry, weaken approval, or promote a learned policy.

### 6. No credential storage

The repository, platform registry, ADRs, prompts, transcripts, model memory,
vector indexes, logs, receipts, and analytics store contain no access token,
refresh token, password, session cookie, OAuth code, recovery code, app secret,
private key, or raw authorization header.

Configuration stores opaque secret references only. An optional GCP deployment
uses Secret Manager and a distinct least privilege service account per adapter.
The runtime resolves a secret only for the process and account that needs it,
redacts it from output, never sends it to a model, and rotates or revokes it on
provider or policy failure. Local development uses an equivalent external
secret provider, never a committed environment file.

## Official policy and API sources

WhatsApp uses the [official Cloud API collection](https://www.postman.com/meta/whatsapp-business-platform/collection/wlk6lh4/whatsapp-cloud-api),
the [Business Messaging Policy](https://business.whatsapp.com/policy), and the
[Cloud API overview](https://developers.facebook.com/docs/whatsapp/cloud-api/overview).

Discord uses [OAuth and permissions](https://docs.discord.com/developers/platform/oauth2-and-permissions),
[Gateway intents](https://docs.discord.com/developers/events/gateway),
[rate limits](https://docs.discord.com/developers/topics/rate-limits), and
[voice connections](https://docs.discord.com/developers/topics/voice-connections).

LinkedIn uses [OAuth](https://learn.microsoft.com/en-us/linkedin/shared/authentication/authorization-code-flow),
[Community Management access](https://learn.microsoft.com/en-us/linkedin/marketing/increasing-access?view=li-lms-2026-08),
the [Posts API](https://learn.microsoft.com/en-us/linkedin/marketing/community-management/shares/posts-api?view=li-lms-2026-08),
and its [automated activity policy](https://www.linkedin.com/help/linkedin/answer/a1340567/automated-activity-on-linkedin).

X uses [OAuth with PKCE](https://docs.x.com/fundamentals/authentication/oauth-2-0/authorization-code),
[post management](https://docs.x.com/x-api/posts/manage-tweets/introduction),
[rate limits](https://docs.x.com/x-api/fundamentals/rate-limits),
[metrics](https://docs.x.com/x-api/fundamentals/metrics), and the
[automation rules](https://help.x.com/en/rules-and-policies/x-automation).

Reddit uses the [Responsible Builder Policy](https://support.reddithelp.com/hc/en-us/articles/42728983564564-Responsible-Builder-Policy),
[Data API Wiki](https://support.reddithelp.com/hc/en-us/articles/16160319875092-Reddit-Data-API-Wiki),
and [Data API Terms](https://redditinc.com/policies/data-api-terms).

Facebook uses the [Pages API](https://developers.facebook.com/docs/pages-api/posts),
[Page Insights](https://developers.facebook.com/docs/platforminsights/page),
[Graph rate limiting](https://developers.facebook.com/docs/graph-api/overview/rate-limiting),
and [automated collection terms](https://www.facebook.com/legal/automated_data_collection_terms).

Threads uses Meta's [official Threads workspace](https://www.postman.com/meta/threads/overview)
and [Threads API collection](https://www.postman.com/meta/threads/documentation/dht3nzz/threads-api).

Instagram uses Meta's [official Instagram API collection](https://www.postman.com/meta/instagram/documentation/6yqw8pt/instagram-api)
and [content publishing documentation](https://developers.facebook.com/docs/instagram-platform/instagram-api-with-instagram-login/content-publishing).

GitHub and Gist use [GitHub App authentication](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/about-authentication-with-a-github-app),
[REST rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api),
[repository traffic](https://docs.github.com/en/rest/metrics/traffic),
[Gist endpoints](https://docs.github.com/en/rest/gists/gists), and the
[Acceptable Use Policy](https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies).

## Risks and mitigations

### Provider and policy drift

An action allowed by an old registry may become restricted. The adapter checks
live state, versions, scopes, and provider errors on every execution. A policy
drift detector may propose a registry update but cannot enable a route. Any
ambiguity stops writes.

### Credential compromise

One broad token could expose several accounts. Each adapter and account uses
the narrowest scopes, an external secret reference, independent runtime
identity, bounded token lifetime, redaction, and revocation monitoring.

### Policy laundering through computer use

An agent may treat manual navigation as equivalent to an API. The central gate
evaluates the route before tool selection, and all browser or computer-use
tools reject any action whose route is `DENY` or whose
`ATTENDED_MANUAL` session lacks the present human owner.

### Duplicate or stale writes

Retries can double publish or act on changed content. Action digests,
idempotency keys, expiry, current target state, provider resource identifiers,
and terminal receipts follow ADR-327. Ambiguous timeout results enter
reconciliation, not blind retry.

### Optimization induced spam or reputation loss

An engagement objective can reward prohibited or low quality behavior.
Optimization remains proposal only, excludes artificial engagement, preserves
provider specific metrics, and requires an approved experiment envelope with a
kill switch and volume cap.

### Metric mismatch

Provider metrics use different definitions and retention windows. Every datum
retains provenance and definition. Unsupported metrics are reported as
unavailable rather than inferred.

## Consequences

The same policy gate can govern API, MCP, local adapter, and attended setup
routes without pretending the platforms are uniform. Provider restrictions are
testable data rather than prompt guidance. The cost is more adapter specific
onboarding, current policy monitoring, runtime quota checks, and human approval
latency for consequential actions.

Reddit provides no automated capability until approval. LinkedIn cannot use
computer use for engagement. Several requested account types, including
personal WhatsApp, personal Facebook publishing, and consumer Instagram
publishing, are intentionally unsupported.

## Acceptance tests

1. Parse `platforms.json` and assert every operation route is exactly
`API_ALLOWED`, `ATTENDED_MANUAL`, or `DENY`. Unknown platform, unknown
operation, malformed registry, and missing route all resolve to `DENY`.

2. Request a LinkedIn browser like, X website post, Discord self bot, personal
WhatsApp message, Facebook personal profile post, consumer Instagram post,
Threads quota bypass, or GitHub branch protection bypass. Each denies before
tool selection or network access.

3. Request an X dynamic AI reply without a verified written X approval artifact.
It denies before content generation or API access.

4. Request any automated Reddit read or write with the current registry. It
denies and names `disabled_pending_explicit_reddit_approval`.

5. Make an authorized API write without an exact action approval or matching
signed campaign envelope. It denies even when the OAuth token has sufficient
scope.

6. Return a live provider quota lower than the advisory snapshot. The adapter
enforces the lower live value. Return an unavailable or ambiguous quota during
a batch write. The adapter stops writes and does not invoke computer use.

7. Replay an exact approved intent. It returns the original receipt or provider
resource reconciliation result. Reuse its identifier with changed content,
target, audience, account, or schedule. It denies.

8. Run a repository and log secret scan after OAuth, webhook, failure, and
receipt tests. It finds zero tokens, cookies, authorization codes, passwords,
private keys, raw authorization headers, or credential-bearing URLs.

9. Ask for unsupported analytics such as Gist traffic or consumer Instagram
insights. The connector reports unavailable and does not scrape or fabricate a
proxy metric.

10. Disable an API route at runtime and request the same action. The result is
`DENY`; no browser, computer-use, alternate account, or duplicate connector call
occurs.

The acceptance boundary is satisfied only when all ten tests pass with terminal
receipts and a network trace proving denied operations made no external call.
