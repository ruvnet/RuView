# ADR-383: ESP32-C5 firmware extension — dual-band (2.4 + 5 GHz) Wi-Fi 6 CSI

| Field | Value |
|-------|-------|
| **Status** | Proposed — P1-P4 done on real C5 silicon (2026-10-05): builds on IDF 5.5.2+, boots on PSRAM and non-PSRAM modules, 5-minute qualification PASS on 2 nodes at 5 GHz and 2.4 GHz with 245-bin HE-SU CSI, band option, CI build job; 802.15.4 time-sync fixed but opt-in (follow-ups in #2162). Pending: calibrated empty-room occupancy (P5; device-side bar met, uncalibrated server fails) and C6 hardware run of the 15.4 fixes |
| **Date** | 2026-10-04 (created) |
| **Deciders** | Mathew Beane (WeaveLogic) |
| **Codename** | **C5-DUALBAND** |
| **Extends** | [ADR-110](ADR-110-esp32-c6-firmware-extension.md) (ESP32-C6 firmware extension — the template this mirrors) |
| **Relates to** | ADR-018 (CSI binary frame format), ADR-029 (RuvSense multistatic — "5 GHz unavailable on S3; C6 for dual-band"), ADR-347 (rate-aware sensing), ADR-346 (fail-closed occupancy), ADR-357 (raw-CSI calibration integrity), ADR-304 (evidence engine), ADR-182 (harness hardening) |
| **Hardware** | ESP32-C5-WROOM-1 (rev v1.0), 16 MB flash, no PSRAM, native USB-Serial/JTAG, on an ESP32-C5-DevKitC-1 |
| **Toolchain** | ESP-IDF **v5.5.2+** (`esp32c5` is a *preview* target — build with `idf.py --preview set-target esp32c5`). 5.5.2 carries the C5 PSRAM reset-hang fix; PSRAM modules need it |
| **Updated** | 2026-10-05 — real cause of the C5 lockup/ROM wedge found and fixed (mmWave probe on the flash bus); PSRAM modules supported; 5 GHz CSI streaming confirmed; HE-SU 245-bin CSI after `force_lltf=0`; 5-min qualification PASS on 2 nodes at 5 GHz and 2.4 GHz; band option added; 802.15.4 RX re-tested on C5 and still delivers nothing (see docs/validation/2026-10-04-esp32-c5-rate-aware-sensing.md) |

---

## 1. Context

ADR-110 brought the RuView CSI node to the ESP32-C6: Wi-Fi 6 (HE) CSI, 802.15.4,
TWT and an LP-core, qualified at 8 Hz on-device DSP. The C6's one hard limit for
sensing is that its Wi-Fi is **2.4 GHz only** — ADR-029 explicitly notes "5 GHz
CSI unavailable on S3; ESP32-C6 for dual-band", but the C6 cannot actually
*source* 5 GHz. The host/provisioning side already understands 5 GHz channels
(`provision.py` accepts 36–177; the firmware README ships a `5ghz-channel`
preset; `csi_collector.c` already hops `{1,6,11,36,40,44}`), so the whole stack
has been waiting for a chip that can transmit there.

The **ESP32-C5** is that chip: dual-band Wi-Fi 6 (2.4 **+ 5 GHz**), BT 5 LE,
IEEE 802.15.4, a 240 MHz HP RISC-V core + LP core. It is the same
`SOC_WIFI_HE_SUPPORT` HE class as the C6, which is why most of the C6 firmware
applies unchanged — and the 5 GHz band's ~6 cm wavelength (vs 12.5 cm at
2.4 GHz) is finer motion detail for sensing, on much cleaner air.

### 1.1 What this ADR is *not*

Not a new CSI pipeline, frame format, evidence methodology or qualification
procedure. The C5 image is a *frozen artifact*; the SAME ADR-110 qualification
harness (`harness/ruview/`, `run_arms.py`, the dated validation `.md`) runs
against it and emits the SAME evidence. This ADR is the bring-up of a new build
target, nothing more.

## 2. Decision

Add `esp32c5` as a third build target alongside `esp32s3` (production) and
`esp32c6` (research), by generalizing the C6's HE-class feature gates to the
C6/C5 class rather than forking the firmware.

### 2.1 Target overlay

