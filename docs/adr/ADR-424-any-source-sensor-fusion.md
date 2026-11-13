# ADR-424: Any-source sensor ingestion and calibrated per-zone fusion

- **Status:** Proposed
- **Date:** 2026-11-09
- **Relates to:** ADR-260 (RuField MFS), ADR-303 (radar reference labels), ADR-423 (ESP32 HE-LTF
  validity gate, presence benchmark), ADR-115 (Home Assistant), ADR-299 (CSI data policy)

## Context

RuView's sensing server ingests WiFi CSI (ESP32, Realtek, MediaTek, Qualcomm) and publishes
presence, motion and vitals per node. Other sensors exist around it but do not combine with it:

- A 60 GHz radar (MR60BHA2) is read by a standalone script (`scripts/mmwave_fusion_bridge.py`) and
  by `scripts/radar_window_logger.py` (ADR-423) as a benchmark reference. Neither feeds the server.
- `GET /api/v1/pose/zones/summary` returns a hardcoded stub. There is no real per-zone result.
- RuField (`vendor/rufield`, ADR-260) defines 17 field-sensing modalities, a common `Observation`
  record, and `FieldAdapter` / `FusionEngine` traits. The server emits its CSI results as RuField
  events. Only CSI replay and ultrasonic replay ingest real data; mmWave and thermal are simulated
  (SYNTHETIC). Nothing ingests a live non-CSI sensor, and a plain time series (temperature, power,
  ECG, sound level, door contact) has no modality to land in.

Outside the repository, hardware catalogs such as the WeftOS Sensor Explorer
(`cogs.weavelogic.ai`) package one small program per part (a "cog") that writes one JSON line per
reading: 24 GHz and 60 GHz radars (RD-03E, LD2450, LD6002), an IMU, an 8x8 time-of-flight array, a
single-lead ECG, a sound detector. That shape (one self-describing JSON line per reading) is the
lowest common denominator for sensors RuView does not have drivers for. Only the catalog's landing
page and the start of its catalog file were read for this ADR; its cog manifest schema was not, so
this ADR defines RuView's own input contract and does not claim wire compatibility.

Nothing in this ADR is measured. Every accuracy statement it enables must come from a hardware run
under the protocols it references.

## Decision

### D1 — One generic input: source readings as JSON lines

The server accepts **source readings**, one JSON object per line, over UDP, MQTT, or a local pipe:

```json
{"timestamp_ms": 1794250000000, "source": "radar-desk-1", "kind": "mmwave_radar",
 "room": "office", "zone": "desk",
 "values": {"presence": 1, "distance_cm": 142.0, "breathing_bpm": 14.2},
 "quality": 0.9, "health": "ok", "synthetic": false}
```

| Field | Required | Meaning |
|---|---|---|
| `timestamp_ms` | yes | Source wall clock, Unix ms |
| `source` | yes | Stable id of the emitting device or program; `[A-Za-z0-9_.-]{1,64}` |
| `kind` | yes | Sensor kind; selects the descriptor (D2) |
| `values` | yes | Flat map of name to finite number; bounded count and name length |
| `room`, `zone` | no | Where the reading applies; without a zone it is context only |
| `quality` | no | Source's own confidence in `[0, 1]` |
| `health` | no | `ok`, `stale`, `no_source`, `degraded` |
| `synthetic` | no, default `false` at the wire but see D4 | Simulator or replay output |

Lines are size-bounded, unknown fields are ignored, and a malformed line is counted and dropped,
never partially applied. The input is unauthenticated data from the LAN: it is validated at the
boundary, bound to loopback by default, and can never change configuration or redirect any stream.
A source id is a claim, not an identity; sources that must be trusted use the existing
authenticated transports.

CSI nodes keep their binary protocols. Internally their per-node classification becomes a source
reading of kind `wifi_csi`, so fusion sees one shape.

### D2 — Sensor descriptors instead of per-sensor code

A descriptor (one small file per `kind`) declares, for each value name: unit, valid range,
expected rate, and **what the value is evidence for**:

`presence`, `motion`, `breathing_rate`, `heart_rate`, `range`, `position`, `count`, or `context`.

Adding a sensor that already speaks JSON lines means adding a descriptor. A Rust adapter is needed
only for sensors with a binary protocol. Values outside the declared range are dropped and counted.
A reading whose `kind` has no descriptor is accepted as context only (D3).

Descriptors map each kind onto a RuField modality where one exists. Kinds that are not
field-sensing (temperature, power, door contact, ECG) are not forced into a modality.

### D3 — Two fusion tiers

- **Tier 1, fused.** Values declared as evidence for a quantity the server estimates (presence,
  motion, vitals, range/position, count) enter calibrated per-zone fusion (D5).
- **Tier 2, context.** Everything else is stored, charted and exposed. It may gate or explain a
  result ("door contact fired, so a presence change is expected") but is never averaged into one.

A source moves from tier 2 to tier 1 only with a descriptor and a measured reliability (D5).

### D4 — Honesty rules, the same for every source

1. **Stale lowers confidence; it never means "empty".** A source that stops reporting, or reports
   `health != ok`, stops contributing and the fused result's confidence falls. Absence of data is
   never evidence of absence.
2. **Synthetic never mixes into measured.** A fused result that used any synthetic reading is
   labelled SYNTHETIC end to end. Replay and simulator sources must set `synthetic: true`; the
   server also marks as synthetic any source configured as replay.
