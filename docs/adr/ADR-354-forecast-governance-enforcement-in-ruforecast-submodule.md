# ADR-354: fal.ai forecast governance and spend enforcement now real, in the vendored RuForecast submodule

- **Status**: Accepted
- **Date**: 2026-09-02
- **Deciders**: ruv
- **Owners**: RuView forecast training, security, and release maintainers
- **Tags**: forecasting, training, fal, governance, submodule, cost-control
- **Parent**: ADR-349
- **Extends**: ADR-348, ADR-349
- **Supersedes**: None

## Decision

ADR-349 specified two governance controls for RuView Forecast's fal.ai
hosted path: operator-verified dataset/schema/policy/recipe authorization,
and an explicit local spend-approval record bound to the request digest and
maximum units. A 9-agent code review of the training stack merged as PR
#1766 found that neither control was actually wired into the code path that
builds a submittable, fal-destined training contract — `FalGovernanceVerifier`
existed with zero callers outside its own tests, and no spend-approval type
existed anywhere in the codebase.

That code no longer lives directly in this repository. Between the review
and this fix, `v2/crates/ruview-forecast-{core,model,train}` was extracted
into its own repository, `github.com/ruvnet/RuForecast`, and is now vendored
back into `v2/crates/ruforecast` as a git submodule (crates renamed to
`ruforecast-{core,model,train}`). This ADR records two things at the RuView
level:

1. **The governance and spend-approval gaps are fixed**, in
   `ruvnet/RuForecast` PR #2 (branch `fix/governance-and-spend-enforcement`).
   The full technical decision record — what changed, why, and the
   requirements/acceptance table — lives in that repository's own
   [ADR-350](https://github.com/ruvnet/RuForecast/blob/main/docs/adr/ADR-350-fal-governance-and-spend-enforcement.md),
   which this ADR defers to rather than duplicating. In summary: a
   fal-destined `TrainSpec` can no longer be constructed without a real,
   operator-signed, single-use-verified governance receipt; a hosted
   reservation can no longer be built without a real, operator-signed spend
   approval bound to the exact request digest and cost ceiling; and several
   related correctness/resilience gaps on the same trust boundary
   (unauthenticated Direct Server webhook, no execution timeout, mismatched
   local/hosted artifact budgets, cancellation permanently wedging a job)
   were fixed in the same pass.
2. **The submodule pointer in this repository is bumped** to the commit
   containing that fix, once `ruvnet/RuForecast`'s own CI is green on that
   PR and it merges. This RuView-side change is deliberately small: a
   pointer bump plus this ADR, not a re-implementation or re-review of code
   that now lives, is tested, and is reviewed in its own repository.

## Context

RuView's own copies of ADR-348 and ADR-349 predate the extraction and still
describe the crates as if they lived directly under `v2/crates/`. They are
left as-is here (not corrected path-by-path) since the canonical, actively
maintained copies of the forecast-specific ADRs now live in
`ruvnet/RuForecast`'s own `docs/adr/`, which was seeded with copies of
ADR-348/349 at extraction time and now also carries ADR-350. Consult that
repository for the forecast subsystem's own decision history going forward;
this repository's ADR series should record RuView-level integration
decisions about the vendored submodule (like this one), not attempt to
re-host the submodule's internal history.

This repo's CLAUDE.md requires an ADR for load-bearing upstream changes.
The governance-enforcement fix is load-bearing for RuView even though none
of its source lines are: it changes what RuView's vendored forecast
subsystem actually requires before it will reach real, billable fal.ai
infrastructure on behalf of a RuView deployment.

## Consequences

Once the submodule pointer here is bumped, any RuView-side code, CLI
wrapper, or documentation that assumes the old, unenforced fal.ai
submission path (a bare `SourceState::synthetic` claim with no verified
governance receipt or spend approval) will no longer compile or run against
the vendored crate — `TrainSpec::new_fal_synthetic` and
`ReservedSyntheticSubmission::reserve` both gained new required parameters.
A repository-wide search found no such RuView-side caller today; this ADR
records the change in case one is added later without checking the
submodule's current API first.

This pointer bump must not merge before `ruvnet/RuForecast` PR #2 merges and
its CI is green — see that PR for status.

## References

- [ADR-349](./ADR-349-governed-local-and-fal-forecast-training.md) (this repository's copy; canonical copy is in `ruvnet/RuForecast`)
- [ruvnet/RuForecast PR #2](https://github.com/ruvnet/RuForecast/pull/2)
- [ruvnet/RuForecast ADR-350](https://github.com/ruvnet/RuForecast/blob/main/docs/adr/ADR-350-fal-governance-and-spend-enforcement.md)
- [`v2/crates/ruforecast`](../../v2/crates/ruforecast) (submodule)
