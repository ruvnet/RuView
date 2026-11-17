# ADR-350: Optional GCP restartable social runtime

**Status:** Proposed

**Date:** 2026-08-29

**Decision owners:** ruv and RuView maintainers

**Scope:** `harness/social-media/deploy/gcp`

**Related decisions:** ADR-321, ADR-327, ADR-347

**Deployment evidence:** No deployment was performed. This ADR and the current
Terraform assets are design and static configuration evidence only.

## Context

The optional GCP deployment is intended to keep the social metaharness
observable and recoverable without granting it social platform authority. The
requested phrase “perpetual runtime” is a false availability premise. Cloud Run
instances can restart at any time, minimum instances are a best effort target,
tokens expire, providers disconnect sessions, quotas change, and regional or
billing failures can stop capacity. A heartbeat proves only that one request
completed at one time. It does not prove continuous execution.

The safe objective is a restartable, idempotent service with durable state at
the consequence boundary. Core Phase 1 remains offline and undeployed. The
separate Phase G1 scope is a static blueprint for an internal, read only Cloud
Run health and capability service with no platform connector, OAuth callback,
webhook receiver, queue consumer, publisher, or optimizer promotion path.

The current module creates a dedicated runtime service account without project
roles, an internal ingress Cloud Run v2 service, deletion protection, a 10
second request timeout, 1 vCPU, 512 MiB, request based idle CPU behavior, zero
minimum instances by default, and three maximum instances by default. It
requires an Artifact Registry image reference ending in an immutable SHA-256
digest. An optional Cloud Scheduler job calls `/healthz` every five minutes
using OIDC and a separate service account whose only service grant is
`roles/run.invoker`.

The Terraform variable declaration rejects `maximum_instances` values above
three. The unit tested JSON plan policy additionally binds the canonical plan,
project, region, service, application image and evidence, heartbeat state, and
review timestamps. It checks an exact managed resource graph across both
`planned_values` and `resource_changes`. Terraform has not been installed or
invoked, so provider output compatibility and authenticated human review remain
unverified gates.

## Inputs

1. A dedicated Google Cloud project and reviewed region.

2. A container image built through reviewed CI and referenced by full Artifact
Registry digest, never by a mutable tag, plus a closed
`SocialImageEvidenceV1` record for its source revision, SBOM, vulnerability
scan, provenance, and evidence time.

3. For a future authorized pilot, a reviewed Terraform plan derived from
`harness/social-media/deploy/gcp`, with `minimum_instances = 0`,
`maximum_instances <= 3`, and `enable_heartbeat = false` by default. The
separate `SocialGcpPlanReviewV1` binds the canonical plan digest, project,
12 digit project number, region, service name, exact application image, canonical evidence record and
digest, expected heartbeat state and resolved URI, review time, and expiry.

4. A human deployment principal with only the roles needed to enable the
declared services, create the runtime resources, attach the service identity,
and review the plan. The deployer is separate from the runtime identity.

5. A cost estimate using the selected region, billing mode, billing account
free tier usage, expected request duration, image size, log volume, and network
path.

6. For a future adapter phase, a separate approved ADR and threat model naming
the platform, callback or worker type, account, required scopes, signature
scheme, secret references, queue, dead letter queue, idempotency store,
retention, rollback, and action authorization policy.

## Intended outputs

1. After a future authorized deployment, an internal Cloud Run service exposing
only `/healthz` and `/v1/capabilities` policy metadata.

2. A least authority runtime service account with no social platform
permission, no Secret Manager accessor role, no project editor or owner role,
and no downloaded key.

3. The internal service name, internal URI, and runtime service account email.
These outputs are identifiers, not credentials.

4. Optionally, one OIDC authenticated Scheduler heartbeat and one dedicated
heartbeat service account with only `roles/run.invoker` on the Cloud Run
service.

5. Future deployment, health, IAM, image digest, scale, cost, and rollback
evidence. None exists yet. Phase G1 static work produces no social post, reply,
message, moderation action, credential, platform token, OAuth code, platform
analytics record, or deployed resource.

## Assumptions

1. Phase G1 remains a static, internal, read only design. It has no public
callback requirement and no external platform authority.

2. The application implements `/healthz` and `/v1/capabilities` without a
database, platform call, model call, secret read, or unbounded work.

