# ESP32-C5 candidate preparation, 2026-10-28

Status, updated 2026-10-29: **installed with bounded real-board boot and CSI
transport evidence; experimental, with AP-dependent cadence**. The initial
no-ROM-response problem and the startup pin conflict described below are resolved.
esptool 5.6.0 identified production ESP32-C5HR8 revision 1.2, 8 MB flash and
8 MB embedded PSRAM. Secure boot and flash encryption were disabled.
These are identification measurements, not proof of firmware operation.
Subsequent installation and runtime findings are recorded below. See the
[release page](https://github.com/ruvnet/RuView/releases/tag/v0.8.17-esp32)
for publication status; this record was prepared before publication.

Track remaining qualification in [issue #2388](https://github.com/ruvnet/RuView/issues/2388).
The [public setup checklist](https://gist.github.com/ruvnet/bacfc5ac6b4feed7cfc1f76c03d12f4c)
contains no credentials, device identifiers or raw captures.

## Candidate configuration

Firmware 0.8.17 targets ESP32-C5 with ESP-IDF 6.0.2. The profile uses a 4 MB
OTA layout, UART console, 240 MHz application CPU, 8 Hz edge DSP, and explicit
2.4 GHz association. Raw CSI transmission retains its independent rate gate.
External PSRAM, display and WASM are not enabled. LED wiring is opt-in rather
than inheriting the S3's GPIO48. Unused managed display dependencies are excluded
from the C5 build. These are configuration choices, not measured speed or
sensing-accuracy results. The radio's vendor-documented 5 GHz capability is
not qualified by this profile.

**MEASURED build artifact (not hardware):** the C5 application built at
1,101,312 bytes and passed the embedded version
and chip-ID checks. The generated IDF flash plan and esptool's C5 ROM definition
agree on bootloader address `0x2000`; S3/C6's `0x0` must not be reused. Other
addresses remain `0x8000`, `0xf000`, and `0x20000`. Host tests cover wrong-chip
applications and the incorrect C5 bootloader address. The operator verifies
bundle checksums, production chip identity and at least 4 MB flash before
writing, and refuses C5 writes through esptool versions older than 5.3.

Build reproducer from the repository root:

```sh
cd firmware/esp32-csi-node
idf.py -B build-c5 -D IDF_TARGET=esp32c5 -D SDKCONFIG=build-c5/sdkconfig build
python test/test_delivery_contract.py --built-app build-c5/esp32-csi-node.bin \
  --target esp32c5 --flasher-args build-c5/flasher_args.json
```

Use the `espressif/idf:v6.0.2` build environment. The build output is software
evidence only. Host firmware contracts and the harness functional/security
tests passed locally. Consult the linked PR for final hosted checks and the
physical findings below for the actual limits of the hardware evidence.

## Comparison with previous ESP32 work

The following are configured profiles or **CLAIMED vendor/SDK capabilities**,
not a fresh matched hardware benchmark. Application CPUs exclude LP cores.

| Property | ESP32-S3 | ESP32-C6 | C5 candidate |
|---|---|---|---|
| Application CPU | Dual-core, up to 240 MHz | Single-core, up to 160 MHz | Single-core, configured 240 MHz |
| Current CSI profile band | 2.4 GHz | 2.4 GHz | 2.4 GHz; 5 GHz not qualified |
| Default edge DSP configuration | 20 Hz | 8 Hz | 8 Hz conservative baseline |
| Bootloader flash address | `0x0` | `0x0` | `0x2000` |
| Live measurement in this session | Not attached | Not attached | Real boot and decoded CSI; bounded samples below |

Sources: [ADR-110](../adr/ADR-110-esp32-c6-firmware-extension.md), the target
sdkconfig overlays and Kconfig, and Espressif's
[C5 Wi-Fi guide](https://docs.espressif.com/projects/esp-idf/en/v5.5.2/esp32c5/api-guides/wifi.html).
The C5 security-layout tool requirement follows the
[esptool 5.3 release](https://github.com/espressif/esptool/releases/tag/v5.3.0).

**CLAIMED historical transport measurements (attributed, not rerun here):** the
[previous C6 validation record](2026-10-20-esp32-csi-input-release-candidate.md)
reports 441 raw CSI packets in 30 seconds on firmware 0.8.15, with mixed
64/128/256-bin packets and no short packets. An earlier 0.8.14 run recorded
28.97 raw packets/s at baseline and 29.60 packets/s with bounded host traffic.
Those are the earlier record's measured results, using different firmware and
AP conditions. They were not repeated
for this C5 session and do not establish a C5 speedup, sensing accuracy, or
long-run reliability. No raw CSI or biometric values are included here.

## Initial connection failure and recovery

COM8 enumerates as a CP210x USB-to-UART bridge. esptool 5.1.0 received no ROM
response, including with a 500 ms boot-strap hold. A passive 115200-baud read
received zero bytes. An isolated esptool 5.6.0 installation also received no
ROM response. Driver enumeration alone does not identify the connected chip.
After the operator's physical intervention, the 2026-10-29 retry succeeded.
The chip, flash and security-state probes above all completed successfully.

## Physical findings and bounded qualification

### Installed-image failure and correction, 2026-10-29

The full 8 MB original flash was preserved privately. Reading at 460800 and
230400 baud failed with corrupt serial data; the complete read at 115200 baud
succeeded, and a separate 64 KiB sample readback matched. No backup, credentials,
device identifiers or raw CSI are committed here.

The exact C5 bundle from successful firmware CI run
[40904823542](https://github.com/ruvnet/RuView/actions/runs/40904823542)
passed checksums and embedded version/chip checks, installed, and booted as
0.8.17. Provisioning used private existing WiFi state and a dedicated local
test sink. **MEASURED failed baseline:** a 60.032-second run received three
valid raw CSI packets and no malformed packets, then repeated `TG0_WDT_HPSYS`
resets with `SPI flash busy detected`. This is not a successful transport run.

The legacy mmWave probe routed UART1 onto GPIO17/18 during startup. Espressif
reserves [GPIO16-22 for C5 flash/PSRAM](https://docs.espressif.com/projects/esp-idf/en/v6.0.2/esp32c5/api-reference/peripherals/gpio.html).
The correction skips unconfigured C5 probes and rejects memory-bus, console,
out-of-range and duplicate sensor pins before UART installation. Host tests
exercise the pin guard. Flash-log qualification also rejects watchdog and
flash-busy evidence even when earlier callbacks were present. The board was
held in download mode while preparing the corrected image.

### Corrected firmware and transport measurements

The corrected local application was 1,101,424 bytes. Firmware 0.8.17 booted
on the real C5, joined WiFi and continued CSI callbacks with no captured
watchdog, flash-busy error or disconnect in fresh 60-second boot captures.
The public SDK serial monitor also returned success on COM8.

**MEASURED bounded transport samples, same C5 and AP:**

| Condition | Window | Raw CSI packets | Raw packets/s | Malformed raw packets |
|---|---:|---:|---:|---:|
| Corrected firmware, initial test sink (includes intentional reset) | 120.026 s | 2,648 | 22.062 | 0 |
| Steady test sink, no external stimulus | 60.001 s | 1,763 | 29.383 | 0 |
| Test sink, 50 Hz small unicast stimulus for 59 s | 60.001 s | 1,766 | 29.433 | 0 |
| Normal port 5005, includes provisioning/startup and intentional reset | 120.222 s | 1,624 | 13.508 | 0 |
| Production SDK receiver on port 5005, steady | 60 s | 754 | 12.57 | 0 |
| Port 5005, 50 Hz small unicast stimulus for 59 s | 60.121 s | 1,050 | 17.465 | 0 |
| Port 5005, 100 Hz small unicast stimulus for 59 s | 60.052 s | 1,010 | 16.819 | 0 |
| Port 5005, 50 Hz 1,200-byte unicast stimulus for 59 s | 60.240 s | 1,008 | 16.733 | 0 |

The production receiver decoded all 971 received packets in its sample,
including CSI, feature state, vitals/features and sync; it reported no unknown
or malformed packets. All raw CSI shapes in these samples had one antenna
and 53 subcarriers. No raw I/Q, inferred vital values or network identifiers
are published here. Sequence discontinuities are not a standalone network-loss
measurement: sequence numbers are assigned before the firmware's UDP rate gate.
The initial reset-spanning window must not be used for a sequence-loss claim.

These are sequential observations, not controlled accuracy or comparative
CPU benchmarks. RF traffic was not held constant. Extra traffic produced no
consistent improvement, so no continuous host stimulus or higher DSP rate was
selected. The baseline stays at 8 Hz edge DSP and 2.4 GHz. The normal target
port was restored to 5005. **The tested AP did not consistently meet the
20 raw packets/s acceptance target; C5 remains experimental.** Long-run
reliability, other board form factors, 5 GHz, PSRAM and sensing accuracy remain
unqualified. The exact board form factor was requested but not confirmed;
the generic profile has no LED or default sensor pin assignment.

Reproduce receiver and serial checks from the repository root (use your port
and sink settings; do not run two listeners on the same UDP port):

```sh
node harness/ruview/bin/cli.js monitor --port COM8 --seconds 60
node harness/ruview/bin/cli.js esp32 --udp-port 5005 --seconds 60 --json
```

Keep full receiver output private: it can include network identifiers and
experimental inferred vital values. For stimulus experiments, the host sent
UDP datagrams only to the node's private LAN address, port 9, bounded to
59 seconds at the stated rate. This is not a recommendation to run continuous
traffic. Final publication status and follow-up measurements belong to the
linked release and tracking issue.

Remaining work includes identifying the exact board form factor, achieving a
repeatable 20 packets/s profile on this AP, and longer matched-condition runs.
No higher DSP rate or 5 GHz mode was selected. The successful install and
bounded transport evidence do not establish a general performance gain,
sensing accuracy or broader hardware qualification.
