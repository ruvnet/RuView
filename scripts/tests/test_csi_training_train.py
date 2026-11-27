"""Synthetic CPU integration tests, never evidence of sensing performance."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

import numpy as np
from csi_training.data import Partition
from csi_training.train import temporal_triplets, triplet_accuracy, run_training


class SamplingTests(unittest.TestCase):
    def test_triplets_respect_session_node_and_time(self):
        t = np.tile(np.array([0, 1, 40, 41, 80, 81.]), 4)
        nodes = ['a'] * 6 + ['b'] * 6 + ['a'] * 6 + ['b'] * 6
        sessions = ['one'] * 12 + ['two'] * 12
        partition = Partition(np.ones((24, 8)), t, nodes, ['room'] * 24, sessions)
        triplets = temporal_triplets(partition, 12)
        self.assertTrue(np.array_equal(triplets, temporal_triplets(partition, 12)))
        for a, p, n in triplets:
            self.assertNotEqual(a, p)
            self.assertEqual(sessions[a], sessions[p])
            self.assertEqual(sessions[a], sessions[n])
            self.assertEqual(nodes[a], nodes[p])
            self.assertEqual(nodes[a], nodes[n])
            self.assertGreater(abs(t[a] - t[p]), 0)
            self.assertLessEqual(abs(t[a] - t[p]), 2)
            self.assertGreater(abs(t[a] - t[n]), 30)

    def test_duplicate_timestamp_only_pairs_rejected(self):
        times = np.array([0, 0, 40, 40, 80, 80.])
        part = Partition(np.tile(np.eye(3, 8), (2, 1)), times,
                         ['a'] * 6, ['r'] * 6, ['s'] * 6)
        with self.assertRaisesRegex(ValueError, '0<dt'):
            temporal_triplets(part, 42)

    def test_equal_timestamp_group_is_excluded_from_positives(self):
        times = np.array([0, 0, 0, 1, 40, 40, 41, 80, 80, 81.])
        part = Partition(np.ones((10, 8)), times, ['a'] * 10, ['r'] * 10, ['s'] * 10)
        triples = temporal_triplets(part, 42, limit=4)
        self.assertEqual(len(triples), 4)
        for a, p, n in triples:
            self.assertGreater(abs(times[a] - times[p]), 0)
            self.assertLessEqual(abs(times[a] - times[p]), 2)
            self.assertGreater(abs(times[a] - times[n]), 30)

    def test_constant_embedding_is_chance(self):
        self.assertEqual(triplet_accuracy(np.ones((3, 128)), np.array([[0, 1, 2]])), 0.5)

    def test_missing_temporal_negatives_fails(self):
        part = Partition(np.ones((3, 8)), np.arange(3), ['a']*3, ['r']*3, ['s']*3)
        with self.assertRaises(ValueError):
            temporal_triplets(part, 42)


@unittest.skipUnless(importlib.util.find_spec('torch') and importlib.util.find_spec('safetensors'), 'install requirements-csi-training.txt for real CPU training tests')
class TrainingTests(unittest.TestCase):
    def fixture(self, root):
        entries = []
        rng = np.random.default_rng(13)
        for i, split in enumerate(['train', 'validation', 'test']):
            rows = []
            for state in range(8):
                center = rng.normal(size=8)
                for offset in range(4):
                    rows.append({'timestamp': state * 40 + offset * 0.5, 'node_id': 'a',
                                 'features': (center + rng.normal(scale=0.02, size=8)).tolist()})
            path = root / f'{split}.jsonl'
            path.write_text(''.join(json.dumps(row) + '\n' for row in rows))
            entries.append({'path': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                            'captured_at': '2026-10-12T00:00:00Z', 'room_id': f'room{i}',
                            'session_id': f'session{i}', 'split': split,
                            'evidence': 'synthetic', 'use_permitted': True})
        manifest = root / 'manifest.json'
        manifest.write_text(json.dumps({'schema_version': 1, 'files': entries}))
        return manifest

    def args(self, manifest, output):
        return argparse.Namespace(manifest=str(manifest), output=str(output), group_by='room_id',
                                  allow_synthetic=True, device='cpu', epochs=2, batch_size=16,
                                  max_triplets=128, seed=42, max_seconds=60,
                                  learning_rate=0.001, temperature=0.1, command='train')

    def test_training_updates_weights_replays_and_exports(self):
        import torch
        from safetensors.torch import load_file
        from csi_training.model import CSIEncoder, PreprocessedEncoder
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            manifest = self.fixture(root)
            a = run_training(self.args(manifest, root / 'a'))
            b = run_training(self.args(manifest, root / 'b'))
            self.assertGreater(a['trainable_parameter_delta_squared'], 0)
            self.assertEqual(a['model_sha256'], b['model_sha256'])
            self.assertEqual(a['history'], b['history'])
            self.assertEqual(a['metrics'], b['metrics'])
            self.assertFalse(a['promotionAuthorized'])
            self.assertEqual(a['evidence'], 'SYNTHETIC')
            model = CSIEncoder().eval()
            model.load_state_dict(load_file(str(root / 'a' / 'model.safetensors')))
            with torch.no_grad():
                embedded = model(torch.ones(3, 8))
            self.assertTrue(torch.isfinite(embedded).all())
            self.assertTrue(torch.allclose(embedded.norm(dim=-1), torch.ones(3), atol=1e-6))
            self.assertEqual(a['model_sha256'], hashlib.sha256((root / 'a' / 'model.safetensors').read_bytes()).hexdigest())
            preprocessing = json.loads((root / 'a' / 'preprocessing.json').read_text())
            self.assertEqual(preprocessing['fit_partition'], 'train')
            self.assertEqual(len(preprocessing['mean']), 8)
            loader = PreprocessedEncoder(root / 'a')
            raw = np.ones((3, 8))
            normalized = torch.from_numpy(((raw - np.array(preprocessing['mean'])) / np.array(preprocessing['scale'])).astype(np.float32))
            with torch.no_grad():
                expected = model(normalized).numpy()
            np.testing.assert_allclose(loader.encode(raw), expected, atol=1e-6)
            with self.assertRaises(ValueError):
                loader.encode(np.full((1, 8), np.nan))
            with self.assertRaisesRegex(ValueError, 'must not exist'):
                run_training(self.args(manifest, root / 'a'))
            status_path = root / 'a' / 'status.json'
            status_path.write_text(json.dumps({'status': 'failed', 'promotionAuthorized': False}))
            with self.assertRaisesRegex(ValueError, 'lifecycle'):
                PreprocessedEncoder(root / 'a')
            status_path.write_text(json.dumps({'status': 'complete', 'promotionAuthorized': False}))
            (root / 'a' / 'preprocessing.json').write_text('{}')
            with self.assertRaisesRegex(ValueError, 'hash mismatch'):
                PreprocessedEncoder(root / 'a')

    def test_elapsed_budget_leaves_failed_nonpromotable_status(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            manifest = self.fixture(root)
            with patch('csi_training.train.time.monotonic', side_effect=[0.0, 61.0]):
                with self.assertRaises(TimeoutError):
                    run_training(self.args(manifest, root / 'out'))
            status = json.loads((root / 'out' / 'status.json').read_text())
            self.assertEqual(status['status'], 'failed')
            self.assertFalse(status['promotionAuthorized'])
            self.assertFalse((root / 'out' / 'model.safetensors').exists())

    def test_invalid_budget_rejected_before_output(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            manifest = self.fixture(root)
            args = self.args(manifest, root / 'out')
            args.max_seconds = float('nan')
            with self.assertRaises(ValueError):
                run_training(args)
            self.assertFalse((root / 'out').exists())


if __name__ == '__main__':
    unittest.main()
