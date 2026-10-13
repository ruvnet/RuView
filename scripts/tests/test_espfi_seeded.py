"""Lossless seeded-map integrity tests."""
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_kernel import fit, predict_features
from csi_training.espfi_generalize import features
from csi_training.espfi_seeded import SPEC, export_seeded, SeededKernelBundle

class SeededTests(unittest.TestCase):
    def test_exact_export_and_tamper_rejection(self):
        amplitudes = np.exp(np.random.default_rng(2).normal(size=(14,52,190)))
        x = features(amplitudes, SPEC['family'])
        state = fit(x, np.arange(14)%7, dict(SPEC, kind='rff'))
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp)/'model'
            report = export_seeded(state, path)
            loaded = SeededKernelBundle(path)
            np.testing.assert_array_equal(predict_features(state,x), loaded.predict(amplitudes))
            self.assertLess(report['model_bytes'], 17000)
            self.assertEqual(report['stored_scalars'], 1967)
            self.assertEqual(report['runtime_scalars'], 61999)
            with (path/'model.safetensors').open('ab') as f:
                f.write(b'bad')
            with self.assertRaisesRegex(ValueError,'hash mismatch'):
                SeededKernelBundle(path)

    def test_wrong_projection_rejected(self):
        x = np.random.default_rng(1).normal(size=(14,468))
        state = fit(x,np.arange(14)%7,dict(SPEC,kind='rff'))
        state['projection'][0,0] += .1
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError,'seed recipe'):
                export_seeded(state,Path(tmp)/'model')

    def test_float32_export_uses_half_runtime_state(self):
        amplitudes = np.exp(np.random.default_rng(3).normal(size=(14,52,190)))
        x = features(amplitudes, SPEC['family'])
        state = fit(x, np.arange(14)%7, dict(SPEC, kind='rff'))
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            report = export_seeded(state, root/'single', precision='float32')
            model = SeededKernelBundle(root/'single')
            self.assertEqual(model.predict(amplitudes).dtype, np.float32)
            self.assertTrue(all(v.dtype == np.float32 for v in model.state.values()))
            self.assertEqual(report['runtime_state_bytes'], 61999*4)
            np.testing.assert_allclose(predict_features(state,x),model.predict(amplitudes),atol=1e-5,rtol=1e-5)
            np.testing.assert_array_equal(predict_features(state,x).argmax(1),model.predict(amplitudes).argmax(1))
            import json
            p = root/'single/metadata.json'
            metadata = json.loads(p.read_text())
            metadata['precision'] = 'float16'
            p.write_text(json.dumps(metadata))
            with self.assertRaisesRegex(ValueError,'Unsupported precision'):
                SeededKernelBundle(root/'single')

    def test_legacy_float64_metadata_still_loads(self):
        x = np.random.default_rng(4).normal(size=(14,468))
        state = fit(x,np.arange(14)%7,dict(SPEC,kind='rff'))
        with tempfile.TemporaryDirectory() as tmp:
            path=Path(tmp)/'model'
            export_seeded(state,path)
            import json
            p=path/'metadata.json'
            metadata=json.loads(p.read_text())
            del metadata['precision']
            p.write_text(json.dumps(metadata))
            loaded=SeededKernelBundle(path)
            self.assertTrue(all(v.dtype==np.float64 for v in loaded.state.values()))
            with self.assertRaisesRegex(ValueError,'Unsupported precision'):
                export_seeded(state,Path(tmp)/'invalid',precision='float16')
