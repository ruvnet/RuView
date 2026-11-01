"""Synthetic-only boundary, leakage and CPU smoke tests; no hardware claims."""
import io
import copy
import json
import struct
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import numpy as np

import train


class TrainingContractTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="ruview-training-test-")
        self.root = Path(self.temporary.name)
        self.npz = self.root / "input.npz"
        self.meta_path = self.root / "input.json"
        self.frames = np.random.default_rng(12).normal(size=(600, 4)).astype(np.float32)
        self.times = 1_700_000_000 + np.arange(600, dtype=np.float64) / 100
        self.sequence = np.arange(600, dtype=np.uint32)
        self.meta = {"schema_version": 1, "provenance": "synthetic", "source_kind": "rac1",
                     "label_status": "unlabeled", "frame_count": 600, "feature_count": 4,
                     "source_files": [{"sha256": "a" * 64, "size_bytes": 123}], "format": {},
                     "sessions": [{"session_id": "synthetic_1", "start": 0, "end": 600}]}
        self.save()

    def tearDown(self):
        self.temporary.cleanup()

    def save(self, **arrays):
        np.savez(self.npz, frames=arrays.get("frames", self.frames),
                 timestamps=arrays.get("timestamps", self.times), sequence=arrays.get("sequence", self.sequence))
        self.meta["dataset_sha256"] = train.sha256(self.npz.read_bytes())
        self.meta_path.write_text(json.dumps(self.meta), encoding="utf-8")

    def load(self):
        return train.load_dataset(self.npz, self.meta_path)

    def args(self, *extra):
        return train.parser().parse_args(["--dataset", str(self.npz), "--metadata", str(self.meta_path),
            "--output", str(self.root / "output"), "--allow-synthetic", "--window", "8", "--hidden", "4",
            "--epochs", "2", "--batch-size", "16", "--max-train-windows", "32",
            "--max-eval-windows", "16", "--max-seconds", "30", "--threads", "1", *extra])

    def identity_data(self, public=False):
        self.meta.update(provenance="real", label_status="identity_labeled",
                         split_unit="source_file" if public else "acquisition_session",
                         acquisition_session_disjointness="unknown" if public else "verified")
        self.meta["sessions"] = [
            {"session_id": f"recording_{i}", "start": i * 100, "end": (i + 1) * 100,
             "identity": str(i % 2), "split": train.SPLITS[i // 2]}
            for i in range(6)]
        self.meta["source_files"] = []
        for i, s in enumerate(self.meta["sessions"]):
            digest = train.sha256(f"synthetic-fixture-{i}".encode())
            s.update(source_sha256=digest, acquisition_session_id=f"acquisition-{i}")
            self.meta["source_files"].append({"sha256": digest, "size_bytes": 123})
        if public:
            self.meta.update(source_kind="ntu_fi_humanid", label_origin="published_dataset", time_unit="sample_index",
                             split_protocol="archive_directory_custom")
            self.times = np.tile(np.arange(1, 101, dtype=np.float64), 6)
            self.meta["source_files"] = []
            for i, s in enumerate(self.meta["sessions"]):
                digest = train.sha256(f"synthetic-fixture-{i}".encode())
                s.update(source_sha256=digest, source_split="test" if s["split"] == "test" else "train")
                self.meta["source_files"].append({"sha256": digest, "size_bytes": 123})
        self.save()
        return self.load()

    def test_numeric_load(self):
        result = self.load()
        self.assertEqual(result["frames"].shape, (600, 4))
        self.assertEqual(result["metadata"]["label_status"], "unlabeled")

    def test_wrong_hash_rejected(self):
        self.meta["dataset_sha256"] = "0" * 64
        self.meta_path.write_text(json.dumps(self.meta))
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            self.load()

    def test_object_pickle_rejected_before_load(self):
        self.save(frames=np.empty((600, 4), dtype=object))
        with self.assertRaisesRegex(ValueError, "numeric dtype"):
            self.load()

    def test_unexpected_dtype_and_dimensions(self):
        for value in (self.frames.astype(np.float64), self.frames[:, :2], self.frames.reshape(600, 2, 2)):
            self.save(frames=value)
            with self.assertRaisesRegex(ValueError, "shape or numeric dtype"):
                self.load()

    def test_nonfinite_features(self):
        for value in (np.nan, np.inf, 1e7):
            self.frames[0, 0] = value
            self.save()
            with self.assertRaisesRegex(ValueError, "frame values"):
                self.load()

    def test_timestamp_regression_and_duplicate(self):
        for delta in (0, -1):
            self.times[5] = self.times[4] + delta
            self.save()
            with self.assertRaisesRegex(ValueError, "timestamps must increase"):
                self.load()

    def test_duplicate_json_key(self):
        self.meta_path.write_text('{"schema_version":1,"schema_version":1}')
        with self.assertRaisesRegex(ValueError, "duplicate JSON"):
            self.load()

    def test_npy_shape_allocation_bomb(self):
        header = io.BytesIO()
        np.lib.format.write_array_header_1_0(header, {"descr": "<f4", "fortran_order": False,
                                                     "shape": (10**12, 4)})
        good = self.npz.read_bytes()
        with zipfile.ZipFile(io.BytesIO(good)) as old:
            members = {name: old.read(name) for name in old.namelist()}
        members["frames.npy"] = header.getvalue()
        with zipfile.ZipFile(self.npz, "w") as out:
            for name, raw in members.items():
                out.writestr(name, raw)
        self.meta["dataset_sha256"] = train.sha256(self.npz.read_bytes())
        self.meta_path.write_text(json.dumps(self.meta))
        with self.assertRaisesRegex(ValueError, "shape or numeric dtype"):
            self.load()

    def malicious_zip(self, change):
        raw = bytearray(self.npz.read_bytes())
        change(raw)
        self.npz.write_bytes(raw)
        self.meta["dataset_sha256"] = train.sha256(raw)
        self.meta_path.write_text(json.dumps(self.meta))

    def test_zip_large_count_and_size_rejected_before_zipfile(self):
        for change in (lambda raw: struct.pack_into("<HH", raw, len(raw) - 14, 60000, 60000),
                       lambda raw: struct.pack_into("<I", raw, len(raw) - 10, 100_000_000)):
            self.save()
            self.malicious_zip(change)
            with patch.object(train.zipfile, "ZipFile", side_effect=AssertionError("constructor must not run")):
                with self.assertRaisesRegex(ValueError, "entry count|byte budget"):
                    self.load()

    def test_zip_count_lie_rejected_before_zipfile(self):
        # EOCD claims three entries, but the physical central directory has four.
        self.save()
        with zipfile.ZipFile(self.npz, "a") as archive:
            archive.writestr("extra.npy", b"x")
        self.malicious_zip(lambda raw: struct.pack_into("<HH", raw, len(raw) - 14, 3, 3))
        with patch.object(train.zipfile, "ZipFile", side_effect=AssertionError("constructor must not run")):
            with self.assertRaisesRegex(ValueError, "count/size disagreement"):
                self.load()

    def test_archive_zip64_and_multidisk_fail_closed_before_zipfile(self):
        def locator(raw):
            raw[-22:-22] = struct.pack("<4sIQI", b"PK\x06\x07", 0, 0, 1)
        changes = [lambda raw: struct.pack_into("<HH", raw, len(raw) - 14, 0xffff, 0xffff),
                   lambda raw: struct.pack_into("<I", raw, len(raw) - 10, 0xffffffff),
                   lambda raw: struct.pack_into("<H", raw, len(raw) - 18, 1), locator]
        for change in changes:
            self.save()
            self.malicious_zip(change)
            with patch.object(train.zipfile, "ZipFile", side_effect=AssertionError("constructor must not run")):
                with self.assertRaisesRegex(ValueError, "ZIP64|disk"):
                    self.load()

    def test_zip64_central_size_cannot_override_preflight(self):
        def change(raw):
            offset = struct.unpack_from("<I", raw, len(raw) - 6)[0]
            struct.pack_into("<I", raw, offset + 24, 0xffffffff)
        self.malicious_zip(change)
        with patch.object(train.zipfile, "ZipFile", side_effect=AssertionError("constructor must not run")):
            with self.assertRaisesRegex(ValueError, "ZIP64 central"):
                self.load()

    def test_normal_npz_zip64_local_headers_and_compression_supported(self):
        for compressed in (False, True):
            writer = np.savez_compressed if compressed else np.savez
            writer(self.npz, frames=self.frames, timestamps=self.times, sequence=self.sequence)
            raw = self.npz.read_bytes()
            self.assertEqual(struct.unpack_from("<II", raw, 18), (0xffffffff, 0xffffffff))
            self.meta["dataset_sha256"] = train.sha256(raw)
            self.meta_path.write_text(json.dumps(self.meta))
            np.testing.assert_array_equal(self.load()["frames"], self.frames)

    def test_checkpoint_preflight_precedes_zipfile_allocation(self):
        checkpoint = self.root / "checkpoint.npz"
        mean, scale = np.zeros(4), np.ones(4)
        np.savez(checkpoint, normalization_mean=mean, normalization_scale=scale)
        raw = checkpoint.read_bytes()
        self.assertEqual(train.load_checkpoint(checkpoint, {}, train.sha256(raw), mean, scale, None), {})
        bad = bytearray(raw)
        struct.pack_into("<HH", bad, len(bad) - 14, 60000, 60000)
        checkpoint.write_bytes(bad)
        with patch.object(train.zipfile, "ZipFile", side_effect=AssertionError("constructor must not run")):
            with self.assertRaisesRegex(ValueError, "entry count"):
                train.load_checkpoint(checkpoint, {}, train.sha256(bad), mean, scale, None)

    def test_sessions_cover_all_rows_without_overlap(self):
        self.meta["sessions"].append({"session_id": "duplicate", "start": 100, "end": 600})
        self.save()
        with self.assertRaisesRegex(ValueError, "without overlap"):
            self.load()

    def test_purged_windows_have_no_shared_rows(self):
        splits = train.build_splits(self.load(), 8, 8)
        occupied = {}
        for name, targets in splits["targets"].items():
            occupied[name] = {int(i) for t in targets for i in range(t - 8, t + 1)}
        self.assertFalse(occupied["train"] & occupied["validation"])
        self.assertFalse(occupied["validation"] & occupied["test"])
        self.assertGreaterEqual(min(occupied["validation"]) - max(occupied["train"]) - 1, 8)
        self.assertGreaterEqual(min(occupied["test"]) - max(occupied["validation"]) - 1, 8)

    def test_purge_less_than_window_rejected(self):
        with self.assertRaisesRegex(ValueError, "purge"):
            train.build_splits(self.load(), 8, 7)

    def test_gap_excludes_all_crossing_windows(self):
        self.times[100:] += 5
        self.save()
        targets = train.build_splits(self.load(), 8, 8)["targets"]["train"]
        self.assertFalse(any(100 <= t < 108 for t in targets))
        self.assertIn(108, targets)

    def test_normalization_ignores_validation_and_test(self):
        dataset = self.load()
        splits = train.build_splits(dataset, 8, 8)
        ranges = splits["intervals"]["train"]
        mean, scale = train.fit_normalization(dataset["frames"], ranges)
        changed = dataset["frames"].copy()
        changed[360:] = 900000
        other_mean, other_scale = train.fit_normalization(changed, ranges)
        np.testing.assert_array_equal(mean, other_mean)
        np.testing.assert_array_equal(scale, other_scale)

    def test_demean_is_per_recording_and_does_not_mutate_input(self):
        data = self.identity_data(public=True)
        original = data["frames"].copy()
        changed, receipt = train.transform_recordings(data, "demean", "original", 1729, "identity")
        for record in data["metadata"]["sessions"]:
            block = changed[record["start"]:record["end"]]
            np.testing.assert_allclose(block.mean(axis=0, dtype=np.float64), 0, atol=1e-7)
        np.testing.assert_array_equal(original, data["frames"])
        self.assertEqual(changed.dtype, np.float32)
        self.assertFalse(receipt["labels_used"])
        self.assertFalse(receipt["forecasting_supported"])
        raw, _ = train.transform_recordings(data, "raw", "original", 1729, "identity")
        np.testing.assert_array_equal(raw, original)

    def test_shuffle_preserves_rows_distribution_and_admin_coordinates(self):
        data = self.identity_data(public=True)
        original = data["frames"].copy()
        times, sequences = data["timestamps"].copy(), data["sequence"].copy()
        shuffled, receipt = train.transform_recordings(data, "raw", "shuffle", 1729, "identity")
        again, same = train.transform_recordings(data, "raw", "shuffle", 1729, "identity")
        other, different = train.transform_recordings(data, "raw", "shuffle", 1730, "identity")
        for record in data["metadata"]["sessions"]:
            a, b = record["start"], record["end"]
            self.assertEqual(sorted(map(tuple, original[a:b])), sorted(map(tuple, shuffled[a:b])))
        np.testing.assert_array_equal(shuffled, again)
        self.assertEqual(receipt["permutation_sha256"], same["permutation_sha256"])
        self.assertNotEqual(receipt["permutation_sha256"], different["permutation_sha256"])
        self.assertFalse(np.array_equal(shuffled, original))
        self.assertFalse(np.array_equal(shuffled, other))
        np.testing.assert_array_equal(data["frames"], original)
        np.testing.assert_array_equal(data["timestamps"], times)
        np.testing.assert_array_equal(data["sequence"], sequences)
        self.assertIn("chronology_destroyed", receipt["timestamp_sequence_semantics"])

    def test_controls_never_use_identity_labels(self):
        data = self.identity_data(public=True)
        relabeled = copy.deepcopy(data)
        for record in relabeled["metadata"]["sessions"]:
            record["identity"] = "1" if record["identity"] == "0" else "0"
        first, info = train.transform_recordings(data, "demean", "shuffle", 1729, "identity")
        second, other = train.transform_recordings(relabeled, "demean", "shuffle", 1729, "identity")
        np.testing.assert_array_equal(first, second)
        self.assertEqual(info, other)

    def test_controls_do_not_mix_partitions_or_fit_heldout_statistics(self):
        data = self.identity_data(public=True)
        altered = copy.deepcopy(data)
        altered["frames"][400:] = altered["frames"][400:] * 100 + 1000
        first, _ = train.transform_recordings(data, "demean", "shuffle", 1729, "identity")
        second, _ = train.transform_recordings(altered, "demean", "shuffle", 1729, "identity")
        np.testing.assert_array_equal(first[:400], second[:400])
        split = train.build_splits(data, 8, 8, "identity", identity_protocol="published_files")
        other = train.build_splits(altered, 8, 8, "identity", identity_protocol="published_files")
        for name in train.SPLITS:
            np.testing.assert_array_equal(split["targets"][name], other["targets"][name])
        mean, scale = train.fit_normalization(first, split["intervals"]["train"])
        other_mean, other_scale = train.fit_normalization(second, split["intervals"]["train"])
        np.testing.assert_array_equal(mean, other_mean)
        np.testing.assert_array_equal(scale, other_scale)

    def test_recording_controls_reject_forecasting_and_private_csi(self):
        data = self.load()
        for representation, order in (("demean", "original"), ("raw", "shuffle")):
            with self.assertRaisesRegex(ValueError, "public NTU custom identity"):
                train.transform_recordings(data, representation, order, 1729, "pretrain")
        data = self.identity_data(public=True)
        with self.assertRaisesRegex(ValueError, "public NTU custom identity"):
            train.transform_recordings(data, "demean", "shuffle", 1729, "pretrain")

    def test_identity_never_invents_labels(self):
        with self.assertRaisesRegex(ValueError, "explicitly labeled"):
            train.build_splits(self.load(), 8, 8, "identity")

    def test_identity_requires_real_provenance(self):
        data = self.identity_data()
        data["metadata"]["provenance"] = "synthetic"
        with self.assertRaisesRegex(ValueError, "requires real"):
            train.build_splits(data, 8, 8, "identity")

    def test_identity_requires_two_people_and_each_split(self):
        data = self.identity_data()
        for session in data["metadata"]["sessions"]:
            session["identity"] = "one"
        with self.assertRaisesRegex(ValueError, "2..64"):
            train.build_splits(data, 8, 8, "identity")
        data = self.identity_data()
        data["metadata"]["sessions"][-1]["identity"] = "0"
        with self.assertRaisesRegex(ValueError, "separate session"):
            train.build_splits(data, 8, 8, "identity")

    def test_unknown_sessions_cannot_claim_strict_protocol(self):
        data = self.identity_data(public=True)
        with self.assertRaisesRegex(ValueError, "verified acquisition"):
            train.build_splits(data, 8, 8, "identity")

    def test_strict_sessions_need_linked_hashes_and_no_shared_acquisition(self):
        data = self.identity_data()
        data["metadata"]["sessions"][-1]["source_sha256"] = data["metadata"]["sessions"][0]["source_sha256"]
        with self.assertRaisesRegex(ValueError, "recording hashes"):
            train.build_splits(data, 8, 8, "identity")
        data = self.identity_data()
        data["metadata"]["sessions"][-1]["acquisition_session_id"] = "acquisition-0"
        with self.assertRaisesRegex(ValueError, "acquisition session crosses"):
            train.build_splits(data, 8, 8, "identity")
        data = self.identity_data()
        del data["metadata"]["sessions"][-1]["source_sha256"]
        with self.assertRaisesRegex(ValueError, "recording hashes"):
            train.build_splits(data, 8, 8, "identity")

    def test_pretrain_cannot_consume_declared_holdout(self):
        for public in (False, True):
            with self.assertRaisesRegex(ValueError, "must not repartition"):
                train.build_splits(self.identity_data(public=public), 8, 8)

    def test_public_split_keeps_declared_test_separate(self):
        data = self.identity_data(public=True)
        split = train.build_splits(data, 8, 8, "identity", identity_protocol="published_files", stride=8)
        self.assertTrue(all(t >= 400 for t in split["targets"]["test"]))
        data["metadata"]["sessions"][-1]["split"] = "validation"
        with self.assertRaisesRegex(ValueError, "declared source test"):
            train.build_splits(data, 8, 8, "identity", identity_protocol="published_files")

    def test_public_requires_explicit_custom_protocol(self):
        self.identity_data(public=True)
        del self.meta["split_protocol"]
        self.save()
        with self.assertRaisesRegex(ValueError, "archive_directory_custom"):
            self.load()

    def test_public_split_rejects_reused_source_files(self):
        data = self.identity_data(public=True)
        data["metadata"]["sessions"][-1]["source_sha256"] = data["metadata"]["sessions"][0]["source_sha256"]
        with self.assertRaisesRegex(ValueError, "hashes must be unique"):
            train.build_splits(data, 8, 8, "identity", identity_protocol="published_files")

    def test_hard_limits_and_nonfinite_options(self):
        for option, value in (("--epochs", "1001"), ("--max-seconds", "nan"), ("--max-seconds", "43201"),
                              ("--hidden", "129"), ("--batch-size", "129"), ("--stride", "0")):
            args = self.args(option, value)
            args.purge = args.window
            with self.assertRaises(ValueError):
                train.validate_config(args, 4)

    def test_git_output_refused(self):
        repo = self.root / "repository"
        repo.mkdir()
        (repo / ".git").write_text("gitdir: elsewhere")
        with self.assertRaisesRegex(ValueError, "outside Git"):
            train.outside_git(repo / "private-data")

    def test_pretrain_cpu_smoke_leaves_test_unread(self):
        result = train.run(self.args())
        self.assertEqual(result["status"], "COMPLETED")
        self.assertIsNone(result["test"])
        self.assertFalse(result["identity_validated"])
        self.assertFalse(result["test_used_for_selection"])
        self.assertEqual(result["evidence"], "SYNTHETIC")
        self.assertEqual(result["model_architecture"]["encoder"], "lstm")
        self.assertEqual(result["parameters"], 4 * 4 * (4 + 4 + 2) + 4 * 4 + 4)
        self.assertIn("persistence_normalized_mse", result["history"][-1]["validation"])
        self.assertEqual(result["latest_validation"]["loss"], result["history"][-1]["validation"]["loss"])
        checkpoint = self.root / "output" / "checkpoint.npz"
        self.assertEqual(result["checkpoint_sha256"], train.sha256(checkpoint.read_bytes()))
        with np.load(checkpoint, allow_pickle=False) as saved:
            self.assertIn("normalization_mean", saved.files)
            self.assertTrue(all(not saved[name].dtype.hasobject for name in saved.files))

    def test_reproducible_cpu_checkpoint_and_explicit_test(self):
        first = train.run(self.args("--evaluate-test"))
        args = self.args("--evaluate-test")
        args.output = str(self.root / "second")
        second = train.run(args)
        self.assertEqual(first["test"], second["test"])
        self.assertEqual(first["history"], second["history"])
        with np.load(self.root / "output" / "checkpoint.npz") as a, np.load(self.root / "second" / "checkpoint.npz") as b:
            for name in a.files:
                np.testing.assert_array_equal(a[name], b[name])

    def test_identity_published_file_cpu_smoke(self):
        self.identity_data(public=True)  # Contract fixture only; never used as measured evidence.
        result = train.run(self.args("--mode", "identity", "--identity-protocol", "published_files", "--stride", "8"))
        self.assertIsNone(result["test"])
        self.assertEqual(result["acquisition_session_disjointness"], "unknown")
        self.assertIn("majority_baseline_accuracy", result["history"][-1]["validation"])
        self.assertEqual(result["history"][-1]["validation"]["recordings"], 2)
        self.assertEqual(result["history"][-1]["validation"]["recording_majority_baseline_accuracy"], 0.5)
        self.assertEqual(result["history"][-1]["validation"]["recordings_skipped_by_window_cap"], 0)
        self.assertNotIn("confusion_matrix", result["history"][-1]["validation"])
        self.assertIn("confusion_matrix", result["best_validation"])
        self.assertIn("recording_confusion_matrix", result["latest_validation"])
        self.assertFalse(result["identity_validated"])

    def test_frozen_checkpoint_scores_without_optimizer_and_refuses_repeat(self):
        training = train.run(self.args())
        original = (self.root / "output" / "checkpoint.npz").read_bytes()
        args = self.args("--evaluate-checkpoint", str(self.root / "output"),
                         "--output", str(self.root / "evaluation"))
        with patch("torch.optim.Adam", side_effect=AssertionError("must not train during evaluation")):
            evaluation = train.run(args)
        self.assertEqual(evaluation["run_kind"], "frozen_checkpoint_evaluation")
        self.assertEqual(evaluation["steps"], 0)
        self.assertIsNotNone(evaluation["test"])
        self.assertEqual(evaluation["checkpoint_sha256"], training["checkpoint_sha256"])
        self.assertEqual(original, (self.root / "output" / "checkpoint.npz").read_bytes())
        args.output = str(self.root / "second_evaluation")
        with self.assertRaises(FileExistsError):
            train.run(args)

    def test_frozen_checkpoint_hash_mismatch_fails_before_holdout(self):
        train.run(self.args())
        with (self.root / "output" / "checkpoint.npz").open("ab") as stream:
            stream.write(b"changed")
        args = self.args("--evaluate-checkpoint", str(self.root / "output"), "--output", str(self.root / "evaluation"))
        with self.assertRaisesRegex(ValueError, "checkpoint hash mismatch"):
            train.run(args)
        self.assertFalse((self.root / "output" / "holdout-evaluation.json").exists())

    def test_frozen_incomplete_config_cannot_change_protocol(self):
        train.run(self.args())
        path = self.root / "output" / "metrics.json"
        receipt = json.loads(path.read_text())
        del receipt["config"]["window"]
        path.write_text(json.dumps(receipt))
        args = self.args("--evaluate-checkpoint", str(self.root / "output"), "--output", str(self.root / "evaluation"))
        with self.assertRaisesRegex(ValueError, "incomplete or unknown"):
            train.run(args)

    def test_gru_cpu_train_and_frozen_checkpoint_load(self):
        trained = train.run(self.args("--encoder", "gru"))
        self.assertIsNone(trained["test"])
        self.assertEqual(trained["parameters"], 3 * 4 * (4 + 4 + 2) + 4 * 4 + 4)
        self.assertEqual(trained["model_architecture"]["encoder"], "gru")
        evaluated = train.run(self.args("--encoder", "lstm", "--evaluate-checkpoint", str(self.root / "output"),
                                       "--output", str(self.root / "evaluation")))
        self.assertEqual(evaluated["model_architecture"]["encoder"], "gru")
        self.assertEqual(evaluated["parameters"], trained["parameters"])
        self.assertEqual(evaluated["steps"], 0)
        self.assertFalse(evaluated["legacy_encoder_default_applied"])

    def test_gru_identity_validation_only(self):
        self.identity_data(public=True)
        result = train.run(self.args("--encoder", "gru", "--mode", "identity",
                                     "--identity-protocol", "published_files", "--stride", "8"))
        self.assertIsNone(result["test"])
        self.assertEqual(result["model_architecture"]["encoder"], "gru")
        self.assertEqual(result["model_architecture"]["output_features"], 2)
        self.assertIn("recording_accuracy", result["best_validation"])

    def test_recognized_legacy_defaults_only_to_lstm(self):
        train.run(self.args())
        path = self.root / "output" / "metrics.json"
        record = json.loads(path.read_text())
        del record["config"]["encoder"]
        del record["config"]["representation"]
        del record["config"]["temporal_order"]
        del record["model_architecture"]
        del record["input_controls"]
        record["trainer_sha256"] = "0b94c5f05b7004fcf54de0888ae1072af6148932e3198bbffa0188e0d16eca0e"
        path.write_text(json.dumps(record))
        evaluated = train.run(self.args("--encoder", "gru", "--evaluate-checkpoint", str(self.root / "output"),
                                       "--output", str(self.root / "evaluation")))
        self.assertEqual(evaluated["model_architecture"]["encoder"], "lstm")
        self.assertTrue(evaluated["legacy_encoder_default_applied"])
        self.assertTrue(evaluated["legacy_controls_defaults_applied"])

    def test_frozen_controls_restore_demean_and_shuffle(self):
        self.identity_data(public=True)
        trained = train.run(self.args("--mode", "identity", "--identity-protocol", "published_files",
                                      "--representation", "demean", "--temporal-order", "shuffle"))
        self.assertIsNone(trained["test"])
        evaluated = train.run(self.args("--evaluate-checkpoint", str(self.root / "output"),
                                       "--output", str(self.root / "evaluation")))
        self.assertEqual(evaluated["config"]["representation"], "demean")
        self.assertEqual(evaluated["config"]["temporal_order"], "shuffle")
        self.assertEqual(evaluated["input_controls"], trained["input_controls"])
        self.assertEqual(evaluated["steps"], 0)
        self.assertFalse(evaluated["legacy_controls_defaults_applied"])

    def test_f6a_legacy_controls_restore_raw_original(self):
        self.identity_data(public=True)
        train.run(self.args("--mode", "identity", "--identity-protocol", "published_files", "--encoder", "gru"))
        path = self.root / "output" / "metrics.json"
        record = json.loads(path.read_text())
        for name in ("representation", "temporal_order"):
            del record["config"][name]
        del record["input_controls"]
        record["trainer_sha256"] = "f6a352e9dd91875edfb8ff61f0e323c2cfc4405fdffd02a37b452e3e3923c007"
        path.write_text(json.dumps(record))
        evaluated = train.run(self.args("--representation", "demean", "--temporal-order", "shuffle",
                                       "--evaluate-checkpoint", str(self.root / "output"),
                                       "--output", str(self.root / "evaluation")))
        self.assertEqual(evaluated["config"]["representation"], "raw")
        self.assertEqual(evaluated["config"]["temporal_order"], "original")
        self.assertEqual(evaluated["model_architecture"]["encoder"], "gru")
        self.assertTrue(evaluated["legacy_controls_defaults_applied"])
        self.assertFalse(evaluated["legacy_encoder_default_applied"])

    def test_frozen_control_receipt_mismatch_fails_before_holdout(self):
        self.identity_data(public=True)
        train.run(self.args("--mode", "identity", "--identity-protocol", "published_files"))
        path = self.root / "output" / "metrics.json"
        record = json.loads(path.read_text())
        record["input_controls"]["temporal_order"] = "shuffle"
        path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "frozen input controls mismatch"):
            train.run(self.args("--evaluate-checkpoint", str(self.root / "output"),
                                "--output", str(self.root / "evaluation")))
        self.assertFalse((self.root / "output" / "holdout-evaluation.json").exists())

    def test_unknown_legacy_cannot_omit_encoder(self):
        train.run(self.args())
        path = self.root / "output" / "metrics.json"
        record = json.loads(path.read_text())
        del record["config"]["encoder"]
        record["trainer_sha256"] = "c" * 64
        path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "recognized legacy"):
            train.run(self.args("--evaluate-checkpoint", str(self.root / "output"),
                                "--output", str(self.root / "evaluation")))

    def test_frozen_encoder_and_tensor_shapes_must_match(self):
        train.run(self.args())
        path = self.root / "output" / "metrics.json"
        record = json.loads(path.read_text())
        record["config"]["encoder"] = "gru"
        path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "architecture mismatch"):
            train.run(self.args("--evaluate-checkpoint", str(self.root / "output"),
                                "--output", str(self.root / "evaluation")))
        record["model_architecture"]["encoder"] = "gru"
        path.write_text(json.dumps(record))
        with self.assertRaisesRegex(ValueError, "tensor shape/dtype mismatch"):
            train.run(self.args("--evaluate-checkpoint", str(self.root / "output"),
                                "--output", str(self.root / "evaluation")))
        self.assertFalse((self.root / "output" / "holdout-evaluation.json").exists())

    def test_recordings_each_get_one_vote_despite_unequal_windows(self):
        self.identity_data(public=True)
        # Two test files, but the first has only two eligible windows.
        self.meta["sessions"][4]["end"] = 410
        self.meta["sessions"][5]["start"] = 410
        self.times = 1_700_000_000 + np.arange(600, dtype=np.float64) / 100
        self.save()
        train.run(self.args("--mode", "identity", "--identity-protocol", "published_files"))
        checkpoint = self.root / "output" / "checkpoint.npz"
        with np.load(checkpoint, allow_pickle=False) as loaded:
            values = {name: loaded[name].copy() for name in loaded.files}
        values["head.weight"][:] = 0
        values["head.bias"][:] = [10, -10]  # Always predict first class.
        np.savez(checkpoint, **values)
        metrics = self.root / "output" / "metrics.json"
        receipt = json.loads(metrics.read_text())
        receipt["checkpoint_sha256"] = train.sha256(checkpoint.read_bytes())
        metrics.write_text(json.dumps(receipt))
        result = train.run(self.args("--evaluate-checkpoint", str(self.root / "output"),
                                     "--output", str(self.root / "evaluation")))
        self.assertEqual(result["test"]["recordings"], 2)
        self.assertEqual(result["test"]["recording_accuracy"], 0.5)
        self.assertLess(result["test"]["accuracy"], 0.5)
        self.assertLess(result["test"]["minimum_windows_per_evaluated_recording"],
                        result["test"]["maximum_windows_per_evaluated_recording"])

    def test_prepare_public_to_trainer_schema(self):
        import prepare
        items = []
        for i in range(6):
            source = self.root / f"source-{i}.mat"
            source.write_bytes(f"synthetic-MAT-stub-{i}".encode())
            items.append({"path": source, "identity": str(i % 2), "source_split": "test" if i >= 4 else "train",
                          "source_folder": "test_amp" if i >= 4 else "train_amp",
                          "split": train.SPLITS[i // 2], "sha256": prepare.sha_file(source), "size_bytes": source.stat().st_size})
        with patch.object(prepare, "public_inventory", return_value=items), \
             patch.object(prepare, "load_public_mat", side_effect=[np.full((500, 342), i + 1, dtype=np.float32) for i in range(6)]):
            prepare.prepare_public(self.root, self.root / "prepared")
        data = train.load_dataset(self.root / "prepared" / "dataset-000.npz", self.root / "prepared" / "dataset-000.json")
        splits = train.build_splits(data, 32, 32, "identity", identity_protocol="published_files", stride=8)
        self.assertEqual(data["frames"].shape, (3000, 342))
        self.assertEqual(splits["classes"], ["0", "1"])
        self.assertTrue(all(t >= 2000 for t in splits["targets"]["test"]))

    def test_max_epochs_and_classes_fit_receipt_reader_budget(self):
        matrix = [[50000] * 64 for _ in range(64)]
        validation = {"loss": 0.25, "accuracy": 0.9, "recording_accuracy": 0.8,
                      "windows": 50000, "recordings": 1024, "confusion_matrix": matrix,
                      "recording_confusion_matrix": matrix}
        history = [train.validation_history_entry(i, 0.1, 200000, validation) for i in range(1, 1001)]
        receipt = {"history": history, "best_validation": validation, "latest_validation": validation}
        path = self.root / "long-receipt.json"
        train.write_json(path, receipt)
        self.assertLessEqual(path.stat().st_size, train.MAX_METRICS_BYTES)
        reloaded = json.loads(train.read_bounded(path, train.MAX_METRICS_BYTES))
        self.assertEqual(len(reloaded["history"]), 1000)
        self.assertEqual(reloaded["best_validation"]["confusion_matrix"], matrix)
        self.assertNotIn("recording_confusion_matrix", reloaded["history"][-1]["validation"])

    def test_receipt_writer_matches_reader_bound_and_preserves_previous(self):
        import run_batch
        self.assertEqual(train.MAX_METRICS_BYTES, run_batch.MAX_METRICS_BYTES)
        path = self.root / "receipt.json"
        train.write_json(path, {"previous": True})
        before = path.read_bytes()
        with self.assertRaisesRegex(ValueError, "reader byte budget"):
            train.write_json(path, {"oversized": "x" * train.MAX_METRICS_BYTES})
        self.assertEqual(path.read_bytes(), before)

    def test_step_cap_honored(self):
        result = train.run(self.args("--epochs", "10", "--max-steps", "1"))
        self.assertEqual(result["steps"], 1)
        self.assertEqual(result["stop_reason"], "step_limit")

    def test_existing_output_never_overwritten(self):
        (self.root / "output").mkdir()
        with self.assertRaisesRegex(ValueError, "new private"):
            train.run(self.args())


if __name__ == "__main__":
    unittest.main()