`sdkconfig.defaults.esp32c5` (new), layered by `idf.py set-target esp32c5`:
- **16 MB flash**, `partitions_16mb.csv` (two 4 MB OTA slots — real dual-image
  OTA, vs the C6's single 4 MB), with `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`
  + coredump folded in (the `sdkconfig.defaults.16mb` settings, which IDF does
  **not** auto-apply beside a target overlay).
- **240 MHz** CPU ceiling (vs the C6's 160).
- USB-Serial/JTAG console (required for `RUVIEW_HELLO_V1` onboarding), CSI
  enabled, WPA3, LP-core, ESP-NOW time-sync (802.15.4 PHY available but kept off
  at runtime for the same reason as the C6 — see ADR-110 D1 / #762).
- `CONFIG_EDGE_DSP_SAMPLE_HZ=8` — **provisional**; the 240 MHz core should
  sustain more, but the qualifying number is MEASURED, not assumed (ADR-347).

### 2.2 Gate generalization (the mechanical core)

The C6 modules were gated `#if defined(CONFIG_IDF_TARGET_ESP32C6)`. Every gate
for a feature the C5 *also* has was widened to
`defined(CONFIG_IDF_TARGET_ESP32C6) || defined(CONFIG_IDF_TARGET_ESP32C5)` — 38
sites across 18 files: `c6_softap_he`, `c6_lp_core`, `c6_twt`, `c6_timesync`,
`thermal`, the `edge_processing`/`csi_collector` DSP defaults, and `main.c`
feature inits. Distinct-value sites got their own C5 branch (`serial_onboarding`
chip name → `"esp32c5"`, `main.c` target name → `"ESP32-C5"`, LED GPIO). The
Kconfig capability menu and `THERMAL_MONITOR`/`EDGE_DSP_SAMPLE_HZ` gates widened
to `IDF_TARGET_ESP32C6 || IDF_TARGET_ESP32C5`. `main/CMakeLists.txt` adds the
`ieee802154 ulp esp_hw_support` requirements and the LP-core `ulp_embed_binary`
step for C5 as for C6.

**Left C6-only:** `c6_antenna_select.c` + `CONFIG_C6_XIAO_ANTENNA_SELECT` — that
is the Seeed **XIAO** ESP32-C6 RF-switch, board-specific to that module, not our
WROOM-1 DevKitC.

**Deferred:** the mmWave companion (`mmwave_sensor.c` C5 pin map) — out of scope
for CSI bring-up; it is a separate board.

### 2.3 5 GHz

The host side already understands 5 GHz. Qualify the C5 on a **non-DFS** channel
(UNII-1: 36/40/44) — the firmware has **no DFS/radar-avoidance**, so UNII-2
(52–144) is out until that is added. Provision the C5 node onto a 5 GHz channel
to exercise the band that is its reason for being.

## 3. Consequences

### 3.1 Wins

- Buildable, flashable C5 CSI node with the full C6 pipeline — **no fork**.
- 16 MB → two 4 MB OTA slots: **73 % app headroom** (vs the C6's 45 % in 4 MB).
- First in-house chip that can source **5 GHz CSI**.

### 3.2 Costs / risks

- `esp32c5` is an IDF **preview** target on v5.5 — pinned to `--preview` and may
  shift as IDF stabilizes it.
- **HE-frame risk**: ADR-110 (issue #1005) records that true 256-bin HE-LTF CSI
  needs IDF **≥ 5.5.2** — the v5.4 blob silently downconverts HE→64-bin HT. We
  are on **v5.5.0**; this must be confirmed empirically at capture time, and IDF
  bumped to 5.5.2+ if the first HE frame comes back 64-bin.
- DSP cadence is provisional until measured on C5 silicon.
- PSRAM: retail C5 modules (N8R8/N16R8) carry in-package PSRAM on the same MSPI
  bus as flash. The overlay enables `CONFIG_SPIRAM` with `CONFIG_SPIRAM_IGNORE_NOTFOUND`,
  so both PSRAM and no-PSRAM modules boot. Never route peripherals to GPIO15-22 on C5.
- C5-DevKitC-1 LED GPIO is a best-guess (27) pending schematic confirmation —
  cosmetic only.

### 3.3 Verification

| Gate | State | Evidence |
|---|---|---|
| `idf.py --preview set-target esp32c5` | **PASS** | `CONFIG_IDF_TARGET="esp32c5"` in sdkconfig (2026-10-04) |
| `idf.py build` (full) | **PASS** | `Project build complete`; `esp32-csi-node.bin` = **1,151,440 B**, 73 % free in 4 MB OTA slot |
| app image SHA-256 | recorded | `3577a9082d8de4703b8e0830ad70d65f65f6c04f9f696d74250574cd423a1281` |
| bootloader SHA-256 | recorded | `a523d2c877fe719e4f780a3f76ab740800187524fbf6daabe427320ee4c4ecf5` |
| flash + hash verify on silicon | **PASS** | `Wrote 1,152,000 B @ 0x20000 … Hash of data verified`; `--chip esp32c5` (2026-10-04) |
| boots + runs on C5 | **PASS** | serial: `ESP32-C5 CSI Node (ADR-018 / ADR-110) — v0.8.12 — Node ID: 1`; CSI collector + bounded serial onboarding up; 240 MHz; coredump partition live; **WiFi `band mode:0x3` (dual-band 2.4+5 GHz active)** (2026-10-04) |
| WiFi join + CSI capture on silicon | **PASS (partial)** | provisioned (NVS), joined WiFi (`Got IP` (LAN address)), auto-detected AP ch 5, promiscuous CSI up, **first CSI callback fired** (`CSI cb #1: len=106 rssi=-42 ch=5`) (2026-10-04) |
| continuous-run stability | **FIXED + VERIFIED (2026-10-05)** | Root cause was not TWT. `mmwave_sensor_init()` fell through to the S3 default UART1 pins **GPIO17/18**, which on the C5 are the flash/PSRAM MSPI bus (MISO=17, WP=18; bus = GPIO15-22). About 6 s after boot the probe rerouted them, the CPU locked up (`rst:0x1a`, PC in `panic_handler`, no core dump), and the flash was left mid-transaction, so every following reset looped in ROM on `SPI flash busy detected(0x0f)` + `TG0_WDT` until a power cycle. That was the "ROM-stage wedge" seen on 2026-10-04. Fix: C5 probe defaults to GPIO4/5 and refuses GPIO15-22; C5 console pins (11/12) used for the overlap check. Verified on an ESP32-C5-WROOM-1 with 8 MB PSRAM: 45 s+ with no reset, probe on TX=4/RX=5, CSI streaming. TWT stays off (`C6_TWT_ENABLE=n`). |
| physical CSI rate (≥ 20 pps raw) | **PENDING** | 5-min `:8032` poll once the TWT-off image runs stably (needs power-cycle) |
| DSP cadence (±1 Hz of configured) | **PENDING** | measure, then pin `CONFIG_EDGE_DSP_SAMPLE_HZ` |
| 5 GHz CSI on silicon | **PASS (partial)** | 2026-10-05, IDF 5.5.2: joined the AP on ch 40 (5200 MHz, 11ax), 861 CSI frames in 30 s at the server (about 29 fps), 81 % PPDU 0x01 (HE-SU), 19 % 0x00 |
| 5 GHz HE CSI frame | **PASS (2026-10-05)** | Root cause of the 53-bin frames: `acquire_csi_force_lltf = 1` (C5-only field, MAC v3) forced every PPDU to report the L-LTF. Set to 0 (as in espressif/esp-csi). MEASURED on 2 nodes, ch 40: 97-98 % of frames are HE-SU **245 bins** (490 B, the IDF esp32c5 table value; not 256), rest legacy 53 / HT 57. Server parses all of them and the #2157 majority grid gate locks on 245. |
| 2.4 GHz CSI on silicon | **PASS (2026-10-05)** | New Kconfig `CSI_WIFI_BAND_MODE` (auto / 2.4 only / 5 only, default auto) pins the band with `esp_wifi_set_band_mode()` in the STA_START handler. Pinned to 2.4 GHz, both nodes joined ch 5 and ran the 5-minute qualification: 41.7 / 41.6 pps raw, DSP 8.0-8.2 Hz, about 98 % HE-SU 245-bin frames, 0 server parse failures. Each node logged a startup ENOMEM burst (6 events, recovered in about 300 ms, right after Got IP), none in steady state. Note: the driver persists the band in its own NVS, so the firmware sets it on every boot, AUTO included |
| Empty-room occupancy | **Device PASS; server FAIL uncalibrated; calibrated pending (2026-10-05)** | Edge vitals on an empty room: 440/448 and 405/452 absent, with 0 absent-with-persons contradictions, which meets the C6 record's bar. Node 7 flagged presence on 10.4 % of packets. Server heuristic (`main` and #2132) reported 1 person in 478/480 samples, uncalibrated. A calibrated run needs a 600 s calibration window plus a hold-out (see the validation record) |
| 802.15.4 time-sync on C5 | **Fixed; off by default (2026-10-05)** | It never received a peer beacon on C6 (#762) or C5. Four bugs in `c6_timesync.c`: (1) RX armed once and `rx_when_idle` never set, so the radio went idle after the first beacon TX; `rx_when_idle` is now set, plus a defensive task-context re-arm after every TX (one minimal-app run saw RX not resume after TX despite `rx_when_idle`; three reruns without the re-arm on IDF 5.5.2 and 5.5.4 did not reproduce it); (2) `ESP_LOGI` in the ISR RX callback aborted (`lock_acquire_generic`); (3) beacons were built in a stack buffer the async transmit read after it was reused, so peers got a valid header plus RAM/register addresses; (4) Wi-Fi + 15.4 coex was never enabled (`esp_coex_wifi_i154_enable()`), so with 15.4 in RX the STA couldn't authenticate or find the AP. Isolated with a minimal two-board reproducer, then fixed. MEASURED with all four fixes, both nodes: STA joins in ~6 s, the follower received 708/708 beacons intact, no aborts, ESP-NOW unaffected. Cost: CSI callback yield drops from ~40 to ~27 pps while 15.4 shares the radio, so `C6_TIMESYNC_ENABLE` stays off by default and ESP-NOW remains the default time-sync transport |

Provisioning note: `provision.py flash_nvs` needs `--no-stub` for the C5 preview
target (the flasher stub isn't available; without it the NVS write silently
fails MD5 verify and leaves the partition at 0xFF, which then ROM-loops). Fixed
in `provision.py` (ADR-383). Flash/reset state on the preview silicon is
fragile — a clean power-cycle recovers a wedged board.

## 4. Implementation phases

- **P1 — Build target (DONE, 2026-10-04):** overlay + gate generalization +
  CMake; builds, flashes, hash-verifies on C5 silicon.
- **P2 — Boot + onboarding (DONE, 2026-10-04):** image boots on C5 silicon,
  prints the `ESP32-C5 CSI Node` banner, brings up the CSI collector and bounded
  serial onboarding, runs at 240 MHz with the WiFi stack in dual-band mode
  (`band mode:0x3`). `RUVIEW_HELLO_V1` handshake over USB-JTAG still to exercise.
- **P3 — CSI capture on 2.4 GHz (DONE, 2026-10-05):** the real lockup cause was
  the mmWave probe on the flash bus (§3.3), not TWT. 5-minute run on 2 nodes,
  band pinned to 2.4 GHz: ~41.7 pps raw, DSP 8.0-8.2 Hz, 0 steady-state errors.
  `CONFIG_EDGE_DSP_SAMPLE_HZ` stays 8.
- **P4 — 5 GHz dual-band CSI (DONE, 2026-10-05):** 5-minute run on 2 nodes on
  ch 40: ~40 pps raw, about 98 % HE-SU frames at **245 bins** (not 256) after
  `force_lltf=0`. It wasn't the IDF version; 5.5.2+ is still required for PSRAM.
- **P5 — Validation record + evidence gate (validation record written; occupancy
  qualification pending):** emit
  `docs/validation/<date>-esp32-c5-*.md` in the ADR-110 format (5-min dual-table
  physical result against the pass bar), run `ruview_claim_check` / `ruview_verify`,
  write the ADR-304 EvidenceRecord. Only then does this ADR move to **Accepted**.

## 5. Open questions

1. ~~IDF 5.5.0 vs 5.5.2 for true HE CSI on C5~~ — resolved 2026-10-05: not the IDF version; `force_lltf` (see §3.3). 5.5.2 is still required for PSRAM modules.
2. ~~C5 sustainable DSP cadence~~ — resolved 2026-10-05: 8 Hz holds (8.0-8.2 Hz over 5 min on 2 nodes, both bands).
3. C5-DevKitC-1 LED GPIO — confirm against schematic.
4. ~~Does C5 802.15.4 RX behave differently from the C6's (#762)?~~ — answered
   2026-10-05: same symptom; four bugs in `c6_timesync.c` (§3.3), all fixed.
   15.4 sync now works next to Wi-Fi, at a CSI-yield cost, so it stays
   opt-in.
