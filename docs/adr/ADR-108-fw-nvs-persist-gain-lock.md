# ADR-108 — FW NVS Persistence of Gain-Lock Values

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `firmware/esp32-csi-node/main/csi_collector.c`
(`rv_gain_load_from_nvs`, `rv_gain_save_to_nvs`, NVS hook in
`rv_gain_lock_process`).

## Context

ADR-100 introduced the FW-side gain-lock (AGC + FFT scale) but the
calibration runs on *every* boot:

1. Collect 300 packets (~3 s at 100 pps, but realistically 6-12 s
   in production where keepalive drives only 25 pps).
2. Take the median of AGC and FFT samples.
3. Call `phy_force_rx_gain` / `phy_fft_scale_force` to freeze.

This means after every reboot — OTA, power blip, watchdog — the chip
goes through 6-12 s where CSI is generated with **unlocked AGC** that
drifts ±20–30 % (the very artefact gain-lock was meant to suppress).
The operator's classifier, ADR-101's NBVI selector, and ADR-103's
baseline comparison all see noisy data during that warm-up.

Pace's ESPectre persists everything calibration-related to NVS so
post-reboot the sensor is back in detect mode in well under a
second. This ADR ports the gain-lock half of that policy
(NBVI lives server-side in RuView, doesn't apply).

## Decisions

### D1 — NVS namespace + keys

```c
#define RV_GAIN_NVS_NS    "csi_cfg"
#define RV_GAIN_NVS_K_AGC "gl_agc"     // u8
#define RV_GAIN_NVS_K_FFT "gl_fft"     // i8
```

`csi_cfg` is the same namespace the WiFi creds / collector IP / node_id
live in (so it's already initialised + checked by `nvs_config_load`).
Two single-byte values — minimal NVS footprint.

### D2 — Two thin helpers

```c
static esp_err_t rv_gain_load_from_nvs(uint8_t *agc, int8_t *fft);
static void      rv_gain_save_to_nvs(uint8_t agc, int8_t fft);
```

Both are local to `csi_collector.c`. Load returns `ESP_ERR_NVS_NOT_FOUND`
on a fresh chip; save logs a warning but never blocks the boot path
if NVS write fails.

### D3 — One-shot NVS load at top of `rv_gain_lock_process`

A static `s_nvs_checked` flag triggers exactly **one** load attempt
on the first packet after boot:

```c
if (!s_nvs_checked) {
    s_nvs_checked = true;
    uint8_t agc; int8_t fft;
    if (rv_gain_load_from_nvs(&agc, &fft) == ESP_OK
        && agc >= RV_GAIN_MIN_SAFE_AGC)
    {
        phy_fft_scale_force(true, fft);
        phy_force_rx_gain(1, (int)agc);
        s_gain_locked = true;
        ESP_LOGI(TAG, "gain-lock RESTORED from NVS: AGC=%u FFT=%d", agc, fft);
        return;
    }
}
```

The `agc >= RV_GAIN_MIN_SAFE_AGC` guard preserves ADR-100's "skip if
signal too strong" safety: a stale low-AGC value that would freeze
the RX path is rejected even if it's in NVS.

### D4 — Save after every successful lock

The existing `phy_*_force` branch in `rv_gain_lock_process` is wrapped
with a save call:

```c
phy_fft_scale_force(true, s_gain_fft_value);
phy_force_rx_gain(1, (int)s_gain_agc_value);
rv_gain_save_to_nvs(s_gain_agc_value, s_gain_fft_value);
ESP_LOGI(TAG, "gain-lock PERSISTED to NVS (%s/%s, %s)",
         RV_GAIN_NVS_NS, RV_GAIN_NVS_K_AGC, RV_GAIN_NVS_K_FFT);
```

So the first boot ever does the full 300-packet calibration **and**
saves; every subsequent boot loads instantly from D3.

### D5 — Invalidation policy

Stored values are tied to: this sensor's physical location + this AP's
MAC + this channel + this antenna orientation. If any of those change,
the saved AGC/FFT may be slightly off-optimal — but **not dangerous**.
The WiFi PHY just receives slightly off-optimal CSI; the host will
see higher baseline noise until the operator triggers a re-calibration.

Today: erase via `idf.py erase-flash` over USB, or `nvs_flash_erase()`
called from a future REST endpoint. No automatic invalidation — the
operator decides when a deployment change is significant enough.

## Files Touched

```
firmware/esp32-csi-node/main/csi_collector.c
  - #include "nvs.h" / "nvs_flash.h"
  - rv_gain_load_from_nvs / rv_gain_save_to_nvs (D2)
  - s_nvs_checked one-shot in rv_gain_lock_process (D3)
  - save call after lock branch (D4)
docs/adr/ADR-108-fw-nvs-persist-gain-lock.md  (this)
```

Implementation commit: `3779bb76`. Flashed to both sensors via OTA
(no USB) — `python3 scripts/ota-deploy.sh`.

## Verified Acceptance

Test sequence:

1. OTA flash new FW to both nodes (first boot, NVS empty).
2. Wait 15 s for FW to complete first calibration + write to NVS.
3. OTA flash the SAME binary again (forces a reboot; new FW has
   values in NVS from step 2).
4. Sample WS amplitude rate in the first 3 s after the second boot.

Before this ADR: ~5-12 s gap between boot and first amp-bearing WS
frame (waiting for fresh calibration). After this ADR: WS shows
**44 Hz raw CSI in the first 3 s** — instant resume.

Logs from a chip that has values in NVS:

```
I (335) main: boot: reset_reason=SW running_partition=ota_1
I (520) csi_collector: gain-lock RESTORED from NVS: AGC=44 FFT=-33
                       (0-packet calibration; clear NVS to recalibrate)
```

vs first-boot ever:

```
I (335) main: boot: reset_reason=POWERON running_partition=ota_0
I (4980) csi_collector: gain-lock APPLIED: AGC=44 FFT=-33
                        (median of 300 packets)
I (4980) csi_collector: gain-lock PERSISTED to NVS (csi_cfg/gl_agc, gl_fft)
```

## Open Items

* **Per-channel cache** — `csi_cfg/gl_<chan>_agc`. If the channel hop
  table (ADR-029) is reactivated, each channel needs its own values.
  ~1 h FW. Deferred — channel hopping is out of scope for the current
  single-channel deployment.

## Closed

* **REST endpoint to clear gain-lock NVS** — shipped via
  `POST /ota/recalibrate` in ADR-109.
* **Track AP MAC alongside AGC/FFT** — shipped via `gl_ap_mac` NVS key
  + boot-time comparison in ADR-109.

## References

* ADR-100 — gain-lock implementation that this ADR persists.
* ADR-101 — classifier that suffers during the 6-12 s warm-up gap
  that this ADR closes.
* `docs/references/ota-pipeline.md` — the WiFi flash flow used to
  deploy this FW change without USB.
* Francesco Pace, *How I Turned My Wi-Fi Into a Motion Sensor —
  Part 2*, "Persisted calibration" — the upstream pattern this ADR
  ports (their NVS payload also includes NBVI indices + baseline,
  which RuView keeps server-side).
