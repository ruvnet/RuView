# ADR-379: An OTA'd image confirms itself with a bounded health check, or rolls back

**Status:** Proposed. Hardware-verified on 2026-09-29 on an ESP32-S3 (node 4) and an ESP32-C6 (node 5).
**Date:** 2026-09-29
**Numbering:** On 2026-10-02 `main` had up to ADR-378 and open PRs claimed
nothing higher. If another branch also takes 379, renumber whichever merges
second.

## Context

Every 16 MB node was flashed with `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y`
(`sdkconfig.defaults.16mb`). With that option the bootloader starts a freshly
OTA'd image in `ESP_OTA_IMG_PENDING_VERIFY`. The app must then call
`esp_ota_mark_app_valid_cancel_rollback()`. If it doesn't, the bootloader marks
the image `ABORTED` at the next reset and boots the previous slot.

Nothing in `firmware/esp32-csi-node/main` made that call, and `nm` on the ELF
confirmed the symbol was not linked. RUNBOOK.md said an
`ota_rollback_boot_check()` "sits in the app", but no such function existed
anywhere in the tree. The consequences:

- An image delivered by `POST /ota` ran until the next reset, then reverted
  silently. OTA updates were not durable.
- While the image was pending, `esp_ota_begin()` refused a second OTA with
  `ESP_ERR_OTA_ROLLBACK_INVALID_STATE`.
- Nothing checked health, so a bad image was never rolled back on purpose. It
  only reverted if it happened to crash or be power-cycled.

A second defect meant none of this was ever reached: **OTA never succeeded
on this firmware before this change.** MEASURED on 2026-09-29, on node 4 (an
ESP32-S3 running the v0.8.12 16 MB build, with the OTA key provisioned):

- A `POST /ota` of a valid 1,228,672 B image logged `OTA update started`, then
  `stack overflow in task httpd`, then `rst:0xc RTC_SW_CPU_RST`. The node came
  back on its old slot. Both attempts failed the same way.
- The cause: the server used `HTTPD_DEFAULT_CONFIG()`, which gives a 4096 B
  task stack. The handler held a 1 KB receive buffer on that stack, while
  calling `esp_ota_*` and formatting a float progress log.

## Decision

At boot, `ota_health_start()` (in `main/ota_health.c`, called early in
`app_main`) reads the running partition's state:

| state | meaning here | action |
|---|---|---|
| `PENDING_VERIFY` | just OTA'd, rollback armed | run the health check |
| `VALID` | USB-flashed with `ota_data_initial.bin` (the bootloader writes VALID), or already confirmed | nothing |
| `UNDEFINED` | selected by a build without rollback; boots unconditionally | nothing |
| `NEW` | bootloader lacks rollback, so it never promoted the image | nothing |
| error (`NOT_FOUND`/`NOT_SUPPORTED`) | no otadata entry, or factory partition | nothing |

For this table I read the ESP-IDF v5.5 source: `bootloader_utility.c`
(`set_actual_ota_seq`, the NEW to PENDING_VERIFY to ABORTED transitions) and
`esp_ota_ops.c`.

**The health check.** A low-priority task polls once a second and feeds
`ota_health_step()`, which is a pure function in `ota_health.h`:

