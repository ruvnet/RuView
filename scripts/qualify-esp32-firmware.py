#!/usr/bin/env python3
"""ESP32 firmware release qualification harness (RuView#2092).

Runs the five minute acceptance procedure that a stable ESP32 firmware
release has to pass, and turns it into a machine-checked verdict instead of
a recollection.

The procedure is the one written down in
``docs/validation/2026-08-31-esp32-s3-rate-aware-transport.md``: after any
S3 timing, WiFi, transport, or task scheduling change, observe the node for
five minutes and pass only when the raw callback yield stays at or above
20 pps, the node frame coverage stays at or above 99 percent, and the device
and the server report zero send failures, offline frames, parser errors,
WebSocket errors, watchdogs, panics, and unexpected resets.

The harness has two sides:

* **server side** -- it opens the sensing server's ``/ws/sensing`` WebSocket
  and counts the frames, the frames that carry the qualified node, the
  frames that do not (the source-offline frames), the parse errors, and the
  gap between consecutive frames;
* **device side** -- it parses the node's 1 Hz controller log and counts the
  raw CSI yield samples, the mesh send failures, the ENOMEM back-offs, the
  uplink watchdogs, and the restarts.

The device side is read from a console capture, because the node's console is
captured with whatever serial tool the operator already has. The capture is
the only place the device counters exist in real time: the authoritative copy
lives in the node's 60 s flash log, which needs a flash dump to read.

Only aggregate counters are read. The harness never touches raw CSI,
amplitudes, or per-person data (ADR-299).

This is a stdlib-only script -- no dependencies, runs anywhere Python 3.8+ does.

Usage::

    python scripts/qualify-esp32-firmware.py --node 1 --seconds 300 \
        --console-log capture.log --environment-note "board, AP, room" \
        --out qualification.json

    python scripts/qualify-esp32-firmware.py --print-md --out qualification.json

Exit codes: 0 = acceptance passed, 1 = acceptance failed, 2 = bad usage or
an unreachable server.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import List, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent))

from esp32_firmware_qualification import (  # noqa: E402
    DEFAULT_MIN_COVERAGE_PERCENT,
    DEFAULT_MIN_YIELD_PPS,
    DEFAULT_SECONDS,
    QualificationError,
    build_result,
    observe_server,
    parse_console,
    render_markdown,
)

# Best-effort UTF-8 stdout (Windows consoles default to cp1252); harmless on
# Linux/CI where it's already UTF-8.
try:  # pragma: no cover - environment-dependent
    sys.stdout.reconfigure(encoding="utf-8", errors="replace")
except Exception:
    pass


def load_console_log(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        raise QualificationError(f"cannot read the console capture {path}: {exc}") from exc


def websocket_url(base: str) -> str:
    """Turn a server base URL into the sensing WebSocket URL."""
    url = base.rstrip("/")
    if url.startswith("https://"):
        url = "wss://" + url[len("https://"):]
    elif url.startswith("http://"):
        url = "ws://" + url[len("http://"):]
    return url + "/ws/sensing"


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(
        description="Run the ESP32 firmware release acceptance procedure.")
    parser.add_argument("--server", default="http://127.0.0.1:8080",
                        help="sensing server base URL (default: %(default)s)")
    parser.add_argument("--node", type=int, default=1,
                        help="logical node id to qualify (default: %(default)s)")
    parser.add_argument("--seconds", type=int, default=DEFAULT_SECONDS,
                        help="observation window in seconds (default: %(default)s)")
    parser.add_argument("--console-log", type=Path,
                        help="node console capture to read the device counters from")
    parser.add_argument("--environment-note", default="",
                        help="board, access point, room, and traffic source of the run")
    parser.add_argument("--firmware-version", default="",
                        help="the firmware version running on the qualified node")
    parser.add_argument("--replay", type=Path,
                        help="re-render a stored result instead of measuring again")
    parser.add_argument("--out", type=Path, help="write the JSON result here")
    parser.add_argument("--print-md", action="store_true",
                        help="print the Markdown report")
    parser.add_argument("--min-yield-pps", type=float, default=DEFAULT_MIN_YIELD_PPS,
                        help="minimum raw CSI yield in pps (default: %(default)s)")
    parser.add_argument("--min-coverage-percent", type=float,
                        default=DEFAULT_MIN_COVERAGE_PERCENT,
                        help="minimum node frame coverage (default: %(default)s)")
    args = parser.parse_args(argv)

    if args.seconds <= 0:
        print("error: --seconds must be positive", file=sys.stderr)
        return 2
    if not args.environment_note.strip():
        print("error: --environment-note is required; a verdict without the "
              "environment it was measured in is not evidence", file=sys.stderr)
        return 2
    if not args.firmware_version.strip():
        print("error: --firmware-version is required; a qualification record "
              "has to name the firmware version it qualifies", file=sys.stderr)
        return 2

    try:
        if args.replay:
            result = json.loads(args.replay.read_text(encoding="utf-8"))
            result["firmware_version"] = args.firmware_version.strip()
            result["environment_note"] = args.environment_note.strip()
        else:
            server = observe_server(websocket_url(args.server), args.node, args.seconds)
            device = None
            if args.console_log:
                device = parse_console(load_console_log(args.console_log))
            result = build_result(args.node, args.environment_note.strip(), server,
                                  device, args.min_yield_pps, args.min_coverage_percent,
                                  args.firmware_version.strip())
    except (QualificationError, OSError, ValueError) as exc:
        print(f"error: {exc}", file=sys.stderr)
        return 2
    verdict = result["verdict"]

    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    if args.print_md:
        print(render_markdown(result))
    else:
        print(f"qualification: {'PASS' if verdict['pass'] else 'FAIL'}")
        for finding in verdict["findings"]:
            mark = "PASS" if finding["pass"] else "FAIL"
            print(f"  [{mark}] {finding['criterion']}: "
                  f"measured {finding['measured']}, required {finding['required']}")
        if args.out:
            print(f"written: {args.out}")

    return 0 if verdict["pass"] else 1


if __name__ == "__main__":
    sys.exit(main())
