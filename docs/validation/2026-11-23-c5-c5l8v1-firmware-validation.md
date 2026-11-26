# ESP32-C5 C5L8/v1 producer: firmware validation (2026-11-23)

This note covers the firmware side of the managed C5 L-LTF8 format. It shows that one
ESP32-C5 board running this producer emits datagrams the server validator accepts, both from
the self-test and from live 5 GHz CSI, and that the image can be replaced over OTA. It is
format evidence only. It does not qualify hardware, sensing accuracy, phase, or the physical
packing of the I/Q bytes.

## Contract implemented

[Managed C5 L-LTF8 format contract v1](https://github.com/ruvnet/RuView/blob/198528075cb1/docs/validation/2026-11-10-c5-native-lltf8-contract.md)
(2026-11-10). The decoder that enforces it, `room_learner::native` with grid
`ant1-bins53-ppdu0-c5lltf8-keep53-drop2-linear-v3`, is at `198528075cb1` on the
app-runtime stack and is not on `main` yet. The validator checks in this note build against
that ref (see "Host contract tests").

The producer is opt-in (`CONFIG_CSI_C5_LLTF8_V1`, ESP32-C5 only, default off) and does the
following:

- **Band and channel:** 5 GHz only, STA bandwidth set to 20 MHz and read back before any
  claim. Primaries are 36–64 and 100–144 (step 4) and 149–177 (step 4).
- **CSI configuration:** legacy plus forced L-LTF, `lltf_bit_mode = 1` (8-bit components),
  zero scaling. HT, VHT and HE acquisition are off.
- **Pre-gate:** callbacks that can never carry the claim are counted and dropped before any
  work: anything other than non-HT OFDM (BB format 1), a secondary channel present, an invalid
  estimate, or a length other than 106 bytes. `c5l8_build()` re-checks every field.
- **Output:** a frame is emitted only as the exact 142-byte datagram, `c5l8_build()` in
  `main/c5_lltf8.c`. Refused callbacks are logged with a reason and no samples.
- **Cadence (added 2026-11-25):** the STA associates 802.11a-only
  (`CONFIG_C5_LLTF8_STA_11A_ONLY`, on in the C5L8 overlay) so the AP's self-ping replies are
  non-HT OFDM, self-ping runs at 25 Hz, and a 20 Hz deadline pacer
  (`CONFIG_C5_LLTF8_PACE_HZ`) on qualifying frames replaces the 20 ms early callback gate.
  A `C5L8 cadence` line is logged every 10 s. See "Cadence for the room learner".
- **First-word sanitising:** when the SDK marks the first word invalid, the first 4 I/Q bytes
  are zeroed, never removed, and ADR-018 flag bit 5 and suffix byte 13 are set.

## Readback provenance (owner decision, 2026-11-23)

With the ESP-IDF build below, `esp_wifi_get_csi_config()` cannot report the 8-bit setting:

- **Read path:** in the C5 Wi-Fi libraries `esp_wifi_get_csi_config()` →
  `hal_mac_get_csi_filter()` rebuilds only bits 0–11 (enable and the `acquire_*` flags) from
  MAC registers. It leaves `val_scale_cfg`, `dump_ack_en` and `lltf_bit_mode` as the caller
  passed them. A zeroed buffer therefore always reads back 12-bit, which was observed on the
  board (see "Live 5 GHz capture").
- **Write path:** `esp_wifi_set_csi_config()` → `ic_csi_set_config()` → `phy_lltf_mask_en()`
  and `phy_chan_dump_cfg()` write those fields to PHY register `0x600A790C`. Bit 12 is
  `lltf_bit_mode` (1 = 8-bit) and bit 3 is bit 0 of `val_scale_cfg`. This was found by
  disassembling `libpp.a` and `libphy.a` from the ESP-IDF build below.
- **What the firmware checks:** bits 0–11 must match through the API, and register
  `0x600A790C` must have bit 12 = 1 and bit 3 = 0. The firmware logs the raw register value.
  For any other target or ESP-IDF version the check fails closed and every frame is withheld,
  because the register map has only been verified for this SDK.

Owner decision: on 2026-11-23 Mathew Beane approved this register read as the provenance for
the contract's "SDK configuration readback". The board read `0x600A790C = 0x00003510`, which
has bit 12 set and bit 3 clear.

## Build

- **ESP-IDF:** v6.2.0 (`espressif/esp-idf` commit `25fe69f946311abdaf9ad56591f25fedbc20ac98`,
  `esp_wifi` libraries `77143eec456e1cdcbce3e1dad8d5983cd28a368f`). ESP-IDF v5.5 has no
  `lltf_bit_mode` field.
- **Toolchain:** `riscv32-esp-elf-gcc` esp-16.1.0_20260609 (GCC 16.1.0).
- **Configuration:** `build_c5l8v1.sh` (target esp32c5) applies `sdkconfig.defaults`, then
  `sdkconfig.defaults.esp32c5`, then `sdkconfig.defaults.esp32c5.c5l8v1`. The resolved
  config has:

  ```text
  CONFIG_CSI_C5_LLTF8_V1=y
  CONFIG_C5_LLTF8_CHANNEL=48
  CONFIG_C5_LLTF8_SERIAL_SELFTEST=y
  CONFIG_CSI_FRAME_HE_TAGGING=y
  CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y
  CONFIG_ESP_CONSOLE_USB_SERIAL_JTAG=y
  ```

  The board uses a 4 MB flash with `partitions_4mb.csv` (two OTA slots, `ota_0` and `ota_1`).
  The console is on the C5's native USB-Serial-JTAG port, so the self-test and USB
  onboarding work on boards with no UART bridge.
- **Wi-Fi credentials:** none are in the build; they are provisioned into NVS.

Image sha256:

| Image | sha256 |
|---|---|
| Image under test (flashed by OTA, all on-board results below) | `b66597787eea5e0661e4f11faddb0cabf1a0c3843bcc919e3af96d3dee1891d7` |
| This branch, rebuilt from the tree (not flashed) | `b9187f1b247a98b974a4b5e9b2d148659824393515cfea73413c3be125be1386` |
| Partition table (both builds; identical to the one on the board) | `4c2cc4ffd52641e23b779bd57b3908014083ac3c1aab395756478c89e70d81f0` |
| `ota_data_initial.bin` (both builds) | `7d2c7ac4888bfd75cd5f56e8d61f69595121183afc81556c876732fd3782c62f` |

The image under test was built from the same producer, collector and overlay logic on the
same ESP-IDF. Porting onto `main` changed comments and put the C5L8-only includes behind
`CONFIG_CSI_C5_LLTF8_V1`, so the host build of `csi_collector.c` still compiles. The two app
images therefore differ.

The branch's bootloader has the same size and configuration as the bootloader on the board.
Only 37 bytes differ, which fits an embedded build timestamp plus the image hash trailer.
OTA replaces only the app slot, so the board kept its bootloader.

## Host contract tests

- **C unit test:** `firmware/esp32-csi-node/test/test_c5_lltf8.c`, run by `make host_tests`.
  It compiles the builder unchanged and checks the exact byte layout, header flags, suffix
  and sanitising for four flag combinations. It checks the full 5 GHz channel table, every
  refusal reason (including a NULL IQ pointer and a short buffer), and both self-test cases.
  A planted suffix defect (byte 11 set to 40 instead of 20) makes it fail with 32 errors.
  `make host_tests` passes on this branch, including every existing test.
- **Rust validator test:** `firmware/esp32-csi-node/tests/c5l8v1-validator`. It compiles the
  firmware's own `c5_lltf8.c` into a small emitter and feeds every frame to
  `validate_adr018_native_format()` and `canonicalize_esp32_with_format()` from the validator
  source. Its four tests pass:
  - Producer frames (14 accepted cases) pass with lossless I/Q, the C5 tone order, a 20 MHz
    bandwidth, DC invalid, and no phase consumed.
  - The default frame matches the `c5-node23-wire-metadata-20261110.json` fixture.
  - Every refusal reason is withheld with no samples (12 cases).
  - The validator rejects near-miss mutations of a producer frame (a bare 106-byte payload, a
    trailing byte, wrong suffix bytes, wrong flags or PPDU, and 2.4 GHz).

  `./prepare-validator-src.sh` archives `v2/` at `VALIDATOR_REF` (default `198528075cb1`)
  plus the `worldgraph` and `rufield` submodules at the commits that ref pins.
  `Cargo.lock` starts from `v2/Cargo.lock` at that ref.

## Serial self-test (no Wi-Fi)

At boot the build prints two SYNTHETIC frames on the console (`C5L8-SELFTEST-SYNTHETIC`):
case 0 has flags `0x10` and case 1 has flags `0x30` (first word sanitised), both at 5240 MHz.
`tests/c5l8v1-validator/bench-serial.sh <port> 30 --reset` captures them after an esptool
watchdog reset and runs `c5l8-check`. On the C5's USB-Serial-JTAG port an RTS pulse latches
ROM download mode, so the reader never toggles DTR or RTS.

Result: 2 passed, 0 failed, on the first flashed build and again on the image under test
after OTA. Both runs used grid `ant1-bins53-ppdu0-c5lltf8-keep53-drop2-linear-v3`. These
frames check the serializer only.

## Live 5 GHz capture

- **Network:** the AP's primary was channel 48 (5240 MHz). The AP advertised 160 MHz, and the
  STA bandwidth read back 20 MHz. All callbacks were 106 bytes, BB format 1, with no secondary
  channel.
- **Before the readback fix:** every callback was withheld as `csi_config_readback_mismatch`,
  because the API echoed `lltf_bit_mode = 0`. The receiver got only the node's other packet
  types and no 142-byte frames.
- **After the fix, on the image under test:** the readback matched, and nothing was withheld
  in the capture window. Callbacks ran at about 10 per second, driven by the firmware's
  gateway self-ping.
- **Capture:** one 100-second UDP capture on port 5005 spanned the OTA. It held 1,023
  datagrams: 763 C5L8/v1 datagrams of 142 bytes, plus 260 datagrams of the node's other packet
  types (32, 48 and 60 bytes), which were counted and not decoded. It was recorded with
  `capture-udp.py` and checked with `c5l8-udp-check`.
- **Validator result:** 763 passed, 0 failed. Every frame was 5,240,000,000 Hz with flags
  `0x10` on grid `ant1-bins53-ppdu0-c5lltf8-keep53-drop2-linear-v3`.
- **Sequence numbers:** the accepted frames had 110 gaps between consecutive sequence numbers.
  The cause was not traced. The candidates are the stream sender's rate limit, or sequence
  numbers consumed by callbacks that were not sent.

Raw captures are not included. Their sha256 values are below; the files are kept outside the
repository.

| Capture | sha256 |
|---|---|
| Serial self-test, first flashed build | `a6429d97111cba1caefb937e077811d92f0aa5a4254bd7d7681d952740663042` |
| Serial, before the readback fix | `1ea8d819a100398c5b443f174a604ea138655709252ac5097e589942cf8663ac` |
| UDP, before the readback fix | `98e4469beb5eac8c6757485ed174320326126dcaeb1aaf4011c0485af064c4b7` |
| Serial across the OTA and the image under test | `439f6a32618508921254a1c67c8eec097866d53d99c129cf923c16dfaa4cb6a0` |
| UDP, 763 accepted frames | `dcc2ca5a48b7ba61dc47fa87ff92acd9eb1018cfdd2424cdc260a59afa337a5f` |

## Cadence for the room learner (2026-11-25)

Guided training on two C5s stopped with `acquisition_observed_cadence` at about 9 frames/s.
The room learner's variable-source profile, `ReceiverCadenceBand::Arrival18To22Burst200MsV1`
in `room_learner/temporal_acquisition.rs`, is **two-sided**: each source must deliver
18–22 packets/s over the window and the whole period, with gaps of at most 200 ms. A source
that is too fast fails as well as one that is too slow, so the producer has to be paced, not
just made faster. A lower band would need a new versioned profile. One missed 102.4 ms
beacon is already a 204.8 ms gap.

Bench: one ESP32-C5 board, one AP on channel 48 at 20 MHz, about 190 s of USB serial per
build, read from the `C5L8 cadence` line (emitted Hz, callbacks by PPDU format, withheld
reasons, frames held back by the pacer). All figures are MEASURED on that board except where
marked.

| Build | STA mode | Qualifying callbacks/s | Emitted C5L8/v1 Hz (10 s periods) |
|---|---|---|---|
| C5L8/v1 as first proposed (self-ping 50 Hz, 20 ms gate) | 11ax | beacons only | 4.6 (180 s average) |
| HE client, CSI selector widened to HE (diagnostic only, not in this branch) | 11ax | 35/s HE-SU from the AP plus legacy beacons | n/a, the frames are not non-HT |
| 11a-only, 25 Hz self-ping, 50 ms bucket pacer (superseded) | 11a | 41/s non-HT from the AP | 18.3 (17.1–20.0) |
| **This branch:** 11a-only, 25 Hz self-ping, 20 Hz deadline pacer | 11a | 49/s non-HT (median, min 45.5) | **20.0 (19.92–20.10)** |

What this shows:

- **Root cause:** with the v1 selector (legacy plus forced L-LTF, HE/VHT/HT acquisition off),
  HE/VHT frames produce no CSI callback at all. The pre-gate's format counter stayed at 0. On
  an HE AP the only qualifying frames were beacons. On top of that, legacy frames arrive in
  bursts, and the 20 ms early gate dropped about 70% of them before they could be sent.
- **11a-only fixes the source:** the AP's unicast replies become non-HT OFDM, are 106 bytes
  and valid, and arrive well above 20/s. The pacer then holds the emitted rate at 20 Hz.
- **CSI from ACKs** (`dump_ack_en`) added no measurable qualifying frames on this AP. It is not
  in this branch.
- **The protocol setting persists:** `esp_wifi_set_protocols()` is stored in the Wi-Fi NVS
  config. An image that does not set protocols keeps associating as 11a after an 11a-only
  build. With `CONFIG_C5_LLTF8_STA_11A_ONLY=n`, this branch re-asserts the a/n/ac/ax default at
  boot.
- **Costs at a customer site:** the node is a legacy client. Some APs refuse legacy-only
  clients, or enable HT protection while one is associated.
- **Not shown:** learner acceptance end to end (to be rerun on channel 157), other APs and
  channels, more than one board at a time, and cross-node pacer alignment. The pacer runs on
  local time.

Sample stats line from the run above (10 s window):

```
C5L8 cadence 10.0s: emitted=200 (20.00 Hz) bb 11b=0 legacy=573 ht=0 vht=0 hesu=0 hemu=0 other=0 | withheld fmt=0 sec=0 est=0 len=0 ch=0 rb=0 bw=0 pace=373 gate=0
```

## OTA

The board was running the first C5L8/v1 build from `ota_0` (state `valid`). The image under
test was sent to `POST /ota` on port 8032:

| Request | Result |
|---|---|
| No `Authorization` header | 403 |
| `Authorization: Bearer <wrong token>` | 403 |
| `Authorization: Bearer <provisioned PSK>` with the 1,115,392-byte image | 200, `OTA update successful. Rebooting...` |

After the reboot, `GET /ota/status` reported `running_partition: ota_1`, `ota_state:
pending_verify`. After 30 s `ota_health` logged `marked valid after Wi-Fi got IP and CSI frame
delivered in 30013 ms`, and the status became `ota_1` and `valid`. The bootloader and
partition table were not changed; OTA replaces only the app slot. The PSK was passed to curl
on stdin and was not written to disk or logged.

## Limits

- **One board, one network:** one ESP32-C5 (4 MB flash), one AP on channel 48, about 100 s
  of live capture (and about 190 s per build for the 2026-11-25 cadence runs). No other channel
  was exercised on hardware.
- **Flag coverage:** live frames all had flags `0x10`. The sanitised (`0x30`) and no-sync
  paths are covered only by synthetic frames.
- **Version string:** the image under test reports the same `version`, `date` and `time` in
  `/ota/status` as the image it replaced. Tell them apart by slot and sha256.
- **Readback scope:** the register readback is verified only for ESP32-C5 on the ESP-IDF
  build above. Nothing was tested on other targets or SDK versions, where the producer fails
  closed.
- **What it does not show:** the result is format and transport evidence only. It is not a
  measurement of physical packing, phase, sensing accuracy or occupied bandwidth, and it does
  not raise the provenance or trust grade.
