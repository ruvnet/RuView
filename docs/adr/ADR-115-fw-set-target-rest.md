# ADR-115 — FW REST endpoint to repoint CSI aggregator without USB

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `firmware/esp32-csi-node/main/ota_update.c`
(`ota_set_target_handler`, `parse_ip_port`, URI registration on port 8032).

## Context

After moving the Mac from Tran Thanh T3 (192.168.1.x) to TP-Link_8340
(192.168.0.x) for low-latency sensor proximity, both ESP32-S3 nodes
held a stale `csi_cfg/target_ip` in NVS — they were silently streaming
CSI into the previous LAN and the new server on `0.0.0.0:5005` saw
zero frames for ~5 minutes despite both nodes being WiFi-reachable
and responding on `:8032/ota/status`.

Existing tools didn't cover this:

* `provision.py` writes `target_ip` via USB serial — requires
  physical access to the sensor.
* `/ota/recalibrate` (ADR-109) only erases gain-lock keys
  (`gl_agc/gl_fft/gl_ap_mac`) — intentionally doesn't touch
  network config.
* Rebuilding FW with a new `CONFIG_CSI_TARGET_IP` would only help if
  NVS is also wiped, since the NVS override always beats the
  compile-time default.

Recurring operational need: every Mac IP change, every network
move, every router swap requires the operator to crawl behind the
sensor with a USB cable. Not acceptable.

## Decisions

### D1 — `POST /ota/set-target` HTTP endpoint

New handler on the existing OTA HTTP server (port 8032). Body is
plain text `"IPv4:PORT"` with optional trailing CR/LF, e.g.
`192.168.0.103:5005`. No JSON dependency — `cJSON` is not used
elsewhere in this FW.

```
POST /ota/set-target HTTP/1.1
Content-Type: text/plain
Authorization: Bearer <psk>   # only if ota_psk provisioned

192.168.0.103:5005
```

Response:

```json
{"status":"ok","target_ip":"192.168.0.103","target_port":5005,"message":"rebooting"}
```

Followed by `vTaskDelay(1s)` + `esp_restart()` so the new value is
picked up by `nvs_config_load` on next boot.

### D2 — Strict body parser (no `inet_pton` dependency)

`parse_ip_port` validates:

* Exactly 4 dot-separated octets, each `0–255`.
* Single `:` separator.
* Port `1–65535`, max 5 digits.
* Trailing whitespace/CR/LF tolerated.

Rejects malformed input with HTTP 400 *before* touching NVS — a
sensor with an unparseable IP would lose its only network identity.

### D3 — Same NVS namespace + keys that `nvs_config.c` reads

```c
nvs_open("csi_cfg", NVS_READWRITE, &h);
nvs_set_str(h, "target_ip", ip);
nvs_set_u16(h, "target_port", port);
nvs_commit(h);
```

Matches the keys already read by `nvs_config_load` at boot, so the
change is picked up without any FW code change beyond this handler.

### D4 — Auth model identical to `/ota/recalibrate`

Uses the same `ota_check_auth` PSK gate (ADR-050). If
`security/ota_psk` is empty, the endpoint is open (dev mode); when
set, requires `Authorization: Bearer <psk>`. Same threat model and
permissive default as `/ota` itself.

### D5 — No partial-write atomicity gymnastics

We write `target_ip`, then `target_port`, then commit. If a power
cut happens between `set_str` and `set_u16`, NVS keeps the previous
`target_port` (since uncommitted writes don't persist) — safe
behaviour. No need for a temp-key + rename dance.

## Files Touched

```
firmware/esp32-csi-node/main/ota_update.c
  + #include "nvs_config.h"  (NVS_CFG_IP_MAX)
  + parse_ip_port helper
  + ota_set_target_handler
  + URI registration in ota_update_start_server
  + log line in startup banner
docs/adr/ADR-115-fw-set-target-rest.md  (this)
```

Binary size delta: `esp32-csi-node.bin` 854 KB → 855 KB (+~1 KB).
58 % of OTA partition free, plenty of margin.

## Verified Acceptance

Sequence on both live nodes (192.168.0.100, 192.168.0.101):

1. `python3 scripts/ota-deploy.sh 192.168.0.100 192.168.0.101` →
   `running_partition` flipped on both (`ota_1↔ota_0`).
2. `curl -X POST -d '192.168.0.103:5005' .../ota/set-target` →
   `{"status":"ok","target_ip":"192.168.0.103","target_port":5005,...}`
   on both nodes.
3. After 25 s reboot+WiFi+CSI startup, sensing-server log:
   ```
   keepalive: learned address for node 2 = 192.168.0.100:63940
   keepalive: ping -i 0.040 192.168.0.100 for node 2
   keepalive: learned address for node 1 = 192.168.0.101:63844
   keepalive: ping -i 0.040 192.168.0.101 for node 1
   ```
4. `GET /api/v1/sensing/latest` → live classification
   (`motion_level: active`, presence: true) with non-zero
   per-node features (`drift_score: 0.41`, `dominant_freq_hz: 6.3`,
   `mean_rssi: -57`).

End-to-end recovery time from broken stream → live CSI: **~3 min**
(build 0, since FW was already built; flash 17 s; set-target +
reboot ~25 s; first ping-driven CSI batch ~5 s).

## Open Items

* **Persist last-known-good target as fallback** — if a bad
  `target_ip` is committed (e.g. operator types Mac's old IP) the
  sensor goes silent until the next set-target call. A
  `csi_cfg/target_ip_lkg` snapshot updated on every successful
  keepalive-driven UDP send would let the sensor self-revert after
  N silent seconds. ~1 h FW.
* **Track AP MAC alongside target** — ADR-108 / ADR-111 already
  invalidate gain-lock on AP change; same pattern could
  auto-invalidate target on subnet change (sensor sees its DHCP
  lease is on a different /24 than `target_ip` → blank target,
  refuse to send until operator confirms). ~1 h FW.
* **REST endpoint to read current target** — `GET /ota/target`
  returning `{"target_ip":..., "target_port":...}`. Operator can
  diagnose "where is this sensor pointed?" without USB. ~15 min FW.

## References

* ADR-050 — OTA PSK auth that gates this endpoint
* ADR-100 — TP-Link WISP deployment that triggered the Mac-IP move
* ADR-108 — FW NVS persistence patterns (same namespace, same approach)
* ADR-109 — `/ota/recalibrate` precedent (same handler shape, same
  reboot semantics)
* `scripts/provision.py` — original USB-only NVS provisioning path
  that this ADR replaces for the network-config case
