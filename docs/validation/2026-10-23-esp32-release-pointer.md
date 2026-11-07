# ESP32 firmware release pointer, verified 2026-10-23

Status: a documentation drift was found and fixed, and the repository now
carries a machine check that keeps the documentation pointed at the release
the source tree builds. No firmware was changed and no sensing claim is made.

Subject: RuView#2092 (firmware release hygiene). Tree state: `version.txt` is
`0.8.15`; the published release `v0.8.15-esp32` (2026-10-22) carries the
three archives the release workflow builds.

## What was wrong

Three documents told a reader which firmware to download, and all three
pointed at the `v0.8.14-esp32` prerelease while the tree already built
0.8.15. The firmware guide contradicted itself: its "Firmware 0.8.15
prerelease" section described the current release, and the Quick Start two
sections below sent the reader to the 0.8.14 download page.

The `v2/` tree has its own copy of the same class of drift, which is what
makes the repository-level check worth having.

## Evidence

Measured on the upstream `main` tree at `fefb0f91`:

```
$ cat firmware/esp32-csi-node/version.txt
0.8.15

$ gh release view v0.8.15-esp32 --json isPrerelease,publishedAt,assets \
    --jq '"prerelease=\(.isPrerelease) published=\(.publishedAt)\n" + ([.assets[].name] | join("\n"))'
prerelease=true published=2026-10-22T15:23:21Z
esp32-csi-node-firmware-4mb.tar.gz
esp32-csi-node-firmware-8mb.tar.gz
esp32-csi-node-firmware-c6-4mb.tar.gz
SHA256SUMS.txt

$ grep -rn 'v0\.8\.1[0-9]-esp32' -- firmware/esp32-csi-node/README.md \
    firmware/esp32-csi-node/release_bins/README.md docs/user-guide.md
firmware/esp32-csi-node/README.md:139:  [v0.8.14 ESP32 prerelease page](.../v0.8.14-esp32).
firmware/esp32-csi-node/release_bins/README.md:8:  [ESP32 0.8.14 prerelease page](.../v0.8.14-esp32)
firmware/esp32-csi-node/release_bins/README.md:11: ... The 0.8.14 prerelease requires
docs/user-guide.md:1929: | [v0.8.14](.../v0.8.14-esp32) | **Prerelease:** ...
```

The release notes for the version the tree builds were already in the tree
(`docs/releases/v0.8.15-esp32.md`), and the `0.8.15` change list was already
in the firmware guide, so the pointer was the only thing left behind.

## What was changed

- `firmware/esp32-csi-node/README.md`, `firmware/esp32-csi-node/release_bins/README.md`
  and `docs/user-guide.md` now name the `v0.8.15-esp32` prerelease, and the
  user-guide paragraph describes 0.8.15's own changes instead of 0.8.14's.
- The historical 0.8.14 section in the firmware guide is left in place: it
  records what that release changed, and it is not a download pointer.
- `scripts/esp32_release_pointer_contract.py` with
  `scripts/verify-esp32-release-pointer.py` checks that the newest ESP32
  release tag named in those documents is the version in `version.txt`, that
  the release notes for that version exist, and that the archives named are
  the ones the release workflow publishes.
- `.github/workflows/esp32-release-pointer-verify.yml` runs the check in CI,
  and compares the published release's assets with the documented ones when
  the release is already published.

## Reproducer

```
$ python scripts/verify-esp32-release-pointer.py
PASS: the ESP32 firmware documentation points at the release this tree builds (0.8.15), in 3 document(s).

$ python -m unittest discover -s scripts/tests -p test_esp32_release_pointer_contract.py
Ran 12 tests in 0.051s

OK
```

The same command fails on the tree as it was, with six findings:

```
FAIL: 6 problem(s) in the ESP32 firmware release pointer
  - firmware/esp32-csi-node/README.md names v0.8.15-esp32: observed False, required True
  - firmware/esp32-csi-node/release_bins/README.md names v0.8.15-esp32: observed False, required True
  - docs/user-guide.md names v0.8.15-esp32: observed False, required True
  - firmware/esp32-csi-node/README.md newest release tag: observed (0, 8, 14), required (0, 8, 15)
  - firmware/esp32-csi-node/release_bins/README.md newest release tag: observed (0, 8, 14), required (0, 8, 15)
  - docs/user-guide.md newest release tag: observed (0, 8, 14), required (0, 8, 15)
```

## What this does not settle

The issue also asks for a stable release to be published, or for a release
cadence to be stated. Both are maintainer decisions: publishing a release
runs from a tag through the release workflow, and a cadence is a policy the
project has to choose. The last stable ESP32 firmware is still `v0.8.8`
(2026-08-31), and 0.8.9 through 0.8.15 were published as prereleases only.
The documentation now names the current prerelease explicitly and keeps
pointing at `v0.8.8` as the previous stable.
