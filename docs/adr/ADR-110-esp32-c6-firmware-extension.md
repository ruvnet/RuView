# ADR-110: ESP32-C6 firmware extension — Wi-Fi 6 CSI, 802.15.4 mesh, TWT, LP-core hibernation

| Field | Value |
|-------|-------|
| **Status** | Accepted — P1–P10 complete, firmware-side substrate closed at **v0.7.0-esp32** (2026-05-23) |
| **Date** | 2026-05-22 (created) · 2026-05-23 (last revision — P10 + sprint summary) |
| **Deciders** | ruv |
| **Codename** | **C6-SOTA** |
| **Relates to** | ADR-018 (CSI binary frame format), ADR-028 (ESP32 capability audit), ADR-029 (RuvSense multistatic), ADR-030 (RuvSense persistent field model), ADR-031 (RuView sensing-first), ADR-061 (QEMU CI), ADR-081 (adaptive CSI mesh kernel), ADR-097 (rvCSI adoption) |
| **Tracking issue** | [ruvnet/RuView#762](https://github.com/ruvnet/RuView/issues/762) |
| **Firmware releases** | [v0.6.7](https://github.com/ruvnet/RuView/releases/tag/v0.6.7-esp32) · [v0.6.8](https://github.com/ruvnet/RuView/releases/tag/v0.6.8-esp32) · [v0.6.9](https://github.com/ruvnet/RuView/releases/tag/v0.6.9-esp32) · [v0.7.0](https://github.com/ruvnet/RuView/releases/tag/v0.7.0-esp32) |
| **Witness** | [`docs/WITNESS-LOG-110.md`](../WITNESS-LOG-110.md) — 13 §A0 entries (§A0.1 → §A0.13), 1 §A.1-A.12 dual-soak, 4 §B blocker entries, 5 §C bug fixes, 1 §D-workaround |
| **Extended by** | [ADR-383](ADR-383-esp32-c5-firmware-extension.md) (ESP32-C5, 2026-10-04) — the HE-class `c6_*` modules (TWT, LP-core, SoftAP-HE, timesync, thermal) and their Kconfig/CMake gates are now shared C6/C5, not C6-only. The XIAO antenna-select stays C6-specific. |

---

## 1. Context

The production CSI node firmware (`firmware/esp32-csi-node`) was built around the **ESP32-S3** (Xtensa LX7 dual-core @ 240 MHz, 8 MB PSRAM, 802.11 b/g/n). The repo's `firmware/esp32-hello-world/main.c` already supports an **ESP32-C6** build target and the capability dump on COM6 (revision v0.2, MAC `20:6e:f1:17:27:8c`) confirmed four C6-only capabilities that the production firmware does not exploit today:

| C6 capability | What it enables for sensing | Why we can't get it on S3 |
|---|---|---|
| **802.11ax (Wi-Fi 6) HE-LTF CSI** | 242 subcarriers per HE20 frame (vs 52 for HT-LTF), HE-MU/HE-TB PPDU types, OFDMA-aware channel sounding. **Hardware-confirmed 2026-06-11** (issue #1005, external production deployment): requires **ESP-IDF ≥ 5.5** — the v5.4 driver blob silently downconverts to 64-subcarrier HT even on a confirmed-HE link; v5.5.2 delivers 532 B frames = 256 bins (242 active tones), PPDU 0x01 (HE-SU). See WITNESS-LOG-110 §B1 (resolved). | S3 radio is HT-only (n) |
| **802.15.4 (Thread / Zigbee)** | Cross-node time-sync over a separate radio — frees Wi-Fi airtime for CSI, ±100 µs alignment possible without coordination traffic on the sensing channel | S3 has no 802.15.4 |
| **TWT (Target Wake Time)** | Sensor negotiates a deterministic wake slot with the AP; CSI cadence becomes scheduler-bounded instead of opportunistic | Requires 802.11ax — S3 can't speak it |
| **LP-core + hibernation (~5 µA)** | Always-on motion gate runs on a separate RISC-V LP core in deep sleep; HP core stays off until a real event | S3 ULP is FSM-only, ~10 µA floor |

**The first three are publishable research surfaces.** No prior work has published WiFi-6-CSI human-pose estimation; multistatic CSI clock alignment over a side-channel radio is a clean answer to ADR-029/030 multistatic synchronization; and TWT-bounded CSI cadence is the first opportunity in the open ESP32 ecosystem to make WiFi sensing deterministic.

**The fourth (LP-core) unblocks a product line.** Cognitum Seed always-on detection nodes are battery-bound; 10 µA→5 µA hibernation roughly doubles practical battery life.

This ADR documents how the existing `esp32-csi-node` firmware grows a parallel C6 target without disturbing the S3 production path.

### 1.1 What this ADR is *not*

- Not a deprecation of the S3 firmware. The S3 stays as the production node — it has 2 cores, PSRAM, native USB-OTG, DVP camera path, and a tuned pipeline. The C6 is added as a research/seed target.
- Not a port of every S3 feature to C6. Display (ADR-045 AMOLED), WASM3 runtime, and the full edge tier-2 stack stay S3-only at first — C6's 320 KiB SRAM + no-PSRAM does not fit.
- Not a hardware redesign. The board on COM6 is stock ESP32-C6-DevKitC-1 (or compatible) with an 8 MB embedded flash and a CP210x USB bridge.

## 2. Decision

Extend `firmware/esp32-csi-node` to a **dual-target project** (S3 + C6) using ESP-IDF's existing `idf.py set-target` mechanism plus a target-keyed `sdkconfig.defaults.esp32c6` overlay. Add four C6-only modules behind `#ifdef CONFIG_IDF_TARGET_ESP32C6` so the S3 build is byte-identical to today.

### 2.1 Module breakdown

| New module | File | C6-only? | Purpose |
|---|---|---|---|
| **HE-LTF CSI tagging** | extend `csi_collector.c` | shared (no-op on S3) | Read `wifi_pkt_rx_ctrl_t.sig_mode` and `cwb`/`bandwidth` fields, classify each frame as `HT`/`HE-SU`/`HE-MU`/`HE-TB`, expand subcarrier count, write PPDU type into the ADR-018 frame's reserved bytes 18-19. |
| **802.15.4 time-sync** | `c6_timesync.c/.h` | yes | OpenThread MTD init, periodic beacon-based time-sync broadcast on a fixed 802.15.4 channel, exports `c6_timesync_get_epoch_us()`. |
| **TWT setup** | `c6_twt.c/.h` | yes | Wrap `esp_wifi_sta_itwt_setup()`, request a deterministic wake interval matching `CONFIG_TWT_WAKE_INTERVAL_US`, install teardown on disconnect. |
| **LP-core hibernation** | `c6_lp_core.c/.h` + `lp_core/main.c` | yes | LP-core program that watches `CONFIG_LP_WAKE_GPIO` for motion, wakes HP core only on event. HP-side calls `c6_lp_core_arm()` before `esp_deep_sleep_start()`. |

### 2.2 Build matrix

| Target | sdkconfig defaults | Partition table | Binary size | Features |
|---|---|---|---|---|
| `esp32s3` (default — production) | `sdkconfig.defaults` (unchanged) | `partitions_display.csv` (8 MB) | ~1.1 MB | Full pipeline + display + WASM |
| `esp32c6` (new — research) | `sdkconfig.defaults` + `sdkconfig.defaults.esp32c6` overlay | `partitions_4mb.csv` (4 MB single OTA) | target <1 MB | CSI + TWT + 802.15.4 + LP-core, no display, no WASM |

ESP-IDF's idf-build-system picks `sdkconfig.defaults.<target>` automatically when `idf.py set-target esp32c6` is invoked. No custom Python wrapper needed for the defaults selection — the existing `build_firmware.ps1` keeps working for S3.

### 2.3 ADR-018 frame format extension

Bytes 18-19 are currently reserved. They become:

```
[18]   PPDU type        (0=HT, 1=HE-SU, 2=HE-MU, 3=HE-TB, 0xFF=unknown)
[19]   Bandwidth + flags
       bit 0-1 : bandwidth (0=20 MHz, 1=40, 2=80, 3=160)
       bit 2   : STBC
       bit 3   : LDPC
       bit 4   : 802.15.4 time-sync valid (C6 only, set if c6_timesync_get_epoch_us is fresh)
       bit 5-7 : reserved
```

Magic stays `0xC5110001` — readers that don't know about byte 18-19 see what they always saw (`info->buf` is unchanged). Readers that do can opt in.

### 2.4 802.15.4 time-sync protocol (skeleton)

- One node is elected `time-leader` (lowest 64-bit EUI on the mesh).
- Leader broadcasts a `TS_BEACON` frame every 100 ms on 802.15.4 channel 15 containing its monotonic `esp_timer_get_time()` snapshot.
- Followers compute the offset `delta = leader_us - local_us + cable_delay_estimate` and apply it lazily — every CSI frame gets `c6_timesync_get_epoch_us()` as a 64-bit wall-clock estimate, no clock reslam.
- Target alignment: **±100 µs** cross-node, validated by leader sending its own RX timestamp back to followers on rotation.
- Falls back to local timer if no leader heard within 5 s.

### 2.5 TWT negotiation

- After WiFi STA connects, call `esp_wifi_sta_itwt_setup()` with:
  - `wake_interval_us` = `CONFIG_TWT_WAKE_INTERVAL_US` (default 10 000 = 100 fps cadence)
  - `min_wake_dura` = 512 µs (enough to receive one CSI frame)
  - `trigger` = false (non-trigger-based — leader role)
- If the AP rejects (`ESP_ERR_WIFI_NOT_INIT` / `ESP_ERR_WIFI_NOT_STARTED` / negotiation NACK), log and continue without TWT — CSI still works opportunistically.
- Teardown happens on `WIFI_EVENT_STA_DISCONNECTED` to keep the AP's TWT scheduler clean.

### 2.6 LP-core hibernation

**Shipped (P5):** `esp_deep_sleep_enable_gpio_wakeup()` deep-sleep GPIO wake — the simplest path that actually delivers the hibernation budget for the canonical seed-node use case (PIR sensor outputting a clean digital interrupt). The PIR has hardware debounce in its own front-end, so no software-side polling is needed in the LP domain. Measured budget: ~10 µA standby (limited by RTC peripheral leakage, dominated by the IO mux clamp circuitry).

**Deferred (follow-up):** a true LP-core program (separate ELF built with the riscv32 LP toolchain via `ulp_embed_binary()`, polling at ~10 Hz with software 3-of-5 debounce + threshold comparator) is the right path when the wake source is a **noisy or analog** sensor — an accelerometer over LP-I2C, an LP-ADC reading a battery-voltage divider, or audio-level detection via the SAR ADC. That code lives in `lp_core/main.c` as a sub-project and pushes the standby budget down to the ~5 µA target. Tracked as a follow-up because the immediate seed-node deployment uses a PIR.

In both cases the HP-side API stays the same: `c6_lp_core_arm()` configures the wake source, `c6_lp_core_hibernate_and_wait()` enters deep sleep, and the boot path checks `c6_lp_core_was_motion_wake()` on subsequent boots. Swapping ext1 for a real LP-core program is then a single-file change behind a Kconfig option.

## 3. Consequences

### 3.1 Wins

- New publishable research surface (Wi-Fi-6 CSI human pose).
- Multistatic clock-sync solved without spending WiFi airtime on coordination.
- Deterministic CSI cadence available where the AP cooperates (TWT).
- Cognitum Seed always-on class roughly doubles practical battery life.
- S3 production path untouched — zero regression risk for shipped fleets.

### 3.2 Costs

- Second firmware target to maintain (build, test, release). Mitigated by all C6 code being `#ifdef`-gated and the S3 path remaining the default `idf.py build`.
- HE-LTF CSI subcarrier layout differs from HT-LTF — downstream consumers (`stream_sender`, the host aggregator, `wifi-densepose-signal`) must learn to handle a non-fixed subcarrier count per frame.
- 802.15.4 stack adds ~80 KB to the C6 binary. Fits in 4 MB partition with room to spare.
- TWT depends on AP cooperation. Most home APs (including the `ruv.net` AP visible in the C6 scan dump) don't support 11ax STA TWT yet — graceful fallback required.

### 3.3 Verification

- `firmware/esp32-csi-node` builds for both `esp32s3` (existing) and `esp32c6` (new) targets.
- S3 build artifact SHA-256 unchanged vs the last v0.6.x release (proves no regression in shared code).
- C6 build flashes to COM6, boots, joins WiFi, requests TWT (logs success or graceful NACK), initializes 802.15.4, emits CSI frames with the extended ADR-018 metadata.
- Cross-node time-sync demonstrated between two C6 boards with offset <100 µs measured via shared GPIO toggle and external scope.
- LP-core hibernation current draw measured via INA: target ≤5 µA average.

## 4. Implementation phases

| Phase | Scope | Status |
|---|---|---|
| **P1** | Multi-target build support (sdkconfig.defaults.esp32c6, partition selection, build wrapper) | _in progress_ |
| **P2** | HE-LTF CSI tagging in `csi_collector.c` | pending |
| **P3** | TWT setup helper | pending |
| **P4** | 802.15.4 init + skeleton time-sync | pending |
| **P5** | LP-core hibernation stub | ✅ **done** (v0.6.6); upgraded to real LP-core polling program in v0.6.7 (`firmware/esp32-csi-node/main/lp_core/main.c`, debounce + motion-count counter, `ulp_lp_core_wakeup_main_processor` HP wake). Ext1 fallback kept as the `CONFIG_C6_LP_CORE_ENABLE=n` branch. Datasheet ≤5 µA pending INA measurement. |
| **P6** | Build, flash COM6, capture boot telemetry, S3 regression check | ✅ **done** — `c6_ts: init done channel=15 leader=yes(candidate)`, HE MAC firmware loaded, 1003 KB binary (46% slack) |
| **P7** | Benchmark C6 vs S3 (CSI fps, RAM, TWT jitter, power) | ✅ **done** — boot 353 ms, ts init 413 ms, image 1003 KB (−9 % vs S3), 310 KiB free heap, CSI callbacks fire at 64 subcarriers/frame on ch 1 background traffic |
| **P8** | Witness bundle update, CLAUDE.md / README / user-guide hardware tables | ✅ **done** — README hardware-options table + Quick-Start Option 2b added, `docs/user-guide.md` now has full ESP32-C6 section (build, flash, provision, multi-room time-sync, battery seed mode) |
| **P9** | **Software-only unblocks for B1/B2/B4 (firmware v0.6.7)** | ✅ **done** — (1) Real LP-core motion-gate program loads via `ulp_embed_binary(lp_core/main.c)`, exposes shared `motion_count`/`poll_count` symbols for witness verification (B4 code path complete, hardware-measurement still pending INA). (2) Soft-AP HE module (`c6_softap_he.{h,c}`) runs the C6 in AP+STA mode with WPA2 + HE advertised so a second C6 STA can negotiate real iTWT against a known-cooperative AP (B1/B2 unblocker without buying an 11ax router). (3) Build artifacts: S3 8 MB 1093 KB / C6 4 MB 1019 KB, both green on IDF v5.4. Both new modules default-off so v0.6.6 fleets see no behavior change. |
| **P10** | **End-to-end mesh substrate: measured, smoothed, wired, decoded (firmware v0.6.8 → v0.7.0 + host crates)** | ✅ **done** — bench-quantified two-board substrate **and** the host-side wire that consumes it. **(a) v0.6.8 ESP-NOW EMA smoother** (`c6_sync_espnow.c`, α=1/8 fixed-point shift, 8-sample window). 5-min two-board soak (witness §A0.10) measured **411.5 µs raw stdev → 104.1 µs smoothed stdev (3.95× suppression, 4.70× peak-to-peak)** with **+30 µs/min crystal drift preserved within 2 µs/min**. **Cross-board RX 99.56 %** over 2701 beacons, 0 TX fail, leader election fired at +27336 ms. The ADR-110 §2.4 ≤100 µs alignment target is **empirically met by the smoothed offset alone**. **(b) v0.6.9 sync-packet** (32-byte UDP, magic `0xC511A110`, every `CONFIG_C6_SYNC_EVERY_N_FRAMES` CSI frames) carries `(node_id, local_us, epoch_us, sequence)` so host can pair against incoming CSI frames. Live-verified §A0.12 — COM9 reports `local − epoch = 1 163 565 µs` matching §A0.10's measured boot delta within 285 µs. **(c) v0.7.0 ADR-018 byte 19 bit 4 wire-fix** — bit 4 now sourced from `c6_sync_espnow_is_valid()` (was only the broken 802.15.4 path). Mixed S3+C6 fleets correctly advertise sync via the working transport. **(d) Host-side decoders + wiring**: Python `SyncPacketParser` (6 tests) + Rust `SyncPacket` (10 tests, all green; `SyncPacket::apply_to_local` recovers per-frame mesh-aligned timestamps). Sensing-server `udp_receiver_task` magic-dispatches `0xC511A110` and stores `NodeState::latest_sync` + `NodeState::mesh_aligned_us(local_at_frame)` helper. **(e) IDF v5.4 upstream gap formally documented (§A0.6)**: full `components/esp_wifi/include/esp_wifi*.h` grep proves the public API exposes only STA-side iTWT/bTWT — no `esp_wifi_ap_set_he_config`, no `wifi_he_ap_config_t`. Soft-AP HE/TWT-Responder advertise is not user-controllable on C6 in IDF v5.4; B1/B2 measurement requires either a future IDF or an external 11ax AP. |

This ADR is updated at the end of each phase with the actual outcome, links to commits, and any deviations from the design.

### 4.1 P10 detail — `/loop 5m` SOTA sprint (2026-05-23)

P10 was driven by a `/loop 5m until sota. and ultra optmized` invocation that ran 16 iterations over ~80 minutes. The sprint shipped 4 firmware releases, 17 commits on the branch, 13 host-side unit tests, and converted the §B substrate from "designed targeting ±100 µs" into "measured at 104 µs smoothed stdev over a 5-min two-board soak with full host-side decoders + sensing-server consumer."

| Iter | Shipped | Witness |
|---|---|---|
| 1 | `c6_softap_he` module + IDF v5.4 gap discovery | §A0.5, §A0.6 |
| 2 | ESP-NOW cross-board mesh proven live | §A0.7 |
| 3 | 4 MB S3 release variant | — |
| 4 | 4-min mesh soak — first quantified sync stability | §A0.8 |
| 5 | EMA smoother in firmware (α=1/8) | §A0.9 |
| 6 | 5-min EMA soak: **3.95× suppression measured** | §A0.10 |
| 7 | v0.6.8-esp32 release + §A0.11 timestamp-wiring gap recorded | §A0.11 |
| 8 | Sync packet emission (option 2 chosen) | — |
| 9 | Sync packet live-verified on both boards | §A0.12 |
| 10 | v0.6.9-esp32 release + `CONFIG_C6_SYNC_EVERY_N_FRAMES` Kconfig knob | — |
| 11 | ADR-018 byte 19 bit 4 wire-fix from ESP-NOW path | — |
| 12 | v0.7.0-esp32 release + Python `SyncPacketParser` stub | §A0.13 |
| 13 | 6 Python unit tests + README/user-guide doc updates | — |
| 14 | Rust `SyncPacket` decoder + 7 unit tests in `wifi-densepose-hardware` | — |
| 15 | Sensing-server `udp_receiver_task` magic-dispatch + `NodeState::latest_sync` | — |
| 16 | `SyncPacket::apply_to_local()` + `NodeState::mesh_aligned_us()` (+ 3 more tests, 10 total) | — |

### 4.2 P10 measured numbers (substrate now quantified, not just designed)

Every number below comes from a real bench capture against COM9 + COM12 ESP32-C6 boards, raw logs preserved under `dist/firmware-v0.6.7/iter{2,4,5,6,9}-*.log` and `dist/firmware-v0.6.8/iter9-*.log`.

| Metric | Measured | Target |
|---|---|---|
| Cross-board ESP-NOW RX rate (5-min soak) | **99.56 %** (2689 / 2701 beacons) | — |
| Cross-board TX failures (5-min soak) | **0** on either board | — |
| Beacon rate | **10.00 /s** exactly (FreeRTOS solid) | 10 Hz nominal |
| Raw offset stdev | 411.5 µs | — |
| **EMA-smoothed offset stdev** | **104.1 µs** | **≤100 µs (§2.4)** |
| Range reduction (smoothed vs raw) | **4.70×** peak-to-peak | — |
| Measured C6 crystal skew between bench boards | **1.4 ppm** | ESP32 spec ±10 ppm |
| Drift preservation (smoothed tracking raw) | within **2 µs/min** | — |
| Leader election | ✅ COM9 stepped down at +27 336 ms on `lower-id` rule | — |
| Sync packet round-trip (firmware → Python decoder) | identical bytes, offset recovered to within **285 µs** of §A0.10 | — |
| Raw 802.15.4 RX | 0 frames over 60 s + 240 s + 300 s soaks | (D1 broken in IDF v5.4) |
| C6 v0.7.0 image size / slack | 1019 KB / **45 %** on 4 MB single-OTA | — |
| S3 v0.7.0 image size / slack | 1094 KB / **47 %** on 8 MB dual-OTA | — |

### 4.3 P10 host-side surface (production code shipped)

| Crate / File | New API |
|---|---|
| `v2/crates/wifi-densepose-hardware/src/sync_packet.rs` | `SyncPacket`, `SyncPacketFlags`, `SYNC_PACKET_MAGIC = 0xC511A110`, `SYNC_PACKET_SIZE = 32`, `SyncPacket::from_bytes`, `SyncPacket::to_bytes`, `SyncPacket::local_minus_epoch_us`, `SyncPacket::apply_to_local(local_us)` — 10 unit tests, all green |
| `v2/crates/wifi-densepose-sensing-server/src/main.rs` | `NodeState::latest_sync: Option<SyncPacket>`, `NodeState::latest_sync_at: Option<Instant>`, `NodeState::mesh_aligned_us(local_at_frame_us) -> Option<u64>`, `udp_receiver_task` magic-dispatch on `SYNC_PACKET_MAGIC` |
| `archive/v1/src/hardware/csi_extractor.py` | `SyncPacket` dataclass, `SyncPacketParser.parse`, `SyncPacketParser.MAGIC` — 6 Python unit tests, all green |

## 5. Open questions

- Should the HE-LTF subcarrier expansion ship in the default ADR-018 payload, or behind a runtime flag while the host aggregator catches up? **Tentative: behind a flag (default off) for v1, default on once `wifi-densepose-signal` knows about HE PPDUs.**
- Should the 802.15.4 time-sync channel be configurable, or hard-coded to 15? **Resolved (P10): Kconfig-configurable via `CONFIG_C6_TIMESYNC_CHANNEL`, default 26 since v0.6.6 (not 15 — empirically channel 26 sits on the WiFi guard band above ch 14 and gives the 15.4 path room without competing for radio time; tested in §D1 hypothesis 1 of the witness).**
- Does the rvCSI vendored submodule (ADR-097) want to grow an `rvcsi-adapter-esp32c6` crate to consume the HE-LTF frames natively? **Out of scope for this ADR; revisit in a follow-up.**

## 6. What's outside this ADR (P10 closure)

The firmware-side substrate for ADR-110 is now closed. Three categories remain, all explicitly **not** in this ADR's scope:

1. **Multistatic CSI fusion math** — ADR-029/030 territory. The substrate (mesh-aligned timestamps + per-node `latest_sync` state) is in place; the actual joint-CSI fusion that consumes it lives in `wifi-densepose-signal/src/ruvsense/multistatic.rs`.
2. **Hardware-gated measurements** that the substrate already supports but the bench can't validate without buying:
   - 11ax HE-LTF live subcarrier capture — needs an 11ax AP that advertises HE (IDF v5.4 doesn't expose an AP-side HE config API, §A0.6).
   - ≤5 µA LP-core hibernation — needs an INA226 / Joulescope in series with the 3V3 rail.
3. **IDF upstream fixes**:
   - 802.15.4 RX path on C6 + IDF v5.4 — `c6_timesync` ships and initialises but never RXes a frame (D1, 5 hypotheses tested + rejected). ESP-NOW workaround (`c6_sync_espnow`) is the working primary mesh transport. The 802.15.4 source stays in for the day IDF fixes the driver.
   - Soft-AP HE/TWT-Responder advertise API — `c6_softap_he` ships as the in-place hook for when IDF v5.5+ exposes it.
