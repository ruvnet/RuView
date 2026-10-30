#!/usr/bin/env python3
"""Verify the Windows WiFi quick start against the repository it documents.

    python scripts/verify-windows-wifi-quickstart.py
"""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))

from windows_wifi_quickstart_contract import GUIDE, verify

ROOT = Path(__file__).resolve().parents[1]


def main():
    failures = verify(ROOT)
    if failures:
        print("FAIL: %d problem(s) in %s" % (len(failures), GUIDE))
        for name, observed, required in failures:
            print("  - %s: observed %r, required %r" % (name, observed, required))
        return 1
    print(
        "PASS: the Windows WiFi quick start matches the import style, the "
        "package layout, the proof's verdict, the collector's refusal and the "
        "captured evidence."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
