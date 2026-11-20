"""Tests for scripts/track_benchmark.py (SYNTHETIC tracks only)."""

import unittest

import track_benchmark as tb


def ref(room, k, persons, health="ok", verified=True, candidates=None):
    start = 1_000_000 + k * 1000
    row = {"timestamp_ms": start + 1000, "window_start_ms": start, "room": room,
           "health": health, "source": {"verified": verified, "simulated": False},
           "persons": [{"track_id": t, "x_m": x, "y_m": y} for t, x, y in persons]}
    if candidates is not None:
        row["count_candidates"] = candidates
    return row


def pred(room, k, persons, synthetic=True):
    return {"timestamp_ms": 1_000_000 + k * 1000 + 500, "room": room, "synthetic": synthetic,
            "persons": [{"track_id": t, "x_m": x, "y_m": y} for t, x, y in persons]}


A, B = ("a", 0.0, 0.0), ("b", 3.0, 0.0)


def steady(room, n=8, people=(A, B), pred_ids=("p1", "p2"), jitter=0.1):
    refs = [ref(room, k, people) for k in range(n)]
    preds = [pred(room, k, [(pid, x + jitter, y) for pid, (_, x, y) in zip(pred_ids, people)])
             for k in range(n)]
    return refs, preds


class TrackBenchmarkTests(unittest.TestCase):
    def test_clean_two_person_rooms_score_fully(self):
        r1, p1 = steady("a")
        r2, p2 = steady("b")
        rep = tb.evaluate(r1 + r2, p1 + p2, offset_ms=0)
        self.assertEqual(rep["classes"]["ambiguous"], 0)
        for f in rep["folds"]:
            self.assertEqual(f["count"]["accuracy"], 1.0)
            self.assertEqual(f["detection"]["f1"], 1.0)
            self.assertEqual(f["tracking"], {"identity_switches": 0, "fragmentations": 0})
            self.assertAlmostEqual(f["position"]["mean_error_m"], 0.1, places=3)
        self.assertEqual(rep["evidence"], "SYNTHETIC")

    def test_close_tracks_are_ambiguous_not_scored(self):
        r1, p1 = steady("a")
        r2, p2 = steady("b", people=(A, ("b", 0.3, 0.0)))  # 0.3 m apart < 0.6 m
        rep = tb.evaluate(r1 + r2, p1 + p2, offset_ms=0)
        fold_b = next(f for f in rep["folds"] if f["held_out_room"] == "b")
        self.assertEqual(fold_b["classes"]["ambiguous"], 8)
        self.assertEqual(fold_b["classes"]["ambiguous_reasons"], {"tracks_too_close": 8})
        self.assertEqual(fold_b["scored_windows"], 0)
        self.assertIsNone(fold_b["count"]["accuracy"])
        self.assertEqual(rep["classes"]["ambiguous_share"], 0.5)

    def test_count_disagreement_and_track_change_are_ambiguous(self):
        r1, p1 = steady("a")
        r2, p2 = steady("b")
        r2[2] = ref("b", 2, (A, B), candidates=[2, 3])
        r2[6] = ref("b", 6, (A,))  # b leaves at window 6, returns at 7
        rep = tb.evaluate(r1 + r2, p1 + p2, offset_ms=0)
        fold_b = next(f for f in rep["folds"] if f["held_out_room"] == "b")
        self.assertEqual(fold_b["classes"]["ambiguous_reasons"],
                         {"count_disagreement": 1, "track_change": 3})
        self.assertEqual(fold_b["classes"]["clear"], 4)

    def test_identity_switch_and_fragmentation_are_counted(self):
        r1, p1 = steady("a")
        r2, p2 = steady("b")
        # Predicted ids swap between the two people from window 4 on.
        for k in range(4, 8):
            p2[k] = pred("b", k, [("p2", 0.1, 0.0), ("p1", 3.1, 0.0)])
        # Person b is missed in window 2, then found again.
        p2[2] = pred("b", 2, [("p1", 0.1, 0.0)])
        rep = tb.evaluate(r1 + r2, p1 + p2, offset_ms=0)
        fold_b = next(f for f in rep["folds"] if f["held_out_room"] == "b")
        self.assertEqual(fold_b["tracking"], {"identity_switches": 2, "fragmentations": 1})
        self.assertEqual(fold_b["detection"]["fn"], 1)
        self.assertLess(fold_b["count"]["accuracy"], 1.0)

    def test_count_baseline_comes_from_other_rooms(self):
        r1, p1 = steady("a")                      # two people
        r2, p2 = steady("b", people=(A,), pred_ids=("p1",))  # one person
        rep = tb.evaluate(r1 + r2, p1 + p2, offset_ms=0)
        fold_b = next(f for f in rep["folds"] if f["held_out_room"] == "b")
        self.assertEqual(fold_b["count"]["majority_count"], 2)
        self.assertEqual(fold_b["count"]["majority_baseline_accuracy"], 0.0)
        self.assertEqual(fold_b["count"]["accuracy"], 1.0)

    def test_unusable_reference_is_dropped_with_reasons(self):
        r1, p1 = steady("a")
        r2, p2 = steady("b")
        extra = [ref("a", 20, (A,), health="stale"), ref("a", 21, (A,), verified=False),
                 {"timestamp_ms": 5, "room": "a", "health": "ok",
                  "source": {"verified": True}, "persons": "two"}]
        rep = tb.evaluate(r1 + r2 + extra, p1 + p2, offset_ms=0)
        self.assertEqual(rep["dropped"], {"malformed": 1, "reference_stale": 1,
                                         "reference_unverified": 1})

    def test_missing_prediction_is_reported_not_scored(self):
        r1, p1 = steady("a")
        r2, p2 = steady("b")
        rep = tb.evaluate(r1 + r2, p1 + p2[:5], offset_ms=0)
        fold_b = next(f for f in rep["folds"] if f["held_out_room"] == "b")
        self.assertEqual(fold_b["classes"]["clear_without_prediction"], 3)
        self.assertEqual(fold_b["scored_windows"], 5)

    def test_measured_only_without_synthetic_and_two_rooms_required(self):
        r1, p1 = steady("a")
        r2, p2 = steady("b")
        for p in p1 + p2:
            p["synthetic"] = False
        self.assertEqual(tb.evaluate(r1 + r2, p1 + p2, 0)["evidence"], "MEASURED")
        with self.assertRaises(ValueError):
            tb.evaluate(r1, p1, 0)

    def test_matching_is_one_to_one_and_radius_bounded(self):
        refp = [("a", 0.0, 0.0), ("b", 1.0, 0.0)]
        predp = [("x", 0.4, 0.0)]
        self.assertEqual([(i, j) for i, j, _ in tb.match(refp, predp, 1.0)], [(0, 0)])
        self.assertEqual(tb.match(refp, [("x", 9.0, 9.0)], 1.0), [])
        self.assertEqual(tb.parse_persons([{"track_id": "a"}, {"track_id": "a"}]), (None, False))


if __name__ == "__main__":
    unittest.main()
