"""Synthetic fixture contract tests, no physical sensing claims."""
import hashlib
import importlib.util
import struct
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.data import DataError, discover, fit_standardizer, load_manifest


class DataContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.manifest = self.base / "manifest.json"
        self.entries = []
        for i, split in enumerate(("train", "validation", "test")):
            self.write_capture(i, [dict(type="feature", timestamp=t, node_id=1,
                                       features=[i * 10 + t + j / 10 for j in range(8)])
                                   for t in (0, 1, 40)])
            self.entries.append(dict(path=f"{i}.jsonl", sha256=self.sha(i),
                                     captured_at=f"2026-10-{10+i}T00:00:00Z",
                                     room_id=f"room{i}", session_id=f"session{i}",
                                     split=split, evidence="synthetic", use_permitted=True))
        self.save()

    def write_capture(self, i, rows):
        (self.base / f"{i}.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))

    def sha(self, i):
        return hashlib.sha256((self.base / f"{i}.jsonl").read_bytes()).hexdigest()

    def save(self):
        self.manifest.write_text(json.dumps(dict(schema_version=1, files=self.entries)))

    def load(self):
        return load_manifest(self.manifest, allow_synthetic=True)

    def test_load_and_train_only_standardization(self):
        data = self.load()
        stats = fit_standardizer(data.partitions["train"].features)
        np.testing.assert_allclose(stats.transform(data.partitions["train"].features).mean(0), 0, atol=1e-6)
        self.assertGreater(stats.transform(data.partitions["test"].features).mean(), 0.5)
        self.assertEqual(data.partitions["train"].session_ids, ["session0"] * 3)
        self.assertEqual(discover(self.manifest)[0]["session_id"], "session2")
        self.assertNotIn("path", data.inventory[0])

    def test_synthetic_requires_opt_in(self):
        with self.assertRaisesRegex(DataError, "synthetic"):
            load_manifest(self.manifest)

    def test_hash_tampering(self):
        with (self.base / "0.jsonl").open("a") as f:
            f.write("\n")
        with self.assertRaisesRegex(DataError, "hash mismatch"):
            self.load()

    def test_group_leakage(self):
        self.entries[1]["room_id"] = "room0"
        self.save()
        with self.assertRaisesRegex(DataError, "across splits"):
            self.load()
        load_manifest(self.manifest, group_by="session_id", allow_synthetic=True)

    def test_session_leakage_even_when_room_names_differ(self):
        self.entries[1]["session_id"] = "session0"
        self.save()
        with self.assertRaisesRegex(DataError, "across splits"):
            self.load()

    def test_duplicate_hash(self):
        self.entries[1]["sha256"] = self.entries[0]["sha256"]
        self.save()
        with self.assertRaisesRegex(DataError, "duplicate file"):
            self.load()

    def test_copied_frames_with_different_session_ids(self):
        rows = [dict(timestamp=t, node_id=1, features=[t+j/10 for j in range(8)]) for t in (0, 1, 40)]
        self.write_capture(1, rows)
        self.entries[1]["sha256"] = self.sha(1)
        self.save()
        with self.assertRaisesRegex(DataError, "duplicate feature"):
            self.load()

    def test_collector_plural_features_shape(self):
        rows = [dict(type="features", node_id=1, seq=i, timestamp_us=t * 1000000,
                     timestamp=t, features=[t+j/10 for j in range(8)],
                     subcarriers=[t+j/10 for j in range(8)], rssi=0.0, noise_floor=0.0)
                for i, t in enumerate((0, 1, 40))]
        self.write_capture(0, rows)
        self.entries[0]["sha256"] = self.sha(0)
        self.save()
        self.assertEqual(len(self.load().partitions["train"].features), 3)

    def test_actual_collector_recording_envelope(self):
        spec = importlib.util.spec_from_file_location("csi_collector", Path(__file__).resolve().parents[1] / "collect-training-data.py")
        collector = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(collector)
        recorder = collector.CsiRecorder(str(self.base), "synthetic-test")
        recorder.open()
        try:
            for i, timestamp in enumerate((0, 1, 40)):
                packet = struct.pack(collector.FEATURE_PKT_FMT, collector.MAGIC_FEATURES,
                                     1, 0, i, timestamp * 1000000,
                                     *[timestamp+j/10 for j in range(8)])
                recorder.write_frame(collector._parse_feature_packet(packet))
        finally:
            recorder.close()
        self.entries[0]["path"] = recorder.file_path.name
        self.entries[0]["sha256"] = hashlib.sha256(recorder.file_path.read_bytes()).hexdigest()
        self.save()
        partition = self.load().partitions["train"]
        self.assertEqual(partition.timestamps.tolist(), [0, 1, 40])
        self.assertEqual(partition.node_ids, ["1"] * 3)
        np.testing.assert_allclose(partition.features[1], [1+j/10 for j in range(8)], atol=1e-6)

    def test_raw_collector_envelope_rejected(self):
        self.write_capture(0, [dict(timestamp=1, subcarriers=[1]*8, rssi=0,
                                   noise_floor=0, features=dict(node_id=1, seq=0))])
        self.entries[0]["sha256"] = self.sha(0)
        self.save()
        with self.assertRaisesRegex(DataError, "eight explicit features"):
            self.load()

    def test_constant_features_at_different_times_are_not_copied_frames(self):
        for i in range(3):
            self.write_capture(i, [dict(timestamp=i*100+t, node_id=1, features=[0]*8)
                                   for t in (0, 1, 40)])
            self.entries[i]["sha256"] = self.sha(i)
        self.save()
        self.assertEqual(len(self.load().partitions["test"].features), 3)

    def test_skipped_input_lines_are_bounded(self):
        with (self.base / "0.jsonl").open("a") as f:
            f.write('\n' + json.dumps(dict(type="vitals")) + '\n')
        self.entries[0]["sha256"] = self.sha(0)
        self.save()
        with patch("csi_training.data.MAX_INPUT_LINES", 4):
            with self.assertRaisesRegex(DataError, "input line limit"):
                self.load()

    def test_invalid_rows(self):
        cases = [dict(timestamp=1, node_id=1, features=[True] * 8),
                 dict(timestamp=1, node_id=1, features=[float("nan")] * 8),
                 dict(timestamp=1, node_id=1, features=[1] * 7),
                 dict(type="raw_csi", timestamp=1, node_id=1, iq_hex="0011")]
        for row in cases:
            with self.subTest(row=row):
                self.write_capture(0, [row])
                self.entries[0]["sha256"] = self.sha(0)
                self.save()
                with self.assertRaises(DataError):
                    self.load()

    def test_duplicate_observation_same_file(self):
        for conflicting in (False, True):
            with self.subTest(conflicting=conflicting):
                self.write_capture(0, [dict(timestamp=0, node_id=1, features=[0]*8),
                                       dict(timestamp=0, node_id=1, features=[int(conflicting)]*8)])
                self.entries[0]["sha256"] = self.sha(0)
                self.save()
                with self.assertRaisesRegex(DataError, "duplicate session/node/timestamp"):
                    self.load()

    def test_duplicate_observation_across_files_same_session(self):
        self.write_capture(3, [dict(timestamp=0, node_id=1, features=[999]*8)])
        self.entries.append(dict(self.entries[0], path="3.jsonl", sha256=self.sha(3)))
        self.save()
        with self.assertRaisesRegex(DataError, "duplicate session/node/timestamp"):
            self.load()

    def test_independent_nodes_at_same_timestamp_allowed(self):
        self.write_capture(0, [dict(timestamp=0, node_id=node, features=[node]*8)
                               for node in (1, 2, 3)])
        self.entries[0]["sha256"] = self.sha(0)
        self.save()
        self.assertEqual(len(self.load().partitions["train"].features), 3)

    def test_timestamp_order(self):
        self.write_capture(0, [dict(timestamp=t, node_id=1, features=[t]*8) for t in (2, 1)])
        self.entries[0]["sha256"] = self.sha(0)
        self.save()
        with self.assertRaisesRegex(DataError, "monotonic"):
            self.load()

    def test_permissions_and_dates(self):
        for field, value in (("use_permitted", False), ("captured_at", "2026-01-01")):
            original = self.entries[0][field]
            self.entries[0][field] = value
            self.save()
            with self.assertRaises(DataError):
                self.load()
            self.entries[0][field] = original

    def test_symlink_and_nonregular(self):
        (self.base / "link.jsonl").symlink_to(self.base / "0.jsonl")
        self.entries[0]["path"] = "link.jsonl"
        self.save()
        with self.assertRaisesRegex(DataError, "symlink"):
            self.load()
        self.entries[0]["path"] = "."
        self.save()
        with self.assertRaisesRegex(DataError, "regular"):
            self.load()

    def test_missing_partition(self):
        self.entries.pop()
        self.save()
        with self.assertRaisesRegex(DataError, "partition test"):
            self.load()


if __name__ == "__main__":
    unittest.main()
