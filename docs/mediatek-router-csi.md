# MediaTek Router CSI (MT7981, experimental)

RuView can take channel state information (CSI) from a MediaTek Filogic
router, such as an MT7981B with an MT7976C radio running OpenWrt with
MediaTek's vendor CSI patch. The router reports CSI. A host-side bridge
converts it to RuView's MTC1 wire format (ADR-267). The sensing server ingests
it like any other UDP source.

**Status: experimental.** This page documents a working data path, measured
on one board model. It makes no sensing-quality claim. Frames from this path
carry the provenance label `mediatek:physical-unvalidated`. The ADR-266
hardware gates are still open: calibration, sequence, timestamp, and
repeatability.

Developed in whitsentry, a RuView-based project.

Evidence tags used below: **MEASURED** (with a reproducer), **CODE-DERIVED**
(read from source or build scripts, not run here), **CLAIMED** (stated, not
independently verified), **SYNTHETIC** (simulated or fabricated data).

## Contents

- [Supported hardware](#supported-hardware)
- [How the path works](#how-the-path-works)
- [Build the OpenWrt image](#build-the-openwrt-image)
- [Arm CSI on the router](#arm-csi-on-the-router)
- [Run the bridge](#run-the-bridge)
- [Start the sensing server](#start-the-sensing-server)
- [What the data is, and is not](#what-the-data-is-and-is-not)
- [Evidence](#evidence)
- [Troubleshooting](#troubleshooting)
- [Reference](#reference)

## Supported hardware

| Board | SoC + radio | Status |
|---|---|---|
| Wavlink WL-WN586X3 **Rev A** | MT7981BA + MT7976CN (switch MT7531AE) | Run on two units. CSI captured from both is analysed offline below (MEASURED). Live delivery to the sensing server is CLAIMED from local run logs that are not in the tree. |
| OpenWrt One | MT7981B + MT7976C | CLAIMED: same silicon per the OpenWrt hardware table and ADR-266. Not run with this patch set. |
| Xiaomi AX3000T | MT7981B + MT7976C | CLAIMED: same silicon per the OpenWrt hardware table. Not run with this patch set. |
| Other MT7981/MT7986 boards | — | Untested. The CSI patch targets the `mt7915` driver family, so other Filogic radios may work, but nothing here shows that. |

The board patch in `firmware/openwrt-wn586x3/` is specific to the WN586X3
**Rev A**. The CSI driver patch and the two userspace fixes do not depend on
the board.

Each MT7976C radio has **two receive chains**. The WN586X3's four external
antennas are a 2.4 GHz pair and a 5 GHz pair. It is not a 4x4 array, and two
routers are two independent receivers, not one phased array.

## How the path works

```
 router (OpenWrt 24.10.8)                          host
 ┌────────────────────────────────┐               ┌──────────────────────────────┐
 │ mt76 + MediaTek CSI patch      │               │ wifi-densepose-mtk-bridge    │
 │   firmware MCU event 0xc2      │  JSON dump    │   groups chains into frames, │
 │ mt76-vendor dump csi  ─────────┼──────────────►│   applies provenance gate,   │
 │ CSIdump (UDP, optional) ───────┼──── UDP ─────►│   emits ADR-267 MTC1         │
 └────────────────────────────────┘               └──────────────┬───────────────┘
                                                                 │ UDP :5005
                                                  ┌──────────────▼───────────────┐
                                                  │ sensing-server               │
                                                  │   --source mediatek          │
                                                  │   /api/v1/csi/mediatek/*     │
                                                  └──────────────────────────────┘
```

Two transports leave the router:

- **`mt76-vendor <iface> dump csi <n> <file>`** writes a JSON array of
  per-chain records: timestamp, transmitter address, RSSI, SNR, bandwidth,
  PPDU mode, tx/rx chain index, chain marker, and I/Q. This is the richer
  path, and the one that carried sustained use. You copy the file to the host
  (for example, a loop over SSH) and replay it through the bridge.
- **`CSIdump`** (MtkCSIdump) streams one antenna per UDP datagram and drops
  RSSI, PPDU mode, and transmitter identity. The bridge emits one 1x1 frame per
  datagram. This path was verified end to end once, as a recording. It has not
  run as a standing service.

## Build the OpenWrt image

The patch set lives in [`firmware/openwrt-wn586x3/`](../firmware/openwrt-wn586x3/README.md).
It is not a firmware image. Build and hash your own.

| Patch | Applies to |
|---|---|
| `1001-mtk-mt76-mt7915-csi-implement-csi-support-REBASED.patch` | `mt76` package (`package/kernel/mt76/patches/`) |
| `mt76-vendor-csi-dump-bounds.patch` | `mt76-vendor` source (`feed/app/mt76-vendor/src/csi.c` in `mediatek/mtk-openwrt-feeds`) |
| `csidump-nl80211-attr-enum.patch` | MtkCSIdump `src/wifi_drv_api/mt76_api.cpp` (only if you use CSIdump) |
| `0001-wn586x3-reva-single-image-sysupgrade-and-generic-spi-nor.patch` | `target/linux/mediatek` (WN586X3 Rev A only) |

The route that produced the tested image (CODE-DERIVED from its build scripts)
used the stock OpenWrt **24.10.8** `mediatek/filogic` SDK and ImageBuilder in
an x86_64 Linux container:

1. **SDK.** Unpack `openwrt-sdk-24.10.8-mediatek-filogic`, then run
   `./scripts/feeds update -a && ./scripts/feeds install -a`.
2. **CSI driver.** Copy the rebased `1001-...` patch into
   `feeds/base/package/kernel/mt76/patches/`. Raise `PKG_RELEASE` in that
   package's `Makefile` (the tested build used `99`) so the ImageBuilder
   prefers your build over the CDN package of the same version.
3. **mt76-vendor.** Add MediaTek's `mt76-vendor` package from
   `mtk-openwrt-feeds` as `package/mt76-vendor`, with the bounds patch applied
   to `src/csi.c`.
4. **Configure and compile.** Select `CONFIG_TARGET_mediatek_filogic_DEVICE_wavlink_wl-wn586x3`,
   `kmod-mt7915e`, `mt76-vendor`, and `libnl-tiny`. Then run
   `make package/feeds/base/mt76/compile package/mt76-vendor/compile`.
5. **Image.** Copy the built `.ipk` files into the ImageBuilder's `packages/`
   and run:
   ```bash
   make image PROFILE=wavlink_wl-wn586x3 \
     PACKAGES="kmod-mt7915e kmod-mt7981-firmware mt7981-wo-firmware mt76-vendor libnl-tiny"
   ```
6. **Check the manifest.** The image's `.manifest` must list `kmod-mt7915e`
   at your raised release (for example `-r99`) and `mt76-vendor`. If it lists
   the CDN release, the image has no CSI.

**Rev A board patch.** An ImageBuilder cannot compile a device tree, and its
`make image` reuses the prebuilt DTB and FIT kernel. The tested Rev A build
compiled the patched DTB itself, rebuilt the FIT kernel around it, added the
patched `lib/upgrade/platform.sh` as a rootfs overlay, then checked that the
final sysupgrade image embedded that DTB. A full OpenWrt buildroot applies
`target/` patches directly and avoids those steps. That route was not tested
here.

Flash only a Rev A unit, keep the stock firmware backup, and follow the
OpenWrt wiki and forum thread for this board. Neither this guide nor the patch
set names a LAN, an account, or a password.

**Firmware blobs.** The CSI report is MCU event `0xc2`. The MT7981 firmware in
the tested image (`kmod-mt7981-firmware`, `mt7981-wo-firmware` from OpenWrt
24.10.8) already emits it, so no firmware blob swap was needed on the tested
board. Other firmware versions are untested.

## Arm CSI on the router

CSI follows traffic from **associated clients**. With nothing associated and
transmitting, there is no CSI. Run these on the router, as root, against the AP
interface (`phy0-ap0` was the 2.4 GHz AP on the tested board). The sequence
below is CODE-DERIVED from the capture scripts used on the tested board:

```sh
mt76-vendor phy0-ap0 set csi ctrl=0,0,0,0      # disarm and reset
mt76-vendor phy0-ap0 set csi interval=1000
mt76-vendor phy0-ap0 set csi ctrl=2,3,0,34     # frame-type filter, as used on
mt76-vendor phy0-ap0 set csi ctrl=2,9,1,0      # the tested board (data frames)
mt76-vendor phy0-ap0 set csi ctrl=1,0,0,0      # arm
```

Check that the driver is collecting:

```sh
cat /sys/kernel/debug/ieee80211/phy0/mt76/csi_stats   # data_cnt should climb
iw dev phy0-ap0 station dump | grep -E "Station|signal"
```

Pull a batch:

```sh
rm -f /tmp/csi.json                         # the tool APPENDS; remove first
mt76-vendor phy0-ap0 dump csi 64 /tmp/csi.json
```

`dump csi` needs the filename argument. Without it, the tool returns nothing.

## Run the bridge

```bash
cd v2
cargo build --release -p wifi-densepose-mtk-bridge
B=./target/release/wifi-densepose-mtk-bridge
```

**Dump path (recommended).** Copy `/tmp/csi.json` to the host, then replay it.
A file cannot prove it came off a radio, so `--replay` refuses to start until
you either attest the hardware or declare the file synthetic:

```bash
$B --replay csi.json --captured-on <model>/<firmware> \
   --node <node-name> --center-freq-khz <khz> --sink 127.0.0.1:5005
```

- `--node` is hashed into the 64-bit `device_id`. Give each router its own name.
- `--center-freq-khz` is the AP channel's centre frequency, for example
  `2437000` for 2.4 GHz channel 6.
- `--captured-on` takes `<model>/<firmware>`, for example
  `WN586X3-RevA/openwrt-24.10.8-csi`. It is logged at startup.

For a continuous feed, repeat dump, copy, and replay in a loop, one loop per
router. Keep a copy of each dump only if you need it, and never commit one
(see below).

**Streaming path (CSIdump).** Start `CSIdump phy0-ap0 <rate> 8888` on the
router. Then, on the host:

```bash
$B --listen 0.0.0.0:8888 --csidump <router-ip>:8888 --listen-allow <router-ip> \
   --node <node-name> --center-freq-khz <khz> --sink 127.0.0.1:5005 \
   --record captures/<node-name>.mtkcap
```

`--listen-allow` is mandatory. Datagrams from any other source are dropped and
counted. CSIdump disarms CSI when it receives SIGTERM. The dump path and
CSIdump cannot share the driver's ring, so run one or the other.

**No router.** The fabricated fixture exercises the whole host path. It must
be replayed with `--synthetic`, and the server labels it `mediatek:simulated`:

```bash
$B --replay crates/wifi-densepose-mtk-bridge/fixtures/FABRICATED-mt76-vendor-dump-mt7981-bw80.synthetic.json \
   --synthetic --node demo --sink 127.0.0.1:5005
```

The ADR-266 simulator is another synthetic source:

```bash
cargo run -p wifi-densepose-hardware --bin mediatek-csi-sim -- \
  --profile mt7981 --udp 127.0.0.1:5005 --frames 1000 --realtime
```

Full flag reference: [`v2/crates/wifi-densepose-mtk-bridge/README.md`](../v2/crates/wifi-densepose-mtk-bridge/README.md).

## Start the sensing server

```bash
cd v2
cargo run --release -p wifi-densepose-sensing-server -- --source mediatek \
  --activity-state <state-dir>/activity-floor.json
```

- `--source mediatek` binds the UDP receiver (loopback by default, ADR-296) and
  does **not** start the ESP32-shaped simulator. Before this source existed,
  `--source mediatek` bound nothing.
- If the bridge runs on another host, bind a routable address with
  `--udp-bind` and name the bridge host in `--udp-allow`.
- `--activity-state` is optional. It persists each device's learned
  quiet-room activity floor so that a restart does not relearn it.
- `--csi-ring <n>` sets frames retained per device for inspection (default
  256, about 0.5 MiB per 2x2 device at 64 subcarriers).

Endpoints:

| Route | Returns |
|---|---|
| `GET /api/v1/csi/mediatek/latest` | Last MTC1 snapshot, any device |
| `GET /api/v1/csi/mediatek/devices` | One snapshot per `device_id`, with `age_ms`. Devices silent for over 30 s are omitted. |
| `GET /api/v1/csi/mediatek/devices/:device_id/frames?n=&fields=amp,phase` | Newest `n` frames of per-chain, per-subcarrier amplitude and phase |
| `GET /api/v1/csi/mediatek/devices/:device_id/summary` | Per-chain amplitude mean/std per subcarrier, RSSI per Rx chain, frame rate, heuristic window stats |
| `GET /api/v1/csi/mediatek/activity?seconds=300` | About 1 Hz history of the relative activity index |
| `GET /api/v1/sensing/latest` | Room update. On this source, `classifier` is `mediatek-amplitude-heuristic-v0`. |

Check the label on every snapshot:

| `source` | Meaning |
|---|---|
| `mediatek:simulated` | `SYNTHETIC` flag set. Nothing downstream can clear it. |
| `mediatek:physical-unvalidated` | Real silicon, not calibrated. Everything this path produces today. |
| `mediatek` | Real and calibrated. Reserved. Nothing emits it yet. |

## What the data is, and is not

**It is** complex channel estimates per receive chain from a real MT7976C
radio, carried end to end with provenance intact.

**It is not:**

- **A sensor with its own clock.** The report rate is the clients' traffic
  rate. An idle phone gives a few frames per second, and a busy one gives
  hundreds. With no associated, transmitting client, there is no data. A
  stationary device that keeps transmitting to the AP (an "illuminator", for
  example an ESP32 sending a steady packet stream) gives a regular rate
  (CLAIMED: the 09-20 and 09-24 captures below include an illuminator's
  frames; transmitter identity was not re-checked in this analysis).
- **Calibrated.** `scale` is 1.0 in raw firmware units. Nothing has been
  checked against a reference. The ADR-266 gates are open.
- **Validated for presence, motion, or vital signs.** The room output on this
  source comes from `mediatek-amplitude-heuristic-v0` (presence and motion from
  mean amplitude) and `activity-index-v0` (fluctuation against a learned
  floor). Both are **CLAIMED**. Neither has been scored against labelled ground
  truth. `activity` blocks carry `is_occupancy_estimate: false`. These outputs
  do not feed the validated ESP32 vital-sign pipeline.
- **Coherent across routers.** Each radio has its own oscillator. Cross-chain
  phase is meaningful within one radio, not between two boxes.
- **Free of personal data.** A raw dump holds client transmitter addresses
  (MACs) and channel samples that encode movement. RuView policy (ADR-299)
  forbids committing captures. Keep them out of the repository, and run
  `scripts/csi-data-policy-check.sh --staged` before committing near capture
  directories.

## Evidence

| Claim | Tag | Basis |
|---|---|---|
| A real WL-WN586X3 frame decodes and reaches the server as `mediatek:physical-unvalidated` | MEASURED (path) | First record, 2026-09-19: legacy OFDM, 20 MHz, 64 subcarriers, one chain. Its shape is pinned by `wifi-densepose-mtk-bridge/tests/hardware_capture.rs`, which uses a fabricated record. The capture itself is not in the tree. |
| Cross-chain phase is steady frame to frame on one MT7976C radio, and raw per-chain phase is not | MEASURED (offline) | See below |
| `mediatek-amplitude-heuristic-v0`, `activity-index-v0` | CLAIMED | No labelled evaluation |
| ADR-266 simulator and bridge fixtures | SYNTHETIC | Labelled on the wire |

**Cross-chain phase stability.** Six local dump captures from two WN586X3 units,
2.4 GHz, 20 MHz HT frames carrying chains (tx0, rx0) and (tx0, rx1), adjacent
frames under 100 ms apart, 56 subcarriers (DC and guard bins excluded). The
two 2026-09-19 captures were analysed whole; the four 09-20 and 09-24 captures
by their first 20 MB each.

| Quantity, per adjacent-frame step | Well-sampled transmitters (6, at 825 to 14,104 frames each) |
|---|---|
| Raw per-chain phase `arg(H0[k])`, per bin, p50 | 1.557 to 1.601 rad, with 49.6 to 50.9 % of steps over π/2 |
| Cross-chain phase `arg(H0[k]·conj(H1[k]))`, per bin, p50 | 0.023 to 0.056 rad |
| Cross-chain phase summed coherently over bins, p50 / p90 / p99 | 0.014–0.039 / 0.205–0.358 / 0.437–0.820 rad |

Uniformly random phase gives a step p50 of π/2 = 1.571 rad, and half of all
steps exceed π/2. Raw per-chain phase is indistinguishable from that, as
expected with per-packet CFO and phase offset. In the cross-chain term those
offsets cancel, and the step drops to hundredths of a radian. Steps are
wrapped, so p99 can never exceed π, and is not evidence by itself.

Transmitters with few, irregular frames (229 to 595 frames) were much worse:
coherent-sum p50 0.177 to 0.456 rad, p99 1.857 to 2.849 rad. So the result
holds only with a steady frame rate.

This shows a **stable phase observable exists** within one radio. It does
**not** show that the observable tracks people, breathing, or anything else.

Reproduce it on your own capture (one `dump csi` array per line):

```bash
python3 scripts/mediatek-csi-phase-stability.py <capture.jsonl> [max_MB]
```

The script prints transmitters as `TX-A`, `TX-B`, and so on, never addresses.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| `csi_stats` `data_cnt` stays at 0 | No associated client transmitting, or CSI not armed | Associate a client and generate traffic, then re-run the arm sequence |
| `data_cnt` stopped after a client roamed or the filter changed | Changing the CSI filter stops capture | Re-run the arm sequence ending in `ctrl=1,0,0,0`. Re-arming periodically is a reasonable guard. |
| `mt76-vendor dump csi` prints nothing | No filename argument | Use `dump csi <n> <file>` |
| `mt76-vendor dump csi` segfaults | Unpatched tool: the driver can return n+1 records | Rebuild with `mt76-vendor-csi-dump-bounds.patch` |
| Bridge warns about repeated `(ta, ts, tx_idx, rx_idx)` chains | The dump file was not removed between dumps, and the tool appends | `rm -f` the file before every dump |
| `--replay` refuses to start | Provenance not declared | Add `--captured-on <model>/<firmware>`, or `--synthetic` for fabricated input |
| `--captured-on` is rejected | The input declares itself synthetic (sidecar, `.synthetic.` name, or capture header) | Expected. Synthetic is sticky. Use `--synthetic`. |
| `--listen` refuses to start | No `--listen-allow` | Name the router's address or subnet |
| `rejected_source` counts climb on `--listen` | The router's address is not in `--listen-allow` | Fix the allowlist |
| CSIdump runs but nothing arrives | Unpatched CSIdump uses an off-by-one netlink attribute enum against this driver | Rebuild with `csidump-nl80211-attr-enum.patch` |
| CSI stopped after CSIdump exited | CSIdump disarms CSI on SIGTERM | Re-run the arm sequence |
| `/api/v1/csi/mediatek/devices` is empty | Server not on `--source mediatek` (or `auto` without UDP), wrong `--sink` port, or a non-loopback bridge not in `--udp-allow` | Check `--source`, port 5005, `--udp-bind`, and `--udp-allow` |
| Two routers show as one device | Same `--node` on both bridges | Give each router its own `--node` (or `--device-id`) |
| A frame shows `ppdu_type: legacy` | Normal. Association and management traffic is pre-HT. | None. ADR-267 Amendment 1 carries these frames rather than dropping them. |
| Room status reads `unknown` for a device | The heuristic abstains below its confidence threshold | Expected with sparse frames. It is not an "absent" call. |
| Room reads `absent` with `confidence: 0.0` | Every device abstained, so there is no evidence either way | Treat confidence 0 as "no information", not as an empty room |
| Image flashes, but there is no `mt76-vendor` or CSI | The ImageBuilder used the CDN `kmod-mt7915e` | Check the manifest for your raised `PKG_RELEASE` |

## Reference

- [ADR-266: MediaTek Filogic CSI platform](adr/ADR-266-mediatek-filogic-csi-platform.md) (Amendment 1: vendor CSI on MT7981)
- [ADR-267: MediaTek MIMO CSI wire protocol](adr/ADR-267-mediatek-mimo-csi-wire-protocol.md) (Amendment 1: `Legacy` and `Unknown` PPDU types)
- [`firmware/openwrt-wn586x3/`](../firmware/openwrt-wn586x3/README.md): patches, licences, origins
- [`v2/crates/wifi-densepose-mtk-bridge/`](../v2/crates/wifi-densepose-mtk-bridge/README.md): bridge transports, provenance rules, frame mapping
- [MediaTek OpenWrt feeds](https://git01.mediatek.com/openwrt/feeds/mtk-openwrt-feeds/), [MtkCSIdump](https://github.com/MtkWifiRev/MtkCSIdump), [OpenWrt One](https://openwrt.org/toh/openwrt/one)
