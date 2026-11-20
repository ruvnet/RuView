# ADR 380: Loopback UDP tee for second consumers of the CSI stream

## Status

Proposed. Unit tested, and smoke tested on loopback: a datagram sent to the
server arrived byte for byte at the tee target. Not yet exercised against a
live multi node fleet.

Date: 2026-09-28

Numbering: On 2026-10-02 `main` had up to ADR 378 and open PRs claimed nothing
higher; ADR 379 is the OTA first-boot health check. If another branch also
takes 380, renumber whichever merges second.

## Context

A five node deployment (four ESP32 S3, one ESP32 C6) streams ADR 018 frames to
one sensing server. A second consumer cannot see the raw stream.

Each node sends to a single `target_ip` (`firmware/esp32-csi-node/main/stream_sender.c`).
The server's WebSocket and REST outputs carry per node amplitude only: no
phase, no I/Q, no sequence number, and no `0xC511A110` sync packets. A fusion
host that aligns CSI with camera, radar, or lidar needs all of those. Binding a
second listener to the CSI port is not an option, and an external relay in
front of the port would make every source loopback, which the ADR 296
allowlist always admits.

## Decision

`--udp-tee` (env `RUVIEW_UDP_TEE`) copies every datagram the source allowlist
admits, byte for byte, to up to four `ip:port` targets.

- Targets must be loopback, so raw CSI never leaves the host through the tee.
- A target may not use port 0 or the listener's own port, which would loop.
- Sends are non blocking. A copy that cannot be queued is dropped and counted,
  so a slow consumer never stalls the receive loop.
- The copy happens after the ADR 296 allowlist check and before any parsing,
  so consumers see exactly what the server admitted, in every wire format.
- The tee is off unless configured.

## Consequences

A fusion host or Cognitum cog can subscribe to the full data plane by reading
a loopback port. A consumer that today binds the CSI port itself can keep its
code and move to the tee port instead.

The tee is byte transparent, so it needs no change when the ADR 018 header or
a later wire version changes.

Each target costs one `send_to` per admitted datagram. Four targets is the
limit.

## Validation

```bash
cd v2
cargo test -p wifi-densepose-sensing-server --no-default-features --lib udp_tee
```
