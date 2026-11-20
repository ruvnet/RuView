# Managed WorldGraph projection contract

`GET /api/v1/worldgraph/engine` projects the configured managed room into a local
Cartesian layout. It does not read the engine graph writer or report perception,
measured occupancy, calibrated height, geographic registration or live RF links.
The endpoint is read-only and requires the same read authority as `/snapshot`.

## Wire identity and compatibility

The response has exactly `protocol_version`, `stream_epoch`, `seq`, `projection`.
`protocol_version` is3. `projection.schema` is
`ruview.worldgraph.managed-engine-projection.v1`. This is a new managed projection,
not an upstream WorldGraph schema-v2 snapshot. Do not remove that identity and
feed its contents into an engine-snapshot parser.

Existing `/api/v1/worldgraph/snapshot` retains protocol2 and its managed-topology
schema and fields. Its cursor is shared with this projection: repeated reads of
unchanged content through either route do not advance the sequence; source room
or privacy changes advance it once. Restarts use a new random epoch. These are
publication cursors, not measurement timestamps or proof of fresh sensor data.

Current mobile clients continue to request `/snapshot`; their strict legacy
engine/envelope parsers must reject protocol3. A future projection-aware consumer
must be separately implemented and qualified. This server PR does not switch a
mobile URL, activate a new layer or qualify a packaged helper.

## Projection fields

| Field | Meaning |
|---|---|
| `schema` | Explicit managed-projection schema above |
| `provenance` | `operator_entered`; no measurement authority is implied |
| `status` | `configured`, `not_configured` or `privacy_denied` |
| `coordinate_frame` | `managed_room_local`; no geographic origin is known |
| `axis_convention` | `x_right_y_up_metres` |
| `registration` | Always null: geographic origin, scale and heading are unknown |
| `next_id`, `next_edge_id` | Bounded local identifier allocators, not evidence |
| `nodes`, `edges` | Allowlisted room/sensor placement records and membership |

The node/edge field names resemble the engine layout for future adaptation, but
nullable fields intentionally differ from its mandatory numeric values:

- A room has rectangular `bounds_enu` and `floor:null`; a configured room does
  not establish its floor number.
- A sensor position retains `east_m=x_m`, `north_m=room_length_m-y_m` and
  `up_m:null`. The source origin is the room's top-left and y points down; the
  projection origin is its bottom-left and y points up. `east_m`/`north_m` are
  local axis names here, not measured global east/north. Valid planar zero
  coordinates remain zero; unknown height is not converted to ground level.
- `located_in` edges retain `since_unix_ms:null`: the source keeps no timestamp
  for the relationship. Sequence numbers do not supply one.

No generic engine `schema_version:2`, identity geographic registration, invented
floor0, height0 or Unix epoch0 is emitted. This prevents an unaware strict parser
from accepting configured layout as ordinary engine evidence. Projection-aware
consumers must preserve nulls and provenance through display, storage and export;
no inference authority follows from successful parsing.

When not configured or privacy-denied, nodes/edges are empty and both next IDs
are1. Privacy-denied responses reveal no room name, dimensions, sensor identifiers,
raw source records, binding digest, installation ID, tracks or semantic history.
The status distinguishes withheld geometry from an empty configured graph.

## Transport and failure behavior

Responses are `private, no-store`. GET/HEAD do not publish a room, change its
revision/binding, select a learner, write a baseline or start a sensor. POST and
other writes are unsupported. Missing read authorization is rejected. If the
store is unavailable, the endpoint returns501; invalid internal projection input
or poisoned locks return500; sequence exhaustion on a changed publication
returns503. Projection validation happens before cursor mutation so a failed
projection cannot make the next valid read appear newer than its source change.

The repository fixture `managed-worldgraph-engine-projection-v1.json` is the
configured example. Tests cover unknowns, valid planar zeros/bounds, privacy,
not-configured, cursor stability/failure, route authorization and legacy snapshot
compatibility. Physical registration or sensing accuracy is outside this contract.
