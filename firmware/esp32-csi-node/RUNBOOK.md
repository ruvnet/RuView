# Firmware runbook — ESP32-C6 CSI node

Operational procedure. `README.md` explains the project; this says exactly what
to run, in what order, and what to check. Follow it top to bottom.

If you are an agent: **read this file before touching firmware.** Everything
below was learned by getting it wrong at least once.

---

## 1. Build

**Pick the command for your board's flash size, then copy it.** Run from the
**repository root**, not this directory.

### 16MB boards

```bash
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd)/firmware/esp32-csi-node:/project" -w /project \
  espressif/idf:v5.4 bash -c \
  "cat sdkconfig.defaults sdkconfig.defaults.esp32c6 sdkconfig.defaults.16mb \
     > sdkconfig.defaults.build && \
   SDKCONFIG_DEFAULTS='sdkconfig.defaults.build' idf.py set-target esp32c6 && \
   idf.py build"
```

### 4MB and 8MB C6 dev boards

**Omit `sdkconfig.defaults.16mb` entirely.** `sdkconfig.defaults.esp32c6`
already carries 4MB geometry, so with the 16MB layer absent the target overlay
is the last word and no override is needed:

```bash
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd)/firmware/esp32-csi-node:/project" -w /project \
  espressif/idf:v5.4 bash -c \
  "cat sdkconfig.defaults sdkconfig.defaults.esp32c6 \
     > sdkconfig.defaults.build && \
   SDKCONFIG_DEFAULTS='sdkconfig.defaults.build' idf.py set-target esp32c6 && \
   idf.py build"
```

That yields `FLASHSIZE "4MB"` with `partitions_4mb.csv` -- two 1.875 MB OTA
slots against a roughly 978 KB binary, so OTA has ample headroom.

**One behavioural difference to know about.** `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE`
is set *only* in `sdkconfig.defaults.16mb`. A 4MB build therefore has no
automatic rollback: an OTA'd image does not boot PENDING_VERIFY and the
bootloader will not revert a bad image on the next boot. Nothing about 4MB
flash forces that -- `partitions_4mb.csv` has an `otadata` partition and two
OTA slots -- it is simply which layer the flag currently lives in. Treat a 4MB
OTA as unguarded until that is changed deliberately, and keep a serial recovery
path available.

### CORRECTED 2026-09-10 — why order matters on the 16MB path

The 16MB command previously read
`... sdkconfig.defaults.16mb sdkconfig.defaults.esp32c6`, and **it silently produced a
4MB image on 16MB hardware** -- not a broken build, just the wrong one, which is
why it survived so long. In a single
concatenated file the *last* assignment wins, and the three files disagree:

| file | sets |
|---|---|
| `sdkconfig.defaults` | `FLASHSIZE "8MB"`, `partitions_display.csv` |
| `sdkconfig.defaults.16mb` | `FLASHSIZE "16MB"`, `partitions_16mb.csv`, `BOOTLOADER_APP_ROLLBACK_ENABLE=y` |
| `sdkconfig.defaults.esp32c6` | `FLASHSIZE "4MB"`, `partitions_4mb.csv` |

With `.esp32c6` last it overrode `.16mb`, so the build came out 4MB with
`partitions_4mb.csv`. Only the rollback flag survived, because `.esp32c6` does
not set it -- which is exactly why this was hard to spot: the one symptom the
old text warned about (missing rollback) was the one symptom that *didn't*
appear.

**MEASURED both ways, 2026-09-10**, same worktree, same container, only the
order changed:

```
.16mb then .esp32c6  ->  FLASHSIZE "4MB"   partitions_4mb.csv    ROLLBACK=y
.esp32c6 then .16mb  ->  FLASHSIZE "16MB"  partitions_16mb.csv   ROLLBACK=y
```

`.16mb` must come **last** because it is the override layer: it is the only
file that describes the fleet's actual flash geometry, and every earlier file
is a more general default.

**This is not a new theory -- it restores what the file always said.**
`sdkconfig.defaults.16mb` documents the correct order in its own header:

```
# Build:
#   idf.py -DSDKCONFIG_DEFAULTS="sdkconfig.defaults;sdkconfig.defaults.esp32c6;sdkconfig.defaults.16mb" build
```

So there are two independent confirmations: the A/B measurement above, and the
file's own docstring. The runbook's `cat` simply had the last two arguments
transposed relative to the instructions sitting inside the file it was
concatenating.

### OPEN: `DYNAMIC_TX_BUFFER_NUM` is 64 here, but the fleet is documented at 128

Even with the corrected order the build yields
`CONFIG_ESP_WIFI_DYNAMIC_TX_BUFFER_NUM=64`. Only `sdkconfig.defaults` sets it
(to 64) and neither other file overrides it, so **no combination of these three
files produces 128.** Either 128 arrives from somewhere not yet found, or the
"fleet runs 128" claim is wrong. Do not "fix" this by editing a defaults file
until that is settled -- read it off a running node first. The verification
grep below therefore expects 64 today, not 128.

