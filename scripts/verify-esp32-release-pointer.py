#!/usr/bin/env python3
"""Verify the ESP32 firmware release pointer against the repository.

    python scripts/verify-esp32-release-pointer.py
    python scripts/verify-esp32-release-pointer.py --print-tag
    python scripts/verify-esp32-release-pointer.py --print-archives
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from esp32_release_pointer_contract import (
    POINTER_DOCS,
    current_tag,
    current_version,
    release_archives,
    verify,
)

ROOT = Path(__file__).resolve().parents[1]


def main(argv):
    if "--print-tag" in argv:
        print(current_tag(ROOT))
        return 0
    if "--print-archives" in argv:
        for archive in release_archives(ROOT):
            print(archive)
        return 0

    failures = verify(ROOT)
    if failures:
        print("FAIL: %d problem(s) in the ESP32 firmware release pointer" % len(failures))
        for name, observed, required in failures:
            print("  - %s: observed %r, required %r" % (name, observed, required))
        return 1
    print(
        "PASS: the ESP32 firmware documentation points at the release this "
        "tree builds (%s), in %d document(s)." % (
            current_version(ROOT),
            len(POINTER_DOCS),
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
