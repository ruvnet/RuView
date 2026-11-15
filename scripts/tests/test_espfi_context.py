"""Synthetic feature and experiment contracts, not real sensing accuracy."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest

import numpy as np
from safetensors.numpy import save_file

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_context import BASELINE, CONTRACT, GRID, WIDTHS, ContextBundle, context_features, search
from csi_training.espfi_kernel import fit
from csi_training.espfi_motion import ROBUST_CONTRACT, motion_features
from csi_training.espfi_train import digest, write_json
from test_espfi_generalize import fixture


def make_baseline(parts, provenance, root):
    root.mkdir()
    state = fit(motion_features(parts['train']['x'], 'motion_acf'), parts['train']['y'], BASELINE)
    save_file(state, str(root / 'model.safetensors'))
    write_json(root / 'status.json', {'status': 'complete', 'promotionAuthorized': False})
    write_json(root / 'metrics.json', {
        'status': 'complete', 'promotionAuthorized': False, 'task': 'public_motion_ablation',
        'contract': ROBUST_CONTRACT, 'selected': {'candidate': BASELINE},
        'model_sha256': digest(root / 'model.safetensors'), 'provenance': provenance,
    })


class ContextTests(unittest.TestCase):
    def test_order_reversal_gain_and_exact_control(self):
        rng = np.random.default_rng(43)
        x = np.exp(rng.normal(size=(2, 52, 190)))
        actual = context_features(x, 'ordered_trend')
        reverse = context_features(x[..., ::-1], 'ordered_trend')
        np.testing.assert_array_equal(actual[:, :468], motion_features(x, 'motion_acf'))
        np.testing.assert_allclose(actual[:, :468], reverse[:, :468], atol=1e-12)
        np.testing.assert_allclose(actual[:, 468:624].reshape(2, 52, 3),
                                   reverse[:, 468:624].reshape(2, 52, 3)[..., ::-1], atol=1e-12)
        np.testing.assert_allclose(actual[:, 624:], -reverse[:, 624:], atol=1e-12)
        self.assertGreater(np.max(np.abs(actual[:, 468:] - reverse[:, 468:])), .01)
        gain = np.exp(rng.normal(size=(2, 52, 1)))
        np.testing.assert_allclose(actual, context_features(x * gain, 'ordered_trend'), atol=1e-12)
        np.testing.assert_array_equal(context_features(x, 'ordered_mean'), actual[:, :624])
        # Independent mathematical reference for the added descriptors.
        z = np.log(x)
        z -= z.mean(-1, keepdims=True)
        z /= np.sqrt(np.mean(z**2, axis=(1, 2), keepdims=True))
        expected = np.stack((z[..., :63].mean(-1), z[..., 63:127].mean(-1), z[..., 127:].mean(-1)), -1)
        np.testing.assert_allclose(actual[:, 468:624], expected.reshape(2, -1), atol=1e-12)
        t = np.linspace(-1., 1., 190)
        np.testing.assert_allclose(actual[:, 624:], np.sum(z * t, -1) / np.sum(t*t), atol=1e-12)

    def test_numerical_edges_and_input_bounds(self):
        for family, width in WIDTHS.items():
            for level in (0., 1e-20, 1., 1e100):
                result = context_features(np.full((1, 52, 190), level), family)
                self.assertEqual(result.shape, (1, width))
                self.assertTrue(np.isfinite(result).all())
            x = np.ones((1, 52, 380))[:, :, ::2]
            x[..., 100] += 1e-12
            self.assertTrue(np.isfinite(context_features(x, family)).all())
        for x in (np.ones((0, 52, 190)), np.ones((1, 52, 189)),
                  -np.ones((1, 52, 190)), np.full((1, 52, 190), np.nan)):
            with self.assertRaises(ValueError):
                context_features(x, 'ordered_mean')
        with self.assertRaises(ValueError):
            context_features(np.ones((1, 52, 190)), 'room_history')

    def test_selection_isolation_control_reload_and_tamper(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(parts, provenance, root / 'baseline')
            report = search(parts, provenance, root / 'baseline', root / 'a')
            changed = copy.deepcopy(parts)
            changed['validation']['x'] = changed['validation']['x'][..., ::-1] * 1.2
            changed['validation']['y'] = (changed['validation']['y'] + 1) % 7
            changed['validation']['identity'][:, 2] = changed['validation']['y'] + 1
            other = search(changed, provenance, root / 'baseline', root / 'b')
            self.assertEqual(len(GRID), 18)
            self.assertEqual(report['selected'], other['selected'])
            self.assertEqual(report['model_sha256'], other['model_sha256'])
            self.assertTrue(report['baseline_state_exact'])
            self.assertEqual(report['baseline_control']['candidate'], BASELINE)
            winner = max(report['candidates'], key=lambda r: (r['minimum_cv_macro_f1'],
                         r['mean_cv_macro_f1'], -r['scalar_count']))
            self.assertEqual(report['selected'], winner)
            self.assertEqual(report['contract'], CONTRACT)
            self.assertEqual(report['evidence'], 'SYNTHETIC')
            self.assertFalse(report['promotionAuthorized'])
            self.assertEqual(report['reload_max_abs_logit_error'], 0)
            self.assertEqual(ContextBundle(root / 'a').predict(parts['validation']['x']).shape, (7, 7))
            # Changing the scored development labels cannot alter the persisted choice.
            selection = json.loads((root / 'a' / 'selection.json').read_text())
            self.assertEqual(selection['selected'], report['selected'])
            self.assertNotIn('development', selection)
            original = (root / 'a' / 'model.safetensors').read_bytes()
            (root / 'a' / 'model.safetensors').write_bytes(original + b'corruption')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                ContextBundle(root / 'a')
            (root / 'a' / 'model.safetensors').write_bytes(original)
            write_json(root / 'a' / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
            with self.assertRaises(ValueError):
                ContextBundle(root / 'a')

    def test_partition_provenance_and_failed_control(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(parts, provenance, root / 'baseline')
            invalid = dict(parts, test=parts['validation'])
            with self.assertRaises(ValueError):
                search(invalid, provenance, root / 'baseline', root / 'bad_split')
            with self.assertRaisesRegex(ValueError, 'provenance mismatch'):
                search(parts, dict(provenance, manifest_sha256='different'), root / 'baseline', root / 'bad_hash')
            # Wrong learned control cannot silently support a comparison.
            changed = copy.deepcopy(parts)
            changed['train']['x'][:, :, 10] *= 2.
            with self.assertRaisesRegex(ValueError, 'did not reproduce'):
                search(changed, provenance, root / 'baseline', root / 'bad_control')
            self.assertEqual(json.loads((root / 'bad_control' / 'status.json').read_text())['status'], 'failed')
            with self.assertRaises(FileExistsError):
                search(parts, provenance, root / 'baseline', root / 'bad_control')


if __name__ == '__main__':
    unittest.main()
