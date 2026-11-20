"""Tests for build_replay_rows.py (ADR-426 D3). All inputs are SYNTHETIC."""

import json
import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import build_replay_rows as b  # noqa: E402


def cap(ts, presence=True, level="present_still", conf=0.8, node=1, **extra):
    rec = {"timestamp_ms": ts, "node_id": node, "source": "esp32", "synthetic": False,
           "feature_schema": b.FEATURE_SCHEMA, "features": [0.1] * 8,
           "incumbent": {"presence": presence, "confidence": conf, "motion_level": level}}
    rec.update(extra)
    return rec


def radar(start, presence, health="ok", simulated=False):
    return {"window_start_ms": start, "timestamp_ms": start + 1000, "health": health,
            "presence": presence, "source": {"verified": True, "simulated": simulated}}


class BuildRows(unittest.TestCase):
    def test_radar_gate_keeps_only_rows_the_radar_confirms(self):
        index = b.RadarIndex([radar(0, True), radar(1000, False), radar(2000, True, health="stale"),
                              radar(5000, True, simulated=True)])
        capture = [cap(500), cap(1500), cap(2500), cap(4000), cap(5500), cap(600, node=2)]
        rows, stats = b.build_rows(capture, room="office", session="s1", split="train",
                                   labels="radar", node=1, condition="still_occupied", radar=index)
        self.assertEqual([r["timestamp_ms"] for r in rows], [500, 5500])
        self.assertEqual(stats["radar_disagrees"], 1)   # radar said empty
        self.assertEqual(stats["no_radar_window"], 2)   # stale window, and a gap
        self.assertEqual(stats["other_node"], 1)
        self.assertTrue(all(r["label_source"] == "radar_gated" for r in rows))
        self.assertEqual(rows[0]["condition"], "still_occupied")
        # A simulated radar makes the row synthetic.
        self.assertEqual([r["synthetic"] for r in rows], [False, True])

    def test_empty_condition_needs_the_radar_to_see_nobody(self):
        index = b.RadarIndex([radar(0, False), radar(1000, True)])
        rows, stats = b.build_rows([cap(500), cap(1500)], room="office", session="s1",
                                   split="heldout", labels="radar", condition="adjacent_only",
                                   radar=index)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["condition"], "adjacent_only")
        self.assertEqual(stats["radar_disagrees"], 1)

    def test_input_activity_reference_and_declared_sessions(self):
        typing = dict(radar(0, True), sensor="input_activity")
        index = b.RadarIndex([typing])
        rows, _ = b.build_rows([cap(500)], room="office", session="s1", split="train",
                               labels="radar", condition="moving_occupied", radar=index)
        self.assertEqual(rows[0]["label_source"], "input_gated")
        # A declared-empty session drops rows where someone was typing.
        rows, stats = b.build_rows([cap(500), cap(5000)], room="office", session="e1",
                                   split="train", labels="declared", condition="quiet_empty",
                                   radar=index)
        self.assertEqual([r["timestamp_ms"] for r in rows], [5000])
        self.assertEqual(rows[0]["label_source"], "operator_declared")
        self.assertEqual(stats["contradicted_by_reference"], 1)

    def test_incumbent_labels_are_marked_as_such(self):
        capture = [cap(1, presence=False, level="absent", conf=0.9), cap(2), cap(3, level="active"),
                   cap(4, presence="yes")]
        rows, stats = b.build_rows(capture, room="office", session="s1", split="train",
                                   labels="incumbent")
        self.assertEqual([r["condition"] for r in rows],
                         ["quiet_empty", "still_occupied", "moving_occupied"])
        self.assertTrue(all(r["label_source"] == "incumbent" for r in rows))
        # The decision itself, never a probability derived from `confidence`.
        self.assertEqual([r["incumbent_probability"] for r in rows], [0.0, 1.0, 1.0])
        self.assertEqual(stats["no_incumbent_label"], 1)

    def test_malformed_capture_rows_are_counted_not_guessed(self):
        capture = [cap(1, feature_schema="other/1"), cap(2, features=[0.1, float("nan")]),
                   cap(3, features=[]), cap("4"), cap(5)]
        rows, stats = b.build_rows(capture, room="office", session="s1", split="train",
                                   labels="incumbent")
        self.assertEqual((len(rows), stats["bad_row"]), (1, 4))

    def test_cli_round_trip_and_required_arguments(self):
        with tempfile.TemporaryDirectory() as d:
            capture, rad, out = (os.path.join(d, n) for n in ("c.jsonl", "r.jsonl", "o.jsonl"))
            with open(capture, "w", encoding="utf-8") as f:
                f.write(json.dumps(cap(500)) + "\nnot json\n" + json.dumps(cap(1500)) + "\n")
            with open(rad, "w", encoding="utf-8") as f:
                f.write(json.dumps(radar(0, True)) + "\n" + json.dumps(radar(1000, True)) + "\n")
            base = ["--capture", capture, "--room", "office", "--session", "s1", "--split", "train",
                    "--out", out]
            self.assertEqual(b.main(base + ["--labels", "radar", "--radar", rad,
                                            "--condition", "moving_occupied"]), 0)
            with open(out, encoding="utf-8") as f:
                rows = [json.loads(line) for line in f]
            self.assertEqual(len(rows), 2)
            self.assertEqual(set(rows[0]), {"room", "session", "split", "condition", "features",
                                            "incumbent_probability", "synthetic", "timestamp_ms",
                                            "label_source"})
            with self.assertRaises(SystemExit):
                b.main(base + ["--labels", "radar"])
            with self.assertRaises(SystemExit):
                b.main(base[:3] + ["bad room"] + base[4:] + ["--labels", "incumbent"])


if __name__ == "__main__":
    unittest.main()
