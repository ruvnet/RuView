#!/usr/bin/env python3
"""Segmented overnight empty-room CSI capture (ADR-135 baseline / MAE corpus).

Binds UDP once and writes fixed-duration JSONL segments with explicit names —
no post-hoc renaming, no glob collisions with other recordings.

Usage:
    python scripts/overnight-empty-capture.py --segments 8 --segment-seconds 3300
"""

import argparse
import json
import os
import socket
import importlib.util
from pathlib import Path
import time


# Share the reviewed ADR-018 decoder with the ordinary UDP recorder so segment
# capture cannot drift to an older header layout.
_parser_spec = importlib.util.spec_from_file_location(
    "ruview_record_csi_udp", Path(__file__).with_name("record-csi-udp.py")
)
_parser_module = importlib.util.module_from_spec(_parser_spec)
_parser_spec.loader.exec_module(_parser_module)
parse_csi_packet = _parser_module.parse_csi_packet


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=5005)
    ap.add_argument("--segments", type=int, default=8)
    ap.add_argument("--segment-seconds", type=int, default=3300)
    ap.add_argument("--output", default="data/recordings")
    ap.add_argument("--prefix", default="overnight-empty")
    args = ap.parse_args()

    os.makedirs(args.output, exist_ok=True)
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(("0.0.0.0", args.port))
    sock.settimeout(2.0)

    for seg in range(1, args.segments + 1):
        path = os.path.join(
            args.output, f"{args.prefix}-seg{seg}-{int(time.time())}.csi.jsonl"
        )
        n = 0
        t_end = time.time() + args.segment_seconds
        with open(path, "w", encoding="utf-8") as f:
            while time.time() < t_end:
                try:
                    data, _ = sock.recvfrom(4096)
                except socket.timeout:
                    continue
                rec = parse_csi_packet(data)
                if rec is not None:
                    f.write(json.dumps(rec) + "\n")
                    n += 1
        print(f"segment {seg}: {n} frames -> {path}", flush=True)

    print("capture complete", flush=True)


if __name__ == "__main__":
    main()
