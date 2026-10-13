"""Synthetic integrity tests, not RF sensing accuracy evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np
import torch
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_train import AmplitudeBundle, AmplitudeClassifier, optimize, optimize_bundle, scores


def fixture():
    rng = np.random.default_rng(7)
    partitions = {}
    for split in ('train', 'validation', 'test'):
        y = np.tile(np.arange(2), 8).astype(np.int64)
        x = rng.normal(size=(len(y), 4, 16)).astype(np.float32)
        x += y[:, None, None] * 2
        partitions[split] = {'x': x, 'y': y}
    return partitions, {'classes': ['class_0', 'class_1'], 'evidence': 'SYNTHETIC'}


class TrainerTests(unittest.TestCase):
    def test_all_models_shape(self):
        for variant in ('statistics', 'temporal16', 'temporal32'):
            model = AmplitudeClassifier(4, 2, variant)
            self.assertEqual(model(torch.zeros(2, 4, 16)).shape, (2, 2))

    def test_macro_f1_includes_missing_predictions(self):
        value = scores(np.array([0, 1]), np.array([0, 0]), 2)
        self.assertAlmostEqual(value['macro_f1'], 1 / 3)

    def test_bundle_normalization_reload_and_corruption(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'run'
            report = optimize(parts, provenance, path, epochs=2)
            prep = json.loads((path / 'preprocessing.json').read_text())
            np.testing.assert_allclose(prep['mean'], parts['train']['x'].mean(axis=(0, 2)), rtol=1e-6)
            bundle = AmplitudeBundle(path)
            a = bundle.predict(parts['test']['x'])
            b = AmplitudeBundle(path).predict(parts['test']['x'])
            np.testing.assert_array_equal(a, b)
            self.assertEqual(report['test'], scores(parts['test']['y'], a.argmax(1), 2))
            self.assertFalse(report['promotionAuthorized'])
            self.assertLessEqual(report['fused_max_abs_logit_error'], 1e-4)
            with open(path / 'model.safetensors', 'ab') as handle:
                handle.write(b'bad')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                AmplitudeBundle(path)

    def test_test_values_cannot_select_model_or_fit_scaler(self):
        parts, provenance = fixture()
        changed = copy.deepcopy(parts)
        changed['test']['x'] += 100
        changed['test']['y'] = 1 - changed['test']['y']
        with tempfile.TemporaryDirectory() as tmp:
            a = optimize(parts, provenance, Path(tmp) / 'a', epochs=2)
            b = optimize(changed, provenance, Path(tmp) / 'b', epochs=2)
            self.assertEqual(a['variant'], b['variant'])
            self.assertEqual(a['model_sha256'], b['model_sha256'])
            self.assertEqual(a['preprocessing_sha256'], b['preprocessing_sha256'])

    def test_temporal_fused_bundle(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'run'
            report = optimize(parts, provenance, path, epochs=1, variants=('temporal16',))
            self.assertTrue(report['fused'])
            self.assertEqual(AmplitudeBundle(path).predict(parts['test']['x']).shape, (16, 2))

    def test_folded_normalization_equivalence(self):
        parts, provenance = fixture()
        # Different channel offsets/scales exercise the affine fold.
        for part in parts.values():
            part['x'] = part['x'] * np.array([1, 2, 0.5, 4], dtype=np.float32)[None, :, None] + 20
        with tempfile.TemporaryDirectory() as tmp:
            source, target = Path(tmp) / 'source', Path(tmp) / 'folded'
            original = optimize(parts, provenance, source, epochs=2, variants=('statistics',))
            report = optimize_bundle(source, parts['validation']['x'], target, repetitions=50)
            self.assertTrue(report['preprocessing_folded'])
            self.assertNotIn('test', report)
            self.assertEqual(report['parent_model_sha256'], original['model_sha256'])
            a = AmplitudeBundle(source).predict(parts['validation']['x'])
            b = AmplitudeBundle(target).predict(parts['validation']['x'])
            np.testing.assert_allclose(a, b, rtol=1e-4, atol=1e-4)
            np.testing.assert_array_equal(a.argmax(1), b.argmax(1))
            self.assertGreater(report['optimization']['p50_speedup'], 0)
            with self.assertRaisesRegex(ValueError, 'unfused'):
                optimize_bundle(target, parts['validation']['x'], Path(tmp) / 'second')

    def test_nonfinite_and_incomplete_rejected(self):
        parts, provenance = fixture()
        parts['train']['x'][0, 0, 0] = np.nan
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                optimize(parts, provenance, Path(tmp) / 'run', epochs=1)
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'run'
            optimize(parts, provenance, path, epochs=1, variants=('statistics',))
            (path / 'status.json').write_text('{"status":"running","promotionAuthorized":false}')
            with self.assertRaisesRegex(ValueError, 'complete'):
                AmplitudeBundle(path)


if __name__ == '__main__':
    unittest.main()
