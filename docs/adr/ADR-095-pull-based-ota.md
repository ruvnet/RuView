# ADR-095: Pull-based OTA Firmware Update

## Status

Proposed

## Context

ESP32 sensing nodes deployed in user homes need firmware updates without
operator-side push access. Push-based OTA (server initiates upgrades to a
known set of node IPs) is operationally heavy for consumer-grade deployments:

- Operators must enumerate every node's IP address and schedule rollouts.
- Nodes that come online intermittently or behind NAT get missed entirely.
- A node in a bad state (e.g. hung at startup) may never receive a push.

For a consumer sensing system where nodes are embedded in rooms and accessed
infrequently, this creates a support burden and leaves nodes on stale firmware.

## Decision

Adopt a pull-based OTA model: each node periodically polls a server manifest
endpoint and self-upgrades when a newer version is available. Operators publish
new firmware to the server; nodes fetch it at their next poll cycle.

## Architecture

### Server side — `firmware_registry` module

`v2/crates/wifi-densepose-sensing-server/src/firmware_registry.rs` provides
a pure-data, transport-agnostic registry:

- `FirmwareRegistry` — in-memory holder for the currently-blessed firmware
  binary: version, SHA-256 hex digest, byte size, file path, compile time.
- `set_current(path)` — reads a file from disk, computes SHA-256, parses the
  version string from either a sidecar `.manifest.json` or the filename
  (patterns: `esp32-csi-node-0.8.0-watchdog.bin`).
- `is_update_available(running_version)` — simple string comparison helper.
- `sha256_bytes(&[u8])` + `sha256_file(Path)` — pure-Rust SHA-256 helpers
  using the `sha2` crate.
- Minimum firmware size: 256 KB (rejects truncated uploads).
- 11 unit tests covering hex encoding, version parsing, manifest sidecar
  priority, size rejection, missing-file rejection, and SHA-256 round-trips.

### Server HTTP endpoints (wired in `main.rs`)

| Method | Path | Purpose |
|--------|------|---------|
| `GET`  | `/api/v1/firmware/latest` | Returns `{available, version, sha256, size, compile_time, download_url}` |
| `GET`  | `/api/v1/firmware/download` | Streams binary with `X-Firmware-Version` + `X-Firmware-Sha256` headers |
| `POST` | `/api/v1/firmware/upload?version=X[&sha256=HEX]` | Operator uploads; server computes SHA-256, optionally verifies client-supplied hash, writes to `<firmware_dir>/esp32-csi-node-<version>.bin` |

On startup the server scans `--firmware-dir` (env `FIRMWARE_DIR`, default
`/app/data/firmware`) for the newest `.bin` by mtime and seeds the registry.
This is non-fatal — the server starts normally if no firmware is staged.

### Firmware client — `ota_pull` module

`firmware/esp32-csi-node/main/ota_pull.c` (+413 LOC):

1. `GET /api/v1/firmware/latest` — parse `{available, version, sha256, size}`.
2. Compare `version` with the compile-time `esp_app_desc.version`.
3. If newer: `GET /api/v1/firmware/download` — write binary to the ESP-IDF
   OTA partition via `esp_ota_ops`.
4. Verify SHA-256 of downloaded bytes against the server-advertised hash.
5. Call `esp_ota_set_boot_partition` and `esp_restart()`.

Guards:
- Waits for `OTA_MIN_UPTIME_SEC` (300 s) before first check — avoids
  boot-loop on a node that OTA'd to bad firmware.
- Stops BLE before flashing to prevent Core 1 StoreProhibited crash.
- Aborts if the download exceeds `OTA_MAX_SIZE`.
- Graceful failure on network error — retries on next poll cycle.

Poll interval: `OTA_CHECK_INTERVAL_SEC` = 300 s (configurable at compile time).

### Rollback (ESP-IDF built-in)

The ESP-IDF OTA partition scheme includes an application rollback mechanism.
After `esp_ota_set_boot_partition`, the new firmware must call
`esp_ota_mark_app_valid_cancel_rollback()` within a configurable window, or
the bootloader rolls back to the previous partition. `ota_pull.c` relies on
the existing `ota_update.c` canary task for this confirmation.

## Consequences

**Positive:**
- Zero operator action for routine upgrades; nodes that come online late catch
  up automatically on their next poll cycle.
- Tolerates intermittent connectivity — retry is just the next poll tick.
- No inbound firewall holes required — nodes initiate all connections.
- Latecomers behind NAT/CGNAT are handled identically to nodes on the LAN.

**Negative:**
- Upgrade latency is up to one poll interval (default 5 minutes).
- The manifest endpoint is discoverable; anyone who can reach the server can
  learn the current firmware version and download the binary. Mitigated by
  network segmentation; manifest signing is out of scope for this ADR.
- Poll traffic at scale: 11 nodes × 1 req/5 min = ~2 req/min steady-state.
  Negligible.

## Related

- Firmware client: `firmware/esp32-csi-node/main/ota_pull.c` + `ota_pull.h`
- Server registry: `v2/crates/wifi-densepose-sensing-server/src/firmware_registry.rs`
- Server wiring: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
  (routes `/api/v1/firmware/*`, `AppStateInner::firmware_registry`, `scan_firmware_dir`)
- ADR-018: ESP32 binary frame format (firmware identity)
- ADR-057: Firmware CSI build guard