3. Request based billing and `cpu_idle = true` remain appropriate because
the intended service performs no background processing.

4. Google Cloud services, prices, free tiers, regions, quotas, IAM behavior,
and product features can change. Current Google responses and the billing
calculator are runtime and budget authority.

5. Internal ingress and IAM are both required. An internal URI is not evidence
that invocation is authorized, and `roles/run.invoker` is not evidence that a
network path is valid.

6. Artifact digest pinning prevents tag drift but does not establish build
provenance, vulnerability status, or source review by itself.

7. Future platform callbacks and continuous consumers are separate services.
They do not expand the proposed Phase G1 runtime identity.

## Decision

### 1. Phase G1 is a static internal read only capability blueprint

Deploy, if explicitly authorized later, one Cloud Run v2 service with internal
ingress, IAM authentication, deletion protection, a digest pinned image,
`minimum_instances = 0`, and `maximum_instances <= 3`. The default remains
zero minimum instances and three maximum instances. Scaling to zero accepts
cold start latency in exchange for near zero idle compute cost.

The service returns process health and static reviewed capability metadata. It
does not receive OAuth redirects or provider webhooks, open a Discord Gateway,
poll a provider, schedule content, publish, reply, message, moderate, delete,
spend, read platform secrets, or promote optimizer output.

The proposed runtime service account receives no platform permission and no
Secret Manager permission. The current required API set does not include
Secret Manager. Phase G1 Terraform accepts no platform credential and declares
no secret value or secret version.

### 2. Immutable image identity is mandatory

`container_image` must be a full Artifact Registry URI ending in `@sha256:`
followed by exactly 64 lowercase hexadecimal characters. The Terraform
variable declaration and closed JSON plan checker enforce that application
image shape and bind the plan to a separate `SocialGcpPlanReviewV1` image
digest. A tag, source deployment, or different application digest denies the
static policy check.

The Containerfile requires an operator supplied `NODE_IMAGE`, but the current
code does not verify that its value is digest pinned or bind it to reviewed
base image evidence. Base image digest, SBOM, vulnerability status, source
revision, and provenance are future build and deployment gates.

In a future pilot, the application digest must be recorded in the plan,
deployment evidence, health metadata, and rollback record. CI must also produce
the base image evidence, SBOM, vulnerability result, source revision, and
provenance attestation. None of those checks or records is implied by the
current static tests.

### 3. The review binds one canonical plan and one exact resource graph

`SocialGcpPlanReviewV1` has a closed field set. It binds `planDigest`,
`projectId`, `projectNumber`, `region`, `serviceName`, `approvedImage`,
`approvedImageEvidence`, `approvedImageEvidenceDigest`, `heartbeatEnabled`,
`heartbeatServiceUri`, `reviewedAt`, and `expiresAt`. The digest is SHA 256 over
recursively key sorted canonical JSON. `approvedImageEvidence` is a closed
`SocialImageEvidenceV1` record for the same image, source revision, SBOM,
vulnerability scan, provenance, and evidence time. Its canonical digest must
match the review and the Cloud Run template annotation. The Artifact Registry
location and project must match the reviewed region and project. The validity
window must be positive and no longer than 24 hours. Mutating any plan value
after review denies the check. The checker continues to report
`reviewAuthorityVerified: false` because structurally valid evidence and a JSON
review record cannot prove who produced or reviewed them.

`projectNumber` is a required reviewed 12 digit value. It binds the canonical
Cloud Run host to the reviewed service and region. The checker does not query
Google to prove that this number belongs to `projectId`; authenticated review
and live project inspection remain open gates.

The accepted plan contains exactly five required API resources, one control
service account, one Cloud Run service, and either zero heartbeat resources or
the exact four resource heartbeat graph selected by the review. Addresses are
unique, modes are managed, and every address has one exact resource type.
`planned_values` and `resource_changes` must contain the same graph and exact
after values. Only create, update, and no op actions are accepted. Delete,
replacement, a nonapplyable or incomplete plan, any unresolved after value, or
a sensitive value denies the check. Resource and nested configuration fields
use closed allowlists. Every planned resource and change names
the short provider `google`, while `configuration.provider_config.full_name`
must be `registry.terraform.io/hashicorp/google`. A provider schema change
therefore fails closed until reviewed.

