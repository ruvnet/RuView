"""Synthetic benchmark contracts; timing here is not sensing evidence."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training import espfi_motion_mixed_benchmark as bench
from csi_training.espfi_motion_compact import export
from csi_training.espfi_train import digest
from test_espfi_context import make_baseline
from test_espfi_generalize import fixture


class MixedBenchmarkTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.root = Path(cls.tmp.name)
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
        with patch.object(bench, 'load_development', return_value=(self.parts, self.provenance)) as loader, \
                patch.object(bench, 'CALLS', 4), patch.object(bench, 'WARMUPS', 1):
            report = bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output)
            loader.assert_called_once_with(self.manifest)
        self.assertEqual(report['status'], 'complete')
        self.assertEqual(report['evidence'], 'SYNTHETIC')
        self.assertFalse(report['promotionAuthorized'])
        self.assertEqual(len(report['rounds']), 3)
        self.assertEqual(report['hashes']['compact_metadata'], digest(self.root/'compact'/'metadata.json'))
        self.assertIn('espfi_motion_mixed.py', report['source_sha256'])
        self.assertGreater(report['memory_bytes']['mixed_workspace'], 0)
        for split in ('train', 'validation'):
            for check in report['parity'][split]['references'].values():
                self.assertLessEqual(check['max_abs_logit_error'], 1e-4)
                self.assertTrue(check['identical_predictions'])
        self.assertTrue(report['float64_default_unchanged'])
        self.assertGreaterEqual(report['parity']['validation']['min_original_top1_top2_margin_half'], 0)
        self.assertEqual(report['parity']['validation']['output_dtype'], 'float32')
        saved = output.read_bytes()
        with self.assertRaises(FileExistsError):
            bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output)
        self.assertEqual(saved, output.read_bytes())

    def test_provenance_mismatch_records_failed_lifecycle(self):
        output = self.root/'mismatch.json'
        wrong = dict(self.provenance, manifest_sha256='wrong')
        with patch.object(bench, 'load_development', return_value=(self.parts, wrong)), \
                self.assertRaisesRegex(ValueError, 'provenance'):
            bench.benchmark(self.manifest, self.root/'compact', self.root/'baseline', output)
        receipt = json.loads(output.read_text())
        self.assertEqual(receipt['status'], 'failed')
        self.assertFalse(receipt['promotionAuthorized'])

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
                order = ['mixed', 'buffered_reference']
                if (call + round_index) % 2: order.reverse()
                for name in order:
                    duration = (50 if round_index < 2 else 110) if name == 'mixed' else 100
                    ticks.extend((current, current + duration))
                    current += 1000
        with patch.object(bench, 'CALLS', 4), patch.object(bench, 'WARMUPS', 0), \
                patch.object(bench.time, 'perf_counter_ns', side_effect=ticks):
            rounds = bench._measure(Bundle('mixed'), Bundle('buffered_reference'), dev)
        self.assertEqual([r['gain_at_least_10percent'] for r in rounds], [True, True, False])
        memory = dict(mixed_model_arrays=50, mixed_workspace=10, reference_model_arrays=100, reference_workspace=20)
        checks, retained, speed = bench._gates(memory, rounds)
        self.assertFalse(retained)
        self.assertFalse(speed)
        self.assertFalse(checks['p95_within_5percent_every_round'])
        self.assertTrue(checks['model_arrays_at_most_half'])
        self.assertTrue(checks['total_retained_at_most_65percent'])
        for round_index in range(3):
            order = np.random.default_rng(42 + round_index).permutation(3)
            for call in range(4):
                pair = events[round_index*8 + call*2:round_index*8 + call*2+2]
                self.assertEqual([p[1] for p in pair], [int(order[call % 3])]*2)
                expected = ['mixed', 'buffered_reference']
                self.assertEqual([p[0] for p in pair], expected[::1 if (call+round_index)%2 == 0 else -1])


if __name__ == '__main__':
    unittest.main()
