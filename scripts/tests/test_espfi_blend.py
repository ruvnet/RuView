"""Synthetic integrity checks, not sensing accuracy evidence."""
import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_blend import run, infer, select_weight
from test_espfi_generalize import fixture

class BlendTests(unittest.TestCase):
    def test_tie_retains_spectral_endpoint(self):
        y = np.arange(7)
        logits = np.eye(7)
        selected, results = select_weight([(logits, logits)], [y])
        self.assertEqual(selected['weight'], 0.)
        self.assertEqual(len(results), 5)

    def test_unused_endpoint_is_not_executed(self):
        from unittest.mock import patch
        logits = np.ones((1, 7))
        with patch('csi_training.espfi_blend.features', return_value=np.ones((1, 468))), patch('csi_training.espfi_blend.predict_features', return_value=logits) as predict:
            np.testing.assert_array_equal(infer([{}, None], 0., None), logits)
            predict.assert_called_once_with({}, unittest.mock.ANY)

    def test_development_cannot_change_selection_or_model(self):
        parts, provenance = fixture()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            first = run(parts, provenance, root/'first')
            changed = copy.deepcopy(parts)
            changed['validation']['x'] *= 1.25
            changed['validation']['y'] = (changed['validation']['y'] + 1) % 7
            second = run(changed, provenance, root/'second')
            self.assertEqual(first['selected'], second['selected'])
            self.assertEqual(first['model_sha256'], second['model_sha256'])
            self.assertTrue(first['reload_exact'])
            self.assertFalse(first['promotionAuthorized'])
            self.assertEqual(json.loads((root/'first/status.json').read_text())['status'], 'complete')
            with self.assertRaisesRegex(ValueError, 'Development partitions'):
                run(dict(parts, test=parts['validation']), provenance, root/'invalid')