The configuration section must contain that one unaliased root provider and
the exact seven Terraform declarations. Module calls, provisioners, unknown
configured resources, importing, generated configuration, deposed instances,
replacement paths, identity metadata, drift, and deferred changes deny the
check. This closes execution paths such as `local-exec` that are not visible in
planned resource values.

The provider block in source is empty. Provider expressions, aliases, module
providers, credentials, access tokens, impersonation, and custom endpoints deny
the plan. The plan variables map is closed to the eight declared inputs and
binds project, region, service, image, image evidence, heartbeat state, and
scale to the reviewed and planned values. Extra variables deny the check.

The canonical JSON digest does not cryptographically bind the binary saved plan
that `terraform apply` consumes. A future authorized deployment retains and
hashes the binary plan, derives reviewed JSON from that exact artifact,
authenticates both digests, and applies only the retained binary. Current
checker output is static policy evidence and never deployment authorization.

The Cloud Run service is bound to the reviewed project, region, and name. It
must retain internal ingress, deletion protection, the deterministic control
identity, a 10 second timeout, integer scale from zero through three, one
reviewed container, no environment or volume, exactly 1 CPU and 512Mi memory,
idle CPU, and the exact `/healthz` startup probe on port 8080. This is stricter
than digest review alone and prevents a reviewer record for one image from
authorizing a different runtime shape. Traffic to an older revision, disabled
invoker IAM, a command or argument override, VPC access, custom audience,
runtime port, or unreviewed field denies the check.

### 4. A heartbeat detects availability but does not create it

The Scheduler heartbeat remains disabled by default. When enabled through an
explicit plan review, a dedicated service account receives only
`roles/run.invoker` on the named service. Scheduler obtains an OIDC token whose
audience is the Cloud Run service URI and calls only `GET /healthz`.

The heartbeat does not keep state alive, prevent instance termination, prove
all routes are healthy, or make the service perpetual. Alerting must distinguish
a missed heartbeat, authentication failure, internal ingress failure, cold
start, application failure, and Scheduler failure. The selected project and
region must demonstrate that the Scheduler request can traverse the configured
internal ingress path before the heartbeat is treated as evidence.

When enabled, the checker requires the exact reviewed project, region, service
name, service specific invoker binding, Scheduler API, job description, five
minute schedule, UTC time zone, GET method, resolved Cloud Run service URI plus
`/healthz`, OIDC service account, and audience. Headers, body, OAuth target
credentials, a public member, a different URI, or an unresolved service URI
deny the plan. Paused state, retry configuration, or a changed attempt deadline
also deny it. `heartbeatServiceUri` separately attests the exact resolved URI,
whose host must bind the reviewed service name and region. The initial plan
keeps heartbeat disabled. If a new service URI
is unresolved during create, heartbeat enablement is reviewed only in a second
plan after the service exists.

### 5. Future public callbacks are separate from the internal service

OAuth redirects and provider webhooks require a separate narrowly public Cloud
Run service or equivalent verified endpoint. They never make the Phase G1
service public.

The callback service validates host, method, content type, body size,
timestamp, nonce, provider account, event type, and the provider signature over
the exact raw body before deserialization, enqueue, or side effect. OAuth state,
PKCE verifier binding, redirect URI, code single use, and expiry are verified
before token exchange. Invalid or replayed callbacks return a bounded error and
produce no queue item.

Each adapter has its own service account. A WhatsApp adapter cannot read an X
secret, a Discord consumer cannot publish to LinkedIn, and a callback service
cannot merge GitHub content. IAM grants only the exact queue, secret reference,
database namespace, and outbound capability required by that adapter.

### 6. Future credentials are external references, never Terraform values

Future adapter configuration stores Secret Manager resource references only.
Token values, app secrets, signing secrets, private keys, cookies, OAuth codes,
and authorization headers never enter Terraform state, variables, outputs,
container images, environment examples, logs, traces, receipts, prompts, model
context, or repository files.

Secret creation, version addition, rotation, disablement, destruction, and
break glass access are separately authorized operations outside this Phase G1
module. Runtimes use attached service identities, never downloaded service
account keys. Each adapter receives Secret Manager access only to its named
secret resources and only when its separate ADR is accepted.

### 7. Future work is queue backed and restartable

