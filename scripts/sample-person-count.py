"""Sample the published person count from a running RuView sensing server.

Reads only the aggregate scalars the server publishes - the estimated person
count, the room classification and the feature energies. It never reads the
raw CSI, the amplitude vectors or anything per-person (ADR-299).

The output is one JSON object per line, so two windows can be compared
directly:

    python scripts/sample-person-count.py --out before.jsonl --seconds 200
    python scripts/analyze-person-count.py before.jsonl

Requires a sensing server already running against live ESP32 nodes, e.g.

    v2/target/debug/sensing-server --source esp32 --udp-port 5005 \\
        --udp-bind 0.0.0.0 --udp-insecure-lan --bind-addr 127.0.0.1
"""

import argparse
import json
import time
import urllib.request


def fetch(url):
    with urllib.request.urlopen(url, timeout=5) as response:
        return json.loads(response.read().decode("utf-8"))


def sample(payload):
    features = payload.get("features") or {}
    classification = payload.get("classification") or {}
    node_features = payload.get("node_features") or []
    node = node_features[0] if node_features else {}
    node_classification = node.get("classification") or {}
    return {
        "t": round(time.monotonic(), 3),
        "wall": round(time.time(), 3),
        "age_ms": payload.get("age_ms"),
        "estimated_persons": payload.get("estimated_persons"),
        "motion_level": classification.get("motion_level"),
        "presence": classification.get("presence"),
        "confidence": classification.get("confidence"),
        "variance": features.get("variance"),
        "dominant_freq_hz": features.get("dominant_freq_hz"),
        "motion_band_power": features.get("motion_band_power"),
        "node_motion_level": node_classification.get("motion_level"),
        "node_presence": node_classification.get("presence"),
        "node_confidence": node_classification.get("confidence"),
        "frame_rate_hz": node.get("frame_rate_hz"),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--url",
        default="http://127.0.0.1:8080/api/v1/sensing/latest",
        help="the sensing server's latest-snapshot endpoint",
    )
    parser.add_argument("--out", required=True, help="JSONL output path")
    parser.add_argument("--seconds", type=float, default=200.0, help="window length")
    parser.add_argument("--interval", type=float, default=0.2, help="poll interval")
    args = parser.parse_args()

    start = time.monotonic()
    deadline = start + args.seconds
    written = 0
    with open(args.out, "w", encoding="utf-8") as handle:
        while time.monotonic() < deadline:
            try:
                payload = fetch(args.url)
            except Exception:
                time.sleep(args.interval)
                continue
            entry = sample(payload)
            entry["t"] = round(entry["t"] - start, 3)
            handle.write(json.dumps(entry) + "\n")
            written += 1
            time.sleep(args.interval)
    print(f"wrote {written} samples to {args.out}")


if __name__ == "__main__":
    main()
