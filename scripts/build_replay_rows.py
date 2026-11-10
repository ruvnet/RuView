#!/usr/bin/env python3
"""Turn a sensing-server feature capture into ``ruview-fann-replay`` rows (ADR-426 D3).

Input is the JSON-lines capture the server writes when built with
``--features fann-shadow`` and started with ``RUVIEW_FANN_SHADOW_LOG`` set: one
record per fresh per-node classification with its encoded features. One call
converts one recorded session; concatenate the outputs of several sessions.

Two label modes, and they are not interchangeable:

``--labels radar``
    The operator records each session under one declared condition
    (``--condition quiet_empty|adjacent_only|still_occupied|moving_occupied``)
    and a radar logged by ``radar_window_logger.py`` gates it: a capture row is
    kept only when a healthy radar window covers it and the radar's presence
    matches the declared condition. Everything else is dropped and counted.
    Rows are written with ``label_source: "radar_gated"``. This is the only
    mode whose replay report is an accuracy result, and it inherits the
    radar's own accuracy (a datasheet figure, CLAIMED).

    The same mode accepts any reference in that window format. With
    ``input_activity_logger.py`` the rows are written ``input_gated``; that
    reference only ever confirms *occupied*, so it cannot gate an empty
    session.

``--labels declared``
    The operator declares the session's condition and nothing confirms it
    (``label_source: "operator_declared"``). Meant for empty-room sessions,
    where the operator starts the recording and leaves. If ``--radar`` is
    given, any row inside a window where that reference saw someone is
    dropped, so a session cannot be declared empty while someone is typing.

``--labels incumbent``
    The label is the server's own classification. A replay on such rows
    measures how well a head imitates the server, nothing else; the rows carry
    ``label_source: "incumbent"`` and the replay report says so.

Usage::

    python scripts/build_replay_rows.py --capture office-s1.jsonl --room office \\
        --session s1 --split train --labels radar --radar office-s1-radar.jsonl \\
        --condition still_occupied --out office-s1.rows.jsonl
"""

from __future__ import annotations

import argparse
import bisect
import json
import re
import sys
from typing import Dict, Iterable, List, Optional, Tuple

CONDITIONS = ("quiet_empty", "adjacent_only", "still_occupied", "moving_occupied")
OCCUPIED = {"still_occupied", "moving_occupied"}
SPLITS = ("train", "validation", "heldout")
FEATURE_SCHEMA = "ruview.node-features/1"
NAME_RE = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")


def parse_jsonl(lines: Iterable[str]) -> Tuple[List[dict], int]:
    """Parsed objects and the number of lines that were not JSON objects."""
    out, bad = [], 0
    for line in lines:
        line = line.strip()
        if not line:
            continue
        try:
            obj = json.loads(line)
        except ValueError:
            bad += 1
            continue
        if isinstance(obj, dict):
            out.append(obj)
        else:
            bad += 1
    return out, bad


class RadarIndex:
    """Healthy radar windows, looked up by the time they cover."""

    def __init__(self, windows: Iterable[dict]):
        spans = []
        self.label_source = "radar_gated"
        for w in windows:
            start, end = w.get("window_start_ms"), w.get("timestamp_ms")
            if (w.get("health") != "ok" or not isinstance(w.get("presence"), bool)
                    or not isinstance(start, int) or not isinstance(end, int) or end < start):
                continue
            src = w.get("source") if isinstance(w.get("source"), dict) else {}
            spans.append((start, end, w["presence"], bool(src.get("simulated"))))
            if w.get("sensor") == "input_activity":
                self.label_source = "input_gated"
        spans.sort()
        self.spans = spans
        self.starts = [s[0] for s in spans]

    def lookup(self, timestamp_ms: int) -> Optional[Tuple[bool, bool]]:
        """(presence, simulated) of the window covering ``timestamp_ms``."""
        i = bisect.bisect_right(self.starts, timestamp_ms) - 1
        if i < 0:
            return None
        start, end, presence, simulated = self.spans[i]
        return (presence, simulated) if start <= timestamp_ms <= end else None


def incumbent_condition(incumbent: dict) -> Optional[str]:
    presence = incumbent.get("presence")
    if not isinstance(presence, bool):
        return None
    if not presence:
        return "quiet_empty"
    level = str(incumbent.get("motion_level", ""))
    return "still_occupied" if level == "present_still" else "moving_occupied"


def incumbent_probability(incumbent: dict) -> Optional[float]:
    """The server's decision as 1.0 / 0.0.

    The server's ``confidence`` is a smoothed person score, not the probability
    of the reported class (live captures show ``presence: true`` with a
    confidence near 0.4), so no probability is derived from it.
    """
    presence = incumbent.get("presence")
    return (1.0 if presence else 0.0) if isinstance(presence, bool) else None


