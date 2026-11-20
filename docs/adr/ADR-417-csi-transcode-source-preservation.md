# ADR-417: Preserve input recordings when converter output aliases the source

- Status: proposed
- Date: 2026-10-11
- Related: ADR-412 (capture converter input quarantine)
- Promotion: extend existing converter PR #2227; no new publication unit

## Context

The public JSONL converter opens output in write mode before reading input. If
both arguments refer to one file, including hard-link or symbolic-link aliases,
it truncates the recording and reports successful conversion with zero frames.
Input validation must not destroy the original evidence it is meant to read.

## Decision

Check existing file identity before opening the output. Reject identical files
with a clear CLI error and exit2, preserving source bytes and links. Keep normal
distinct-path conversion and the existing incomplete-IQ quarantine unchanged.
This does not add in-place conversion or a broader transactional output policy.

## Verification

Actual public Python CLI receives owned input captures with identical, hard-link
and symbolic-link output paths. Baseline overwrites each source; fixed runs
leave the original bytes intact and reject the arguments. Existing signed IQ,
quarantine and later-frame controls pass alongside these fixtures. All eight
harness gates run. Original capture files outside temporary fixtures are never
mutated. This extends the existing converter PR as one reviewed unit.
