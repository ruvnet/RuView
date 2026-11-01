# ESP32 CSI input release candidate, updated 2026-10-21

Status: software checks and a short ESP32-C6 boot/transport check passed.
Full release acceptance remains gated on the latest hosted checks. No sensing
accuracy or ESP32-S3 physical qualification is claimed.

Candidate source: `f03d5ac06` on `integrate/release-20261017`. Changes cover
#2286, #2287, #2302, configuration validation and rollback recovery (#2289),
and the Rust audit/dependency fixes (#2315).

## Software evidence

- Firmware `make host_tests` passed under `espressif/idf:v5.4`, including
  production-code STBC, self-ping, configuration rules, write-through NVS
  rollback fault injection, onboarding, and OTA-health checks.
- The rollback regression executes the production HTTP handler with real
  cJSON and deterministic write-through NVS failure injection. It covers
  partial writes, failed restore/erase/commit, retained recovery state, write
  locking, and boot-time rollback.
- ESP32-C6 built with ESP-IDF 6.0.2. Image size is 1,123,408 bytes
  (`0x112450`), within the 1,900,544-byte app slot (`0x1d0000`).
- Local `cargo audit` from `v2/` found zero unignored vulnerabilities. The
  hosted Rust dependency audit passed on the preceding release head; the
  corrected current head is running fresh checks.
- The focused BFLD changelog test passed 5/5 after keeping the existing
  unreleased BFLD entry ahead of the 0.8.15 release heading.

## Attached hardware

The attached ESP32-C6 revision 0.2 has 8 MB flash. A read-only full flash
backup of 8,388,608 bytes completed at 115200 baud and is stored outside the
repository under the user's local qualification directory. No backup or raw
device data is included in this repository.

The 0.8.15 application was written to the existing `ota_0` app slot at
`0x20000`; the partition table and NVS were not changed. esptool verified the
write. `/ota/status` reported version `0.8.15`, `running_partition=ota_0`, and
`ota_state=valid` after reboot.

A 30-second in-memory UDP sample after the update counted 441 raw CSI packets,
26 vitals packets, and 30 feature packets. Raw packet subcarrier counts were
256: 199, 64: 237, and 128: 5. The sample had zero short packets, zero
presence/count contradictions, and zero vitals timestamp reversals. Raw CSI
and vitals values were not persisted.

An earlier paired 30-second transport check on 0.8.14 recorded 28.97 raw
packets/s at baseline and 29.60 packets/s while a bounded host UDP traffic
source ran. This is AP-specific transport evidence, not proof of sensing
accuracy or a universal minimum rate.

The C6 check does not qualify the S3-specific STBC correction. It was a short
boot/stream check, not a five-minute stability run or an accuracy test. The
CP210x UART bridge did not provide the native USB serial/JTAG boot log, so
physical IDF 6 USB onboarding receive behavior remains unverified.

## Release boundary

The firmware workflow publishes ESP32 tags as prereleases and states that CI
and host tests do not establish physical-board qualification or sensing
accuracy. The release remains a prerelease. The latest hosted run on the
previous candidate passed firmware matrices, security checks, and audit; one
workspace test failed because the 0.8.15 changelog heading interrupted an
existing BFLD unreleased section. That layout is corrected, and current-head
checks must finish green before merge and publication.
