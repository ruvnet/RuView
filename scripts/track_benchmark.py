#!/usr/bin/env python3
"""Track-aware benchmark: per-person labels, held-out rooms, ambiguity class.

ADR-424 D7. Separate from the presence benchmark (ADR-423): that one answers
"is anyone in the zone"; this one scores how many people and which track is
which, and it reports the windows where the reference itself cannot tell.

Inputs (JSONL, one object per window):

* reference: ``{"timestamp_ms", "window_start_ms", "room", "health",
  "source": {"verified", "simulated"},
  "persons": [{"track_id": "a", "x_m": 0.4, "y_m": 1.3}, ...],
  "count_candidates": [2, 3]}``
  ``track_id`` is session-local and pseudonymous. ``count_candidates`` is
  optional: every count the reference's own sources reported for the window.
* predictions: ``{"timestamp_ms", "room", "persons": [{"track_id", "x_m",
  "y_m"}], "synthetic": false}``. A prediction is matched to the reference
  window that contains its timestamp after ``--clock-offset-ms``.

Window classes (exactly one per reference window):

* ``unusable``  - reference stale, unverified, simulated, or malformed.
  Dropped and counted by reason.
* ``ambiguous`` - the reference is healthy but cannot resolve who or how
  many: its sources disagree on the count, two reference tracks are closer
  than ``--min-separation-m``, a track appeared or disappeared in this window
  (``--settle-windows`` on each side), or a position is missing. Reported as
  its own class with its share. Never scored as a count or a track, and never
  silently dropped.
* ``clear``     - everything else. Only these are scored.

Scores, on clear windows of the held-out room only:

* count: accuracy, mean absolute error, and a majority-count baseline fitted
  on the other rooms;
* detection: precision / recall / F1 from one-to-one matching of predicted to
  reference persons within ``--match-radius-m`` (fitted on the other rooms
  from ``--radius-candidates``);
* tracking: identity switches and fragmentation of matched tracks;
* position: mean error of matched pairs, metres.

Evidence is MEASURED only when no prediction is marked synthetic.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from collections import Counter, defaultdict
from itertools import permutations
from typing import Dict, Iterable, List, Optional, Sequence, Tuple

MAX_PERSONS = 8  # bounds the exact matching below
Person = Tuple[str, float, float]


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


def parse_persons(raw: object) -> Tuple[Optional[List[Person]], bool]:
    """Return (persons, positions_complete). persons is None if malformed."""
    if not isinstance(raw, list) or len(raw) > MAX_PERSONS:
        return None, False
    out, complete, seen = [], True, set()
    for p in raw:
        if not isinstance(p, dict) or not isinstance(p.get("track_id"), str) or p["track_id"] in seen:
            return None, False
        seen.add(p["track_id"])
        x, y = p.get("x_m"), p.get("y_m")
        ok = all(isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(v) for v in (x, y))
        complete &= ok
        out.append((p["track_id"], float(x) if ok else math.nan, float(y) if ok else math.nan))
    return out, complete


def classify_reference(rows: Sequence[dict], min_separation_m: float,
                       settle_windows: int) -> Tuple[List[dict], Counter]:
    """Tag every reference window clear / ambiguous / unusable."""
    by_room: Dict[str, List[dict]] = defaultdict(list)
    dropped: Counter = Counter()
    for r in rows:
        src = r.get("source") or {}
        end = int(r["timestamp_ms"])
        start = int(r.get("window_start_ms", end - 1000))
        persons, complete = parse_persons(r.get("persons"))
        if not isinstance(r.get("room"), str) or not r["room"]:
            dropped["no_room"] += 1
        elif r.get("health") != "ok":
            dropped["reference_" + str(r.get("health"))] += 1
        elif not src.get("verified") or src.get("simulated"):
            dropped["reference_unverified"] += 1
        elif persons is None or start >= end:
            dropped["malformed"] += 1
        else:
            by_room[r["room"]].append({"room": r["room"], "start": start, "end": end,
                                       "persons": persons, "complete": complete,
                                       "candidates": r.get("count_candidates"), "reasons": []})
    windows: List[dict] = []
    for room_windows in by_room.values():
        room_windows.sort(key=lambda w: w["start"])
        ids = [frozenset(t for t, _, _ in w["persons"]) for w in room_windows]
        for i, w in enumerate(room_windows):
            n = len(w["persons"])
            cands = w["candidates"]
            if isinstance(cands, list) and any(c != n for c in cands):
                w["reasons"].append("count_disagreement")
            if not w["complete"]:
                w["reasons"].append("missing_position")
            elif any(math.dist(a[1:], b[1:]) < min_separation_m
                     for k, a in enumerate(w["persons"]) for b in w["persons"][k + 1:]):
                w["reasons"].append("tracks_too_close")
            lo, hi = max(0, i - settle_windows), min(len(ids) - 1, i + settle_windows)
            if any(ids[j] != ids[i] for j in range(lo, hi + 1)):
                w["reasons"].append("track_change")
            w["cls"] = "ambiguous" if w["reasons"] else "clear"
            windows.append(w)
    return windows, dropped


def attach_predictions(windows: Sequence[dict], preds: Sequence[dict], offset_ms: int) -> Counter:
    """Attach the last prediction inside each window. Returns drop counts."""
    by_room: Dict[str, List[Tuple[int, List[Person]]]] = defaultdict(list)
    dropped: Counter = Counter()
    for p in preds:
        persons, complete = parse_persons(p.get("persons"))
        if not isinstance(p.get("room"), str) or persons is None or not complete:
            dropped["bad_prediction"] += 1
            continue
        by_room[p["room"]].append((int(p["timestamp_ms"]) + offset_ms, persons))
    for v in by_room.values():
        v.sort(key=lambda t: t[0])
    for w in windows:
        inside = [ps for t, ps in by_room.get(w["room"], ()) if w["start"] <= t < w["end"]]
        w["pred"] = inside[-1] if inside else None
    return dropped


def match(ref: Sequence[Person], pred: Sequence[Person], radius: float) -> List[Tuple[int, int, float]]:
    """One-to-one matching maximising pairs within radius, then minimising distance."""
    if not ref or not pred:
        return []
    small, large, swapped = (ref, pred, False) if len(ref) <= len(pred) else (pred, ref, True)
    best: Tuple[int, float, List[Tuple[int, int, float]]] = (-1, 0.0, [])
    for perm in permutations(range(len(large)), len(small)):
        pairs = []
        for i, j in enumerate(perm):
            d = math.dist(small[i][1:], large[j][1:])
            if d <= radius:
                pairs.append((j, i, d) if swapped else (i, j, d))
        total = sum(d for _, _, d in pairs)
        if len(pairs) > best[0] or (len(pairs) == best[0] and total < best[1]):
            best = (len(pairs), total, pairs)
    return best[2]


def score(windows: Sequence[dict], radius: float, majority_count: Optional[int]) -> dict:
    """Score clear windows that have a prediction."""
    scored = [w for w in windows if w["cls"] == "clear" and w["pred"] is not None]
    tp = fp = fn = switches = fragments = 0
    count_ok = base_ok = abs_err = 0
    dist_sum = 0.0
    last_pred_for: Dict[str, str] = {}
    was_matched: Dict[str, bool] = {}
    for w in sorted(scored, key=lambda w: (w["room"], w["start"])):
        ref, pred = w["persons"], w["pred"]
        count_ok += len(ref) == len(pred)
        base_ok += majority_count is not None and len(ref) == majority_count
        abs_err += abs(len(ref) - len(pred))
        pairs = match(ref, pred, radius)
        tp += len(pairs)
        fp += len(pred) - len(pairs)
        fn += len(ref) - len(pairs)
        dist_sum += sum(d for _, _, d in pairs)
        matched_refs = set()
        for i, j, _ in pairs:
            key = f"{w['room']}/{ref[i][0]}"
            matched_refs.add(key)
            if key in last_pred_for and last_pred_for[key] != pred[j][0]:
                switches += 1
            if key in was_matched and not was_matched[key]:
                fragments += 1
            last_pred_for[key] = pred[j][0]
        for t, _, _ in ref:
            key = f"{w['room']}/{t}"
            was_matched[key] = key in matched_refs
    n = len(scored)
    precision = tp / (tp + fp) if tp + fp else 0.0
    recall = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    r4 = lambda v: round(v, 4)  # noqa: E731
    return {
        "scored_windows": n,
        "count": {"accuracy": r4(count_ok / n) if n else None,
                  "mean_abs_error": r4(abs_err / n) if n else None,
                  "majority_baseline_accuracy": r4(base_ok / n) if n and majority_count is not None else None,
                  "majority_count": majority_count},
        "detection": {"tp": tp, "fp": fp, "fn": fn, "precision": r4(precision),
                      "recall": r4(recall), "f1": r4(f1), "match_radius_m": radius},
        "tracking": {"identity_switches": switches, "fragmentations": fragments},
        "position": {"mean_error_m": r4(dist_sum / tp) if tp else None},
    }


def class_summary(windows: Sequence[dict]) -> dict:
    total = len(windows)
    amb = [w for w in windows if w["cls"] == "ambiguous"]
    clear = [w for w in windows if w["cls"] == "clear"]
    reasons = Counter(r for w in amb for r in w["reasons"])
    return {"usable_windows": total, "clear": len(clear), "ambiguous": len(amb),
            "ambiguous_share": round(len(amb) / total, 4) if total else None,
            "ambiguous_reasons": dict(sorted(reasons.items())),
            "clear_without_prediction": sum(w["pred"] is None for w in clear)}


def evaluate(ref_rows: Sequence[dict], pred_rows: Sequence[dict], offset_ms: int,
             min_separation_m: float = 0.6, settle_windows: int = 1,
             radius_candidates: Sequence[float] = (0.5, 0.75, 1.0, 1.5)) -> dict:
    windows, dropped = classify_reference(ref_rows, min_separation_m, settle_windows)
    dropped.update(attach_predictions(windows, pred_rows, offset_ms))
    rooms = sorted({w["room"] for w in windows})
    if len(rooms) < 2:
        raise ValueError(f"need >= 2 rooms for a held-out room, got {rooms}")
    folds = []
    for held in rooms:
        train = [w for w in windows if w["room"] != held]
        test = [w for w in windows if w["room"] == held]
        train_clear = [w for w in train if w["cls"] == "clear"]
        counts = Counter(len(w["persons"]) for w in train_clear)
        majority = counts.most_common(1)[0][0] if counts else None
        # Radius fitted on the other rooms only; ties go to the smaller radius.
        radius = max(radius_candidates, key=lambda r: (score(train, r, None)["detection"]["f1"], -r))
        folds.append({"held_out_room": held, "train_windows": len(train),
                      "classes": class_summary(test), **score(test, radius, majority)})
    synthetic = any(p.get("synthetic", False) for p in pred_rows)
    any_scored = any(f["scored_windows"] for f in folds)
    return {
        "task": "per-person count, detection and tracking (per room window)",
        "evidence": "SYNTHETIC" if synthetic or not any_scored else "MEASURED",
        "clock_offset_ms": offset_ms,
        "min_separation_m": min_separation_m,
        "settle_windows": settle_windows,
        "dropped": dict(sorted(dropped.items())),
        "classes": class_summary(windows),
        "folds": folds,
    }


def main(argv: Optional[List[str]] = None) -> int:
    ap = argparse.ArgumentParser(description="Track-aware held-out-room benchmark with an ambiguity class")
    ap.add_argument("--reference", nargs="+", required=True)
    ap.add_argument("--predictions", nargs="+", required=True)
    ap.add_argument("--clock-offset-ms", type=int, required=True,
                    help="reference clock minus prediction clock, ms (measure it)")
    ap.add_argument("--min-separation-m", type=float, default=0.6,
                    help="reference tracks closer than this make the window ambiguous")
    ap.add_argument("--settle-windows", type=int, default=1)
    ap.add_argument("--out")
    args = ap.parse_args(argv)
    if args.min_separation_m <= 0 or args.settle_windows < 0:
        ap.error("--min-separation-m must be > 0 and --settle-windows >= 0")
    try:
        ref = [r for p in args.reference for r in read_jsonl(p)]
        preds = [r for p in args.predictions for r in read_jsonl(p)]
        report = evaluate(ref, preds, args.clock_offset_ms, args.min_separation_m, args.settle_windows)
    except ValueError as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    text = json.dumps(report, indent=2)
    if args.out:
        with open(args.out, "w", encoding="utf-8") as f:
            f.write(text + "\n")
    else:
        print(text)
    c = report["classes"]
    print(f"[{report['evidence']}] {c['usable_windows']} usable windows: {c['clear']} clear, "
          f"{c['ambiguous']} ambiguous ({c['ambiguous_share']})", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
