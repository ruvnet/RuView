# ADR-415: Allocate distinct training capture files on session-name collisions

- Status: proposed
- Date: 2026-10-11
- Related: ADR-018 (collector wire input)

## Context

The training collector names sessions to whole seconds and opens recordings in
append mode. Two captures with the same label during that second append samples
to one file and overwrite its metadata with only the later frame count. Dataset
manifests then undercount the actual recording and the original session loses
its identity. Fractional CLI durations and concurrent collection make the
collision reachable.

## Decision

Allocate each recording with exclusive file creation. Retain existing names
when available; on a data or metadata collision, try a numeric suffix until
allocation succeeds. Update session ID, data path and metadata path together.
Preserve prior recordings and metadata instead of implicitly resuming them.
No CLI resume contract exists; ordinary collection creates a new session.

## Verification

Two actual public collector CLI processes receive distinct owned loopback
packets under an isolated fixed datetime fixture that reproduces a same-second
collision deterministically. Baseline leaves one combined data file and one
miscounted metadata entry. Fixed captures retain distinct IDs, one sample per
file, matching metadata counts and a two-session/two-frame manifest. All eight
harness gates run. No hardware capture or physical accuracy is claimed.