Provider events and approved actions enter a durable queue before processing.
Each adapter has a dead letter queue, maximum delivery attempts, bounded retry
schedule, expiry, and operator replay procedure. A transactional idempotency
record binds platform, account, operation, target, content digest, approval,
nonce, and provider resource identifier.

Delivery is treated as at least once. A timeout after a provider request enters
reconciliation against provider state. It is not blindly retried. Duplicate
delivery returns the original terminal result when the intent is identical and
denies when an identifier is reused with changed content or target.

Continuous consumers such as a Discord Gateway connection use a Cloud Run
worker pool or another explicitly restartable consumer, not the Phase G1 request
service. Consumers reconnect with bounded jitter, restore only validated
checkpoint state, and tolerate replacement at any time.

Every worker handles `SIGTERM`, stops accepting new work, extends or releases
the queue lease as supported, writes a bounded checkpoint, flushes the decision
receipt without secrets, and exits within the platform termination window. A
checkpoint is an optimization. Correctness remains in the durable queue,
idempotency store, and provider reconciliation.

## Security boundary

### Identity and IAM

The deployment principal, proposed Phase G1 runtime, heartbeat caller, public callback,
and each future adapter use distinct service accounts. No runtime has project
owner, project editor, service account key administrator, broad token creator,
or wildcard Secret Manager access. The proposed Phase G1 runtime has no platform scope
and no permission to mutate its own IAM, image, scale, ingress, or Scheduler.

### Network

The Phase G1 design uses internal ingress and authenticated invocation. No `allUsers` or
`allAuthenticatedUsers` Cloud Run binding is allowed. A future public callback
exposes only required callback paths, enforces TLS and bounded requests, and
performs provider signature verification before enqueue. Administration,
capabilities, metrics, debugging, and health details remain private.

### Data and logs

Health and capability responses contain no credential, account token, private
content, personal data, raw webhook body, prompt, transcript, or model memory.
Structured logs allowlist fields and redact authorization, cookies, query
tokens, callback codes, signatures, message bodies, and provider payloads.
Retention, sinks, and access are reviewed because free log capacity is not a
reason to retain sensitive data.

### Supply chain

The application image URI is required to be digest pinned by Terraform source
and the closed plan checker. Base image pinning is an operator build
requirement that is not yet enforced or evidenced. A future CI and deployment
gate must bind source revision, dependency lock state, base and application
images, SBOM, scan result, provenance, and reviewer. The proposed runtime
identity has no declared image push or replacement permission, but effective
IAM has not been verified in GCP.

### Action authority

Cloud deployment does not change ADR-347 routes or ADR-327 authorization.
Network reachability, a secret reference, an OAuth scope, queue delivery, or a
valid provider signature is not permission to publish. Each consequential
action still needs a registered route, exact approval, live platform policy and
quota checks, idempotency, and a witnessed receipt.

## Current rough cost bands

These are planning bands in USD using prices viewed on 2026-08-29. Google list
prices and free tier descriptions are external `CLAIMED` evidence. The
calculations and workload bands below are `SYNTHETIC` until an authorized
deployment produces a billing export. They are not quotes or `MEASURED` cost.
Region, currency, billing mode, free tier already consumed by other projects,
request duration, concurrency, logs, image size, network egress, Secret Manager
operations, and future platform traffic can materially change the total. The
operator must use the current Google Cloud calculator before approval.

### Phase G1 scale to zero estimate

With zero minimum instances, low internal traffic, heartbeat disabled, one
small image, and modest logs, the `SYNTHETIC` band is **$0 to $5 per month**. Cloud
Run request based billing includes monthly free allocations of 180,000 vCPU
seconds, 360,000 GiB seconds, and two million requests in `us-central1` pricing.
The first 0.5 GiB month of Artifact Registry storage and first 50 GiB per
project month of standard Cloud Logging storage are currently free. Costs can
exceed this band through image storage, builds, vulnerability scanning, logs,
networking, or shared billing account free tier exhaustion.

An optional five minute heartbeat makes about 8,640 requests in a 30 day month,
well below the Cloud Run request free allocation if it is otherwise available.
Cloud Scheduler currently includes three jobs per billing account per month and
then charges $0.10 per job per 31 days. Execution compute and network remain
separate.

### Warm Phase G1 estimate

Keeping one request based 1 vCPU and 0.5 GiB minimum instance idle for a 30 day
month has a `SYNTHETIC` calculation from current `us-central1` list prices of approximately
**$9.72 per month before discounts and other charges**:

