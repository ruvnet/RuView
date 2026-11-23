# ADR-408: Exclude absent LD2410 targets from distance means

- Status: proposed
- Date: 2026-10-11
- Related: ADR-373 (shared mmWave summary)

## Context

The LD2410 decoder explicitly emits present=false and distanceCm=0 for target
state none. The aggregate treats that no-target sentinel as a measured range.
A 90 cm detection followed by an absent target consequently reports 45 cm; an
all-absent stream reports 0 cm instead of having no distance observation. The
presence fraction already identifies these absent frames correctly.

## Decision

Exclude explicitly absent target frames only from the distance mean. Keep their
raw distance sentinel, frame counts, rates, and presence contribution. Continue
averaging moving and stationary target measurements normally. MR60BHA2 distance
frames are unchanged; they use their own range-validity field in the decoder.
This distinguishes absence from a distance measurement, without new physical
range limits or an accuracy claim.

## Verification

The actual Python serial pump and production process runner read complete
LD2410 frames from an isolated serial module. Public readMmwave must report 90 cm
for present 90/absent, null for all-absent, and 150 cm for moving 90/static 210.
Presence fractions and raw last-frame zero controls remain intact. The baseline
fails both absence regressions and passes the positive-distance control. No
hardware port opens or radar transmissions occur.
