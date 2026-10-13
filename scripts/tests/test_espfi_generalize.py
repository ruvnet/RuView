"""Synthetic contract checks. These are not sensing performance evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_generalize import (
    FAMILIES, GeneralizationBundle, compare_development, features, fit_ridge, folds, optimize_export)
from csi_training.espfi_train import optimize


def fixture():
    rng = np.random.default_rng(19)
    parts = {}
    for split, identities in (
        ('train', [(r, s, a, 1) for r in (1, 2) for s in range(1, 7) for a in range(1, 8)]),
        ('validation', [(3, 7, a, 1) for a in range(1, 8)])):
        ids = np.asarray(identities, dtype=np.int64)
        x = np.exp(rng.normal(0, 0.2, (len(ids), 52, 190))).astype(np.float32)
        parts[split] = {'x': x, 'y': ids[:, 2] - 1, 'identity': ids}
    return parts, {'classes': [f'activity_{i}' for i in range(1, 8)], 'manifest_sha256': 'synthetic_fixture', 'evidence': 'SYNTHETIC'}


class GeneralizationTests(unittest.TestCase):
    def test_gain_invariance_finite_zero_and_bounded_dimension(self):
        rng = np.random.default_rng(9)
        x = np.exp(rng.normal(size=(3, 52, 190)))
        gain = np.exp(rng.normal(size=(3, 52, 1)))
        for family in FAMILIES:
            y = features(x, family)
            self.assertEqual(y.shape, (3, 156 if family == 'raw_statistics' else 468))
            self.assertTrue(np.isfinite(features(np.zeros_like(x), family)).all())
            if family != 'raw_statistics':
                np.testing.assert_allclose(y, features(x * gain, family), atol=1e-12, rtol=1e-12)
        with self.assertRaises(ValueError):
            features(-x, FAMILIES[0])

    def test_folds_disjoint_room_and_person(self):
        parts, _ = fixture()
        ids, y = parts['train']['identity'], parts['train']['y']
        for train, validation in folds(ids, y):
            for col in (0, 1):
                self.assertFalse(set(ids[train, col]) & set(ids[validation, col]))
        ids[0, 0] = 4
        with self.assertRaises(ValueError):
            folds(ids, y)

    def test_scaler_fits_only_passed_training_rows(self):
        train = np.arange(42, dtype=float).reshape(14, 3)
        state = fit_ridge(train, np.arange(14) % 7, 1)
        np.testing.assert_array_equal(state['mean'], train.mean(0))
        np.testing.assert_allclose(state['scale'], train.std(0))

    def test_selection_ignores_development_and_bundle_tamper(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            baseline_parts = copy.deepcopy(parts)
            baseline_parts['test'] = copy.deepcopy(parts['validation'])
            optimize(baseline_parts, provenance, root / 'baseline', epochs=1, variants=('statistics',))
            a = compare_development(parts, provenance, root / 'baseline', root / 'a')
            changed = copy.deepcopy(parts)
            changed['validation']['x'] *= 1.25
            changed['validation']['y'] = (changed['validation']['y'] + 1) % 7
            changed['validation']['identity'][:, 2] = changed['validation']['y'] + 1
            b = compare_development(changed, provenance, root / 'baseline', root / 'b')
            self.assertEqual(a['selected'], b['selected'])
            self.assertEqual(a['model_sha256'], b['model_sha256'])
            self.assertNotIn('test', a)
            self.assertFalse(a['promotionAuthorized'])
            bundle = GeneralizationBundle(root / 'a')
            self.assertEqual(bundle.predict(parts['validation']['x']).shape, (7, 7))
            self.assertEqual(a['reload_max_abs_logit_error'], 0)
            exported = optimize_export(root / 'a', parts['validation']['x'], root / 'baseline', root / 'float32', repetitions=50)
            self.assertLess(exported['model_bytes'], a['model_bytes'])
            self.assertEqual(exported['parent_model_sha256'], a['model_sha256'])
            reduced = GeneralizationBundle(root / 'float32')
            self.assertEqual(reduced.state['weights'].dtype, np.float32)
            np.testing.assert_array_equal(bundle.predict(parts['validation']['x']).argmax(1), reduced.predict(parts['validation']['x']).argmax(1))
            self.assertLessEqual(exported['optimization']['max_abs_logit_error'], 1e-4)
            with open(root / 'a' / 'model.safetensors', 'ab') as handle:
                handle.write(b'corrupt')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                GeneralizationBundle(root / 'a')
            bad = copy.deepcopy(parts)
            bad['test'] = bad['validation']
            with self.assertRaises(ValueError):
                compare_development(bad, provenance, root / 'baseline', root / 'bad')


if __name__ == '__main__':
    unittest.main()