`2,592,000 seconds × ($0.0000025 idle CPU + 0.5 × $0.0000025 idle memory)`

Three identical idle minimum instances are approximately **$29.16 per month**
before active requests, network, logs, and other services. The Phase G1 default
remains zero because a warm instance reduces latency but does not guarantee
availability. Google states that minimum instances can restart at any time and
recommends three only when the latency and availability value justifies the
cost.

### Future queue and worker phase

Google’s current Cloud Run pricing example is `CLAIMED` evidence for one continuously running
1 vCPU and 512 MiB worker pool instance in `europe-west1` at **$11.61 per month
with the illustrated free tier and $16.83 without it**. Three comparable
workers therefore provide a `SYNTHETIC` order of magnitude planning band of roughly
**$35 to $51 per month for compute alone**. Region and current pricing remain
authoritative.

Cloud Tasks currently provides the first one million billable operations per
month free and then charges $0.40 per million, with operations chunked at 32
KiB. Pub/Sub currently provides the first 10 GiB of basic throughput per billing
account month free and then charges $40 per TiB, with storage and cross region
transfer separate. A low volume future adapter stack with one continuous worker,
public callback, queue, idempotency store, logs, secrets, and modest egress has
a `SYNTHETIC` planning band of roughly **$20 to $75 per month**. Multiple
adapters, three warm workers, high log volume, cross region traffic, NAT,
database capacity, or high egress have a `SYNTHETIC` planning band of **$75 to
$250 or more per month** until measured.

Official sources:

