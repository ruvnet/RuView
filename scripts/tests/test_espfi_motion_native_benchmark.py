"""Synthetic benchmark contracts; timing here is not sensing evidence."""
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training import espfi_motion_native_benchmark as bench
from csi_training.espfi_motion_compact import export
from csi_training.espfi_motion_native_build import build
from csi_training.espfi_train import digest
from test_espfi_context import make_baseline
from test_espfi_generalize import fixture


class NativeBenchmarkTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.tmp.name)
        cls.build_info = build(cls.root/'kernel-build')
        cls.kernel = cls.root/'kernel-build'/'kernel.so'
        cls.build_receipt = cls.root/'kernel-build'/'build.json'
        cls.kernel_sha256 = cls.build_info['library_sha256']
        cls.manifest = cls.root/'manifest.json'
        cls.manifest.write_text('{"fixture":"SYNTHETIC"}')
        cls.parts, cls.provenance = fixture()
        cls.provenance['manifest_sha256'] = digest(cls.manifest)
        make_baseline(cls.parts, cls.provenance, cls.root/'baseline')
        export(cls.parts, cls.provenance, cls.root/'baseline', cls.root/'compact')

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def test_real_runtime_parity_receipt_and_exclusive_output(self):
        output = self.root/'result.json'
        def bounded_digest(path):
            self.assertNotIn(Path(path), (self.kernel, self.build_receipt))
            return digest(path)
        with patch.object(bench, 'digest', side_effect=bounded_digest), \
                patch.object(bench, 'load_development', return_value=(self.parts, self.provenance)) as loader, \
                patch.object(bench, 'CALLS', 4), patch.object(bench, 'WARMUPS', 1):
            report = bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output, self.kernel, self.kernel_sha256.upper(), self.build_receipt)
            loader.assert_called_once_with(self.manifest)
        self.assertEqual(report['status'], 'complete')
        self.assertEqual(report['evidence'], 'SYNTHETIC')
        self.assertFalse(report['promotionAuthorized'])
        self.assertEqual(len(report['rounds']), 3)
        self.assertEqual(report['hashes']['compact_metadata'], digest(self.root/'compact'/'metadata.json'))
        self.assertIn('espfi_motion_native.py', report['source_sha256'])
        self.assertGreater(report['memory_bytes']['native_workspace'], 0)
        for split in ('train', 'validation'):
            for check in report['parity'][split]['references'].values():
                self.assertLessEqual(check['max_abs_logit_error'], 1e-10)
                self.assertTrue(check['identical_predictions'])
        saved = output.read_bytes()
        with self.assertRaises(FileExistsError):
            bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output, self.kernel, self.kernel_sha256, self.build_receipt)
        self.assertEqual(saved, output.read_bytes())

    def test_provenance_mismatch_records_failed_lifecycle(self):
        output = self.root/'mismatch.json'
        wrong = dict(self.provenance, manifest_sha256='wrong')
        with patch.object(bench, 'load_development', return_value=(self.parts, wrong)), \
                self.assertRaisesRegex(ValueError, 'provenance'):
            bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output, self.kernel, self.kernel_sha256, self.build_receipt)
        receipt = json.loads(output.read_text())
        self.assertEqual(receipt['status'], 'failed')
        self.assertFalse(receipt['promotionAuthorized'])

    def test_build_provenance_rejected_before_kernel_loading(self):
        bad = dict(self.build_info, flags=['-ffast-math'])
        receipt = self.root/'bad-build.json'
        receipt.write_text(json.dumps(bad))
        output = self.root/'bad-build-result.json'
        with patch.object(bench, 'load_development') as loader, self.assertRaisesRegex(ValueError, 'build provenance'):
            bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output,
                            self.kernel, self.kernel_sha256, receipt)
        loader.assert_not_called()
        self.assertEqual(json.loads(output.read_text())['status'], 'failed')

    def test_fifo_inputs_rejected_before_loading(self):
        fifo = self.root/'input.fifo'
        os.mkfifo(fifo)
        for name, kernel, receipt in (('kernel', fifo, self.build_receipt),
                                      ('receipt', self.kernel, fifo)):
            output = self.root/('fifo-' + name + '.json')
            with self.subTest(name=name), patch.object(bench, 'load_development') as loader, \
                    self.assertRaisesRegex(ValueError, 'regular'):
                bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output,
                                kernel, self.kernel_sha256, receipt)
            loader.assert_not_called()
            self.assertEqual(json.loads(output.read_text())['status'], 'failed')

    def test_frozen_measurement_order_and_every_round_gate(self):
        events = []
        class Bundle:
            def __init__(self, name): self.name = name
            def predict(self, x): events.append((self.name, int(x[0, 0, 0])))
        dev = np.arange(3).reshape(3, 1, 1)
        ticks = []
        current = 0
        # First two rounds pass; the third fails despite favorable earlier rounds.
        for round_index in range(3):
            for call in range(4):
                order = ['native', 'buffered_reference']
                if (call + round_index) % 2: order.reverse()
                for name in order:
                    duration = (50 if round_index < 2 else 110) if name == 'native' else 100
                    ticks.extend((current, current + duration))
                    current += 1000
        with patch.object(bench, 'CALLS', 4), patch.object(bench, 'WARMUPS', 0), \
                patch.object(bench.time, 'perf_counter_ns', side_effect=ticks):
            rounds = bench._measure(Bundle('native'), Bundle('buffered_reference'), dev)
        self.assertEqual([r['gain_at_least_10percent'] for r in rounds], [True, True, False])
        self.assertFalse(all(r['gain_at_least_10percent'] for r in rounds))
        for round_index in range(3):
            order = np.random.default_rng(42 + round_index).permutation(3)
            for call in range(4):
                pair = events[round_index*8 + call*2:round_index*8 + call*2+2]
                self.assertEqual([p[1] for p in pair], [int(order[call % 3])]*2)
                expected = ['native', 'buffered_reference']
                self.assertEqual([p[0] for p in pair], expected[::1 if (call+round_index)%2 == 0 else -1])


if __name__ == '__main__':
    unittest.main()
