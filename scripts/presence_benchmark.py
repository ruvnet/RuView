#!/usr/bin/env python3
"""Score CSI presence against radar labels with leave-one-room-out (ADR-423).

Inputs (JSONL, one object per line):

* radar windows from ``radar_window_logger.py``: ``timestamp_ms``,
  ``window_start_ms``, ``health``, ``source``, ``presence``, ``room``, ``zone``.
* CSI predictions: ``{"timestamp_ms": ..., "room": "office", "zone": "desk",
  "score": 0.83}`` (``score`` in [0, 1]) or ``"presence": true``. Add
  ``"synthetic": true`` for anything not produced by real hardware.

Method:

1. A radar window is a label only when ``health == "ok"``, the source is
   verified and not simulated, and ``presence`` is set. Everything else is
   counted as dropped with a reason, never guessed.
2. Predictions for the same room and zone inside the window (after applying
   the required ``--clock-offset-ms``, radar clock minus CSI clock) are
   averaged; a window with fewer than ``--min-predictions`` is dropped.
3. Leave one room out: for each room, the decision threshold is chosen on the
   other rooms only (max F1), then applied to the held-out room. The baseline
   predicts the training rooms' majority class. Nothing from the held-out room
   tunes anything.

The report's ``evidence`` is ``MEASURED`` only when every label came from a
verified radar and no prediction is marked synthetic; otherwise ``SYNTHETIC``.
Presence only: this says nothing about pose, counting, or identity.
"""

from __future__ import annotations

import argparse
import json
import sys
from collections import Counter, defaultdict
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

Label = Tuple[str, str, int, int, bool]  # room, zone, start_ms, end_ms, present


def read_jsonl(path: str) -> List[dict]:
    rows = []
    with open(path, encoding="utf-8") as f:
        for i, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                row = json.loads(line)
            except json.JSONDecodeError as e:
                raise ValueError(f"{path}:{i}: invalid JSON ({e.msg})") from None
            if not isinstance(row, dict) or not isinstance(row.get("timestamp_ms"), (int, float)):
                raise ValueError(f"{path}:{i}: object with numeric timestamp_ms required")
            rows.append(row)
    return rows


def radar_labels(rows: Iterable[dict]) -> Tuple[List[Label], Counter]:
    labels: List[Label] = []
    dropped: Counter = Counter()
    for r in rows:
        src = r.get("source") or {}
        if not r.get("room") or not r.get("zone"):
            dropped["no_room_zone"] += 1
        elif r.get("health") != "ok":
            dropped["radar_" + str(r.get("health"))] += 1
        elif not src.get("verified") or src.get("simulated"):
            dropped["radar_unverified"] += 1
        elif not isinstance(r.get("presence"), bool):
            dropped["no_presence"] += 1
        else:
            end = int(r["timestamp_ms"])
            start = int(r.get("window_start_ms", end - 1000))
            if start >= end:
                dropped["bad_timestamps"] += 1
                continue
            labels.append((str(r["room"]), str(r["zone"]), start, end, r["presence"]))
    return labels, dropped


def prediction_score(p: dict) -> Optional[float]:
    s = p.get("score")
    if isinstance(s, (int, float)) and not isinstance(s, bool) and 0.0 <= s <= 1.0:
        return float(s)
    if isinstance(p.get("presence"), bool):
        return 1.0 if p["presence"] else 0.0
    return None


def join(labels: Sequence[Label], preds: Sequence[dict], offset_ms: int,
         min_predictions: int) -> Tuple[List[Tuple[str, bool, float]], Counter]:
    """Return (room, truth, mean score) per usable window."""
    by_key: Dict[Tuple[str, str], List[Tuple[int, float]]] = defaultdict(list)
    for p in preds:
        s = prediction_score(p)
        if s is not None and p.get("room") and p.get("zone"):
            by_key[(str(p["room"]), str(p["zone"]))].append((int(p["timestamp_ms"]) + offset_ms, s))
    for v in by_key.values():
        v.sort()
    out, dropped = [], Counter()
    for room, zone, start, end, truth in labels:
        scores = [s for t, s in by_key.get((room, zone), ()) if start <= t < end]
        if len(scores) < min_predictions:
            dropped["too_few_predictions"] += 1
            continue
        out.append((room, truth, sum(scores) / len(scores)))
    return out, dropped