1. [Cloud Run pricing](https://cloud.google.com/run/pricing)

2. [Cloud Run minimum instances](https://docs.cloud.google.com/run/docs/configuring/min-instances)

3. [Cloud Scheduler pricing](https://cloud.google.com/scheduler/pricing)

4. [Artifact Registry pricing](https://cloud.google.com/artifact-registry/pricing)

5. [Cloud Logging pricing](https://cloud.google.com/logging/pricing)

6. [Cloud Tasks pricing](https://cloud.google.com/tasks/pricing)

7. [Pub/Sub pricing](https://cloud.google.com/pubsub/pricing)

## Rollout

### Phase G1: Static blueprint and closed plan policy

1. The current source declares the exact five required APIs, one dedicated
runtime service account, one internal Cloud Run service, and an optional all or
none heartbeat set.

2. The unit tested plan checker requires a separate current
`SocialGcpPlanReviewV1` that binds the canonical plan digest, exact project,
12 digit project number, region, service, application image and canonical
evidence record, heartbeat state and resolved URI, and a maximum 24 hour review
window. It checks unique addresses, exact resource types,
`planned_values` and `resource_changes` parity, no destructive action, and no
unresolved or sensitive plan value. It also requires an applyable, complete,
nonerrored plan, the exact closed variables map, the official Google provider,
and zero modules or provisioners.

3. The exact Cloud Run gate checks internal ingress, deletion protection,
deterministic runtime identity, timeout, scale bounds, one container, image,
empty environment and volumes, CPU, memory, idle CPU, and startup probe. The
exact optional heartbeat gate checks project, region, service, IAM, Scheduler
API, job identity, description, schedule, UTC, GET, service URI, `/healthz`,
OIDC caller and audience, active state, no retry override, and empty headers and
body. Closed field allowlists reject provider schema drift and executable or
network overrides including traffic, command, arguments, and VPC access.

4. The Terraform variable declarations require integer scale bounds from zero
through three, maximum at least one, minimum no greater than maximum, and a full
Artifact Registry application image digest URI.

5. Terraform format, initialization, provider lock, validation, and a real plan
remain open because Terraform is unavailable in the validated environment.
The container build and base image evidence remain open because no usable
container daemon was available. Effective IAM, internal connectivity, and
endpoint behavior in GCP are also unverified.

No container was built and no GCP deployment occurred as part of this ADR.

### Phase G2: Explicitly authorized internal pilot

1. A human reviews project, region, image digest, IAM, APIs, scale, ingress,
deletion protection, expected cost, alert budget, and rollback revision.

2. Deploy with `minimum_instances = 0`, `maximum_instances = 3`, and heartbeat
disabled. Capture image, revision, IAM, ingress, and scale evidence.

3. Invoke both routes through an authorized internal identity. Verify an
unauthorized identity, public path, unknown path, non GET method, and oversized
request fail closed.

4. Observe cold start, latency, error, instance, log, and cost behavior for at
least 24 hours before considering the heartbeat.

5. If heartbeat value exceeds its added IAM and diagnostic complexity, enable
the single OIDC Scheduler job through a second reviewed plan. Test internal
network reachability and distinguish Scheduler failure from service failure.

### Future adapter phases

Each public callback or worker receives a separate ADR, project plan, service
account, secret references, signature test vectors, queue and dead letter
policy, idempotency and reconciliation tests, provider sandbox evidence, cost
budget, and rollback. No Phase G1 IAM role or service is mutated into a broad
multi platform connector.

## Rollback

1. For an application regression, stop traffic to the failing revision and
restore the last reviewed digest. Do not roll back to a mutable tag.

2. Disable the Scheduler job first when heartbeat configuration, authentication,
or alerting is causal. Remove only its service specific invoker grant after
confirming no approved caller depends on it.

3. Keep `minimum_instances = 0`. If cost or abuse rises, reduce maximum
instances within the approved plan, restrict invokers, and stop traffic. Do not
weaken ingress or authentication to recover availability.

4. If project, IAM, or supply chain integrity is uncertain, deny all invocation,
preserve audit evidence, revoke deployment authority, and require a clean image
and plan review.

5. Resource destruction is a separate explicit action because the current
service has deletion protection. Review Terraform state, retained logs,
Artifact Registry retention, service accounts, and billing effects before
disabling deletion protection or destroying resources.

6. Phase G1 has no platform token or permission to revoke. A future adapter
rollback additionally disables provider credentials, callback subscriptions,
queue delivery, and platform app access according to its own ADR.

## Risks and mitigations

### Perpetual runtime illusion

A warm instance or heartbeat can be mistaken for continuity. The service is
designed for replacement, and health evidence states its timestamp, revision,
and narrow route scope. Durable queues and idempotency, not process lifetime,
protect future work.

### Cold start latency

Zero minimum instances can add hundreds of milliseconds or more depending on
image, language, region, and dependency startup. Keep the image small, avoid
startup network calls, measure p50 and p95, and add a warm instance only when a
measured latency objective is worth roughly the current $10 monthly idle list
cost.

### IAM expansion

Future adapters could accumulate platform and secret access in one identity.
Reject shared adapter identities, wildcard secret access, project roles, and
runtime self modification. Diff effective IAM in every plan.

### Public callback spoofing

Network reachability can be confused with provider authenticity. A future
callback validates provider signatures, raw body, timestamp, nonce, account,
OAuth state, and replay status before enqueue or response dependent work.

### Duplicate external action

Queue redelivery and ambiguous provider timeouts can duplicate a post or
message. Use transactional idempotency, provider reconciliation, exact action
digests, expiries, and terminal receipts. Never blindly retry an ambiguous
write.

### Secret leakage

Terraform state, environment variables, traces, error bodies, and callback URLs
can leak tokens. Phase G1 has no secrets. Future phases use external references,
field allowlists, redaction tests, bounded payload logging, and per adapter
access.

### Supply chain substitution

A digest can still identify a malicious build. Require reviewed source,
dependency locks, SBOM, vulnerability result, provenance, and an approved digest
before deployment.

### Cost drift

Logs, network egress, minimum instances, worker count, and cross region queues
can dominate low compute cost. Use budget alerts, per service labels, log volume
caps, same region resources, scale ceilings, current calculators, and monthly
cost review. Budget alerts detect cost but do not automatically authorize
destructive shutdown.

### Scale configuration drift

An unreviewed module change could raise the Phase G1 ceiling. Variable
declarations, static tests, closed plan policy, and future human review must all
reject `maximum_instances > 3`.

## Acceptance tests

### Current Phase G1 static gates

1. Source inspection confirms internal ingress, deletion protection, request
   based idle CPU, 1 vCPU, 512 MiB, a 10 second timeout, integer scale bounds,
   and a full Artifact Registry application image digest requirement.

2. Closed plan policy fixtures require a current `SocialGcpPlanReviewV1`, its
   matching canonical plan digest, exact reviewed project, region, service,
   12 digit project number, application image and canonical evidence record,
   expected heartbeat state and resolved URI, and normalized timestamps with no
   more than a 24 hour validity window.

3. The fixtures require unique managed addresses and exact types in both
   `planned_values` and `resource_changes`. Unknown resources, count mismatch,
   after value mismatch, destructive action, any unresolved after value,
   sensitive plan value, alternate provider, provider expression, module,
   provisioner, extra variable, incomplete heartbeat sets, public principals, unexpected IAM roles,
   environment variables, volumes, mutable or different application images,
   traffic overrides, command or argument overrides, VPC access, disabled
   invoker IAM, paused or retried heartbeat, invalid scale bounds, and project
   substitutions fail static policy.

4. The adversarial fixture preserves a matching reviewed plan digest while it
   attempts a wrong service region and name, 32 CPU, 128Gi memory, every minute
   Scheduler execution, POST, attacker URI and OIDC audience, authorization
   header, body, older revision traffic, command, arguments, VPC access,
   disabled invoker IAM, paused heartbeat, retry override, and unknown field.
   Every mutation is rejected by a specific invariant.

5. Static source and package scans find no platform credential, authorization
   header, OAuth code, cookie, signing secret, private key, secret version,
   callback, queue, worker, publisher, or optimizer execution resource.

6. Every Phase G1 report states that reviewer authority, effective IAM, image
   provenance, runtime behavior, internal network reachability, billing, and
   deployment are unverified.

### Open Phase G2 deployment gates

1. Install a reviewed Terraform version, initialize providers, create the
   provider lock, run format and validation, produce a real plan, and pass the
   closed policy against a separately authenticated review record.

2. Build the container from a reviewed base image digest and bind source
   revision, dependency lock, SBOM, vulnerability result, provenance, base
   image, and resulting application image.

3. Inspect effective IAM and prove the runtime cannot read secrets, mint tokens
   for another identity, mutate IAM, ingress, scale, image, or Scheduler, or
   invoke a social platform.

4. Verify authorized internal route access and rejection of unauthorized,
   public, unknown, mutation, and oversized requests. Capture a network trace
   showing no platform or model call.

5. With heartbeat disabled, prove no Scheduler or invoker resource exists. If
   later enabled, verify the complete bounded OIDC set, exact audience, internal
   reachability, and service specific invoker grant.

6. Measure cold start, latency, errors, logs, instance count, and billing. Label
   billing export results `MEASURED` and block rollout above the approved cost.

7. Exercise revision rollback while preserving internal ingress and narrow IAM.
   Review deletion protection separately before any destructive action.

Future callback and worker phases additionally require official signature
vectors, replay rejection, queue and dead letter behavior, idempotency,
reconciliation after ambiguous provider timeouts, graceful termination, and no
duplicate external action.

ADR 350 is accepted only as a Phase G1 static blueprint when its current gates
pass. It is not accepted as deployment evidence. Every release record must say
that no Terraform validation, container build, or GCP deployment was performed
unless new captured evidence proves otherwise.

## References

1. [Cloud Run overview and worker pools](https://docs.cloud.google.com/run/docs/overview/what-is-cloud-run)

2. [Cloud Run minimum instances](https://docs.cloud.google.com/run/docs/configuring/min-instances)

3. [Cloud Run container lifecycle](https://docs.cloud.google.com/run/docs/configuring/services/containers)

4. [Cloud Run pricing](https://cloud.google.com/run/pricing)

5. [Authenticated Cloud Scheduler invocation](https://docs.cloud.google.com/run/docs/triggering/using-scheduler)

6. [Cloud Scheduler pricing](https://cloud.google.com/scheduler/pricing)

7. [Secret Manager best practices](https://docs.cloud.google.com/secret-manager/docs/best-practices)

8. [Service account best practices](https://docs.cloud.google.com/iam/docs/best-practices-service-accounts)

9. [Pub/Sub dead letter topics](https://docs.cloud.google.com/pubsub/docs/dead-letter-topics)

10. [Cloud Tasks pricing](https://cloud.google.com/tasks/pricing)

11. [Pub/Sub pricing](https://cloud.google.com/pubsub/pricing)

12. [Artifact Registry pricing](https://cloud.google.com/artifact-registry/pricing)

13. [Cloud Logging pricing](https://cloud.google.com/logging/pricing)
