#!/usr/bin/env python3
"""Log keyboard/mouse activity as an occupied-only presence reference (ADR-426 D3).

A person typing or moving the mouse is at the desk. That makes the operating
system's "time since last input" an independent reference for *occupied* in
the zone around a workstation, with no extra hardware. It records one number,
the idle time; never keys, pointer positions or window titles.

Output is the radar-window JSONL shape that ``build_replay_rows.py`` and
``ruview-groundtruth`` read, one object per window::

    {"timestamp_ms": ..., "window_start_ms": ..., "health": "ok",
     "source": {"verified": true, "simulated": false},
     "presence": true, "idle_ms": 420,
     "room": "office", "zone": "desk", "sensor": "input_activity"}

The reference is one-sided. Recent input means occupied. No recent input means
nothing: the person may be reading, or gone. Such windows are written with
``health: "unknown"`` and no ``presence``, so they can never label a row
"empty". Empty sessions need another reference (a radar, or an operator who
declares the room empty and leaves).

Windows only (``GetLastInputInfo``).

Usage::

    python scripts/input_activity_logger.py --room office --zone desk \\
        --out office-desk-input.jsonl --duration-s 600
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from typing import Callable, Iterator, Optional

NAME_RE = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")
DEFAULT_WINDOW_MS = 1000
DEFAULT_ACTIVE_MS = 3000


def windows_idle_ms() -> int:
    """Milliseconds since the last keyboard or mouse input on this session."""
    import ctypes

    class LASTINPUTINFO(ctypes.Structure):
        _fields_ = [("cbSize", ctypes.c_uint), ("dwTime", ctypes.c_uint)]

    info = LASTINPUTINFO()
    info.cbSize = ctypes.sizeof(info)
    if not ctypes.windll.user32.GetLastInputInfo(ctypes.byref(info)):
        raise OSError("GetLastInputInfo failed")
    # Both tick counters are 32-bit and wrap together every 49.7 days.
    return (ctypes.windll.kernel32.GetTickCount() - info.dwTime) & 0xFFFFFFFF


def window(now_ms: int, window_ms: int, max_idle_ms: Optional[int], active_ms: int,
           room: str, zone: str) -> dict:
    """One reference window. ``max_idle_ms`` is the largest idle time seen in it,
    or ``None`` when the idle time could not be read."""
    out = {
        "timestamp_ms": now_ms, "window_start_ms": now_ms - window_ms,
        "source": {"verified": True, "simulated": False},
        "room": room, "zone": zone, "sensor": "input_activity",
    }
    if max_idle_ms is not None and max_idle_ms < active_ms:
        out.update(health="ok", presence=True, idle_ms=max_idle_ms)
    else:
        # Not "empty": no input says nothing about who is in the room.
        out.update(health="unknown", idle_ms=max_idle_ms)
    return out


def run(idle_ms: Callable[[], int], clock_ms: Callable[[], int], sleep: Callable[[float], None],
        *, room: str, zone: str, window_ms: int, active_ms: int,
        duration_s: Optional[float]) -> Iterator[dict]:
    start = clock_ms()
    next_edge = start + window_ms
    worst: Optional[int] = None
    failed = False
    while duration_s is None or clock_ms() - start < duration_s * 1000:
        try:
            sample = idle_ms()
            worst = sample if worst is None else max(worst, sample)
        except OSError:
            failed = True
        now = clock_ms()
        if now >= next_edge:
            yield window(now, window_ms, None if failed else worst, active_ms, room, zone)
            next_edge, worst, failed = now + window_ms, None, False
        sleep(0.1)


def main(argv: Optional[list] = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--room", required=True)
    ap.add_argument("--zone", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--duration-s", type=float)
    ap.add_argument("--window-ms", type=int, default=DEFAULT_WINDOW_MS)
    ap.add_argument("--active-ms", type=int, default=DEFAULT_ACTIVE_MS,
                    help="input within this long counts as occupied (default 3000)")
    args = ap.parse_args(argv)
    for name in (args.room, args.zone):
        if not NAME_RE.match(name):
            ap.error(f"{name!r}: use 1-64 letters, digits, '_', '.', '-'")
    if not 100 <= args.window_ms <= 60_000 or not 500 <= args.active_ms <= 60_000:
        ap.error("--window-ms must be 100-60000 and --active-ms 500-60000")
    if sys.platform != "win32":
        print("error: input activity is only implemented for Windows", file=sys.stderr)
        return 2
    n = occupied = 0
    with open(args.out, "a", encoding="utf-8", newline="\n") as f:
        try:
            for w in run(windows_idle_ms, lambda: int(time.time() * 1000), time.sleep,
                         room=args.room, zone=args.zone, window_ms=args.window_ms,
                         active_ms=args.active_ms, duration_s=args.duration_s):
                f.write(json.dumps(w) + "\n")
                f.flush()
                n += 1
                occupied += w["health"] == "ok"
        except KeyboardInterrupt:
            pass
    print(json.dumps({"out": args.out, "windows": n, "occupied_windows": occupied}),
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
