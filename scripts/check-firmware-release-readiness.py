#!/usr/bin/env python3
"""ESP32 firmware stable-release readiness gate (RuView#2092).

The ESP32 firmware release has two channels, and they are not the same thing:

* **prerelease** -- the channel the CI workflow publishes. It is built and
  host-tested by CI, and it carries no physical-board qualification and no
  sensing-accuracy claim;
* **stable** -- a release that a person publishes after the firmware has
  passed a hardware qualification on real boards.

The stable channel is the one users are told to flash, so its preconditions
have to be checkable rather than remembered. This script is that check. It
answers one question: *can the candidate version be published as a stable
ESP32 firmware release, on the evidence that is in this tree?*

The preconditions it enforces:

* the firmware version in the tree is the candidate version, because the
  release workflow's version guard refuses a tag that does not match
  ``firmware/esp32-csi-node/version.txt``;
* a release note exists for the candidate, because a stable release is a
  curated note, not a build log;
* a hardware qualification record for the candidate exists in
  ``docs/validation/`` and reports a pass, because a stable release without
  measured hardware evidence is not a stable release;
* the release workflow accepts the candidate's tag form, because otherwise
  pushing the tag publishes nothing.

Findings that do not block the release are reported as advisories.

This is a stdlib-only script -- no dependencies, runs anywhere Python 3.8+ does.

Usage::

    python scripts/check-firmware-release-readiness.py --version 0.8.16
    python scripts/check-firmware-release-readiness.py --version 0.8.16 --json

Exit codes: 0 = ready, 1 = not ready, 2 = bad usage.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Optional

sys.path.insert(0, str(Path(__file__).resolve().parent))

from esp32_firmware_release_readiness import (  # noqa: E402
    VERSION_RE,
    evaluate_readiness,
    verify_records,
)

# Best-effort UTF-8 stdout (Windows consoles default to cp1252); harmless on
# Linux/CI where it's already UTF-8.
try:  # pragma: no cover - environment-dependent
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Check whether a version can be published as a stable "
                    "ESP32 firmware release.")
    parser.add_argument("--version",
                        help="the candidate firmware version, for example 0.8.16")
    parser.add_argument("--verify-records", action="store_true",
                        help="check that the declared qualification records are "
                             "self-consistent, and exit")
    parser.add_argument("--json", action="store_true",
                        help="print the machine-readable result")
    args = parser.parse_args(argv)

    if args.verify_records:
        result = verify_records()
        if args.json:
            print(json.dumps(result, indent=2))
        else:
            print(f"qualification records: {result['records']} declared, "
                  f"{'consistent' if result['ok'] else 'INCONSISTENT'}")
            for item in result["findings"]:
                mark = "PASS" if item["pass"] else "FAIL"
                print(f"  [{mark}] {item['check']}: {item['detail']}")
        return 0 if result["ok"] else 1

    if not args.version:
        print("error: --version is required unless --verify-records is used",
              file=sys.stderr)
        return 2

    candidate = args.version.strip()
    if candidate.startswith("v"):
        candidate = candidate[1:]
    if not VERSION_RE.match(f"v{candidate}"):
        print(f"error: --version must look like 0.8.16, not {args.version!r}",
              file=sys.stderr)
        return 2

    result = evaluate_readiness(candidate)
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"stable release readiness for v{candidate}-esp32: "
              f"{'READY' if result['ready'] else 'NOT READY'}")
        for item in result["findings"]:
            if item["severity"] == "advisory":
                mark = "NOTE"
            else:
                mark = "PASS" if item["pass"] else "FAIL"
            print(f"  [{mark}] {item['check']}: {item['detail']}")
    return 0 if result["ready"] else 1


if __name__ == "__main__":
    sys.exit(main())
