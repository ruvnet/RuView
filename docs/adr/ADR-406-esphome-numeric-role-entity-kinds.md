# ADR-406: Assign numeric radar roles only to ESPHome sensor entities

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 Amendment 3 (ESPHome native API reader)

## Context

The reader infers heart, breathing and distance roles from entity names without
checking their protocol kind. A binary Heart alarm or text Distance status can
therefore be selected before the actual numeric sensor. Boolean true becomes
1 bpm through arithmetic coercion; a numeric-looking text status becomes a
fabricated distance. The actual numeric sensor is then ignored. A loopback
native API producer reproduces this with valid entity metadata and states.

## Decision

Require sensor kind for heartBpm, breathingBpm and distanceCm roles, matching
the existing targets rule. Keep binary presence inference. Preserve binary and
text entities and their raw states in the entities result, without assigning
them numeric radar roles. Numeric scalar filtering and unit handling remain
unchanged. No medical accuracy or sensor-name taxonomy is introduced.

## Verification

A real TCP producer lists binary alarm and text status entities before valid
numeric heart, distance and breathing sensors. The public reader must select
72 bpm,120 cm and15 bpm rather than1,50 and1. A producer with only nonnumeric
entities must return null numeric summaries while retaining its3 raw updates.
Existing ESPHome presence, missing-state and numeric sensor controls run too.
These are deterministic native API fixtures, not radio or clinical evidence.