Takes ~3 minutes cold, well under a minute incremental.

### The three defaults files are NOT all automatic

IDF picks up `sdkconfig.defaults` and `sdkconfig.defaults.<target>` on its own.
It does **not** pick up `sdkconfig.defaults.16mb`, and that file is where
`CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE=y` lives.

Build without it and you get an image that is correct in every visible respect
and **cannot roll back a bad OTA** — while `ota_rollback_boot_check()` sits in
the app expecting the capability. Silent, and only discoverable by pushing a
deliberately bad image.

That is why the command above concatenates all three explicitly rather than
relying on discovery.

### Never `rm -rf sdkconfig` without a backup

The README's headline example opens with `rm -rf build sdkconfig`. `sdkconfig`
is **untracked and gitignored**. `partitions_16mb.csv` was lost to exactly this,
and `CLAUDE.md` warns about it *even when a README documents it*.

```bash
cp sdkconfig "sdkconfig.backup-$(date +%Y%m%d-%H%M%S)"   # then regenerate
```

A stale `sdkconfig` is a real hazard, not a hypothetical: one found on
2026-09-03 said `FLASHSIZE=4MB`, `partitions_4mb.csv`,
`DYNAMIC_TX_BUFFER_NUM=64` — against a fleet running 16MB and 128. Anything
built from it would silently not be the configuration under test.

### Verify before you flash anything

```bash
grep -E "^CONFIG_(IDF_TARGET|ESPTOOLPY_FLASHSIZE|PARTITION_TABLE_CUSTOM_FILENAME|\
ESP_WIFI_DYNAMIC_TX_BUFFER_NUM|BOOTLOADER_APP_ROLLBACK_ENABLE)=" sdkconfig
```

All five must read:

| key | expected |
|---|---|
| `CONFIG_IDF_TARGET` | `"esp32c6"` |
| `CONFIG_ESPTOOLPY_FLASHSIZE` | `"16MB"` |
| `CONFIG_PARTITION_TABLE_CUSTOM_FILENAME` | `"partitions_16mb.csv"` |
| `CONFIG_ESP_WIFI_DYNAMIC_TX_BUFFER_NUM` | `128` |
| `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE` | `y` |

**Do not use the native `C:\Espressif\...esp-idf-v5.5.5`.** Wrong version, its
venv does not match the default python, and a container-built `build/` refuses
a native build outright. Docker only.

---

## 2. What OTA can and cannot reach

This is the distinction that decides whether a change needs a cable.

| region | offset | reachable by OTA? |
|---|---|---|
| bootloader | `0x0` | **NO — USB only** |
| partition table | `0x8000` | **NO — USB only** |
| otadata | `0xF000` | rewritten by OTA |
| ota_0 | `0x20000` | yes (4 MB) |
| ota_1 | `0x420000` | yes (4 MB) |
| coredump | `0x820000` | no |
| storage (fat) | `0x830000` | no |
| nvs | `0x9000` | never written by a flash — provisioning survives |

**OTA replaces the app partition only.** Anything in the bootloader —
`CONFIG_BOOTLOADER_*`, notably `APP_ROLLBACK_ENABLE` — or any partition-table
change **requires USB on every board**.

There is **no bootloader version on the wire**. A node's sync packet reports app
health, not bootloader capability, so you cannot determine remotely which boards
have a rollback-capable bootloader. Track it per board, or do a USB pass across
all of them.

Corollary: "every node reports health" proves the **app** is current and proves
nothing about the bootloader. Do not answer a bootloader question with app
evidence.

---

## 3. Deploying

### Over the air — app-only changes

Works end-to-end and is the default for anything that is not a bootloader or
partition change. Roll to **one node**, soak it, then the rest.

### Over USB — bootloader, partition table, or recovery

```bash
python -m esptool --chip esp32c6 -b 460800 \
  --before default_reset --after hard_reset write_flash \
  --flash_mode dio --flash_size 16MB --flash_freq 80m \
  0x0     build/bootloader/bootloader.bin \
  0x8000  build/partition_table/partition-table.bin \
  0xf000  build/ota_data_initial.bin \
  0x20000 build/esp32-csi-node.bin
```

`0x9000` is not in that list, so SSID, password and `node_id` survive.

### Safe single-board diagnostic flash, with a revert path

Nodes run from whichever slot `otadata` selects. **Parse it before flashing** —
two 32-byte entries at offsets 0 and 0x1000; active slot is
`(max ota_seq - 1) % 2`.

