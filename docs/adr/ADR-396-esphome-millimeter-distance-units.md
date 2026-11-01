# ADR-396: Convert ESPHome millimetre distances to reported centimetres

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 Amendment 3 (ESPHome radar reader)

## Context

ESPHome sensor discovery advertises each entity's unit. The reader maps entities
named distance into `distanceCmMean`, converting metres to centimetres but leaving
millimetres unchanged. An advertised 500 mm measurement therefore became 500 cm
instead of 50 cm, a factor-of-ten numerical error.

## Decision

Apply unit scales of 100 for metres and 0.1 for millimetres. Preserve centimetre
and existing unspecified/other-unit behavior, positive-distance filtering, and
entity metadata. Unit matching remains case insensitive. This is a correction
to reported units, not a change to sensor discovery or access permissions.

## Verification

The public reader connects to an owned loopback TCP fixture speaking the native
ESPHome protocol. Discovery and state updates carry 500 mm, 0.5 m, and 50 cm.
The millimetre baseline fails with 500 cm; metre and centimetre controls pass.
All three report 50 cm after the correction. This proves software protocol/unit
handling only; no real radar or external network service was accessed.
