"""Synthetic motion-ablation integrity; no sensing accuracy evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_motion import CONTRACT, ROBUST_CONTRACT, GRID, WIDTHS, MotionBundle, motion_features, search
from csi_training.espfi_generalize import features
from csi_training.espfi_kernel import fit
from csi_training.espfi_seeded import SPEC, export_seeded
from csi_training.espfi_train import write_json
from test_espfi_generalize import fixture


class MotionTests(unittest.TestCase):
    def test_frozen_dimensions_gain_and_carrier_invariance(self):
        rng = np.random.default_rng(10)
        x = np.exp(rng.normal(size=(3, 52, 190)))
        gain = np.exp(rng.normal(size=(3, 52, 1)))
        self.assertEqual(len(GRID), 24)
        for family, width in WIDTHS.items():
            actual = motion_features(x, family)
            self.assertEqual(actual.shape, (3, width))
            np.testing.assert_allclose(actual, motion_features(x*gain, family), atol=1e-11, rtol=1e-11)
            for value in (0., 1., 1e-20, 1e100):
                self.assertTrue(np.isfinite(motion_features(np.full_like(x, value), family)).all())
            with self.assertRaises(ValueError):
                motion_features(-x, family)
        np.testing.assert_allclose(motion_features(x[:, ::-1], 'motion_invariant'),
                                   motion_features(x, 'motion_invariant'), atol=1e-12, rtol=1e-12)

    def test_acf_definition_and_temporal_reversal(self):
        x = np.exp(np.random.default_rng(12).normal(size=(2, 52, 190)))
        values = motion_features(x, 'motion_acf').reshape(2, 52, 9)
        logged = np.log(x); centered = logged-logged.mean(-1, keepdims=True)
        for i, lag in enumerate(CONTRACT['acf_lags']):
            expected = (centered[..., lag:]*centered[..., :-lag]).sum(-1)/(centered**2).sum(-1)
            np.testing.assert_allclose(values[..., i], expected, atol=1e-12, rtol=1e-12)
        self.assertTrue(np.all(np.abs(values[..., :6]) <= 1+1e-12))
        np.testing.assert_allclose(values.reshape(2, -1), motion_features(x[..., ::-1], 'motion_acf'), atol=1e-12)

    def test_stft_explicit_windows_and_tail(self):
        x = np.ones((1, 52, 190)); x[..., -1] = 20
        actual = motion_features(x, 'local_spectral')
        logged = np.log(x); relative = logged-logged.mean(-1, keepdims=True)
        relative /= np.sqrt(np.mean(relative**2))
        windows = np.stack([relative[..., i:i+32] for i in CONTRACT['stft']['starts']], axis=-2)*np.hanning(32)
        power = np.abs(np.fft.rfft(windows, axis=-1))**2/32**2
        bands = np.stack((power[..., 1:5].sum(-1), power[..., 5:17].sum(-1)), axis=-1)
        local = np.stack([b.mean(-2) for b in np.array_split(bands, 4, axis=-2)], axis=-2)
        expected = np.concatenate((local.reshape(1, -1), np.sqrt(np.mean(relative**2, axis=-1))), axis=1)
        np.testing.assert_array_equal(actual, expected)
        self.assertEqual(CONTRACT['stft']['starts'][-1]+32, 190)

    def test_selection_provenance_reload_and_lifecycle(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            state = fit(features(parts['train']['x'], SPEC['family']), parts['train']['y'], dict(SPEC, kind='rff'))
            original = export_seeded(state, root/'baseline', precision='float32')
            receipt = root/'receipt.json'
            write_json(receipt, {'model_sha256': original['model_sha256'], 'provenance': provenance})
            a = search(parts, provenance, root/'baseline', receipt, root/'a')
            changed = copy.deepcopy(parts)
            changed['validation']['x'] *= 1.5
            changed['validation']['y'] = (changed['validation']['y']+1) % 7
            changed['validation']['identity'][:, 2] = changed['validation']['y']+1
            b = search(changed, provenance, root/'baseline', receipt, root/'b')
            self.assertEqual(a['selected'], b['selected'])
            self.assertEqual(a['model_sha256'], b['model_sha256'])
            robust = search(parts, provenance, root/'baseline', receipt, root/'robust', selection='worst')
            winner = max(robust['candidates'], key=lambda row: (
                min(f['macro_f1'] for f in row['folds']), row['mean_cv_macro_f1'], -row['scalar_count']))
            self.assertEqual(robust['selected'], winner)
            self.assertEqual(robust['contract'], ROBUST_CONTRACT)
            self.assertEqual(MotionBundle(root/'robust').predict(parts['validation']['x']).shape, (7, 7))
            self.assertFalse(a['promotionAuthorized'])
            self.assertEqual(a['reload_max_abs_logit_error'], 0)
            self.assertNotIn('test', a)
            loaded = MotionBundle(root/'a')
            self.assertEqual(loaded.predict(parts['validation']['x']).shape, (7, 7))
            with (root/'a/model.safetensors').open('ab') as handle:
                handle.write(b'bad')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                MotionBundle(root/'a')
            write_json(root/'b/status.json', {'status': 'running', 'promotionAuthorized': False})
            with self.assertRaisesRegex(ValueError, 'completed'):
                MotionBundle(root/'b')
            wrong = dict(provenance, manifest_sha256='wrong')
            with self.assertRaisesRegex(ValueError, 'provenance'):
                search(parts, wrong, root/'baseline', receipt, root/'bad')
            parts['test'] = parts['validation']
            with self.assertRaisesRegex(ValueError, 'development-only'):
                search(parts, provenance, root/'baseline', receipt, root/'test')


if __name__ == '__main__':
    unittest.main()
