# ADR-057: Firmware CSI Build Guard and sdkconfig.defaults

| Field       | Value                                       |
|-------------|---------------------------------------------|
| **Status**  | Accepted                                    |
| **Date**    | 2026-03-12                                  |
| **Authors** | ruv                                         |
| **Issues**  | #223, #238, #234, #210, #190                |

## Context

Multiple GitHub issues (#223, #238, #234, #210, #190) report firmware problems
that fall into two categories:

1. **CSI not enabled at runtime** - The committed `sdkconfig` had
   `# CONFIG_ESP_WIFI_CSI_ENABLED is not set` (line 1135), meaning users who
   built from source or used pre-built binaries got the runtime error:
   `E (6700) wifi:CSI not enabled in menuconfig!`

   Root cause: `sdkconfig.defaults.template` existed with the correct setting
   (`CONFIG_ESP_WIFI_CSI_ENABLED=y`) but ESP-IDF only reads
   `sdkconfig.defaults` - not `.template` suffixed files. No `sdkconfig.defaults`
   file was committed.

2. **Unsupported ESP32 variants** - Users attempting to use original ESP32
   (D0WD) and ESP32-C3 boards. The firmware targets ESP32-S3 only
   (`CONFIG_IDF_TARGET="esp32s3"`, Xtensa architecture) and this was not
   surfaced clearly enough in documentation or build errors.

## Decision

### Fix 1: Commit `sdkconfig.defaults` (not just template)

Copy `sdkconfig.defaults.template` → `sdkconfig.defaults` so that ESP-IDF
applies the correct defaults (including `CONFIG_ESP_WIFI_CSI_ENABLED=y`)
automatically when `sdkconfig` is regenerated.

### Fix 2: `#error` compile-time guard in `csi_collector.c`

Add a preprocessor guard:

```c
#ifndef CONFIG_ESP_WIFI_CSI_ENABLED
#error "CONFIG_ESP_WIFI_CSI_ENABLED must be set in sdkconfig."
#endif
```

This turns a confusing runtime crash into a clear compile-time error with
instructions on how to fix it.

### Fix 3: Fix committed `sdkconfig`

Change line 1135 from `# CONFIG_ESP_WIFI_CSI_ENABLED is not set` to
`CONFIG_ESP_WIFI_CSI_ENABLED=y`.

## Consequences

- **Positive**: New builds will always have CSI enabled. Users building from
  source will get a clear compile error if CSI is somehow disabled.
- **Positive**: Pre-built release binaries will include CSI support.
- **Neutral**: Original ESP32 and ESP32-C3 remain unsupported. This is by
  design - only ESP32-S3 has the CSI API surface we depend on. Future ADRs
  may address multi-target support if demand warrants it.
- **Negative**: None identified.

## Hardware Support Matrix

| Variant      | CSI Support | Firmware Target | Status        |
|--------------|-------------|-----------------|---------------|
| ESP32-S3     | Yes         | Yes             | Supported     |
| ESP32 (orig) | Partial     | No              | Unsupported   |
| ESP32-C3     | Yes (IDF 5.1+) | No           | Unsupported   |
| ESP32-C6     | Yes         | No              | Unsupported   |

## Notes

- ESP32-C3 and C6 use RISC-V architecture; a separate build target
  (`idf.py set-target esp32c3`) would be needed.
- Original ESP32 has limited CSI (no STBC HT-LTF2, fewer subcarriers).
- Users on unsupported hardware can still write custom firmware using the
  ADR-018 binary frame format (magic `0xC5110001`) for interop with the
  Rust aggregator.
