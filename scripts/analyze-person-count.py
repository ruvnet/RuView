"""Summarise the person-count stability of one or more RuView sampling windows.

Reads the JSONL files written by `scripts/sample-person-count.py` and reports,
for each window: the sample count, the window length, the published-count
distribution, how many times the published count changed, the room
classification distribution and the longest run of a single published count.

    python scripts/analyze-person-count.py before.jsonl after.jsonl
"""

import argparse
import json
import sys


def load(path):
    rows = []
    with open(path, "r", encoding="utf-8") as handle:
        for line in handle:
            line = line.strip()
            if line:
                rows.append(json.loads(line))
    return rows


def summarise(path):
    rows = load(path)
    if not rows:
        return None
    counts = [row.get("estimated_persons") for row in rows]
    levels = [row.get("motion_level") for row in rows]

    distribution = {}
    for value in counts:
        distribution[value] = distribution.get(value, 0) + 1

    changes = []
    for index in range(1, len(counts)):
        if counts[index] != counts[index - 1]:
            changes.append(
                {
                    "t": round(rows[index]["t"], 2),
                    "from": counts[index - 1],
                    "to": counts[index],
                }
            )

    runs = []
    start = 0
    for index in range(1, len(counts) + 1):
        if index == len(counts) or counts[index] != counts[start]:
            runs.append((counts[start], round(rows[index - 1]["t"] - rows[start]["t"], 2)))
            start = index

    room_distribution = {}
    for level in levels:
        room_distribution[level] = room_distribution.get(level, 0) + 1

    return {
        "file": path,
        "samples": len(rows),
        "window_s": round(rows[-1]["t"] - rows[0]["t"], 1),
        "count_distribution": dict(sorted(distribution.items(), key=lambda kv: str(kv[0]))),
        "published_count_changes": len(changes),
        "changes": changes,
        "longest_run_s": max((length for _, length in runs), default=0),
        "room_classification": dict(sorted(room_distribution.items(), key=lambda kv: str(kv[0]))),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("windows", nargs="+", help="JSONL files written by the sampler")
    args = parser.parse_args()

    for path in args.windows:
        result = summarise(path)
        if result is None:
            print(f"{path}: no samples")
            continue
        print(f"=== {result['file']} ===")
        print(f"  samples            : {result['samples']}")
        print(f"  window (s)         : {result['window_s']}")
        print(f"  count distribution : {result['count_distribution']}")
        print(f"  published changes  : {result['published_count_changes']}")
        print(f"  longest stable run : {result['longest_run_s']} s")
        print(f"  room classification : {result['room_classification']}")
        for change in result["changes"][:12]:
            print(f"    t={change['t']}s  {change['from']} -> {change['to']}")
        if len(result["changes"]) > 12:
            print(f"    ... {len(result['changes']) - 12} more")
        print()


if __name__ == "__main__":
    main()
