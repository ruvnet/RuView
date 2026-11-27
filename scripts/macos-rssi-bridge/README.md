# macos-rssi-bridge

Mac WiFi card → RuView sensing-server bridge. Lets you do RSSI-based motion
sensing on a stock Mac with **no CSI hardware** while you wait for ESP32-S3
boards to arrive (or as a permanent extra "node" once they do).

## What it does

1. A small Swift helper (`mac_wifi`) wraps Apple's CoreWLAN framework and
   prints one JSON line per visible BSSID, ~once per second. It replaces the
   single-AP v1 helper at `archive/v1/src/sensing/mac_wifi.swift` with the
   multi-BSSID format that `v2/.../macos_scanner.rs` expects.
2. A Rust binary (`macos-rssi-bridge`) spawns the helper, maintains a rolling
   per-AP RSSI history, and packs each scan into an ESP32 CSI frame
   (`magic = 0xC511_0001`) emitted over UDP — the same wire format
   `wifi-densepose-sensing-server` expects from real ESP32 nodes.

The sensing-server pipeline runs unmodified. RSSI per AP becomes a
pseudo-subcarrier amplitude; per-AP variance becomes the Q channel.

## Why it works

Every AP in range beacons every ~100 ms. When a person moves, multipath
reflections shift and RSSI fluctuates. With 5–15 visible APs (your router +
neighbors'), running variance and cross-AP correlation gives a real motion
signal — coarser than CSI but useful for through-wall presence and
room-level activity. See [`README.md`](../../README.md) (line ~134, "Any WiFi:
RSSI-only") and [ADR-022](../../docs/adr/ADR-022-multi-bssid-scanning.md).

## Build

```bash
make build           # compiles mac_wifi (Swift) + macos-rssi-bridge (Rust)
make scan            # smoke test: print JSON for one scan
make test            # unit tests (frame builder, variance, RSSI mapping)
```

Toolchain: `swiftc` 6.0+ and Cargo 1.80+. Tested on macOS 26 (Tahoe) arm64.

## Run

One command — builds everything, launches the full three-process pipeline,
opens both UIs, and cleans up on Ctrl-C:

```bash
make start           # → sensing-server (:8080) + bridge (:9090/dashboard) + presence injector
make stop            # kill any leftover process from the trio
```

The pipeline is three processes:

1. **sensing-server** — UDP receiver, motion stats, WebSocket stream, UI
   (pinned to `--source esp32` so it binds UDP and consumes the bridge's
   frames instead of falling back to its simulation generator).
2. **macos-rssi-bridge** — Swift scan → Rust UDP emitter → ESP32 frame format.
3. **presence_to_pose.py** — polls the bridge's `/aps` endpoint and POSTs a
   single honest pose to `/api/v1/pose/external` so the Observatory renders
   one figure modulated by RSSI variance instead of the placeholder
   five-skeleton fallback.

Or run each manually if you want logs side-by-side. In one terminal, the
sensing-server (UI + UDP receiver):

```bash
cd ../../v2 && cargo run --release -p wifi-densepose-sensing-server --no-default-features -- --source esp32
# UI: http://localhost:8080
# WS: ws://localhost:8765/ws/sensing
```

In another, the bridge:

```bash
make run             # default: target 127.0.0.1:5005, 1.5 s scan interval
# or: make run-verbose   for per-frame stats on stderr
```

In a third, the presence injector (only needed for the 3D Observatory pose):

```bash
make run-presence    # polls :9090/aps, posts a pose to :8080/api/v1/pose/external
```

Open http://localhost:8080 and walk around. Motion should register within a
few seconds (the pipeline needs ~5 frames of baseline before it'll classify).

## Tuning

| Variable      | Default      | Notes                                                 |
|---------------|--------------|-------------------------------------------------------|
| `TARGET_HOST` | `127.0.0.1`  | Where the sensing-server is listening                 |
| `TARGET_PORT` | `5005`       | UDP port — matches `--udp-port` on sensing-server     |
| `INTERVAL`    | `1.5`        | Seconds between active scans (helper enforces 0.5 floor) |

```bash
TARGET_HOST=192.168.1.50 TARGET_PORT=5005 make run
```

## macOS Location Services note

macOS Sonoma 14.4+ redacts BSSIDs to `00:00:00:00:00:00` and SSIDs to empty
strings unless the calling process holds the `com.apple.wifi.scan` entitlement
or has been granted Location Services authorization. The bridge handles the
redacted case automatically — the Swift helper synthesizes stable per-AP
identifiers from `(channel, RSSI bucket, ordinal)` so downstream code still
sees distinct virtual APs.

To get **real** SSIDs and BSSIDs (better tracking quality across scans):

1. **System Settings → Privacy & Security → Location Services → On**
2. Scroll to your terminal app (Terminal.app, iTerm2, Cursor, etc.) and toggle it on.
3. Re-run `make scan` — SSIDs and BSSIDs should now be populated.

This is optional. Sensing works without it; it's just less stable across scans
because the synthetic ordinal can shuffle.

## Limits vs. real CSI

| Capability                     | RSSI bridge | ESP32-S3 CSI |
|-------------------------------|-------------|--------------|
| Coarse presence / motion      | Yes (~3–5 m) | Yes (~5 m)   |
| Through-wall detection        | Limited     | Yes          |
| Breathing rate                | No (~0.5 Hz frame rate is too slow) | Yes (real-time) |
| Heart rate                    | No          | Yes          |
| 17-keypoint pose              | No          | Yes (with trained model) |
| Fall detection                | Coarse      | Yes (<200 ms) |
| Multi-person counting         | No          | Yes (3–5 per AP) |

Treat the bridge as a "node 0" you keep around for environmental fingerprinting
and motion baseline, alongside the real ESP32 mesh.

## Files

- `mac_wifi.swift` — CoreWLAN multi-BSSID scanner, JSON-lines output
- `src/main.rs` — Rust UDP emitter (ESP32 frame format `0xC511_0001`)
- `Cargo.toml` — standalone Cargo project; not a workspace member, so it
  doesn't perturb the v2 build
- `Makefile` — `build`, `scan`, `run`, `listen`, `test`, `clean`

## Wire format

Reproduced from `v2/crates/wifi-densepose-sensing-server/src/csi.rs`:

```
bytes  0..4   u32 magic = 0xC511_0001 (LE)
byte   4      u8  node_id
byte   5      u8  n_antennas      = 1
byte   6      u8  n_subcarriers   = 56
byte   7      _   skipped
bytes  8..10  u16 freq_mhz (LE)
bytes 10..14  u32 sequence (LE)
byte  14      i8  rssi (strongest AP)
byte  15      i8  noise_floor (mean across APs)
bytes 16..20  _   header padding (iq_start = 20)
bytes 20..    i8  I/Q pairs (n_antennas * n_subcarriers, I=amp, Q=variance)
```