def confusion(samples: Iterable[Tuple[bool, bool]]) -> dict:
    tp = fp = fn = tn = 0
    for truth, pred in samples:
        if truth and pred:
            tp += 1
        elif pred:
            fp += 1
        elif truth:
            fn += 1
        else:
            tn += 1
    n = tp + fp + fn + tn
    precision = tp / (tp + fp) if tp + fp else 0.0
    recall = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {"n": n, "tp": tp, "fp": fp, "fn": fn, "tn": tn,
            "precision": round(precision, 4), "recall": round(recall, 4),
            "f1": round(f1, 4), "accuracy": round((tp + tn) / n, 4) if n else 0.0}


def best_threshold(train: Sequence[Tuple[bool, float]]) -> float:
    """Threshold maximising F1 on training windows (ties: the lower one)."""
    candidates = sorted({s for _, s in train} | {0.5})
    best_t, best_f1 = 0.5, -1.0
    for t in candidates:
        f1 = confusion((y, s >= t) for y, s in train)["f1"]
        if f1 > best_f1:
            best_t, best_f1 = t, f1
    return best_t


def leave_one_room_out(windows: Sequence[Tuple[str, bool, float]]) -> dict:
    rooms = sorted({r for r, _, _ in windows})
    if len(rooms) < 2:
        raise ValueError(f"need >= 2 rooms for a held-out room, got {rooms}")
    folds, pooled_model, pooled_base = [], [], []
    for held in rooms:
        train = [(y, s) for r, y, s in windows if r != held]
        test = [(y, s) for r, y, s in windows if r == held]
        t = best_threshold(train)
        majority = Counter(y for y, _ in train).most_common(1)[0][0]
        model = [(y, s >= t) for y, s in test]
        base = [(y, majority) for y, _ in test]
        pooled_model += model
        pooled_base += base
        folds.append({"held_out_room": held, "threshold": round(t, 4),
                      "train_windows": len(train), "model": confusion(model),
                      "majority_baseline": {"predicts": majority, **confusion(base)}})
    return {"folds": folds, "pooled": {"model": confusion(pooled_model),
                                       "majority_baseline": confusion(pooled_base)}}


def evaluate(radar_rows: Sequence[dict], pred_rows: Sequence[dict], offset_ms: int,
             min_predictions: int = 3) -> dict:
    labels, dropped = radar_labels(radar_rows)
    windows, d2 = join(labels, pred_rows, offset_ms, min_predictions)
    dropped.update(d2)
    synthetic = any(p.get("synthetic", False) for p in pred_rows)
    report = {
        "task": "presence (per room/zone window)",
        "evidence": "SYNTHETIC" if synthetic or not windows else "MEASURED",
        "clock_offset_ms": offset_ms,
        "min_predictions": min_predictions,
        "labelled_windows": len(windows),
        "positive_rate": round(sum(y for _, y, _ in windows) / len(windows), 4) if windows else None,
        "dropped": dict(sorted(dropped.items())),
        "rooms": dict(sorted(Counter(r for r, _, _ in windows).items())),
    }
    report.update(leave_one_room_out(windows))
    return report


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="Leave-one-room-out CSI presence benchmark")
    ap.add_argument("--radar", nargs="+", required=True, help="radar window JSONL file(s)")
    ap.add_argument("--predictions", nargs="+", required=True, help="CSI prediction JSONL file(s)")
    ap.add_argument("--clock-offset-ms", type=int, required=True,
                    help="radar clock minus CSI clock, ms (measure it; never assumed)")
    ap.add_argument("--min-predictions", type=int, default=3)
    ap.add_argument("--out", help="write the JSON report here (default stdout)")
    args = ap.parse_args(argv)
    if args.min_predictions < 1:
        ap.error("--min-predictions must be >= 1")
    try:
        radar = [r for p in args.radar for r in read_jsonl(p)]
        preds = [r for p in args.predictions for r in read_jsonl(p)]
        report = evaluate(radar, preds, args.clock_offset_ms, args.min_predictions)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    text = json.dumps(report, indent=2)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(text + "\n")
    else:
        print(text)
    pooled = report["pooled"]
    print(f"[{report['evidence']}] {report['labelled_windows']} windows, "
          f"model F1 {pooled['model']['f1']} vs majority baseline accuracy "
          f"{pooled['majority_baseline']['accuracy']} (model accuracy {pooled['model']['accuracy']})",
          file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
