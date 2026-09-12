# ADR 353: Social metaharness CI and branch integrity

## Status

Proposed. The workflow and deterministic package lock are implemented. This
decision grants no package publication, account connection, external action,
credential access, deployment, branch protection change, or history rewrite.

## Context

ADR 351 defines strong local gates, but local results are not durable merge
evidence. A branch name also does not prove change scope. A social branch can
contain unrelated product code, generated build output, or sensitive material
while the social package itself remains unchanged.

The release boundary therefore needs two independent properties. The branch
must be constructed from a reviewed base with only the intended social commits,
and every social package change must receive repeatable CI evidence before
merge.

## Inputs

1. The exact Git commit and its merge base with `main`.

2. The changed path list and object names, reviewed independently of the branch
   name and commit message.

3. `harness/social-media/package-lock.json`, the closed package manifest, the
   package source and tests, and ADR 345 through ADR 353.

4. Node 20 and Node 22 Linux runners plus one macOS runner with
   `/usr/bin/sandbox-exec`.

## Outputs

The workflow produces GitHub check results and bounded console evidence for the
doctor, tests, security suite, manifest, dependency audit, package dry run, and
operating system network denial gate. It does not produce a release artifact,
signature, SBOM, provider receipt, deployment record, or publication authority.

## Decision

### Branch integrity

Reviewers must compare the changed object list with the intended scope before
trusting any test result. Generated archives, build caches, credentials,
unrelated submodule changes, and unrelated product changes fail the scope gate.
A contaminated commit remains quarantined for incident response and cannot be
merged or used as release evidence.

The sensing server can resolve its browser session signing secret under either
`data/session-secret` or `v2/data/session-secret`, depending on its launch
directory. Repository policy must ignore both exact runtime paths and test both
rules. This prevents recurrence without hiding other tracked datasets.

Branch repair must preserve forensic reachability until credential rotation and
remote remediation are complete. Clean work continues from the reviewed local
`main` commit containing the governed Phase 1 package. Rewriting a shared remote
branch remains an explicit maintainer action.

### Deterministic dependency state

The package commits a version 3 npm lockfile even though it has zero runtime
dependencies. CI uses `npm ci` with lifecycle scripts disabled. This makes the
Node toolchain input explicit and enables `npm audit` without creating mutable
dependency state during the gate.

### CI gate

The path scoped workflow runs the complete deterministic check, the focused
security suite, dependency audit, and package dry run on Node 20 and Node 22.
A separate macOS job runs the operating system network denial profile. Actions
are pinned by commit digest, checkout credentials are not persisted, and the
workflow receives read only repository permission.

The workflow has no secrets, write token, artifact publication, deployment,
scheduled execution, account connector, or promotion step. A manual dispatch
reruns the same read only evidence gate and grants no new authority.

## Tradeoffs

The design schedules three runner jobs per relevant change. Local test compute
is below one minute on the reference checkout, while runner startup makes an
estimated wall time of 3 to 8 minutes. The macOS job increases billed runner
cost relative to Linux, but it tests the only current operating system level
network denial control. Removing it saves one runner allocation while reducing
the network isolation claim from enforced to source and process level evidence.

The largest remaining uncertainty is repository branch protection. A workflow
file does not make its checks mandatory. A maintainer must configure the Linux
and macOS checks as required before this gate can block merge.

## Failure handling

1. Quarantine any branch containing a credential shaped object or unrelated
   generated output. Do not print the value.

2. Rotate a potentially valid credential through its owning system before
   rewriting remote history.

3. Rebuild the branch from the reviewed base, rerun every gate, inspect the
   resulting object list, then perform any authorized remote update.

4. Treat a skipped, cancelled, degraded, or unrequired check as missing
   evidence, not a pass.

## Acceptance test

Open a pull request that changes one packaged social source file. Acceptance
requires two green Linux Node checks, one green macOS network denial check, a
clean dependency audit, an unchanged or intentionally updated manifest, and a
diff containing only reviewed social, workflow, lockfile, repository policy
test, and ADR paths. Confirm that the repository rejects merge when any one
required check is forced to fail.
