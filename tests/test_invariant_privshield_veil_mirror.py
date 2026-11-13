"""Invariant: firmware/privshield/esp32 and wifi-veil/firmware/esp32 must stay in sync.

These two trees are intentionally mirrored copies of the same ESP32 firmware
(history shows drift already happened twice: the esp_mqtt->mqtt CMake bug
e692427d and the WiFi-provisioning format fix both had to be applied to both
copies). If a fix lands in one tree only, this test fails with the exact copy
command to restore parity — the same gate style as test_invariant_Cargo.py.
"""

import os

import pytest

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CANONICAL = os.path.join(REPO_ROOT, "firmware", "privshield", "esp32")
MIRROR = os.path.join(REPO_ROOT, "wifi-veil", "firmware", "esp32")

pytestmark = pytest.mark.invariants


def _tree_files(base: str):
    """Return {relative_path: file_bytes} for every file under base."""
    out = {}
    for dirpath, dirnames, filenames in os.walk(base):
        dirnames[:] = [d for d in dirnames if d not in ("build", ".git")]
        for name in filenames:
            if name.endswith(".pyc"):
                continue
            full = os.path.join(dirpath, name)
            rel = os.path.relpath(full, base).replace(os.sep, "/")
            with open(full, "rb") as f:
                out[rel] = f.read()
    return out


def test_privshield_and_wifi_veil_firmware_in_sync():
    if not (os.path.isdir(CANONICAL) and os.path.isdir(MIRROR)):
        pytest.skip("one of the mirrored firmware trees is absent in this checkout")
    canon = _tree_files(CANONICAL)
    mirror = _tree_files(MIRROR)
    assert canon, "canonical tree firmware/privshield/esp32 is empty"

    only_canonical = sorted(set(canon) - set(mirror))
    only_mirror = sorted(set(mirror) - set(canon))
    differing = sorted(p for p in set(canon) & set(mirror) if canon[p] != mirror[p])

    drift = []
    if only_canonical:
        drift.append(f"missing in wifi-veil copy: {only_canonical}")
    if only_mirror:
        drift.append(f"missing in privshield copy: {only_mirror}")
    if differing:
        drift.append(f"content differs: {differing}")
    assert not drift, (
        "Mirrored firmware trees drifted. Apply the fix to both copies, e.g.:\n"
        "  cp -r firmware/privshield/esp32/. wifi-veil/firmware/esp32/\n"
        "Problems: " + "; ".join(drift)
    )