1. Back up `otadata` (8 KB) **and** the live app slot with `esptool read-flash`.
2. Flash the diagnostic build — it lands in `ota_0` and rewrites `otadata`.
3. **Revert = write the 8 KB `otadata` backup back.** Production in `ota_1` was
   never touched, so no rebuild is needed.

Validate any backup image before trusting it: byte 0 must be `0xE9`, and the
`esp_app_desc_t` magic at offset `0x20` must be `0xABCD5432`.

**Never flash the whole fleet from a workstation, and never use the server's OTA
endpoint for a diagnostic.** USB, one board, revert path secured first.

---

## 4. Serial

CH343 USB-serial at 115200.

To read a running board **without resetting it**, open with
`DtrEnable=$false; RtsEnable=$false`. esptool asserts RTS and *will* reset the
board, which loses the state you were trying to observe.

---

## 5. Changing config without a cable

Most parameters do not need a flash at all:

```bash
export RUVIEW_OTA_PSK_FILE=<path to the psk file>       # never inline the key
python config_push.py --node <node-ip> --get
python config_push.py --node <node-ip> --set led_brightness=10
```

Keys marked `assoc: true` in `config_api.c` (`wifi_ssid`, `wifi_password`,
`channel_hop_count`, `dwell_ms`, `csi_channel`) are applied as a **trial**: the
node banks its current values, reboots, and keeps the change only if it
re-associates. Everything else applies without that risk.

`config_push.py` has **no unset**. To restore a default you must set it
explicitly. Current defaults:

| key | default | source |
|---|---|---|
| `vital_interval_ms` | 1000 | `Kconfig.projbuild` |
| `swarm_heartbeat_sec` | 30 | `nvs_config.c` |
| `swarm_ingest_sec` | 5 | `nvs_config.c` |
| `beacon_period_ms` | 0 = derive from fleet size | `nvs_config.c` |

---

## 6. Reading a node remotely

`GET /api/v1/mesh` returns per-node `health`: `min_heap_kib`, `die_c`,
`thermal_state`, `tx_dbm`, `reset_reason`, and from sync proto v3 the TX-path
counters `send_fail` / `rate_skip` / `early_drop`.

`min_heap_kib` is a **minimum-since-boot watermark**, not an instantaneous
reading. It resets on reboot and only ever falls. Sampling it within a minute of
a restart tells you nothing — that mistake produced a wrong "no effect" verdict
on 2026-09-03; the real answer, once it had settled, was −56 KiB.

`/api/v1/nodes` `rssi_dbm` is **not** the AP link — it is whatever link landed
most recently and swings 20+ dB with no physical change. Filter `/api/v1/links`
by the AP's BSSIDs instead, and read `csi_fps_ema` alongside, because link count
alone hides a starved node.

---

## 7. Tunables, and which ones depend on your house

`idf.py menuconfig` exposes these. The defaults are what a nine-node ESP32-C6
fleet in one house settled on; several are **not** universal, and the point of
listing them here is that you can pick and measure rather than inherit someone
else's building.

| option | default | depends on your house? |
|---|---|---|
| `CSI_GATE_MESH_ALIGNED` | y | **yes** |
| `CSI_SEQ_DIAG` | n | diagnostic only |
| `THERMAL_MONITOR` | y | no |
| `THERMAL_THROTTLE` | n | **yes** |
| `THERMAL_THROTTLE_C` | 72 | **yes** |
| `THERMAL_HYSTERESIS_C` | 8 | no |
| `UPLINK_WATCHDOG` | y | no |
| `UPLINK_WATCHDOG_TIMEOUT_S` | 1200 | **yes** |

### The ones worth measuring yourself

**`CSI_GATE_MESH_ALIGNED`** decides whether nodes keep the *same* frames or
merely the same *number* of frames, which is what makes cross-node pairing
possible at all. Whether alignment helps depends on how much your nodes
actually overhear in common, and that is a property of your walls and your
transmitters. In one measured nine-node fleet, aligning windows bought
+0.5 pp of pairing while selecting frames by `rx_seq` bought +31.7 pp -- so do
not assume the default is doing the heavy lifting in your building.

Turn on `CSI_SEQ_DIAG` to measure it. It logs `(source MAC, rx_seq)` per CSI
callback, which is what lets you compute the fraction of frames two nodes hold
in common instead of guessing.

**`THERMAL_THROTTLE` / `THERMAL_THROTTLE_C`** depend on enclosure and placement,
not on the chip. A bare bench board never reaches 72 C; the same board sealed in
a printed case on a sunny wall can. Read `die_c` from `/api/v1/mesh` for a few
days before deciding, because throttling transmit power costs range.

**`UPLINK_WATCHDOG_TIMEOUT_S`** is a bet about your AP. 1200 s suits a network
where a wedge is rare and a reboot is cheap. If your uplink drops briefly and
often, a short timeout turns a recoverable gap into a reboot loop; if it wedges
hard, a long one leaves a node dark for twenty minutes.

