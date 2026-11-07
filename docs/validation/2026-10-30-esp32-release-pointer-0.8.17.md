# ESP32 release pointer: the gate caught the 0.8.17 drift (2026-10-30)

## What was found

The release pointer contract in
`scripts/esp32_release_pointer_contract.py` was merged into this tree and
applied to the current `upstream/main`. It failed with nine findings, because
the tree had moved to firmware 0.8.17 while the documentation still described
0.8.16:

```
FAIL: 9 problem(s) in the ESP32 firmware release pointer
  - release notes for 0.8.17: observed False, required True
  - firmware/esp32-csi-node/README.md names v0.8.17-esp32: observed False, required True
  - firmware/esp32-csi-node/release_bins/README.md names v0.8.17-esp32: observed False, required True
  - docs/user-guide.md names v0.8.17-esp32: observed False, required True
  - firmware/esp32-csi-node/README.md newest release tag: observed (0, 8, 16), required (0, 8, 17)
  - firmware/esp32-csi-node/release_bins/README.md newest release tag: observed (0, 8, 15), required (0, 8, 17)
  - docs/user-guide.md newest release tag: observed (0, 8, 16), required (0, 8, 17)
  - firmware/esp32-csi-node/README.md archive names: observed ('esp32-csi-node-firmware-4mb.tar.gz', 'esp32-csi-node-firmware-8mb.tar.gz', 'esp32-csi-node-firmware-c6-4mb.tar.gz'), required ('esp32-csi-node-firmware-4mb.tar.gz', 'esp32-csi-node-firmware-8mb.tar.gz', 'esp32-csi-node-firmware-c5-4mb.tar.gz', 'esp32-csi-node-firmware-c6-4mb.tar.gz')
  - docs/user-guide.md archive names: observed ('esp32-csi-node-firmware-4mb.tar.gz', 'esp32-csi-node-firmware-8mb.tar.gz', 'esp32-csi-node-firmware-c6-4mb.tar.gz'), required ('esp32-csi-node-firmware-4mb.tar.gz', 'esp32-csi-node-firmware-8mb.tar.gz', 'esp32-csi-node-firmware-c5-4mb.tar.gz', 'esp32-csi-node-firmware-c6-4mb.tar.gz')
```

The tree builds `0.8.17` (`firmware/esp32-csi-node/version.txt`), the published
release is `v0.8.17-esp32`, and the 0.8.17 release adds the experimental C5
bundle - so the workflow publishes four archives while the two documents that
name archives still listed three. The gate found the same class of drift the
earlier record describes for 0.8.14 -> 0.8.15, one release later.

## What was changed

- `docs/user-guide.md`, `firmware/esp32-csi-node/README.md` and
  `firmware/esp32-csi-node/release_bins/README.md` now name `v0.8.17-esp32`,
  and the two archive tables list `esp32-csi-node-firmware-c5-4mb.tar.gz`.
- `docs/releases/v0.8.17-esp32.md` records the published 0.8.17 release: the
  changes, the measured evidence and the limits, transcribed from the
  published release. The C5 bundle remains experimental and the cadence target
  was not met; neither is claimed otherwise.
- `scripts/tests/test_esp32_release_pointer_contract.py` no longer pins a
  firmware version. It reads the current version from the tree and derives the
  stale tag and the bumped version from it, so a new release does not
  invalidate the suite. That is the failure this file had when `0.8.15` was
  written into it.

The earlier record, `2026-10-23-esp32-release-pointer.md`, documents the
2026-10-23 state and is left as it is: it records what was observed then.

## Verification

```
$ python -m unittest discover -s scripts/tests -p 'test_esp32_release_pointer_contract.py'
Ran 12 tests in 0.053s
OK

$ python scripts/verify-esp32-release-pointer.py
PASS: the ESP32 firmware documentation points at the release this tree builds (0.8.17), in 3 document(s).

$ python scripts/check_fix_markers.py
All 27 fix markers present.
```

MEASURED on the merged tree of this branch, on Windows, with Python 3.15.
