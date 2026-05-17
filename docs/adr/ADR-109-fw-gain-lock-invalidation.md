# ADR-109 — FW Gain-Lock Invalidation (REST trigger + AP-MAC binding)

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `firmware/esp32-csi-node/main/ota_update.c`,
`firmware/esp32-csi-node/main/csi_collector.c`. Closes both Open Items in
ADR-108.

## Context

ADR-108 persists the FW-side gain-lock (AGC + FFT scale) to NVS so a
reboot resumes detect mode in ~0.5 s. Two follow-ups remained:

1. **No way to clear the cache without USB.** When an operator moved a
   sensor or swapped the AP, they had to plug the device in and run
   `idf.py erase-flash` to force a re-calibration. Defeats the whole
   point of OTA-only ops.
2. **No automatic invalidation on AP swap.** Gain-lock is tied to a
   specific RF path (AP location, distance, multipath). Connecting the
   same sensor to a different AP and re-using the cached AGC/FFT yields
   either over-saturated or under-amplified CSI for the whole session
   until manual intervention.

## Decisions

### D1 — `POST /ota/recalibrate` REST trigger

New HTTP handler registered on the existing port 8032 next to `/ota`
and `/ota/status`. Same Bearer-token auth path as the firmware upload
endpoint (reuses `ota_check_auth`).

Behaviour:

1. Open NVS namespace `csi_cfg` RW.
2. Erase three keys: `gl_agc`, `gl_fft`, `gl_ap_mac` (D2).
3. `nvs_commit` + close.
4. Send `200 OK {status:"ok"}` JSON.
5. `vTaskDelay(1 s)` to flush the response, then `esp_restart()`.

Next boot: `rv_gain_load_from_nvs` returns `ESP_ERR_NVS_NOT_FOUND` →
the existing 300-packet calibration runs as on a never-calibrated chip.

### D2 — `gl_ap_mac` NVS key (6-byte blob)

Stored alongside `gl_agc` / `gl_fft` whenever the calibration writes
back. Source: `esp_wifi_sta_get_ap_info(&ap).bssid`. Read at the same
moment as AGC/FFT during the one-shot NVS short-circuit at the top of
`rv_gain_lock_process`.

Comparison rule on boot:

| Saved MAC          | Current AP MAC          | Action                                |
|--------------------|-------------------------|---------------------------------------|
| all-zero (legacy)  | any                     | Use cached gain-lock (wildcard match) |
| matches current    | same                    | Use cached gain-lock                  |
| differs            | any                     | Log warning, fall through to full cal |
| any                | AP info unavailable     | Defensive: fall through to full cal   |

The all-zero wildcard is the one-time upgrade case: NVS blobs written
by ADR-108 builds predate ADR-109 and have no MAC. Treating them as
match-anything avoids forcing every existing deployment to re-calibrate
on the first ADR-109 boot. The next save (post-re-cal or at the next
natural calibration trigger) populates the real MAC, after which the
strict comparison applies.

### D3 — `rv_gain_save_to_nvs` writes MAC too

Signature changes from `(uint8_t agc, int8_t fft)` to
`(uint8_t agc, int8_t fft, const uint8_t mac[6])`. The caller reads
`ap.bssid` at save time so the saved MAC reflects the AP the
calibration actually ran against (not whatever AP the sensor is
connected to N seconds later, which on a roaming-capable mesh could
differ).

If the save-time AP MAC is unavailable (extremely rare — the gain-lock
hook only fires from a CSI callback, and CSI callbacks require an
established WiFi link), the saved MAC is left as all-zero. The next
boot then takes the wildcard path, preserving the current behaviour
rather than failing closed.

### D4 — Recalibrate handler also clears `gl_ap_mac`

Even though removing only AGC/FFT would force a re-cal by virtue of
the missing keys, also erasing `gl_ap_mac` is cleaner: the next write
will repopulate it from the current AP, and there's no stale MAC
sitting in NVS that could be partially restored by a future bug.

## Trade-offs

* **One-time false re-cal on first ADR-109 boot for chips that ever
  saw an AP swap before this ADR shipped.** Acceptable: gain-lock
  re-cal takes 6-12 s and produces a brief noise spike, but it's a
  one-time event and the result is correct from that point onward.
* **No multi-AP cache.** If a sensor roams between two APs (rare in
  this deployment: each sensor is parked next to a fixed TP-Link)
  it will re-calibrate on every AP swap. Multi-AP storage would need
  per-AP-MAC sub-keys (`gl_agc:<bssid>`, etc.) — explicitly out of
  scope; cross-references ADR-108's per-channel cache item which has
  the same "wait until needed" disposition.
* **`gl_ap_mac` blob doubles NVS size of the gain-lock bundle from
  2 bytes to 8 bytes.** Negligible — the gain-lock namespace `csi_cfg`
  already holds SSID/password/IP and a few other keys totalling a few
  hundred bytes.

## Files Touched

```
firmware/esp32-csi-node/main/ota_update.c
  - ota_recalibrate_handler (D1, D4)
  - register POST /ota/recalibrate

firmware/esp32-csi-node/main/csi_collector.c
  - RV_GAIN_NVS_K_AP_MAC define (D2)
  - rv_gain_load_from_nvs: optional MAC out-param + wildcard support
  - rv_gain_save_to_nvs: MAC in-param + nvs_set_blob (D3)
  - rv_gain_lock_process: AP-MAC comparison branch (D2)
  - rv_gain_lock_process: read current bssid before save (D3)

docs/adr/ADR-109-fw-gain-lock-invalidation.md  (this)
```

## Verified Acceptance

1. `idf.py build` clean (only the pre-existing `wifi_promiscuous_cb`
   unused warning, unchanged by this ADR).
2. After OTA flash of both nodes:
   * `curl -X POST http://192.168.0.100:8032/ota/recalibrate`
   * `curl -X POST http://192.168.0.101:8032/ota/recalibrate`
   Both return `{"status":"ok","message":"gain-lock NVS cleared; rebooting"}`.
3. Boot log on next reboot shows `gain-lock APPLIED:` (full cal) +
   `gain-lock PERSISTED to NVS (AGC=N FFT=M AP=…)` instead of the
   `gain-lock RESTORED from NVS:` line that fast-path boots produce.
4. AP-swap path verified by manually flipping the WiFi credentials to
   a different SSID via `provision.py`, re-flashing, and confirming
   the boot log shows `gain-lock NVS MISS: saved AP=… → current=…
   Re-calibrating.` followed by a full cal.

## References

* ADR-108 — NVS persistence of gain-lock. Both Open Items in ADR-108
  resolved by this ADR (REST trigger, AP-MAC binding).
* ADR-050 — OTA Bearer-token auth. Same `ota_check_auth` shared with
  the new endpoint.
* `docs/references/ota-pipeline.md` — port 8032 recipe; gains a new
  bullet for `/ota/recalibrate`.
