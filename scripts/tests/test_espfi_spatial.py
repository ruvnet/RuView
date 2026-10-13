"""Synthetic feature and experiment contracts, not real sensing accuracy."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
from safetensors.numpy import load

import numpy as np
from safetensors.numpy import save_file

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_spatial import BASELINE, CONTRACT, GRID, WIDTHS, SpatialBundle, spatial_features, search
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


class SpatialTests(unittest.TestCase):
    def test_correlation_reference_gain_reversal_control(self):
        rng = np.random.default_rng(43)
        x = np.exp(rng.normal(size=(2, 52, 190)))
        actual = spatial_features(x, 'motion_cross')
        np.testing.assert_array_equal(actual[:, :468], motion_features(x, 'motion_acf'))
        np.testing.assert_array_equal(actual[:, 468:], spatial_features(x, 'cross_corr'))
        np.testing.assert_allclose(actual, spatial_features(x[..., ::-1], 'motion_cross'), atol=1e-12)
        gain = np.exp(rng.normal(size=(2, 52, 1)))
        np.testing.assert_allclose(actual, spatial_features(x * gain, 'motion_cross'), atol=1e-12)
        expected = np.stack([np.corrcoef(np.log(row))[np.triu_indices(52, 1)] for row in x])
        np.testing.assert_allclose(actual[:, 468:], expected, atol=1e-12)
        signal = np.linspace(-1, 1, 190)
        logged = np.zeros((1, 52, 190))
        logged[0, 0] = signal
        logged[0, 1] = signal
        logged[0, 2] = -signal
        corr = spatial_features(np.exp(logged), 'cross_corr')
        self.assertAlmostEqual(corr[0, 0], 1.)
        self.assertAlmostEqual(corr[0, 1], -1.)
        self.assertEqual(corr[0, 2], 0.)
        self.assertLessEqual(np.abs(corr).max(), 1.+1e-12)

    def test_chunk_independence_and_input_bounds(self):
        x = np.exp(np.random.default_rng(51).normal(size=(35, 52, 190)))
        for family in WIDTHS:
            np.testing.assert_array_equal(spatial_features(x, family),
                np.concatenate([spatial_features(row[None], family) for row in x]))
            self.assertTrue(np.isfinite(spatial_features(np.zeros((1,52,190)),family)).all())
        for invalid in (np.ones((0,52,190)),np.ones((1,52,950)),
                        -np.ones((1,52,190)),np.full((1,52,190),np.nan)):
            with self.assertRaises(ValueError):
                spatial_features(invalid,'cross_corr')
        with self.assertRaises(ValueError):
            spatial_features(x,'unknown')

    def test_selection_isolation_control_reload_and_tamper(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            make_baseline(parts, provenance, root / 'baseline')
            original_features = spatial_features
            def guarded(values, family):
                if len(values) == len(parts['validation']['y']):
                    self.assertTrue((root/'a'/'selection.json').is_file())
                return original_features(values, family)
            with patch('csi_training.espfi_spatial.spatial_features', side_effect=guarded):
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
            self.assertEqual(SpatialBundle(root / 'a').predict(parts['validation']['x']).shape, (7, 7))
            # Changing the scored development labels cannot alter the persisted choice.
            selection = json.loads((root / 'a' / 'selection.json').read_text())
            self.assertEqual(selection['selected'], report['selected'])
            self.assertNotIn('development', selection)
            original = (root / 'a' / 'model.safetensors').read_bytes()
            (root / 'a' / 'model.safetensors').write_bytes(original + b'corruption')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                SpatialBundle(root / 'a')
            (root / 'a' / 'model.safetensors').write_bytes(original)
            state = load(original)
            state['feature_scale'][0] = 0.
            save_file(state, str(root/'a'/'model.safetensors'))
            forged = dict(report, model_sha256=digest(root/'a'/'model.safetensors'))
            write_json(root/'a'/'metrics.json', forged)
            with self.assertRaisesRegex(ValueError,'Invalid spatial model state'):
                SpatialBundle(root/'a')
            (root/'a'/'model.safetensors').write_bytes(original)
            write_json(root/'a'/'metrics.json', report)
            write_json(root / 'a' / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
            with self.assertRaises(ValueError):
                SpatialBundle(root / 'a')

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