3. **Reliability comes from a benchmark, not a datasheet.** A source's fusion weight is the
   reliability measured under ADR-423's held-out-room protocol or the track-aware protocol (D7).
   Until measured, it uses a conservative default weight and the result says so.
4. Every published number carries its evidence grade and the list of sources that contributed.
   A fused zone is `SYNTHETIC` if any contributing reading is simulated or replayed; otherwise
   `CLAIMED` if any contributing reading came from an unauthenticated sender; `MEASURED` only when
   every contributing source is verified hardware; `NONE` when no source is live.
5. **A sender cannot verify itself.** `verified` is set by the server for readings it produced
   from its own authenticated or in-process sources and is never read from the wire. Everything
   posted to the open ingest route is a claim until a transport authenticates the sender.

### D5 — Fused per-zone presence replaces the stub

`GET /api/v1/zones` (and the existing zone-summary route, for compatibility) returns, per zone:

- `presence_probability` in `[0, 1]`, or `null` when no healthy tier-1 source covers the zone;
- `confidence`, lowered by stale or missing sources;
- `sources[]`: each contributing source with its own probability, weight, age and health;
- `agreement`: whether healthy sources agree, and which disagree;
- `evidence`: MEASURED, CLAIMED, SYNTHETIC or NONE (D4).

Fusion is **late**: each source yields a presence probability, and they are combined in log-odds
space weighted by reliability and discounted by age. Late fusion is chosen because sources differ
in rate, failure mode and physics, a missing source degrades it gracefully, and each source's
contribution stays inspectable.

Follow-on fusion work, in order, each gated on a measured benchmark:

1. **Radar teaches CSI.** Radar-labelled windows (`ruview-groundtruth`, ADR-303) calibrate each
   room's CSI model and supply count labels. The radar stays on the label side; it is fused at
   runtime only as its own tier-1 source.
2. **Vitals.** Breathing rate fuses CSI and radar weighted by signal quality. Heart rate comes
   from radar; CSI motion invalidates a reading rather than contributing to it.
3. **Cross-chip calibration** (ADR-423 D3): per-link normalisation so different chips feed one model.
4. **Multi-node geometry.** With three or more nodes, weight links by where they cross the room
   for a coarse per-zone position. Requires scheduled per-node transmit rates.

### D6 — Visual surface

1. **Live room view:** floor plan, zones shaded by fused presence probability, node and sensor
   positions, links coloured by current quality.
2. **Per-source evidence strip:** one timeline row per source of any kind plus the fused result,
   so disagreement is visible. A new source appears as soon as it sends data.
3. **CSI waterfall** with markers where the ADR-423 validity gate dropped frames.
4. **Evidence badges and sensor health** on every number. A stale reading is never drawn as live.
5. **Setup page from mDNS:** nodes advertising `_ruview-node._tcp` (ADR-423 follow-up) are listed
   and assigned to zones.

### D7 — A separate, track-aware benchmark

ADR-423's presence benchmark answers "is anyone in the zone". It cannot support any statement
about how many people or which track is which. A second protocol and scorer cover that:

- **Per-person labels.** The reference gives, per window, a set of person tracks (position and a
  session-local track id). Track ids are pseudonymous and never carry identity across sessions.
- **Held-out rooms.** Every tunable parameter is fitted on other rooms only; each room is scored
  as a held-out fold, and per-fold results are published.
- **An explicit ambiguity class.** A window is `ambiguous` when the reference itself cannot
  resolve who or how many: reference sources disagree on count, tracks are closer than the
  reference's resolution, a track has just appeared, disappeared or swapped, or the reference is
  stale. Ambiguous windows are reported as their own class with their share of the session. They
  are never forced into a count or a track, and never silently dropped: a result with a large
  ambiguous share is a weak result and must say so.
- **Scores.** Count accuracy against a majority-count baseline, per-person detection
  precision/recall, track continuity (identity switches, fragmentation), and position error where
  the reference has positions, all on non-ambiguous windows, plus the ambiguous share.

This protocol is separate from ADR-423's. A presence result must not be quoted as a counting or
tracking result, or the reverse.

## Consequences

- Any sensor that can print a JSON line can feed RuView without RuView-specific code. Only
  sensors with a descriptor and a measured reliability influence a fused result.
- The stub zone endpoint becomes a real one with inspectable inputs.
- More input surface: an unauthenticated LAN input needs strict bounds and a loopback default.
- Fusion weights are placeholders until benchmark runs exist; the output must say so.
- Counting and tracking gain a way to be measured, with ambiguity reported instead of hidden.
  They remain unclaimed until a hardware run under D7.

## Rollout

1. JSON-lines ingest, descriptors for `wifi_csi` and `mmwave_radar`, and the fused per-zone
   presence endpoint with those two sources. Tests on SYNTHETIC readings.
2. Track-aware protocol document and scorer (D7). Tests on SYNTHETIC tracks.
3. Evidence strip and room view on the endpoint.
4. Radar-taught calibration; vitals fusion; cross-chip calibration; geometry. Each after its
   benchmark.

## Open questions

- Whether descriptors live in the repository, are user-supplied, or both.
- Whether to add a RuField modality for generic scalar time series or keep those outside RuField.
- Wire compatibility with an external cog catalog needs its manifest schema read first.
