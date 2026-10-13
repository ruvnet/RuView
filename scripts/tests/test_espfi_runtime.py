"""Synthetic runtime integrity, not sensing accuracy evidence."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_kernel import fit
from csi_training.espfi_generalize import features
from csi_training.espfi_seeded import SPEC, export_seeded, SeededKernelBundle
from csi_training.espfi_runtime import BufferedKernelBundle


class RuntimeTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.tmp.name)
        cls.x = np.exp(np.random.default_rng(29).normal(size=(21, 52, 190)))
        state = fit(features(cls.x, SPEC['family']), np.arange(21) % 7, dict(SPEC, kind='rff'))
        for precision in ('float32', 'float64'):
            export_seeded(state, cls.root/precision, precision=precision)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_parity_batch_and_strided_input(self):
        for precision in ('float32', 'float64'):
            reference = SeededKernelBundle(self.root/precision)
            for fuse in (False, True):
                model = BufferedKernelBundle(self.root/precision, fuse=fuse)
                values = self.x[::2, :, ::-1]
                expected, actual = reference.predict(values), model.predict(values)
                np.testing.assert_allclose(actual, expected, atol=1e-4, rtol=0)
                np.testing.assert_array_equal(actual.argmax(1), expected.argmax(1))
                np.testing.assert_array_equal(actual, np.concatenate([model.predict(x[None]) for x in values]))

    def test_extreme_and_near_constant_features(self):
        model = BufferedKernelBundle(self.root/'float32')
        rng = np.random.default_rng(7)
        values = [np.zeros((52, 190)), np.ones((52, 190)),
                  np.full((52, 190), 1e-20), np.full((52, 190), 1e100),
                  1e8 * (1 + rng.normal(0, 1e-8, (52, 190))),
                  np.exp(rng.uniform(-100, 100, (52, 190)))]
        for sample in values:
            model._extract(sample)
            np.testing.assert_allclose(model._features, features(sample[None], SPEC['family']),
                                       atol=1e-12, rtol=1e-12)
            expected = SeededKernelBundle(self.root/'float32').predict(sample[None])
            np.testing.assert_allclose(model.predict(sample[None]), expected, atol=1e-4, rtol=0)
        # A float32 input must be promoted before applying the reference floor.
        sample = np.exp(rng.uniform(-30, 2, (52, 190))).astype(np.float32)
        model._extract(sample)
        np.testing.assert_allclose(model._features, features(sample[None], SPEC['family']),
                                   atol=1e-12, rtol=1e-12)
        sample = np.zeros((52, 190), dtype=np.float32)
        sample[:, 95:] = np.float32(1.0000001e-6)
        model._extract(sample)
        np.testing.assert_array_equal(model._features, features(sample[None], SPEC['family']))

    def test_inputs_are_unchanged_and_outputs_do_not_alias(self):
        model = BufferedKernelBundle(self.root/'float32')
        values = self.x[:2].copy()
        before = values.copy()
        actual = model.predict(values)
        saved = actual.copy()
        model.predict(self.x[2:4])
        np.testing.assert_array_equal(values, before)
        np.testing.assert_array_equal(actual, saved)

    def test_concurrent_calls_serialize_workspace_safely(self):
        model = BufferedKernelBundle(self.root/'float32')
        samples = [x[None] for x in self.x]
        expected = [model.predict(x) for x in samples]
        with ThreadPoolExecutor(max_workers=4) as pool:
            actual = list(pool.map(model.predict, samples))
        for a, b in zip(actual, expected):
            np.testing.assert_array_equal(a, b)

    def test_invalid_inputs_and_recovery(self):
        model = BufferedKernelBundle(self.root/'float32')
        for value in (np.zeros((0, 52, 190)), np.zeros((1, 52, 189)),
                      np.full((1, 52, 190), np.nan), np.full((1, 52, 190), np.inf),
                      -np.ones((1, 52, 190)), np.ones((1, 52, 190), dtype=complex),
                      np.ones((1, 52, 190), dtype=object),
                      np.broadcast_to(0., (4097, 52, 190))):
            with self.assertRaises(ValueError):
                model.predict(value)
        self.assertTrue(np.isfinite(model.predict(self.x[:1])).all())

    def test_inference_has_no_training_import(self):
        code = ('import sys,numpy as np; from csi_training.espfi_runtime import BufferedKernelBundle; '
                'm=BufferedKernelBundle(sys.argv[1]); m.predict(np.ones((1,52,190))); '
                'assert "torch" not in sys.modules; assert "scipy" not in sys.modules; '
                'assert "csi_training.espfi_train" not in sys.modules')
        subprocess.run([sys.executable, '-c', code, str(self.root/'float32')], check=True,
                       env=dict(os.environ, PYTHONPATH=str(Path(__file__).resolve().parents[1])))


if __name__ == '__main__':
    unittest.main()
