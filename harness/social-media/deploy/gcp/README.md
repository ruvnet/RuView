# Optional GCP deployment

This module is the proposed Phase G1 deployment blueprint for the read only
control plane. The current evidence is source inspection and unit tested static
plan policy. Terraform is not available in the validated environment, so no
`terraform fmt`, `terraform validate`, provider lock, real plan, image build,
or GCP deployment has been completed. The blueprint does not include an account
adapter, OAuth callback, webhook receiver, message worker, publisher, or
optimizer promotion service.

## Availability model

Cloud Run instances can restart. Minimum instances reduce cold starts but do
not create a perpetual process. The safe model is restartable workers, durable
queues, idempotency keys, and dead letter handling.

The proposed default is zero minimum instances and three maximum instances.
Cold start latency has not been measured for this image. A warm instance can
reduce latency but cannot guarantee continuity. Verify current [Cloud Run
pricing](https://cloud.google.com/run/pricing) with the project calculator
before changing the default.

## Inputs

1. A dedicated Google Cloud project.

2. A full Artifact Registry application image URI pinned by digest and a
closed `SocialImageEvidenceV1` record. Set `image_evidence_digest` to the
canonical digest of that record.

3. A reviewed Terraform plan and an operator with deployment authority.

The module accepts no platform credential. It creates no secret value.

## Build

Resolve and review a current official Node base image digest before building.
The Containerfile requires the operator to supply `NODE_IMAGE`, but the current
plan checker validates the resulting Artifact Registry application image and
binds digest references for source revision, SBOM, scan, and provenance. It
does not inspect those referenced artifacts, prove the base image digest, or
authenticate who produced the evidence.

```bash
docker build \
  --build-arg NODE_IMAGE=node:22-alpine@sha256:REVIEWED_DIGEST \
  -f Containerfile \
  -t REGION-docker.pkg.dev/PROJECT/social/ruv-social:BUILD_ID .
```

Push through an approved CI identity, resolve the resulting application image
digest, then set `container_image` to the full Artifact Registry `@sha256:`
reference. These are operator steps, not completed deployment evidence.

## Plan

```bash
cd deploy/gcp
terraform init
terraform fmt -check
terraform validate
terraform plan -out=reviewed.plan -var-file=reviewed.tfvars
terraform show -json reviewed.plan > reviewed.plan.json
node check-plan.mjs reviewed.plan.json reviewed-policy.json
```

`reviewed-policy.json` must be a current `SocialGcpPlanReviewV1` record with
exactly these fields: `schema`, `planDigest`, `projectId`, `projectNumber`,
`region`, `serviceName`, `approvedImage`, `approvedImageEvidence`,
`approvedImageEvidenceDigest`, `heartbeatEnabled`, `heartbeatServiceUri`,
`reviewedAt`, and `expiresAt`. `planDigest` is SHA 256 over the recursively key
sorted canonical JSON plan. `approvedImageEvidence` is a closed
`SocialImageEvidenceV1` record that binds the same image, source revision,
SBOM, vulnerability scan, provenance, and evidence time. Its canonical digest
must match both the review and the Cloud Run template annotation. The image
registry location and project must match the reviewed region and project. The
review window must be positive and no longer than 24 hours. Any missing or
additional review field, plan mutation, expired review, different project,
region, service, image, evidence, or heartbeat state denies the check.

`projectNumber` must be the separately reviewed 12 digit number for
`projectId`. The checker uses it to require the exact canonical Cloud Run host.
It does not call Google to prove the ID to number mapping, so authenticated
review and later live project inspection remain mandatory.

The review creation process is outside this package. It must authenticate the
reviewer and compute the digest from the final `terraform show -json` file.
Passing the checker means only that the exact reviewed JSON plan fits the
closed Phase G1 resource policy. It returns the recomputed `planDigest` and
`reviewAuthorityVerified: false`; it does not prove reviewer identity,
effective cloud IAM, image provenance, or runtime behavior.

The canonical JSON digest does not cryptographically bind the binary saved plan
that `terraform apply` consumes. A future authorized workflow must retain and
hash `reviewed.plan`, derive the reviewed JSON directly from that exact
artifact, authenticate both digests, and apply only the retained binary. The
current checker output is static policy evidence, never deployment
authorization.

Apply is intentionally omitted. A human with deployment authority must review
the plan, project, region, immutable application image, IAM members, scale
bounds, and expected cost before any deployment. None of the plan commands
above have been run as part of the current evidence.

The checker requires `planned_values` and `resource_changes` to contain the
same exact unique managed resource graph, types, and after values. Create,
update, and no op are the only allowed actions. Delete, replacement,
nonapplyable or incomplete plans, any unresolved after value, sensitive plan values, unknown
resource or nested configuration fields, and count mismatches deny the check.
A provider schema change fails closed until its new fields and semantics are
reviewed.

The plan configuration must name only
`registry.terraform.io/hashicorp/google`, use no alias or module provider, and
contain the exact seven declared resources. Module calls and all provisioners,
including `local-exec`, are denied. Every planned resource and resource change
must use the Terraform JSON short provider name `google`. The source provider
block is empty so credentials, access tokens, impersonation, and custom
endpoints cannot enter provider expressions. Importing, generated configuration,
deposed instances, replacement paths, identity metadata, drift, and deferred
changes are denied.

The plan variables map is closed to the eight declared inputs. Each variable
contains only `value` and must match the reviewed project, region, service,
image, image evidence, heartbeat state, and the scale recorded in the planned
Cloud Run service. Extra variables, including credential variables, deny the
check. These fields follow HashiCorp's [Terraform JSON output
format](https://developer.hashicorp.com/terraform/internals/json-format).

The service uses internal ingress. The optional Cloud Scheduler heartbeat uses
OIDC and a service account with only `roles/run.invoker`. The all or none
heartbeat graph is accepted only when `heartbeatEnabled` is true and the Cloud
Run URI is resolved in the plan and exactly matches `heartbeatServiceUri`. The
URI must use the reviewed service name and region in the standard
`service-projectNumber.region.run.app` form. This makes heartbeat enablement a
second plan after the service exists when an initial create leaves that URI
unknown. Google
documents the authenticated pattern in [Scheduler authentication for Cloud
Run](https://docs.cloud.google.com/run/docs/triggering/using-scheduler).

## Future adapter phase

Each adapter requires its own service account, Secret Manager references, a
durable queue, a dead letter queue, idempotency storage, platform signature
validation, current quota discovery, and exact approval verification. Follow
Google's [Secret Manager best
practices](https://docs.cloud.google.com/secret-manager/docs/best-practices).
Never download a service account key.

Discord Gateway or other continuous consumers should use a worker pool or
another restartable worker design. OAuth callbacks and webhooks belong in a
separate public service with signature validation and a narrow ingress policy.

## Cost evidence

Google list prices and free tier descriptions are external `CLAIMED` evidence.
Arithmetic derived from those prices and workload assumptions is `SYNTHETIC`
until a billing export measures the deployed service. The current planning
bands are about 0 to 5 USD per month for low traffic scale to zero, about 9.72
USD per month for one continuously warm 1 vCPU and 0.5 GiB request based
instance under the documented assumptions, and about 29.16 USD for three.
Region, billing mode, free tier usage, logs, storage, egress, and pricing drift
can materially change them.

## Acceptance test

A static Phase G1 plan is acceptable for review only when the Terraform JSON
and a separate current `SocialGcpPlanReviewV1` pass the closed plan checker.
The plan must contain exactly the reviewed internal service, the required five
APIs, its dedicated service account, and the exact reviewed heartbeat state.
The service must retain internal ingress, deletion protection, its deterministic
runtime identity, 10 second timeout, zero through three scaling, one reviewed
container, 1 CPU, 512Mi memory, idle CPU, and the exact HTTP startup probe. An
enabled heartbeat must use the reviewed project, region, service, five minute
UTC schedule, GET request to the resolved service `/healthz` URI, exact OIDC
identity and audience, active state, no retry override, and no headers or body.
The service must have no traffic override, entrypoint or argument override, VPC
access, disabled invoker IAM, or unreviewed configuration field. The plan must
have no public
principal, project editor or owner role, secret value, runtime environment
variable, volume, mutable application image, unknown resource, destructive
change, or platform execution path.

Deployment acceptance remains open. It requires successful Terraform format,
validation, provider initialization, reviewed plan, base and application image
evidence, effective IAM inspection, authorized internal and unauthorized route
tests, network traces, billing evidence, rollback, and a live canary. Until
then, `/healthz` and `/v1/capabilities` behavior is local application evidence,
not GCP runtime evidence.
