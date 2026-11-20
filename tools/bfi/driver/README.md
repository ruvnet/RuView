# MT7927 monitor band/context trial patch

Status (2026-09-29): **MEASURED build and independently validated BFI reception.**
Target beacons were received repeatedly with the two upstream fixes below. A later
configuration, including the experimental RX-filter patch below, captured 21
HE feedback reports over 45.0334 seconds. Every report matched tshark 4.2.2;
see [the measured evidence](../VALIDATION.md) for the current acceptance result.
Received signal conditions varied, so these observations do not establish that
the experimental patch caused the feedback reception. A repeat captured zero
reports; sustained sampling remains unproven.

## Scope and provenance

`mt7927-monitor-band-context.patch` targets the source layout of
`mediatek-mt7927/2.10`. It changes two files and preserves existing behavior for
other chipsets:

- `mt76/mt7925/mcu.c`: choose the MT7927 sniffer band from the channel context,
  falling back to the PHY channel; apply the same selection to sniffer channel
  configuration. Adapted from upstream
  [7274d6cc43bc87d90f96534777ff3fc1f0b3a639](https://github.com/openwrt/mt76/commit/7274d6cc43bc87d90f96534777ff3fc1f0b3a639).
  This package calls its existing helper `mt7925_band_idx(dev, band)`, rather
  than the upstream `mt7927_band_idx(band)`.
- `mt76/mt792x_core.c`: use `NO_VIRTUAL_MONITOR` for MT7927, so the driver can
  receive real monitor interfaces with their channel contexts. Adapted from
  upstream
  [aa5a2b4fee38e32e3e76f35e8c2ddb17e2a535bf](https://github.com/openwrt/mt76/commit/aa5a2b4fee38e32e3e76f35e8c2ddb17e2a535bf).

Both commits are by Sean Wang and were committed to mt76 on 2026-07-21. The
installed source used band index 0 for both sniffer commands, although its
MT7927 helper maps 5/6 GHz to band 1. These changes address that mismatch and
monitor interface handling; they add no BFI decoder.

The upstream files carry `Copyright (C) 2023 MediaTek Inc.` and SPDX license
`BSD-3-Clause-Clear`. Both patches in this directory are distributed under the
bundled [BSD-3-Clause-Clear license](LICENSE), including the MediaTek notice,
conditions and disclaimer. Preserve these notices when distributing the patches
or patched source. The repository's root MIT license does not replace this
upstream license. The [SPDX reference](https://spdx.org/licenses/BSD-3-Clause-Clear.html)
provides the standard license text.

Patch SHA-256:
`13f6603e208ca169a84002b5aed09cb91abd03393d4d310fe3008bc02c7b2968`.

## Reproduce the build

Measured environment: Linux `6.17.0-20-generic`, matching Ubuntu kernel
headers, GCC 13.3.0, and `mediatek-mt7927/2.10` source. Secure Boot was disabled
and `CONFIG_MODULE_SIG_FORCE` was unset. These facts apply to that trial host;
check them independently on another machine.

Run from the repository root on Linux. This builds in a private temporary copy
and leaves installed modules and DKMS registration unchanged:

```bash
set -euo pipefail
patch_file=$(realpath tools/bfi/driver/mt7927-monitor-band-context.patch)
source_dir=/usr/src/mediatek-mt7927-2.10
kernel_release=$(uname -r)
test -d "/lib/modules/$kernel_release/build"
test -f "$source_dir/mt76/Kbuild"
trial_dir=$(mktemp -d "${TMPDIR:-/var/tmp}/mt7927-monitor.XXXXXXXX")
mkdir -m 700 "$trial_dir/source"
cp -R -- "$source_dir/." "$trial_dir/source/"
patch --batch --fuzz=0 --dry-run -d "$trial_dir/source" -p1 < "$patch_file"
patch --batch --fuzz=0 -d "$trial_dir/source" -p1 < "$patch_file"
timeout --signal=TERM --kill-after=5s 300s \
  make -j4 -C "/lib/modules/$kernel_release/build" \
  M="$trial_dir/source/mt76" modules 2>&1 | tee "$trial_dir/build.log"
```

The measured build exited 0. `vermagic` matched the running kernel. BTF
generation was skipped because `vmlinux` was unavailable; a compiler identity
warning was also emitted. Neither prevented module linking. The build produced
the Wi-Fi module set without building or replacing Bluetooth modules.

If patch context differs or a build fails, inspect the cause before retrying.
Do not apply this package-specific patch to a newer driver that already carries
these fixes.

## Experimental aggregate RX-filter band mirror

`mt7927-rxfilter-band-mirror-experimental.patch` is a separate local experiment,
applied **after** `mt7927-monitor-band-context.patch`. It is not an upstream fix.
It changes only `mt76/mt7925/mcu.c`, retaining the source's existing notices and
license. Source inspection of the same driver package found:

- `mt76/mac80211.c:688` initializes the primary PHY band index to zero.
- `mt76/mt7925/main.c:2153` changes the interface's band index during channel
  assignment; it does not change the PHY index.
- `mt76/mt7925/mcu.c:3911` selects that PHY index for RX-filter commands, whereas
  the earlier sniffer patch selects band one for a 5 GHz channel.
- `mt76/mt7925/main.c:819` assembles aggregate filter flags. Its enable bit is
  present for normal operation as well as monitor operation, so leaving monitor
  mode still supplies a nonzero aggregate update.

These locations refer to the source after the two upstream changes, before the
experimental patch. Current upstream retained the same RX-filter selection when
reviewed. The mismatch suggests a firmware-filter hypothesis; the source alone
does not prove the command's effects on received packets.

The experiment first sends the existing RX-filter command. On success, for
MT7927 and a nonzero aggregate `fif` value, it sends the same command to band one.
It preserves bitmap-only updates (`fif == 0`), older chipsets, and the global PHY
index. It returns an MCU failure, although the existing `configure_filter`
callback does not propagate that return value. A successful build or module load
therefore cannot establish that firmware applied the second command.

Patch SHA-256:
`ce2a64a0a8c14fcff2fa930f7be1f26f91582ab4614c7cd962ca8ddc4903907e`.

Measured build: the same kernel and compiler described above; exit status zero.
The changed `mt7925-common.ko` had source version
`4D09C449A14BBDD01C60F14` and SHA-256
`a3e7fccd745b05fd38b90000376f846de578b3b7bf2f6fd69be5c5f375ffc7c6`.
Other module hashes matched the earlier trial. These values identify the measured
build; different build environments can produce different module hashes.

To reproduce, complete the earlier private-copy build, then apply this additional
patch to that copy. Do not patch installed source or overwrite its modules:

```bash
experimental_patch=$(realpath tools/bfi/driver/mt7927-rxfilter-band-mirror-experimental.patch)
sha256sum "$experimental_patch"
patch --batch --fuzz=0 --dry-run -d "$trial_dir/source" -p1 < "$experimental_patch"
patch --batch --fuzz=0 -d "$trial_dir/source" -p1 < "$experimental_patch"
timeout --signal=TERM --kill-after=5s 300s \
  make -j4 -C "/lib/modules/$kernel_release/build" \
  M="$trial_dir/source/mt76" modules 2>&1 | tee "$trial_dir/build-rxfilter.log"
modinfo -F srcversion "$trial_dir/source/mt76/mt7925/mt7925-common.ko"
sha256sum "$trial_dir/source/mt76/mt7925/mt7925-common.ko"
```

For a causal comparison, retain two separate source/build directories, one for
each variant. Keep AP settings, client, traffic, placement, channel, capture flags
and duration constant; compare complete feedback counts and independently decoded
angles. Existing observations had varying signal conditions and do not meet that condition.
The in-memory trial and original-driver rollback below apply to either variant.

## Temporary runtime trial and rollback

One coordinator must own all interface and module changes. Before a trial:

1. Verify independent wired management, record the current Wi-Fi connection
   UUID/interface state, and prepare restoration before disconnecting Wi-Fi.
2. Record `modinfo -n`, `srcversion`, `vermagic`, and SHA-256 for the installed
   module chain. Keep copies of those files outside the repository. Record the
   same metadata for the trial `.ko` files.
3. Check module users and signing/lockdown policy. Never force module removal
   or weaken signing policy. Stop if another active device needs a dependency.

For an **in-memory trial**, leave `/lib/modules` and DKMS untouched. After the
coordinator releases Wi-Fi, unload the old chain without force:

```bash
sudo modprobe -r mt7925e mt7925_common mt792x_lib mt76_connac_lib mt76
```

Load the trial files in dependency order with `insmod`: `mt76.ko`,
`mt76-connac-lib.ko`, `mt792x-lib.ko`, `mt7925/mt7925-common.ko`, then
`mt7925/mt7925e.ko`, all beneath `$trial_dir/source/mt76/`. If any load fails,
stop and roll back before attempting capture. Compare the **loaded**
`/sys/module/*/srcversion` values with the trial metadata; `modinfo` by module
name alone still describes the installed files.

Rollback after stopping the coordinator-owned capture and releasing Wi-Fi:

```bash
sudo modprobe -r mt7925e mt7925_common mt792x_lib mt76_connac_lib mt76
sudo modprobe mt7925e
```

Because installed files were unchanged, `modprobe` loads the original driver.
Restore the saved NetworkManager ownership and exact connection UUID, verify
association and the wired route, and compare loaded source versions with the
original metadata. Resolve a renamed interface from the saved device identity.
Do not delete the trial directory or backups until restoration is verified.

## Persistent DKMS installation

An in-memory trial does not survive module reload or reboot. Persistent DKMS
packaging is a separate deployment: assign a distinct version, retain the
original package and firmware, account for module signing, and validate rollback
before installation. A newer DKMS package may replace both driver and firmware;
that is a broader change than this patch. No persistent installation is part of
this artifact.

Require reproducible own-network reception, complete independently checked BFI
reports, and stated report rates before calling the capture path validated.
Keep packet captures, network identifiers, machine-specific manifests, and
compiled modules outside the repository.
