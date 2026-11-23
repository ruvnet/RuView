"""Tests for scripts/presence_benchmark.py (SYNTHETIC data only)."""

import json
import os
import tempfile
import unittest

import presence_benchmark as pb


def radar(room, zone, start, present, health="ok", verified=True):
    return {"timestamp_ms": start + 1000, "window_start_ms": start, "health": health,
            "source": {"verified": verified, "simulated": False},
            "presence": present, "room": room, "zone": zone}


def preds(room, zone, start, score, n=4, synthetic=True):
    return [{"timestamp_ms": start + 100 + 200 * i, "room": room, "zone": zone,
             "score": score, "synthetic": synthetic} for i in range(n)]


def session(room, offset=0, sep=True):
    """10 windows alternating empty/occupied; scores separate classes if sep."""
    r, p = [], []
    for k in range(10):
        start = 1_000_000 + k * 1000
        present = k % 2 == 1
        r.append(radar(room, "z", start, present))
        score = (0.8 if present else 0.2) if sep else 0.5
        p += preds(room, "z", start - offset, score)
    return r, p


class BenchmarkTests(unittest.TestCase):
    def test_separable_rooms_score_perfectly_and_beat_baseline(self):
        r1, p1 = session("a")
        r2, p2 = session("b")
        rep = pb.evaluate(r1 + r2, p1 + p2, offset_ms=0)
        self.assertEqual(rep["labelled_windows"], 20)
        self.assertEqual(rep["pooled"]["model"]["f1"], 1.0)
        self.assertEqual(rep["pooled"]["majority_baseline"]["accuracy"], 0.5)
        self.assertEqual(rep["evidence"], "SYNTHETIC")
        self.assertEqual([f["held_out_room"] for f in rep["folds"]], ["a", "b"])

    def test_threshold_is_learned_without_the_held_out_room(self):
        # Room b's scores are shifted up; its threshold must come from room a only.
        r1, p1 = session("a")
        r2, p2 = session("b")
        for p in p2:
            p["score"] = min(1.0, p["score"] + 0.5)  # empty -> 0.7, occupied -> 1.0
        rep = pb.evaluate(r1 + r2, p1 + p2, offset_ms=0)
        fold_b = next(f for f in rep["folds"] if f["held_out_room"] == "b")
        self.assertLessEqual(fold_b["threshold"], 0.8)
        self.assertEqual(fold_b["model"]["fp"], 5)  # calibration does not transfer

    def test_clock_offset_is_applied(self):
        r1, p1 = session("a", offset=400)
        r2, p2 = session("b", offset=400)
        rep0 = pb.evaluate(r1 + r2, p1 + p2, offset_ms=0, min_predictions=4)
        rep = pb.evaluate(r1 + r2, p1 + p2, offset_ms=400, min_predictions=4)
        self.assertEqual(rep["labelled_windows"], 20)
        self.assertLess(rep0["labelled_windows"], 20)

    def test_unusable_radar_windows_are_dropped_with_reasons(self):
        r1, p1 = session("a")
        r2, p2 = session("b")
        extra = [radar("a", "z", 0, True, health="stale"),
                 radar("a", "z", 0, True, verified=False),
                 {"timestamp_ms": 5, "health": "ok", "room": "a", "zone": "z",
                  "source": {"verified": True}}]
        rep = pb.evaluate(r1 + r2 + extra, p1 + p2, offset_ms=0)
        self.assertEqual(rep["dropped"], {"no_presence": 1, "radar_stale": 1, "radar_unverified": 1})

    def test_real_predictions_and_verified_radar_are_measured(self):
        r1, p1 = session("a")
        r2, p2 = session("b")
        for p in p1 + p2:
            p["synthetic"] = False
        self.assertEqual(pb.evaluate(r1 + r2, p1 + p2, 0)["evidence"], "MEASURED")

    def test_single_room_is_refused(self):
        r1, p1 = session("a")
        with self.assertRaises(ValueError):
            pb.evaluate(r1, p1, offset_ms=0)

    def test_bad_scores_are_ignored(self):
        self.assertIsNone(pb.prediction_score({"score": 1.5}))
        self.assertIsNone(pb.prediction_score({"score": True}))
        self.assertEqual(pb.prediction_score({"presence": True}), 1.0)

    def test_cli_requires_clock_offset_and_writes_report(self):
        r1, p1 = session("a")
        r2, p2 = session("b")
        with tempfile.TemporaryDirectory() as d:
            rp, pp, out = (os.path.join(d, n) for n in ("r.jsonl", "p.jsonl", "o.json"))
            for path, rows in ((rp, r1 + r2), (pp, p1 + p2)):
                with open(path, "w", encoding="utf-8") as f:
                    f.write("\n".join(json.dumps(x) for x in rows))
            with self.assertRaises(SystemExit):
                pb.main(["--radar", rp, "--predictions", pp])
            self.assertEqual(pb.main(["--radar", rp, "--predictions", pp,
                                      "--clock-offset-ms", "0", "--out", out]), 0)
            with open(out, encoding="utf-8") as f:
                self.assertEqual(json.load(f)["pooled"]["model"]["f1"], 1.0)
            with open(pp, "a", encoding="utf-8") as f:
                f.write("\nnot json\n")
            self.assertEqual(pb.main(["--radar", rp, "--predictions", pp,
                                      "--clock-offset-ms", "0"]), 2)


if __name__ == "__main__":
    unittest.main()
