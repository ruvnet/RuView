# ESP32-S3 CSI quick start

A version-pinned path from a bare ESP32-S3 board to a sensing server that
accepts its CSI frames, and a deterministic way to find out why it does not.

This guide replaces the tutorial body of [RuView#34](https://github.com/ruvnet/RuView/issues/34),
which was pinned to firmware `v0.6.6-esp32` and no longer matched the
repository.

## Contents

- [What does this guide cover?](#what-does-this-guide-cover)
- [What do I need before I start?](#what-do-i-need-before-i-start)
- [Which firmware release do I install?](#which-firmware-release-do-i-install)
- [How do I flash the node?](#how-do-i-flash-the-node)
- [How do I put the node on my network?](#how-do-i-put-the-node-on-my-network)
- [How do I start a server that can accept the node's frames?](#how-do-i-start-a-server-that-can-accept-the-nodes-frames)
- [How do I know the server is accepting live frames?](#how-do-i-know-the-server-is-accepting-live-frames)
- [Why does the dashboard say SIMULATED?](#why-does-the-dashboard-say-simulated)
- [What was measured, and on what?](#what-was-measured-and-on-what)
- [Where does each piece live?](#where-does-each-piece-live)
- [See also](#see-also)
- [References](#references)

## What does this guide cover?

An ESP32-S3 or ESP32-C6 board running the RuView CSI node firmware, sending
ADR-018 binary frames over UDP to a `wifi-densepose-sensing-server` on the
same LAN, and the dashboard that displays them.

It does not cover pose estimation, multi-node TDM fusion, the Cognitum Seed
bridge, or the Cognitum Spaces cloud path. It does not make any accuracy
claim: a node that streams frames is a node that streams frames.

The original issue tutorial was kept open by the maintainer for one reason:

> Keeping this tutorial issue open to consolidate a version-pinned Docker/UDP
> diagnostic flow; `SIMULATED` means the server has not accepted live frames.

That is what this guide is.

## What do I need before I start?

| Item | Requirement | Source |
|---|---|---|
| Board | ESP32-S3 (S3) or ESP32-C6 (C6) | `firmware/esp32-csi-node/README.md` |
| Host | Linux, macOS or Windows with Rust 1.89+ | `v2/rust-toolchain.toml` |
| Flashing | `esptool` 5.x (`pip install esptool`) | `firmware/esp32-csi-node/provision.py` |
| Firmware build (optional) | Docker with the ESP-IDF v5.4 image | `firmware/esp32-csi-node/README.md` |
| Network | 2.4 GHz AP; the node and the server on the same subnet | — |
| Server build (optional) | A prebuilt container | `docker/docker-compose.yml` |

The node and the server must be able to reach each other. The node sends
frames to a single address, configured at provisioning time, and the server
listens on one UDP port. Neither side discovers the other.

## Which firmware release do I install?

Firmware version: `0.8.15` (`firmware/esp32-csi-node/version.txt`).

Releases are tagged `v<version>-esp32` and are published as prereleases. Each
tag ships three archives plus one checksum file:

| Asset | Board | Partition table | App image | Partition image |
|---|---|---|---|---|
| `esp32-csi-node-firmware-8mb.tar.gz` | ESP32-S3, 8 MB | `partitions_display.csv` | `esp32-csi-node.bin` | `partition-table.bin` |
| `esp32-csi-node-firmware-4mb.tar.gz` | ESP32-S3, 4 MB | `partitions_4mb.csv` | `esp32-csi-node-4mb.bin` | `partition-table-4mb.bin` |
| `esp32-csi-node-firmware-c6-4mb.tar.gz` | ESP32-C6, 4 MB | `partitions_4mb.csv` | `esp32-csi-node-c6.bin` | `partition-table-c6.bin` |

`SHA256SUMS.txt` covers the three archives. Verify before flashing.

Each archive contains the same five files, named per variant:

```
bootloader.bin
ota_data_initial.bin
version.txt
<app image>
<partition image>
```

The variant table and the archive names are produced by the firmware release
workflow (`.github/workflows/firmware-ci.yml`), so they cannot drift apart
silently.

## How do I flash the node?

The app image always starts at `0x20000` in every partition table. The
bootloader goes to `0x0`, the partition table to `0x8000` and the OTA data to
`0xf000`.

| Flash size | Partition table | `nvs` | `otadata` | `phy_init` | `ota_0` | `ota_1` |
|---|---|---|---|---|---|---|
| 4 MB | `partitions_4mb.csv` | `0x9000` | `0xf000` | `0x11000` | `0x20000` | `0x1f0000` |
| 8 MB | `partitions_display.csv` | `0x9000` | `0xf000` | `0x11000` | `0x20000` | `0x220000` |
| 16 MB | `partitions_16mb.csv` | `0x9000` | `0xf000` | `0x11000` | `0x20000` | `0x420000` |

The 8 MB layout adds `spiffs` at `0x420000` and `storage` at `0x600000`. The
16 MB layout adds `coredump` at `0x820000` and `storage` at `0x830000`.

Flash the 8 MB board with:

```bash
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write_flash --flash_mode dio --flash_size 8MB \
  0x0     bootloader.bin \
  0x8000  partition-table.bin \
  0xf000  ota_data_initial.bin \
  0x20000 esp32-csi-node.bin
```

The 4 MB board uses the same four offsets, `--flash_size 4MB`, and the `-4mb`
image names.

Writing a smaller partition table over a larger board relocates `nvs` and
destroys the node's provisioning. Match the table to the flash size.

## How do I put the node on my network?

`provision.py` writes the node's NVS partition over USB. It needs the WiFi
credentials and the address of the machine that will run the server.

Keep the WiFi password out of the command line. `--password` is visible in
the process list and in the shell history; `--password-file` reads it from a
file instead.

```bash
python firmware/esp32-csi-node/provision.py --port COM7 \
  --ssid "YourWiFi" \
  --password-file ./wifi-pass.txt \
  --target-ip 192.168.0.45 \
  --target-port 5005 \
  --node-id 1 \
  --edge-tier 2 \
  --force-partial
```

`--target-ip` is the address of the **server host**, not of the node. It is
the single most common reason a node streams to nowhere, and the diagnostic
below is built around it.

`--force-partial` keeps the NVS keys you did not pass, so a later run does
not wipe the OTA key or the WiFi credentials. Use `--reset` when you want the
partition rewritten from scratch.

To change the server address later without USB, push it over the network:

```bash
python firmware/esp32-csi-node/config_push.py --node 192.168.0.107 \
  --set target_ip=192.168.0.45
```

The node reads its configuration at boot, so it restarts to apply the change.
The push is authenticated with the node's OTA PSK, read from a file with
`--psk-file`.

## How do I change the node's target address?

Firmware 0.8.15 exposes a bounded onboarding console on the node's serial
line. It needs no PSK, no reflash and no USB rewriting of the NVS partition,
and it leaves the WiFi credentials and the OTA key untouched.

Ask the node what it is sending to:

```
RUVIEW_HELLO_V1 4f2c9a1b7e0d4c6f8a3b5d1e9c7f0a2b
RUVIEW_HELLO_OK_V1 nonce=4f2c9a1b7e0d4c6f8a3b5d1e9c7f0a2b chip=esp32s3 version=0.8.15 node_id=1 target_ip=192.168.0.57 target_port=5005 configured=1 device_digest=0123456789abcdef
```

The nonce is 32 hexadecimal characters of your choosing and the reply must
arrive within 60 seconds. The `device_digest` is the first eight bytes of a
hash of the node's base MAC address: it identifies the board, so treat it as
an identifier and do not publish it. Then write the new configuration with that nonce:

```
RUVIEW_CONFIG_V1 4f2c9a1b7e0d4c6f8a3b5d1e9c7f0a2b 1 192.168.0.45 5005 KEEP KEEP
RUVIEW_CONFIG_OK_V1 nonce=4f2c9a1b7e0d4c6f8a3b5d1e9c7f0a2b node_id=1 rebooting=1
```

`KEEP KEEP` keeps the WiFi credentials. Give the SSID and the password
instead, base64 encoded, to change them at the same time. The node restarts
and comes back on the new address.

The target address must be a private IPv4 address, and the node id must be
between 1 and 255. Both are checked before anything is written.

This console is the node's own UART line. The README notes that a plain
UART bridge does not carry the ESP32-C6's native USB serial console; on the
ESP32-S3 board measured here, the CH343 bridge does carry it.

## How do I start a server that can accept the node's frames?

Two things must be true, and both are refused by default.

**The API must not be exposed without a token.** Starting on a routable
address with API auth off is refused:

```
ERROR: Refusing to serve the API on routable address 0.0.0.0 with API auth off:
/api/v1/* and /ws/sensing would be readable by anyone who can reach this host.
Set RUVIEW_API_TOKEN=<token> (or RUVIEW_OAUTH_ISSUER) to enable auth,
use --bind-addr 127.0.0.1, or set RUVIEW_ALLOW_UNAUTHENTICATED=1 on a
trusted network only.
```

**The UDP receiver must be allowed to bind a routable address.** By default it
binds `127.0.0.1:5005`, which cannot receive a frame sent from another
machine:

```
UDP data plane security: bind 127.0.0.1:5005 — loopback-only (not reachable
off-host); no source allowlist
```

Set the source allowlist to the node's address and the bind to `0.0.0.0`.
The allowlist restricts who may send; the bind decides whether anyone can
reach the socket at all:

```
UDP data plane security: bind 0.0.0.0:5005 — ROUTABLE, source allowlist
enforced; allowlist active (1 entry), loopback always allowed
```

A native run that can accept a LAN node:

```bash
cargo run -p wifi-densepose-sensing-server --release -- \
  --source esp32 \
  --udp-port 5005 \
  --http-port 8080 \
  --ws-port 8765 \
  --bind-addr 127.0.0.1 \
  --udp-bind 0.0.0.0 \
  --udp-allow 192.168.0.107 \
  --ui-path ../ui
```

`--udp-bind` and `--udp-allow` are also the environment variables
`RUVIEW_UDP_BIND` and `RUVIEW_UDP_ALLOW`, which is how the container sets
them. The capture below used the environment form; the two are equivalent.
`--udp-insecure-lan` accepts a routable bind with no allowlist at all, and
gives up the protection against a spoofed source.

The CLI defaults are `--http-port 8080`, `--ws-port 8765`, `--udp-port 5005`,
`--ui-path ../ui` and `--source auto`. The `3000` and `3001` ports belong to
the container, which publishes `127.0.0.1:3000` for the API and
`127.0.0.1:3001` for the WebSocket. Use the port your process actually binds.

The Docker path needs the same three decisions, made through the environment:

```bash
export RUVIEW_API_TOKEN=$(openssl rand -hex 32)
export CSI_SOURCE=esp32
export RUVIEW_UDP_ALLOW=192.168.0.107
docker compose -f docker/docker-compose.yml up
```

`CSI_SOURCE=auto` fails hard with exit code 78 when neither an ESP32 nor a
host WiFi source is detected. There is no silent fallback to synthetic data.

## How do I know the server is accepting live frames?

Ask the server, not the dashboard. The dashboard shows a client-side state
that can be one step behind the server.

| Question | Endpoint | Live | Not live |
|---|---|---|---|
| Are frames arriving? | `GET /api/v1/status` | `"source_state":"live_unverified"` | `"source_state":"disconnected"` |
| Is the source live? | `GET /api/v1/status` | `"waiting_for_frames":false` | `"waiting_for_frames":true` |
| How old is the last frame? | `GET /api/v1/status` | `"last_frame_age_ms"` is a small number | `"last_frame_age_ms":null` |
| Is the node registered? | `GET /api/v1/nodes` | `"total":1` | `"nodes":[]` |

The state names come from ADR-295 and are not negotiable: a synthetic source
is `synthetic`, a live source with no frame yet is `disconnected`, one whose
frames stopped is `stale`, and a fresh hardware feed is `live_unverified`.
There is no `live_verified`, because this path carries no attestation.

The dashboard reports the same state in its banner:

| Dashboard banner | Meaning |
|---|---|
| `LIVE — ESP32 Hardware Connected` | The server is sending measured CSI frames. |
| `SIMULATED DATA — No Hardware Detected` | The server runs without hardware. Nothing is being measured. |
| `RECONNECTING TO SERVER...` | The browser lost the WebSocket and is retrying. |
| `NO DATA — Server Unreachable, Display Is Stale` | The server is not answering at all. |
| `INVENTED DATA — Generated By Your Browser, Not Measured` | The browser fell back to its own simulation. |
| `API TOKEN REQUIRED` | The server requires a token the browser does not hold. |

## Why does the dashboard say SIMULATED?

Work down this list. Each step has a symptom you can observe.

**1. The node sends to an address that is not this host.** This is the most
common cause. The node keeps the server address it was given at provisioning
time; if the host address changed, the node keeps sending to the old one.

On the node's serial console:

```
W (4529) stream_sender: sendto ENOMEM to 192.168.0.57:5005 - the aggregator
may not be answering on that address (ARP unresolved) or lwIP is out of
packet buffers; backing off for 100 ms (streak 1)
W (4849) stream_sender: sendto suppressed (ENOMEM backoff, 1 dropped)
```

The backoff doubles until it reaches 2 s, and the adaptive controller starts
throttling the CSI yield because the sends are failing — 20.0 Hz down to
9.6 Hz in the run below:

```
I (11259) adaptive_ctrl: medium tick: state=5 yield=5pps dsp=10.1Hz motion=0.00 presence=0.00 rssi=-48
```

One line in the log can look like success while nothing is delivered:

```
I (10209) stream_sender: sendto to 192.168.0.57:5005 succeeded after 6 failed attempts
```

Fix: ask the node where it is sending with the onboarding hello, then write
the current host address to it. See
[How do I change the node's target address?](#how-do-i-change-the-nodes-target-address).

**2. The server's UDP receiver is bound to loopback.** The server log says
`loopback-only (not reachable off-host)`. A native run needs
`RUVIEW_UDP_ALLOW` and `RUVIEW_UDP_BIND=0.0.0.0`, as shown above.

**3. The frames arrive but are dropped as a source.** With an allowlist
configured, a frame from an address outside it is refused. The server log
names the allowed entries; the node's address must be one of them.

**4. Docker Desktop on Windows collapses multi-source UDP.** Every node but
one appears to send from the same address at the WSL/Hyper-V boundary, and
the others are dropped silently. This affects multi-node meshes. The
workaround is the host relay described in `docs/TROUBLESHOOTING.md` §9.

**5. The dashboard is showing its own simulation.** The browser falls back to
client-side simulation after repeated reconnects, and labels it
`INVENTED DATA`. That is a browser state, not a server state. Check
`/api/v1/status` before changing anything on the node.

**6. The API needs a token.** With `RUVIEW_API_TOKEN` set and no valid token
in the browser, the banner reads `API TOKEN REQUIRED` and no data is shown.
This is an auth problem, not a CSI problem.

## What was measured, and on what?

MEASURED on an ESP32-S3 (QFN56 rev 0.2, 16 MB flash, CH343 USB bridge,
`COM5`) running firmware `0.8.15` on a home LAN, with the sensing server
built from this repository at `main` (`fcf3e93f`).

Reproducer: capture the node's boot log over the serial port, and query the
server's REST API. All four captures are committed with this guide:

- [`evidence/esp32-s3-quickstart-2026-10-22/boot-log.txt`](evidence/esp32-s3-quickstart-2026-10-22/boot-log.txt)
  — the node's boot and runtime log while it was sending to an address that
  does not exist, with the SSID, BSSID and node MAC redacted.
- [`evidence/esp32-s3-quickstart-2026-10-22/server-log.txt`](evidence/esp32-s3-quickstart-2026-10-22/server-log.txt)
  — the two server runs and the host's network state at the time.
- [`evidence/esp32-s3-quickstart-2026-10-22/boot-log-live.txt`](evidence/esp32-s3-quickstart-2026-10-22/boot-log-live.txt)
  — the same node booting with the server already listening.
- [`evidence/esp32-s3-quickstart-2026-10-22/server-log-live.txt`](evidence/esp32-s3-quickstart-2026-10-22/server-log-live.txt)
  — the server registering that node.

Real CSI is person data: it encodes breathing, movement, presence and body
geometry. The published captures therefore keep the transport and
registration evidence and drop the occupancy-derived fields — the presence
classification, the person count, the band powers, the subcarrier amplitude
vector and the 17-keypoint pose. `scripts/verify-esp32-quickstart.py` fails
the build if any of them, a raw MAC address or an unmasked password reappears
in the evidence.

| Fact | Value | Where it came from |
|---|---|---|
| Flash layout in use | 16 MB (`coredump` partition found at `820000`) | boot log: `Found partition 'coredump' @ 820000 65536 bytes` |
| Firmware on the node | `0.8.15`, ESP-IDF v5.4 | boot log: `App version: 0.8.15` |
| Node address on the LAN | `192.168.0.107` | boot log: `sta ip: 192.168.0.107` |
| Node's configured server address | `192.168.0.57` | boot log: `NVS override: target_ip=192.168.0.57` |
| CSI frame shape | `len=376 rssi=-52 ch=6` | boot log: `CSI cb #1: len=376 rssi=-52 ch=6` |
| Edge DSP cadence | 20 Hz at start | boot log: `edge DSP cadence=20Hz` |
| Node AP association | `phy: bgn, rssi: -49` | boot log: `wifi:security: WPA2-PSK, phy: bgn, rssi: -49` |
| Server refuses a routable bind without a token | exit before serving | server log: `Refusing to serve the API on routable address 0.0.0.0 with API auth off` |
| Server UDP receiver default | `127.0.0.1:5005`, loopback only | server log: `UDP data plane security: bind 127.0.0.1:5005 — loopback-only` |
| Server UDP receiver with an allowlist | `0.0.0.0:5005`, allowlist enforced | server log: `UDP data plane security: bind 0.0.0.0:5005 — ROUTABLE` |
| Server state with no frames | `"source_state":"disconnected"`, `"waiting_for_frames":true` | `GET /api/v1/status` |
| Server node list with no frames | `{"nodes":[],"total":0}` | `GET /api/v1/nodes` |
| Node repointed at this host | `target_ip=192.168.0.45`, streaming active | boot log: `NVS override: target_ip=192.168.0.45` |
| Server state with live frames | `"source_state":"live_unverified"`, `"waiting_for_frames":false` | `GET /api/v1/status` |
| Age of the last live frame | `"last_frame_age_ms":9` | `GET /api/v1/status` |
| Registered node | `"csi_status":"active"`, `"rssi_dbm":-52.0`, `"total":1` | `GET /api/v1/nodes` |

The ARP state of the node's configured target was also checked from the host:
`192.168.0.57` resolved to `00-00-00-00-00-00` with neighbour state
`Unreachable`, which is what produced the `ARP unresolved` backoff above.

Not measured here: the presence, breathing and heart-rate figures the
firmware computes. Those depend on the room, the AP and the occupant, and
are outside the scope of a setup guide. The repository's own
[capability proof](../proof-of-capabilities.md) and
[ADR-110](../adr/) records are the authority for what the system claims.

## Where does each piece live?

| Piece | Path |
|---|---|
| Firmware version | `firmware/esp32-csi-node/version.txt` |
| Firmware README and build | `firmware/esp32-csi-node/README.md` |
| Firmware runbook | `firmware/esp32-csi-node/RUNBOOK.md` |
| Provisioning over USB | `firmware/esp32-csi-node/provision.py` |
| Configuration over the network | `firmware/esp32-csi-node/config_push.py` |
| Partition tables | `firmware/esp32-csi-node/partitions_display.csv`, `firmware/esp32-csi-node/partitions_4mb.csv`, `firmware/esp32-csi-node/partitions_16mb.csv` |
| Server CLI | `v2/crates/wifi-densepose-sensing-server/src/cli.rs` |
| Container | `docker/docker-compose.yml`, `docker/docker-entrypoint.sh` |
| Dashboard | `ui/` |
| Troubleshooting | `docs/TROUBLESHOOTING.md` |
| User guide | `docs/user-guide.md` |

## See also

- [docs/TROUBLESHOOTING.md](../TROUBLESHOOTING.md) — symptom-first fixes for
  a running system.
- [firmware/esp32-csi-node/README.md](../../firmware/esp32-csi-node/README.md)
  — the firmware reference, including what this firmware does not do.
- [docs/user-guide.md](../user-guide.md) — the server's data sources,
  REST API and deployment options.

## References

- RuView#34 — "Tutorial: ESP32-S3 CSI Pipeline End-to-End Setup (ADR-018)".
- ADR-018 — the in-frame binary frame format the node sends.
- ADR-296 — the UDP source guard (`RUVIEW_UDP_ALLOW`, `RUVIEW_UDP_BIND`).
- ADR-110 — the ESP32-S3 field deployment record.
- `.github/workflows/firmware-ci.yml` — the producer of the release archives
  and the variant table above.
