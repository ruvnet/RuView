# Generic house fixture (SYNTHETIC)

A made-up ~1,500 sq ft single-story house with a basement: two bedrooms, one
bathroom, kitchen, family room, hallway, six distributed nodes, one
centralized AP, and a five-emitter set that exercises every
`Emitter.status` value (`pending`, `selected`, `approved`, `excluded`,
`surveyed`).

Every coordinate, MAC address (`02:00:00:00:00:0X`, locally-administered so
it is obviously fake), and room label here is invented. It does not describe
any real residence, fleet, or device.

**Why this exists:** `v2/data/room_config.json` is the real fleet's config —
surveyed node positions, house dimensions, and real device MACs — and
`.githooks/pre-push` refuses to publish it to a public remote for exactly
that reason. Anything that needs a realistic multi-room, multi-floor,
multi-emitter `RoomConfig` for a demo, screenshot, or manual test (Room
Builder, storeys, emitter triage) should point at this fixture instead of
ever touching the real file.

Point the sensing server at it directly — nothing else to set up:

```bash
RUVIEW_DATA_DIR=v2/data/examples/generic-house cargo run -p wifi-densepose-sensing-server
```

This only seeds `room_config.json`. There is no CSI traffic behind it, so
the UI will show the room/node/emitter layout with no live links.

**On this PR specifically:** this branch's `RoomConfig` only understands
`width_m`/`depth_m`/`nodes`/`ap_position`/`ap_floor`/`floors`/`walls` and,
as of this PR, `footprint` — so running the server against this fixture here
will show the outer footprint outline, the two storeys, the six nodes and
the AP, which is what this PR adds. The `rooms` (named boxes) and `emitters`
(triage) entries in the file are carried for forward compatibility with
later PRs that add those fields to `RoomConfig`; on this branch they're
present in the JSON but not yet rendered, since the server doesn't have
anywhere to put them yet.
