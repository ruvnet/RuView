"""Tests for scripts/csi_presence_recorder.py (SYNTHETIC server payloads)."""

import unittest

import csi_presence_recorder as rec
import presence_benchmark as pb


def update(source="esp32", ts=1700000000.25, nodes=None):
    if nodes is None:
        nodes = [
            {"node_id": 1, "stale": False, "classification": {"presence": True, "confidence": 0.9}},
            {"node_id": 2, "stale": False, "classification": {"presence": False, "confidence": 0.7}},
        ]
    return {"timestamp": ts, "source": source, "node_features": nodes}


class RecorderTests(unittest.TestCase):
    def setUp(self):
        self.zones = {1: "desk", 2: "sofa"}

    def test_rows_scores_and_dedupe(self):
        seen = set()
        rows = rec.predictions_from_update(update(), self.zones, "office", seen)
        self.assertEqual([(r["zone"], r["score"]) for r in rows], [("desk", 0.9), ("sofa", 0.3)])
        self.assertTrue(all(r["timestamp_ms"] == 1700000000250 and not r["synthetic"] for r in rows))
        self.assertEqual(rec.predictions_from_update(update(), self.zones, "office", seen), [])

    def test_rows_are_valid_benchmark_predictions(self):
        (row, _) = rec.predictions_from_update(update(), self.zones, "office", set())
        self.assertEqual(pb.prediction_score(row), 0.9)

    def test_simulated_source_is_synthetic(self):
        rows = rec.predictions_from_update(update(source="simulated"), self.zones, "o", set())
        self.assertTrue(all(r["synthetic"] for r in rows))
        for src in ("esp32", "realtek_csi", "mediatek", "qualcomm"):
            self.assertTrue(rec.is_real_source(src), src)
        for src in ("simulated", "test", "wifi:home", "esp32:offline", None):
            self.assertFalse(rec.is_real_source(src), src)

    def test_offline_stale_unmapped_and_bad_entries_are_skipped(self):
        self.assertEqual(rec.predictions_from_update(update(source="esp32:offline"), self.zones, "o", set()), [])
        self.assertEqual(rec.predictions_from_update({"status": "stale"}, self.zones, "o", set()), [])
        nodes = [
            {"node_id": 1, "stale": True, "classification": {"presence": True, "confidence": 0.9}},
            {"node_id": 9, "stale": False, "classification": {"presence": True, "confidence": 0.9}},
            {"node_id": 2, "stale": False, "classification": {"presence": True, "confidence": 1.5}},
        ]
        self.assertEqual(rec.predictions_from_update(update(nodes=nodes), self.zones, "o", set()), [])

    def test_argument_validation(self):
        self.assertEqual(rec.parse_node_zone(["1=desk", "2=sofa"]), {1: "desk", 2: "sofa"})
        for bad in ["desk", "x=desk", "300=desk", "1=../etc", "1="]:
            with self.assertRaises(ValueError):
                rec.parse_node_zone([bad])
        self.assertEqual(rec.validate_server("http://127.0.0.1:8080/"), "http://127.0.0.1:8080")
        for bad in ["file:///etc/passwd", "ftp://x", "127.0.0.1:8080"]:
            with self.assertRaises(ValueError):
                rec.validate_server(bad)


if __name__ == "__main__":
    unittest.main()
