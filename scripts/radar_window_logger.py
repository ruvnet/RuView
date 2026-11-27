#!/usr/bin/env python3
"""Record an MR60BHA2 (60 GHz) radar as ADR-303 radar-window JSONL.

The output is the input format of ``ruview-groundtruth`` (``parse_radar_jsonl``),
so a session recorded here can label CSI presence for the ADR-423 per-zone
presence benchmark. One JSON object is written per fixed window::

    {"timestamp_ms": ..., "window_start_ms": ..., "health": "ok",
     "source": {"verified": true, "simulated": false},
     "presence": true, "distance_cm": 140.0,
     "room": "office", "zone": "desk", "sensor": "mr60bha2"}

``room`` and ``zone`` are extra fields: ``ruview-groundtruth`` ignores unknown
fields, and the benchmark uses them for leave-one-room-out splits.

The radar is read from the ESPHome debug log of its host board (the same log
lines ``mmwave_fusion_bridge.py`` parses). ESPHome reports a sensor only when
it changes, so presence and distance are carried forward between reports; a
window with no log line at all for ``stale_after_ms`` is marked
``health: "stale"`` and carries no presence, so it labels as Unknown rather
than as an invented "empty".

Usage::

    python scripts/radar_window_logger.py --port COM4 --room office --zone desk \\
        --out sessions/office-desk-radar.jsonl --duration-s 600
    python scripts/radar_window_logger.py --replay esphome.log --room office --zone desk

Evidence grade: the labels inherit the radar's accuracy, which is a datasheet
figure (CLAIMED) until checked against an independent reference.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from dataclasses import dataclass, field
from typing import Iterable, Iterator, List, Optional, Tuple

# Same ESPHome patterns as scripts/mmwave_fusion_bridge.py (kept local so this
# module imports without pyserial for tests and replays).
RE_PRESENCE = re.compile(r"'Person Information'.*?state\s+(ON|OFF)", re.IGNORECASE)
RE_DISTANCE = re.compile(r"'Distance to detection object'.*?(\d+\.?\d*)\s*cm", re.IGNORECASE)
RE_ANSI = re.compile(r"\x1b\[[0-9;]*m")
NAME_RE = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")

DEFAULT_WINDOW_MS = 1000
DEFAULT_STALE_MS = 10_000


@dataclass
class RadarState:
    """Last values reported by the radar; ESPHome only reports on change."""

    presence: Optional[bool] = None
    distance_cm: Optional[float] = None
    last_line_ms: Optional[int] = None
    lines_in_window: int = 0

    def feed(self, line: str, t_ms: int) -> bool:
        """Update from one log line. Returns True if the line was a radar report."""
        clean = RE_ANSI.sub("", line)
        hit = False
        m = RE_PRESENCE.search(clean)
        if m:
            self.presence = m.group(1).upper() == "ON"
            hit = True
        m = RE_DISTANCE.search(clean)
        if m:
            self.distance_cm = float(m.group(1))
            hit = True
        if hit:
            self.last_line_ms = t_ms
            self.lines_in_window += 1
        return hit


@dataclass
class WindowConfig:
    room: str
    zone: str
    window_ms: int = DEFAULT_WINDOW_MS
    stale_after_ms: int = DEFAULT_STALE_MS
    verified: bool = True
    sensor: str = "mr60bha2"
    extra: dict = field(default_factory=dict)


def make_window(state: RadarState, start_ms: int, end_ms: int, cfg: WindowConfig) -> dict:
    """Summarise the radar state at the end of one window."""
    stale = state.last_line_ms is None or (end_ms - state.last_line_ms) > cfg.stale_after_ms
    w = {
        "timestamp_ms": end_ms,
        "window_start_ms": start_ms,
        "health": "stale" if stale else "ok",
        "source": {"verified": bool(cfg.verified), "simulated": False},
        "room": cfg.room,
        "zone": cfg.zone,
        "sensor": cfg.sensor,
        "reports_in_window": state.lines_in_window,
    }
    if not stale:
        if state.presence is not None:
            w["presence"] = state.presence
        if state.distance_cm is not None and state.presence:
            w["distance_cm"] = state.distance_cm
    return w


def windows_from_lines(lines: Iterable[Tuple[int, str]], cfg: WindowConfig) -> Iterator[dict]:
    """Turn timestamped log lines into fixed windows.

    ``lines`` yields ``(t_ms, text)`` in non-decreasing time. Windows start at
    the first line's time and are emitted up to the last line's time.
    """
    if cfg.window_ms <= 0:
        raise ValueError("window_ms must be positive")
    state = RadarState()
    start: Optional[int] = None
    for t_ms, text in lines:
        if start is None:
            start = t_ms
        while t_ms >= start + cfg.window_ms:
            yield make_window(state, start, start + cfg.window_ms, cfg)
            state.lines_in_window = 0
            start += cfg.window_ms
        state.feed(text, t_ms)
    if start is not None:
        yield make_window(state, start, start + cfg.window_ms, cfg)


def parse_replay_line(raw: str, fallback_ms: int) -> Tuple[int, str]:
    """Replay lines may be prefixed with an epoch-ms timestamp and a tab."""
    head, sep, rest = raw.partition("\t")
    if sep and head.isdigit():
        return int(head), rest
    return fallback_ms, raw


def validate_name(kind: str, value: str) -> str:
    if not NAME_RE.match(value):
        raise argparse.ArgumentTypeError(f"--{kind} must match {NAME_RE.pattern}")
    return value


def serial_lines(port: str, baud: int, duration_s: float, raw_log) -> Iterator[Tuple[int, str]]:
    import serial  # lazy: only needed for live capture

    deadline = time.monotonic() + duration_s if duration_s > 0 else None
    with serial.Serial(port, baud, timeout=1) as ser:
        while deadline is None or time.monotonic() < deadline:
            raw = ser.readline()
            now_ms = int(time.time() * 1000)
            text = raw.decode("utf-8", errors="replace").rstrip("\r\n") if raw else ""
            if raw_log is not None and text:
                raw_log.write(f"{now_ms}\t{text}\n")
            # An empty read still advances time so silent radars produce stale windows.
            yield now_ms, text


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    src = ap.add_mutually_exclusive_group(required=True)
    src.add_argument("--port", help="serial port of the radar's ESPHome host board")
    src.add_argument("--replay", help="recorded log (optionally 'epoch_ms<TAB>line' per line)")
    ap.add_argument("--baud", type=int, default=115200)
    ap.add_argument("--room", required=True, type=lambda v: validate_name("room", v))
    ap.add_argument("--zone", required=True, type=lambda v: validate_name("zone", v))
    ap.add_argument("--window-ms", type=int, default=DEFAULT_WINDOW_MS)
    ap.add_argument("--stale-after-ms", type=int, default=DEFAULT_STALE_MS)
    ap.add_argument("--duration-s", type=float, default=0, help="0 = until Ctrl-C")
    ap.add_argument("--out", help="output JSONL (default stdout)")
    ap.add_argument("--raw-log", help="also keep the timestamped raw log for replay")
    ap.add_argument("--unverified", action="store_true",
                    help="mark windows source.verified=false (labels become Unknown)")
    args = ap.parse_args(argv)
    if args.window_ms <= 0 or args.stale_after_ms <= 0:
        ap.error("--window-ms and --stale-after-ms must be positive")

    cfg = WindowConfig(room=args.room, zone=args.zone, window_ms=args.window_ms,
                       stale_after_ms=args.stale_after_ms,
                       verified=not args.unverified)
    out = open(args.out, "w", encoding="utf-8") if args.out else sys.stdout
    raw_log = open(args.raw_log, "w", encoding="utf-8") if args.raw_log else None
    n = 0
    try:
        if args.replay:
            # Replay of a --raw-log capture is real radar data; pass --unverified
            # for anything else (hand-edited or synthetic logs).
            with open(args.replay, encoding="utf-8", errors="replace") as f:
                lines = (parse_replay_line(l.rstrip("\r\n"), i * 100) for i, l in enumerate(f))
                for w in windows_from_lines(lines, cfg):
                    out.write(json.dumps(w) + "\n"); n += 1
        else:
            for w in windows_from_lines(serial_lines(args.port, args.baud, args.duration_s, raw_log), cfg):
                out.write(json.dumps(w) + "\n"); out.flush(); n += 1
    except KeyboardInterrupt:
        pass
    finally:
        if out is not sys.stdout:
            out.close()
        if raw_log is not None:
            raw_log.close()
    print(f"wrote {n} radar windows", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
