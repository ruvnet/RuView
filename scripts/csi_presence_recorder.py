#!/usr/bin/env python3
"""Record per-node CSI presence from a running sensing server as JSONL.

Writes the prediction input of ``presence_benchmark.py``::

    {"timestamp_ms": ..., "room": "office", "zone": "desk", "node_id": 1,
     "presence": true, "confidence": 0.82, "score": 0.82,
     "source": "esp32", "synthetic": false}

It polls ``GET /api/v1/sensing/latest`` and emits one row per fresh per-node
classification (``node_features[]``). The server has no real zone model yet,
so the operator maps node ids to zones (``--node-zone 1=desk``); unmapped
nodes are skipped. Rows are de-duplicated on (node, update timestamp).

``score`` assumes the server's ``confidence`` is the confidence of the
reported class, so ``score = confidence`` when present and
``1 - confidence`` when absent; the raw fields are kept so the scorer's
input can be re-derived if that assumption turns out wrong.

``synthetic`` is true unless the update's ``source`` is a real-hardware
source (start the server with ``--source esp32``; ``auto`` does not bind the
CSI receiver on a host that has its own WiFi adapter). Stale node entries
and stale/empty responses are skipped, never recorded as "absent".
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.parse
import urllib.request
from typing import Dict, Iterable, List, Optional, Set, Tuple

# Hardware CSI sources as named by the sensing server (effective_source()).
REAL_SOURCE_PREFIXES = ("esp32", "realtek", "mediatek", "qualcomm")
NAME_RE = re.compile(r"^[A-Za-z0-9_.-]{1,64}$")


def parse_node_zone(items: Iterable[str]) -> Dict[int, str]:
    out: Dict[int, str] = {}
    for item in items:
        node, sep, zone = item.partition("=")
        if not sep or not node.isdigit() or not 0 <= int(node) <= 255 or not NAME_RE.match(zone):
            raise ValueError(f"--node-zone expects <0-255>=<name>, got {item!r}")
        out[int(node)] = zone
    return out


def is_offline(source: object) -> bool:
    return isinstance(source, str) and source.endswith(":offline")


def is_real_source(source: object) -> bool:
    return (isinstance(source, str) and not is_offline(source)
            and source.lower().startswith(REAL_SOURCE_PREFIXES))


def predictions_from_update(update: dict, node_zone: Dict[int, str], room: str,
                            seen: Set[Tuple[int, float]]) -> List[dict]:
    """Rows for the fresh, mapped per-node classifications in one update."""
    if not isinstance(update, dict) or update.get("status") in ("stale", "no data yet"):
        return []
    if is_offline(update.get("source")):
        return []  # receiver went quiet: the classification is old, not "absent"
    ts = update.get("timestamp")
    if not isinstance(ts, (int, float)):
        return []
    synthetic = not is_real_source(update.get("source"))
    rows = []
    for nf in update.get("node_features") or []:
        node = nf.get("node_id")
        cls = nf.get("classification") or {}
        if node not in node_zone or nf.get("stale") or (node, ts) in seen:
            continue
        presence, conf = cls.get("presence"), cls.get("confidence")
        if not isinstance(presence, bool) or not isinstance(conf, (int, float)) or not 0 <= conf <= 1:
            continue
        seen.add((node, ts))
        rows.append({
            "timestamp_ms": int(round(ts * 1000)),
            "room": room, "zone": node_zone[node], "node_id": node,
            "presence": presence, "confidence": float(conf),
            "score": round(float(conf) if presence else 1.0 - float(conf), 6),
            "source": update.get("source"), "synthetic": synthetic,
        })
    return rows


def validate_server(url: str) -> str:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme not in ("http", "https") or not parsed.hostname:
        raise ValueError(f"--server must be an http(s) URL, got {url!r}")
    return url.rstrip("/")


def fetch(url: str, timeout_s: float) -> Optional[dict]:
    try:
        with urllib.request.urlopen(url, timeout=timeout_s) as r:  # noqa: S310 (validated scheme)
            return json.loads(r.read(4 * 1024 * 1024))
    except (OSError, ValueError):
        return None


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="Record per-node CSI presence for presence_benchmark.py")
    ap.add_argument("--server", default="http://127.0.0.1:8080")
    ap.add_argument("--room", required=True)
    ap.add_argument("--node-zone", action="append", required=True, help="node_id=zone, repeatable")
    ap.add_argument("--interval-ms", type=int, default=250)
    ap.add_argument("--duration-s", type=float, default=0, help="0 = until Ctrl-C")
    ap.add_argument("--out", required=True)
    args = ap.parse_args(argv)
    try:
        server = validate_server(args.server)
        node_zone = parse_node_zone(args.node_zone)
        if not NAME_RE.match(args.room):
            raise ValueError(f"--room must match {NAME_RE.pattern}")
        if args.interval_ms < 50:
            raise ValueError("--interval-ms must be >= 50")
    except ValueError as e:
        ap.error(str(e))

    url = server + "/api/v1/sensing/latest"
    seen: Set[Tuple[int, float]] = set()
    n = misses = 0
    deadline = time.monotonic() + args.duration_s if args.duration_s > 0 else None
    with open(args.out, "a", encoding="utf-8") as out:
        try:
            while deadline is None or time.monotonic() < deadline:
                update = fetch(url, timeout_s=2.0)
                rows = predictions_from_update(update, node_zone, args.room, seen) if update else []
                misses += update is None
                for row in rows:
                    out.write(json.dumps(row) + "\n")
                n += len(rows)
                out.flush()
                if len(seen) > 100_000:
                    seen.clear()
                time.sleep(args.interval_ms / 1000)
        except KeyboardInterrupt:
            pass
    print(f"wrote {n} predictions ({misses} failed polls)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