### A worked example: the same two options, in one house

MEASURED on a nine-node ESP32-C6 fleet in a single detached house. **These are
our numbers, not yours.** They are here to show the shape of the decision, and
how much a building can move it.

Four arms as a 2x2 factorial, 20-minute runs, three Latin-square blocks so slow
drift landed on every arm equally. Metric is the fraction of transmissions held
in common by two or more nodes.

| arm | mean |
|---|---|
| `elapsed` (per-node gate) | 0.2434 |
| `mesh` (aligned windows) | 0.2480 |
| `seq` (keep `rx_seq` divisible by 8) | **0.5605** |
| `mesh` + `seq` | 0.5452 |

`seq` roughly doubled pairing, +31.7 pp, about 70x the measured drift floor and
reproduced in all three blocks. Mesh alignment bought **+0.5 pp** -- inside
noise. Stacking them was very slightly *worse* than `seq` alone.

**Why that happened here, and why yours may differ.** These nodes already
overheard a great deal in common -- two boards side by side heard the same
transmissions 72% of the time -- so there was a large pool of shared
transmissions for `seq` to select from, and little left for window alignment to
recover. A house with fewer shared transmitters, thicker interior walls, or
nodes spread so that few pairs hear the same radios has less for `seq` to work
with, and phase alignment may matter more rather than less. We did not measure
such a house and do not claim to know.

**It is a trade, not a free win.** At period 8 the `seq` arms carried ~75-95k
transmissions per 20 minutes against ~120-160k ungated: pairing is bought with
raw frame rate. Absolute paired transmissions per minute still rose (2629 vs
2524), because the discarded frames are disproportionately ones only one node
would have kept. Whether that trade is right depends on what consumes your
stream. On-device edge processing was **unaffected** here -- frames processed
stayed ~1900 per node per 4 minutes in both modes, because that pipeline is
separately rate-limited -- so the cost fell on the uplink, not on local sensing.
If your uplink is the scarce resource, that changes the answer.

**The two options differ in kind, not just in effect.** Mesh alignment is an
agreement between receivers: it needs a shared clock, so it inherits whatever
the sync topology does. Selecting by `rx_seq` needs no agreement at all. The
802.11 sequence control field is assigned by the TRANSMITTER and is identical
at every receiver, so it is a coordination-free name for one transmission --
two nodes that have never heard of each other, sit in different sync domains,
or run with no mesh whatsoever still keep the same subset of frames.

In the code the seq test runs before any sync check and reads only the frame;
only the mesh branch calls `c6_sync_espnow_is_valid()`. That asymmetry is the
reason `seq` had nothing to degrade in our runs, and it is worth weighing
directly: if your fleet's sync is fragile, `seq` is not merely the better
performer, it is the one with no dependency that can quietly fail.

**A limitation of our own measurement, which cuts against the mesh arm.** The
mesh gate uses mesh-time buckets only while `c6_sync_espnow_is_valid()`; a node
with no valid sync falls back to the elapsed gate rather than gating on a
meaningless epoch. That is the right behaviour, but it means our `mesh` arms
were partly `elapsed` arms: three of nine nodes never synchronised, so for those
three the setting under test was silently not in effect. Read the +0.5 pp as
"mesh where sync held, elapsed where it did not" rather than as a clean
fleet-wide measurement of alignment.

This is itself a house-dependent property. Time sync here is leader-elected and
hub-and-spoke -- one node broadcasts and the rest follow. Whether every node can
hear that leader is a question about your building, not about the firmware. In a
larger or more divided house the leader may not reach everyone, the fleet
fragments into sync domains, and any option that depends on shared time degrades
quietly for the nodes that fell out. Check `is_leader` and sync validity in
`/api/v1/mesh` before concluding that an alignment setting did or did not help.

**A prediction we got wrong, kept because it is the useful part.** Our mesh had
split into two leader domains with three nodes never synchronising at all. We
expected `seq` to win most on pairs spanning that split. Per-pair yield said
otherwise: same-domain 5.82x, cross-domain 6.24x, involving a never-synced node
**5.16x** -- the lowest, the opposite of the prediction -- and within-group
spread (3.6x-8.1x) dwarfed every between-group difference.

So `seq` is **not** a workaround for broken time sync; it wins roughly
uniformly, including where sync is healthy. The consequence is negative and
worth having: repairing the mesh split would probably not have closed the gap,
and that effort would have been spent for nothing.

### Measuring rather than guessing

A gate comparison needs a fixed environment, repeated blocks, and every arm run
on every node -- otherwise node placement is confounded with the setting. The
harness used for the published comparison is in the repository; use it rather
than eyeballing a frame rate, which moves for reasons unrelated to the setting.
