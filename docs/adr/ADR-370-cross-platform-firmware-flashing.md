# ADR-370: Cross-platform, verified ESP32 firmware flashing from npm

- **Status**: proposed
- **Date**: 2026-09-30
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: firmware, esp32, esptool, supply-chain, hardware-evidence
- **Related**: ADR-028 (capability audit / witness), ADR-369, firmware
  `README.md` §0 and §2

## Context

The documented flash procedure is four `esptool write_flash` offsets. It is
typed by hand, and a wrong chip, wrong flash size, or tampered image can brick
a board or silently run the wrong firmware. The previous `ruview_node_flash`
was Windows-only and never flashed. Project rules also say that a successful
build or write is **not** hardware validation; that requires a captured
boot/runtime log from real silicon.

## Decision

`src/firmware.js` implements plan → verify → preflight → write → evidence:

1. **Variants**:
   - `s3-8mb` → `esp32s3` / 8MB
   - `s3-4mb` → `esp32s3` / 4MB; prefers `*-4mb.bin` images
   - `c6` → `esp32c6` / 4MB

   Offsets follow the shipped partition tables: bootloader `0x0`, partition
   table `0x8000`, optional OTA data `0xf000`, app `0x20000`. NVS is never
   written, so WiFi and node configuration survive.
2. **Bundle verification**. A bundle is an extracted release flash bundle,
   `release_bins/<variant>`, or an ESP-IDF `build/` directory.
   - Every image must match `SHA256SUMS` or `SHA256SUMS.txt`. A mismatch is
     always fatal.
   - Missing checksums are refused unless `allow_unverified` is set, which is
     for local builds only.
   - Images must be regular, non-empty files of at most 16 MiB, inside the
     bundle; symlinks that escape the bundle are refused.
   - An 8 MB variant refuses a 4 MB partition table.
   - A bundle without OTA data produces an explicit warning (a node on `ota_1`
     keeps booting `ota_1`).
3. **Argv, not shell.**
   - The port must match `COM<n>` or `/dev/(tty|cu)*`.
   - Baud comes from a fixed set.
   - The command is `python -m esptool --chip … --port … --baud … write_flash
     --flash_mode dio --flash_size … <offset> <file>…`. The underscore forms
     work in both esptool 4.x and 5.x.
4. **Chip preflight.** `esptool chip_id` must report the variant's chip. The
   parser covers the 4.x "Chip is" and 5.x "Chip type:" / "Detecting chip
   type…" formats. A mismatch or missing response stops the flow before any
   write.
5. **Evidence.** After a write, a read-only pyserial capture (default 15 s,
   port and duration passed as argv) is summarized: line count, CSI
   callbacks, MGMT+DATA upgrade, panic markers, firmware version.
   - `hardwareValidated: true` only when CSI callbacks appear with no panic;
     the result's `evidence` is then `MEASURED` with a boot-log reason.
   - Otherwise the result states the board is not validated.
6. **Authority.** Without `confirm` the result is a plan. MCP needs the
   `hardware-write` grant plus `confirm`. `ruview_firmware_plan` and
   `ruview_firmware_ports` are read-only.
7. **Provisioning stays out.** WiFi credentials remain in `provision.py`, so
   they never enter harness arguments, logs, or MCP transcripts.

## Evidence

- `npx @ruvnet/ruview doctor --group firmware` verified all four checked-in
  `release_bins` bundles (c6-adr110, c6-onboarding, s3-adr110,
  s3-fair-adr110) against their `SHA256SUMS.txt` in this change's development
  container.
- The write and preflight paths are covered by tests with injected fake
  hardware: mismatch refusal, probe failure, flash failure, boot-log
  validation, and a silent log that yields no validation.
- **No real board was flashed** during this change, so real-silicon validation
  is outstanding.

## Consequences

- Operators on any OS get one verified command. Agents can review plans
  without hardware authority.
- The workflow depends on Python, esptool, and pyserial. `doctor` reports each
  one with a fix.
- Follow-up: an ADR-028 witness record could store the boot-log summary and
  image digests.
