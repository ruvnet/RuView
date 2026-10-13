"""Synthetic contracts for frozen resolution ablation; not sensing evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_resolution import (BASELINE, CONTROL, GRID, CONTRACT, ResolutionBundle,
    resolution_features, model_input, search)
from csi_training.espfi_motion import motion_features
from test_espfi_context import make_baseline
from test_espfi_generalize import fixture as pooled_fixture


def fixture():
    parts, prior = pooled_fixture()
    for part in parts.values():
        part['pooled'] = part['x']
        part['x'] = np.repeat(part['pooled'].astype(np.float64), 5, axis=-1)
    provenance = dict(prior, parent_manifest_sha256=prior['manifest_sha256'],
                      manifest_sha256='synthetic_resolution', pooled_exact=True,
                      raw_dtype='float64', test_payload_decoded=False)
    return parts, provenance, prior


def baseline(parts, prior, path):
    pooled = {k: dict(v, x=v['pooled']) for k, v in parts.items()}
    make_baseline(pooled, prior, path)


class ResolutionTests(unittest.TestCase):
    def test_control_parity_raw_reversal_gain_and_shape(self):
        rng = np.random.default_rng(29)
        pooled = np.exp(rng.normal(size=(2, 52, 190))).astype(np.float32)
        np.testing.assert_array_equal(resolution_features(pooled, 'pooled190'), motion_features(pooled, 'motion_acf'))
        raw = np.exp(rng.normal(size=(2, 52, 950)))
        for family in ('raw950_matched', 'raw950_native'):
            actual = resolution_features(raw, family)
            self.assertEqual(actual.shape, (2, 468))
            np.testing.assert_allclose(actual, resolution_features(raw[..., ::-1], family), atol=1e-12)
            np.testing.assert_allclose(actual, resolution_features(raw*2, family), atol=1e-12)
            np.testing.assert_array_equal(resolution_features(np.ones_like(raw), family), np.zeros_like(actual))
            with self.assertRaises(ValueError):
                resolution_features(pooled, family)
        self.assertGreater(np.max(np.abs(resolution_features(raw, 'raw950_matched') - resolution_features(raw, 'raw950_native'))), .001)
        for invalid in (np.ones((1, 52, 949)), np.ones((0, 52, 950)),
                        np.full((1, 52, 950), np.inf), -np.ones((1, 52, 950))):
            with self.assertRaises(ValueError):
                resolution_features(invalid, 'raw950_native')
        with self.assertRaises(ValueError):
            resolution_features(raw, 'pooled190')

    def test_raw_chunk_bound_preserves_per_record_arithmetic(self):
        x = np.exp(np.random.default_rng(81).normal(size=(35, 52, 950)))
        for family in ('raw950_matched', 'raw950_native'):
            actual = resolution_features(x, family)
            expected = np.concatenate([resolution_features(x[i:i+1], family) for i in range(len(x))])
            np.testing.assert_array_equal(actual, expected)

    def test_search_control_isolation_reload_tamper(self):
        parts, provenance, prior = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            baseline(parts, prior, root/'baseline')
            original_features = resolution_features
            def guarded(values, family):
                if len(values) == len(parts['validation']['y']):
                    self.assertTrue((root/'first'/'selection.json').is_file())
                return original_features(values, family)
            with patch('csi_training.espfi_resolution.resolution_features', side_effect=guarded):
                result = search(parts, provenance, root/'baseline', root/'first')
            changed = copy.deepcopy(parts)
            changed['validation']['x'] *= 1.3
            changed['validation']['y'] = (changed['validation']['y']+1)%7
            changed['validation']['identity'][:, 2] = changed['validation']['y']+1
            other = search(changed, provenance, root/'baseline', root/'second')
            self.assertEqual(len(GRID), 18)
            self.assertEqual(result['selected'], other['selected'])
            self.assertEqual(result['model_sha256'], other['model_sha256'])
            self.assertTrue(result['baseline_state_exact'])
            self.assertEqual(result['baseline_control']['candidate'], CONTROL)
            winner = max(result['candidates'], key=lambda r:(r['minimum_cv_macro_f1'],r['mean_cv_macro_f1']))
            self.assertEqual(result['selected'],winner)
            self.assertEqual(result['contract'], CONTRACT)
            self.assertFalse(result['promotionAuthorized'])
            bundle = ResolutionBundle(root/'first')
            self.assertEqual(bundle.predict(model_input(parts['validation'], bundle.candidate['family'])).shape,(7,7))
            selected = json.loads((root/'first'/'selection.json').read_text())
            self.assertNotIn('development',selected)
            with (root/'first'/'model.safetensors').open('ab') as handle:
                handle.write(b'tampered')
            with self.assertRaisesRegex(ValueError,'hash mismatch'):
                ResolutionBundle(root/'first')

    def test_failed_lifecycle_precision_provenance(self):
        parts, provenance, prior = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            baseline(parts, prior, root/'baseline')
            invalid = copy.deepcopy(parts)
            invalid['train']['x'] = invalid['train']['x'].astype(np.float32)
            with self.assertRaises(ValueError):
                search(invalid,provenance,root/'baseline',root/'precision')
            with self.assertRaises(ValueError):
                search(dict(parts,test=parts['validation']),provenance,root/'baseline',root/'test')
            with self.assertRaises(ValueError):
                search(parts,dict(provenance,parent_manifest_sha256='wrong'),root/'baseline',root/'provenance')
            mismatched = copy.deepcopy(parts)
            mismatched['train']['pooled'][:, :, 10] *= 2
            with self.assertRaisesRegex(ValueError, 'did not reproduce'):
                search(mismatched,provenance,root/'baseline',root/'bad_control')
            self.assertEqual(json.loads((root/'bad_control'/'status.json').read_text())['status'],'failed')
            with patch('csi_training.espfi_resolution.fit',side_effect=RuntimeError('injected')):
                with self.assertRaises(RuntimeError):
                    search(parts,provenance,root/'baseline',root/'failed')
            self.assertEqual(json.loads((root/'failed'/'status.json').read_text())['status'],'failed')
            with self.assertRaises(ValueError):
                ResolutionBundle(root/'failed')
            with self.assertRaises(FileExistsError):
                search(parts,provenance,root/'baseline',root/'failed')

if __name__ == '__main__':
    unittest.main()
