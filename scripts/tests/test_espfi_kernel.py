"""Synthetic nonlinear-search integrity; not sensing accuracy evidence."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_kernel import GRID, KernelBundle, fit, predict_features, search
from csi_training.espfi_generalize import compare_development
from csi_training.espfi_train import optimize
from test_espfi_generalize import fixture


class KernelTests(unittest.TestCase):
    def test_frozen_grid_and_deterministic_random_projection(self):
        self.assertEqual(len(GRID), 56)
        x = np.random.default_rng(1).normal(size=(28, 468))
        y = np.arange(28) % 7
        candidate = next(c for c in GRID if c['kind'] == 'rff')
        first, second = fit(x, y, candidate), fit(x, y, candidate)
        for key in first:
            np.testing.assert_array_equal(first[key], second[key])
        np.testing.assert_allclose(first['feature_mean'], x.mean(0))
        self.assertEqual(first['projection'].shape, (468, 128))
        self.assertEqual(predict_features(first, x).shape, (28, 7))
        self.assertNotIn('support_vectors', first)

    def test_centroid_agrees_with_explicit_distances(self):
        x = np.random.default_rng(2).normal(size=(28, 468))
        y = np.arange(28) % 7
        state = fit(x, y, {'kind': 'centroid'})
        z = (x - state['feature_mean']) / state['feature_scale']
        means = np.stack([z[y == i].mean(0) for i in range(7)])
        explicit = np.square(z[:, None, :] - means[None, :, :]).sum(-1).argmin(1)
        np.testing.assert_array_equal(explicit, predict_features(state, x).argmax(1))

    def test_external_development_cannot_select_and_export_is_verified(self):
        parts, metadata = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            initial = copy.deepcopy(parts)
            initial['test'] = copy.deepcopy(parts['validation'])
            optimize(initial, metadata, root / 'initial', epochs=1, variants=('statistics',))
            compare_development(parts, metadata, root / 'initial', root / 'incumbent')
            first = search(parts, metadata, root / 'incumbent', root / 'first')
            mutated = copy.deepcopy(parts)
            mutated['validation']['x'] *= 2
            mutated['validation']['y'] = (mutated['validation']['y'] + 1) % 7
            mutated['validation']['identity'][:, 2] = mutated['validation']['y'] + 1
            second = search(mutated, metadata, root / 'incumbent', root / 'second')
            self.assertEqual(first['selected'], second['selected'])
            self.assertEqual(first['model_sha256'], second['model_sha256'])
            self.assertEqual(len(first['candidates']), 56)
            bundle = KernelBundle(root / 'first')
            self.assertEqual(bundle.predict(parts['validation']['x']).shape, (7, 7))
            self.assertFalse(first['promotionAuthorized'])
            self.assertNotIn('test', first)
            with open(root / 'first' / 'model.safetensors', 'ab') as stream:
                stream.write(b'bad')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                KernelBundle(root / 'first')


if __name__ == '__main__':
    unittest.main()