- **Signals, both latched:** the STA got an IP (`IP_EVENT_STA_GOT_IP`, via
  `ota_health_note_got_ip()` in `main.c`'s handler), and the stream sender
  accepted at least one CSI frame (`csi_collector_get_send_ok_count() > 0`).
  The second signal covers driver callback, serialization and `sendto()`
  together.
- **Minimum uptime** `CONFIG_OTA_HEALTH_MIN_UPTIME_S` (default 30 s): the image
  is not confirmed before this point even with both signals present. An image
  that crashes soon after starting its pipelines still reverts, because a crash
  while pending is a rollback.
- **Deadline** `CONFIG_OTA_HEALTH_TIMEOUT_S` (default 120 s, measured from
  boot). Both signals are required by then. The minimum uptime is clamped to
  the deadline.
- **Pass:** `esp_ota_mark_app_valid_cancel_rollback()`, which logs
  `marked valid after <reason> in <ms> ms`.
- **Fail:** logs `health check failed: <reason>, rolling back`, then calls
  `esp_ota_mark_app_invalid_rollback_and_reboot()`. If no other bootable slot
  exists, that call returns an error. The node then logs and stays up rather
  than reboot-looping.
- **Terminal verdict:** once the check has confirmed or rolled back, it never
  changes.

**Negative-test hook.** `CONFIG_OTA_HEALTH_FORCE_FAIL` (default n, test only)
makes the check roll back at the point where it would have confirmed. Use it to
prove rollback end to end.

**Remote visibility.** `GET /ota/status` now includes `ota_state` (`valid`,
`pending_verify`, `new`, `undefined`, `other`, or `none`). An operator can
confirm durability without a serial console.

**The httpd stack.** The OTA/WASM HTTP server now sets `config.stack_size =
CONFIG_OTA_HTTPD_STACK_SIZE`, which defaults to 8192. Three smaller changes go
with it:

- The OTA receive buffer is static rather than on the stack. Handlers run one
  at a time on the single httpd task, so one buffer is enough.
- The progress log prints an integer percent instead of a float.
- `GET /wasm/list` keeps its 2 KB JSON buffer on the heap, not the stack.

`recv_wait_timeout` stays at 30 s. Each `POST /ota` and `POST /wasm/upload` now
logs `httpd stack after <handler>: N of M bytes never used`, so the size can
be re-tuned from device measurements.

Sizing: this is an estimate from static data, not a device measurement. It
comes from a `-fstack-usage` build of the S3 16 MB configuration with IDF v5.5.

- **Frame sizes along the OTA path** (bytes):

  | function | bytes |
  |---|---|
  | `httpd_thread` | 144 |
  | `httpd_sess_process` | 32 |
  | `httpd_req_new` | 176 |
  | `httpd_uri` | 64 |
  | `ota_upload_handler` | 192 (1,216 with the old 1 KB buffer) |
  | `esp_ota_end` | 48 |
  | `image_validate` | 304 |
  | `esp_image_verify` | 32 |
  | `image_load` | 80 |
  | `process_segments` | 112 |
  | SHA-256 | about 110 |
  | mmap/flash read | about 190 |

- That chain comes to about 1.5 KB now, and about 2.5 KB with the old buffer.
- Two things are not in the static data:
  - libc's `vfprintf` behind every `ESP_LOG`. This build uses full newlib,
    not nano, and the old progress line formatted a float.
  - The flash-chip driver, which is reached through function pointers.

  Together they plausibly add 1 to 1.5 KB. That explains an overflow at 4096
  with the old handler.
- The WASM upload path adds `wasm_upload_process` (336) and
  `rvf_verify_signature` (352), plus a wasm3 module load that wasn't measured.
- 8192 leaves roughly 4 to 5 KB above the estimated worst case, and costs
  4 KB of internal RAM. The high-water-mark log is the check on that estimate.

When the build has no `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE` (the 8 MB S3 and
4 MB C6 CI lanes), both entry points compile to no-ops.

## Consequences

- OTA updates become durable on rollback-enabled nodes once the image passes
  the check. A second OTA is accepted after confirmation, which can come as
  early as `MIN_UPTIME_S`.
- A bad image now reverts instead of limping, but "bad" only means it failed
  to reach the network or deliver CSI. An image that passes and then
  misbehaves later is not caught.
- If the AP is down when a node boots an OTA'd image, a good image rolls back.
  That is the safe direction: push it again later.
- Mock/QEMU builds that skip Wi-Fi can never pass. They must not enable
  bootloader rollback, and none do today.
- Bootloader capability is still invisible remotely (RUNBOOK §2). A node
  whose bootloader lacks rollback reports `ota_state: "new"` after an OTA. That
  is the first remote hint of which bootloader a board has.

## Evidence

- Host test `firmware/esp32-csi-node/test/test_ota_health.c` runs in
  `make host_tests`, which CI runs. It covers pass, each timeout reason, the
  terminal verdict, force-fail, the clamp, and zero soak. SYNTHETIC: it
  exercises the decision function, not a device.
- ESP-IDF v5.5 S3 and C6 builds with rollback enabled link
  `esp_ota_mark_app_valid_cancel_rollback` (verified with `nm`). A build is not
  hardware evidence.
- The httpd overflow is MEASURED on hardware (node 4, above).
- Hardware verification, MEASURED on 2026-09-29 on node 4 (ESP32-S3,
  16 MB, rollback bootloader, OTA key provisioned). The
  build from this branch was first written over USB. The pushes were made from
  a Pi 5 with the `sensor-ota-push` cog.
  1. Positive. An image built from this branch was pushed to `ota_1`: HTTP 200,
     1,230,848 B in 22 s.
     - The node logged "httpd stack after POST /ota: 4900 of 8192 bytes never
       used", so the upload's peak stack use is about 3.3 KB. With the old 1 KB
       on-stack buffer that is over 4 KB, which is consistent with the
       overflow.
     - It rebooted and logged "OTA image pending verify on ota_1".
     - `/ota/status` then read `ota_state: valid`.
     - After a hard reset it came back on the same build in `ota_1`, still
       `valid`, so the update is durable.
  2. Negative. A `CONFIG_OTA_HEALTH_FORCE_FAIL=y` build was pushed to `ota_0`:
     HTTP 200.
     - `/ota/status` showed it running as `pending_verify` for about 30 s.
     - The node then rebooted and came back on the previous build in `ota_1`,
       `valid`, so rollback works.
  3. One push attempt failed mid-upload when a serial capture was opened on the
     node's USB-Serial-JTAG port at the same moment. The node reset and was
     unharmed. Don't hold the USB console open while pushing OTA to a
     USB-attached node.
- C6, MEASURED on 2026-09-29 on node 5 (ESP32-C6, 8 MB
  flash, 4 MB layout, rollback bootloader).
  - The branch build was first written over USB and the OTA key provisioned.
  - A second build was pushed from the Pi 5 cog to `ota_1`: HTTP 200 in 11 s.
  - `/ota/status` showed `pending_verify`, then `valid` about 28 s later.
  - After a hard reset it stayed on the new build, `valid`.
- Not yet verified: the 120 s no-IP timeout path; a real power-cycle (a hard
  reset via RTS was used instead); the rollback test on the C6 (run on the S3
  only).
