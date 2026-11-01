# ADR-382: `spatial.evidence.v1` export of RF Gaussians and link observations

- **Status**: proposed
- **Date**: 2026-10-03
- **Deciders**: RuView maintainers
- **Tags**: rf, gaussian, spatial, evidence, wire-format, interop
- **Related**: ADR-273 (unified RF world model), ADR-275 (RF-aware Gaussian
  memory), ADR-142 (temporal voxel evidence). Origin of the wire format:
  WeftOS ADR-107 §7 ("Spatial evidence engine and workspace"), amended
  2026-10-03.
- **Crate**: `v2/crates/ruview-spatial-evidence`

Numbering: on 2026-10-03 `main` had up to ADR-378, open PRs claimed 379
and 380, and 381 was taken by another branch.

## Context

`ruview-unified` keeps a map of RF Gaussians (ADR-275) and a Friis channel
model that turns a TX/RX link into an expected amplitude. A spatial evidence
engine fuses many sensor types (tape measurements, radar, ToF, UWB, RF) into
an occupancy octree and promotes stable geometry (walls, furniture) into room
models. That engine reads a versioned JSONL format, `spatial.evidence.v1`,
specified in WeftOS ADR-107 §7.

RuView should be able to feed it without depending on it, and without a
reader of this repository having to go to another project to learn what the
export writes. This ADR records the part of the contract RuView emits.

## Decision

1. A leaf crate, `ruview-spatial-evidence`, converts `ruview-unified` output
   to `spatial.evidence.v1` lines. Its only internal dependency is
   `ruview-unified`. The wire shapes are local serde structs; the contract is
   the JSON, not a shared crate.
2. It emits two record types, `rf_gaussian` and `rf_link_observation`. It
   does not mirror the other v1 types and rejects them as unknown when
   parsing.
3. Every record is validated against the v1 rules below before it is
   serialised, so a bad value fails in RuView rather than in the consumer.
4. If WeftOS ADR-107 §7 changes the envelope or either RF body, this ADR is
   amended in the same change that updates the crate.

### Envelope (every record)

| Field | Rule |
|---|---|
| `schema` | exactly `spatial.evidence.v1` |
| `type` | `rf_gaussian` or `rf_link_observation` here |
| `t_ns` | u64, ns since the Unix epoch |
| `frame` | `room_enu` |
| `region` | region id, e.g. `region/urth/meso/<room>` |
| `source_id` | device or operator id, never a person |
| `uncertainty_m` | 1σ position uncertainty, metres, in (0, 100] |
| `provenance` | `{receipt, producer, proof}`; `proof` is `MEASURED`, `CODE` or `SYNTHETIC` |

Ids (`region`, `source_id`, `receipt`, `producer`) match
`[A-Za-z0-9._:/-@]{1,128}`. A line is at most 16 KiB.

**Frame.** `room_enu` is room-local, origin at the room's SW floor corner,
x east, y north, z up, metres. Coordinates are within ±1000 m.

**Angles.** Every yaw in v1 is degrees counter-clockwise from room +x
(east): 0 faces +x, 90 faces +y. The two RF types have no yaw field. The
`rf_gaussian` orientation is a body-to-room quaternion, and a positive
rotation about +z turns the Gaussian's local x axis counter-clockwise from
east, the same sense.

### `rf_gaussian`

| Field | Rule |
|---|---|
| `position` | [x, y, z] m |
| `scale` | per-axis σ, m, each in [1e-6, 1e4] |
| `orientation` | quaternion `[w, x, y, z]`, finite, norm > 1e-6 |
| `occupancy` | peak extinction, nepers/m, in [0, 1e6] |
| `confidence` | [0, 1] |
| `motion` | `static`, `slow` or `fast` |
| `role`? | `absorber` or `reflector`; set only on priors sent back to RuView |

The exporter maps `RfGaussian` field for field. `uncertainty_m` is the
geometric mean of the three σ, clamped to the envelope range, because
`RfGaussian` has no separate position covariance. The receipt is
`gauss:<device>:<t_ns>:<index>`. A Gaussian whose provenance is `synthetic`
is always tagged `SYNTHETIC`; a caller can lower the proof tag, never raise
it.

### `rf_link_observation`

| Field | Rule |
|---|---|
| `tx`, `rx` | antenna positions, m |
| `freq_hz` | [1e8, 3e11] |
| `excess_loss_db` | `20·log10(free / measured)`, in [−60, 200]; negative is constructive multipath |

The free-space reference comes from `ruview-unified`'s channel model with an
empty map, so it is the same Friis amplitude the ADR-275 inverse update uses.

### Examples

These two lines are the WeftOS ADR-107 §7 examples. The crate's tests parse
them and check that its own output has the same shape.

```jsonl
{"schema":"spatial.evidence.v1","type":"rf_link_observation","t_ns":1759500001300000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"node-2","uncertainty_m":1.0,"provenance":{"receipt":"csi:node1-node2:000881","producer":"ruview-adapter@0.1","proof":"MEASURED"},"tx":[6.20,3.50,1.10],"rx":[0.20,0.20,1.10],"freq_hz":2437000000.0,"excess_loss_db":4.2}
{"schema":"spatial.evidence.v1","type":"rf_gaussian","t_ns":1759500002000000000,"frame":"room_enu","region":"region/urth/meso/test-room","source_id":"ruview-unified","uncertainty_m":0.5,"provenance":{"receipt":"gauss:7f3a","producer":"ruview-unified@0.3","proof":"CODE"},"position":[3.2,0.0,1.2],"scale":[0.05,1.6,1.2],"orientation":[1,0,0,0],"occupancy":0.7,"confidence":0.8,"motion":"static","role":"absorber"}
```

### Versioning

Additive optional fields may appear inside v1 and older readers ignore them.
New record types may also be added inside v1 (the 2026-10-03 amendment added
`tof_depth` and `radar_range`, and an optional `sensor` block on
`radar_track_point`); a reader that does not know a type rejects it as
unknown rather than misreading it. Removing, renaming or changing the
meaning of a field requires `spatial.evidence.v2`, and the parser reports a
v2 line as a version mismatch.

## Consequences

- RuView output can feed a spatial evidence engine with no new runtime
  dependency, and the contract is readable from this repository.
- The engine can send promoted walls and objects back as `rf_gaussian`
  priors with a `role`. That gives ADR-275's inverse update a geometric
  starting point instead of an empty map.
- The struct definitions are duplicated between the two projects. The
  golden-line tests catch drift in the RF types; a change on the WeftOS side
  still needs someone to amend this ADR and the crate.

## Limitations

- Nothing calls the crate yet. The sensing server does not export records,
  and there is no adapter that turns an inbound `rf_gaussian` prior into an
  `RfGaussian` in the live map.
- The tests are CODE-level (deterministic conversions and validation). No
  MEASURED claim is made about what the exported evidence does to a room
  model.
- `uncertainty_m` for a Gaussian is a stand-in derived from its extent.