def build_rows(capture: Iterable[dict], *, room: str, session: str, split: str, labels: str,
               node: Optional[int] = None, condition: Optional[str] = None,
               radar: Optional[RadarIndex] = None) -> Tuple[List[dict], Dict[str, int]]:
    stats = {"capture_rows": 0, "kept": 0, "other_node": 0, "bad_row": 0, "no_radar_window": 0,
             "radar_disagrees": 0, "no_incumbent_label": 0, "contradicted_by_reference": 0}
    rows = []
    for rec in capture:
        stats["capture_rows"] += 1
        feats, ts, inc = rec.get("features"), rec.get("timestamp_ms"), rec.get("incumbent")
        if (rec.get("feature_schema") != FEATURE_SCHEMA or not isinstance(ts, int)
                or not isinstance(inc, dict) or not isinstance(feats, list) or not feats
                or not all(isinstance(v, (int, float)) and v == v and abs(v) != float("inf")
                           for v in feats)):
            stats["bad_row"] += 1
            continue
        if node is not None and rec.get("node_id") != node:
            stats["other_node"] += 1
            continue
        synthetic = bool(rec.get("synthetic"))
        if labels == "radar":
            hit = radar.lookup(ts) if radar else None
            if hit is None:
                stats["no_radar_window"] += 1
                continue
            presence, simulated = hit
            if presence != (condition in OCCUPIED):
                stats["radar_disagrees"] += 1
                continue
            row_condition, source = condition, radar.label_source
            synthetic = synthetic or simulated
        elif labels == "declared":
            hit = radar.lookup(ts) if radar else None
            if hit is not None and hit[0] != (condition in OCCUPIED):
                stats["contradicted_by_reference"] += 1
                continue
            row_condition, source = condition, "operator_declared"
        else:
            row_condition = incumbent_condition(inc)
            if row_condition is None:
                stats["no_incumbent_label"] += 1
                continue
            source = "incumbent"
        rows.append({
            "room": room, "session": session, "split": split, "condition": row_condition,
            "features": [float(v) for v in feats],
            "incumbent_probability": incumbent_probability(inc),
            "synthetic": synthetic, "timestamp_ms": ts, "label_source": source,
        })
        stats["kept"] += 1
    return rows, stats


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--capture", required=True)
    ap.add_argument("--room", required=True)
    ap.add_argument("--session", required=True)
    ap.add_argument("--split", required=True, choices=SPLITS)
    ap.add_argument("--labels", required=True, choices=("radar", "declared", "incumbent"))
    ap.add_argument("--radar", help="reference-window JSONL from radar_window_logger.py or "
                                    "input_activity_logger.py (required for --labels radar)")
    ap.add_argument("--condition", choices=CONDITIONS,
                    help="condition the session was recorded under "
                         "(required for --labels radar and declared)")
    ap.add_argument("--trim-s", type=float, default=0.0,
                    help="drop this many seconds from each end of the capture, e.g. the "
                         "time it takes the operator to leave and come back")
    ap.add_argument("--node", type=int, help="keep one node id only")
    ap.add_argument("--out", required=True)
    args = ap.parse_args(argv)

    for name in (args.room, args.session):
        if not NAME_RE.match(name):
            ap.error(f"{name!r}: use 1-64 letters, digits, '_', '.', '-'")
    radar = None
    if args.labels == "radar" and not args.radar:
        ap.error("--labels radar needs --radar")
    if args.labels in ("radar", "declared") and not args.condition:
        ap.error(f"--labels {args.labels} needs --condition")
    if args.radar and args.labels != "incumbent":
        with open(args.radar, encoding="utf-8") as f:
            radar = RadarIndex(parse_jsonl(f)[0])
    with open(args.capture, encoding="utf-8") as f:
        capture, bad_lines = parse_jsonl(f)
    if args.trim_s > 0:
        times = [c["timestamp_ms"] for c in capture if isinstance(c.get("timestamp_ms"), int)]
        if times:
            lo, hi = min(times) + args.trim_s * 1000, max(times) - args.trim_s * 1000
            capture = [c for c in capture
                       if isinstance(c.get("timestamp_ms"), int) and lo <= c["timestamp_ms"] <= hi]
    rows, stats = build_rows(capture, room=args.room, session=args.session, split=args.split,
                             labels=args.labels, node=args.node, condition=args.condition,
                             radar=radar)
    stats["bad_lines"] = bad_lines
    with open(args.out, "w", encoding="utf-8", newline="\n") as f:
        for row in rows:
            f.write(json.dumps(row) + "\n")
    print(json.dumps({"out": args.out, "labels": args.labels, **stats}), file=sys.stderr)
    return 0 if rows else 1


if __name__ == "__main__":
    sys.exit(main())
