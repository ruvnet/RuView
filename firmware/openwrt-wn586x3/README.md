# WL-WN586X3 CSI patches

OpenWrt patches that made a Wavlink WL-WN586X3 Rev A emit CSI into
`wifi-densepose-mtk-bridge`. Chipset on the board that was flashed: MT7981BA,
MT7976CN, MT7531AE. Image base: OpenWrt 24.10.8. This directory is the patch
set. It is not a firmware image, a sysupgrade binary, or a capture.

Developed in whitsentry, a RuView-based project. The user guide, covering
the build, arming CSI, the bridge, the server, and limitations, is
[`docs/mediatek-router-csi.md`](../../docs/mediatek-router-csi.md).

Provenance of frames from this path is `mediatek:physical-unvalidated` until
the ADR-266 gates (calibration, sequence, timestamp, repeatability) pass on
this model. Two of these radios are two receivers. They are not one phased array.

## Files

| Patch | What it is | License of the base |
|---|---|---|
| `patches/1001-mtk-mt76-mt7915-csi-implement-csi-support-REBASED.patch` | MediaTek's 2022 mt7915 CSI patch, rebased onto `openwrt/mt76` `eb567bc7` (the revision OpenWrt 24.10.8 pins). Two dead-code warnings were removed so `-Werror` builds. | BSD-3-Clause-Clear, MediaTek |
| `patches/0001-wn586x3-reva-single-image-sysupgrade-and-generic-spi-nor.patch` | Board support ported from [dadogroove/openwrt `wl-wn586x3`](https://github.com/dadogroove/openwrt/tree/wl-wn586x3) @ `b080a655` onto the 24.10.8 tag. Single-image sysupgrade, generic SPI-NOR (the Boya flash path is a deletion of quad-SPI width, not a new chip driver), factory MAC offsets. | OpenWrt target patches, GPL-2.0 |
| `patches/mt76-vendor-csi-dump-bounds.patch` | `mt76-vendor dump csi` wrote past its buffer once a dump held more records than requested. The driver seeds its counter and then decrements, so a request for N can return N+1. Also bounds `snprintf` and checks attribute presence. Base: `mediatek/mtk-openwrt-feeds` @ `a15454c8`, `feed/app/mt76-vendor/src/csi.c`. | GPL-2.0, MediaTek |
| `patches/csidump-nl80211-attr-enum.patch` | [MtkCSIdump](https://github.com/MtkWifiRev/MtkCSIdump) @ `276b4b08` was one attribute off from this driver (`STA_INTERVAL`, `DATA_NUM`). The dump callback returned nothing and the ring sat full. | Apache-2.0 (MtkCSIdump `LICENSE.txt`) |

Do not copy `mt76` itself, the imagebuilder download cache, `out/`, stock
backups, or anything under a `captures/` directory into this repo.

### Licences

These files are patches against third-party code. Each one keeps the licence
of the code it modifies, as listed in the table above. They are not covered
by the repository's MIT/Apache-2.0 licence. In particular, the `0001-` board
patch and `mt76-vendor-csi-dump-bounds.patch` modify GPL-2.0 sources, and the
`1001-` CSI patch is MediaTek's BSD-3-Clause-Clear work with its original
author line kept. Nothing here is a firmware blob, a private header, or SDK
code (ADR-266 decision 5).

## What the radio actually did

CSI follows client traffic. An idle associated station is a few frames per
second. The vendor dump tool opens its output with `fopen(path, "a+")`, so a
loop that does not remove the file replays every earlier batch. Changing the
CSI filter stops capture until `ctrl=1,0,0,0` is sent again. `CSIdump` disarms
CSI on SIGTERM; an explicit disarm is the right stop, and `kill -9` is how the
lab kept the radio armed, which is a bug in the tool rather than a procedure
to keep.

`mt76-vendor` JSON is the path that keeps RSSI, SNR, transmit address, bandwidth,
PPDU mode, and per-chain indices. The CSIdump UDP datagram has none of those
and sends antennas one after another, so the bridge emits one 1x1 frame per
datagram. Prefer the dump path when the chain layout matters. Neither stock
tool prints `pkt_sn`, which the firmware event already carries, so the bridge
cannot see loss on an unpatched dumper.

Each radio is two chains. The four external antennas are a 2.4 GHz pair and a
5 GHz pair. Live CSI in the lab was 2x2 on 2.4 GHz. The 5 GHz radio was idle
for sensing.

## Build outline

Apply the rebased CSI patch to the `mt76` tree OpenWrt 24.10.8 builds, and the
board patch to `target/linux/mediatek`. Point the image at the patched
`mt76-vendor` and, if you use it, the patched CSIdump. Flash only a Rev A unit,
and only an image you built and hashed yourself. This README does not name a
LAN, an account, or a password.

Host side, after the radio is emitting:

```bash
cd v2
cargo build -p wifi-densepose-mtk-bridge
./target/debug/wifi-densepose-mtk-bridge --help
```

Replay stays on loopback unless you choose otherwise. `--listen` refuses to
start without `--listen-allow`. `--replay` of a file refuses to start until
you pass `--synthetic` or `--captured-on <model>/<firmware>`. A filename
containing `.synthetic.` cannot be attested as hardware.
