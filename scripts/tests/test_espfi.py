"""Synthetic intake boundary tests. No captured CSI enters the repository."""
import io
import hashlib
import zipfile
import json
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import numpy as np
from scipy.io import savemat
from csi_training import espfi


class EspfiTests(unittest.TestCase):
    def mat(self, x):
        b = io.BytesIO()
        savemat(b, {'CSIamp': x})
        return b.getvalue()

    def test_pool_and_orientation(self):
        x = np.arange(950 * 52, dtype=np.float64).reshape(950, 52)
        expected = x.reshape(190, 5, 52).mean(1).T.astype(np.float32)
        np.testing.assert_array_equal(espfi.decode_amplitude(self.mat(x)), expected)
        np.testing.assert_array_equal(espfi.decode_amplitude(self.mat(x.T)), expected)

    def test_reject_nonfinite_negative_complex_shape(self):
        for x in (np.full((950, 52), np.nan), -np.ones((950, 52)),
                  np.ones((950, 52), dtype=complex), np.ones((2, 52))):
            with self.assertRaises(ValueError):
                espfi.decode_amplitude(self.mat(x))

    def test_identity_and_traversal(self):
        self.assertEqual(espfi.parse_identity('room/mat/4-8-7-10.mat'), (4, 8, 7, 10))
        for name in ('../1-1-1-1.mat', '/1-1-1-1.mat', '1-9-1-1.mat',
                     '1-1-1-11.mat', 'foo\\1-1-1-1.mat'):
            with self.assertRaises(ValueError):
                espfi.parse_identity(name)

    def test_joint_split_is_disjoint(self):
        groups = {s: (set(), set()) for s in ('train', 'validation', 'test')}
        counts = dict.fromkeys(groups, 0)
        for r in range(1, 5):
            for p in range(1, 9):
                split = espfi.split_identity((r, p, 1, 1))
                if split:
                    groups[split][0].add(r)
                    groups[split][1].add(p)
                    counts[split] += 70
        self.assertEqual(counts, {'train': 840, 'validation': 70, 'test': 70})
        for a, b in [('train', 'validation'), ('train', 'test'), ('validation', 'test')]:
            self.assertFalse(groups[a][0] & groups[b][0])
            self.assertFalse(groups[a][1] & groups[b][1])

    def test_archive_pin_rejects_before_parsing(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'source.rar'
            p.write_bytes(b'not a dataset')
            with self.assertRaisesRegex(ValueError, 'pinned'):
                espfi.prepare(p, Path(tmp) / 'out')

    def fixture(self, tmp):
        records, arrays = [], {}
        for split in ('train', 'validation', 'test'):
            ids = [(r, p, a, t) for r in range(1, 5) for p in range(1, 9)
                   for a in range(1, 8) for t in range(1, 11)
                   if espfi.split_identity((r, p, a, t)) == split]
            arrays[split + '_identity'] = np.array(ids, dtype=np.int64)
            arrays[split + '_y'] = np.array([i[2] - 1 for i in ids], dtype=np.int64)
            arrays[split + '_x'] = np.stack([np.full((52, 10),
                i[0] * 10000 + i[1] * 1000 + i[2] * 100 + i[3], dtype=np.float32) for i in ids])
        selected = {tuple(i): hashlib.sha256(x.tobytes()).hexdigest()
                    for split in ('train', 'validation', 'test')
                    for i, x in zip(arrays[split + '_identity'], arrays[split + '_x'])}
        for r in range(1, 5):
            for p in range(1, 9):
                for a in range(1, 8):
                    for t in range(1, 11):
                        i = (r, p, a, t)
                        records.append({'identity': i, 'split': espfi.split_identity(i),
                            'signal_sha256': selected.get(i, hashlib.sha256(str(i).encode()).hexdigest())})
        npz = Path(tmp) / 'amplitudes.npz'
        np.savez_compressed(npz, **arrays)
        meta = dict(schema_version=1, classes=espfi.CLASSES, source_revision=espfi.REVISION,
                    source_url=espfi.SOURCE_URL, archive_sha256=espfi.ARCHIVE_SHA256,
                    license='CC-BY-4.0', split_policy='joint_room_subject_intersection_v1',
                    temporal_mean_pool=95, window_samples=10, records=records,
                    counts={s: len(arrays[s + '_y']) for s in ('train', 'validation', 'test')},
                    npz=npz.name, npz_sha256=espfi.digest(npz))
        manifest = Path(tmp) / 'manifest.json'
        manifest.write_text(json.dumps(meta))
        return manifest, npz, arrays, meta

    def test_loader_signal_binding_and_duplicate_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest, npz, arrays, meta = self.fixture(tmp)
            parts, _ = espfi.load_dataset(manifest)
            self.assertEqual(len(parts['train']['y']), 840)
            arrays['train_identity'][1] = arrays['train_identity'][0]
            np.savez_compressed(npz, **arrays)
            meta['npz_sha256'] = espfi.digest(npz)
            manifest.write_text(json.dumps(meta))
            with self.assertRaisesRegex(ValueError, 'provenance'):
                espfi.load_dataset(manifest)

    def test_development_never_deserializes_test_arrays(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest, _, _, _ = self.fixture(tmp)
            npz_class = np.lib.npyio.NpzFile
            original = npz_class.__getitem__
            accessed = []
            def guarded(data, key):
                if key.startswith('test_'):
                    raise AssertionError('test payload accessed during development')
                accessed.append(key)
                return original(data, key)
            with patch.object(npz_class, '__getitem__', guarded):
                parts, meta = espfi.load_development(manifest)
            self.assertEqual(set(parts), {'train', 'validation'})
            self.assertEqual(set(accessed), {s + suffix for s in parts
                                            for suffix in ('_x', '_y', '_identity')})
            self.assertEqual(parts['train']['identity'].shape, (840, 4))
            self.assertEqual(meta['loaded_partitions'], ['train', 'validation'])

    def test_forged_numpy_shape_rejected_before_allocation(self):
        with tempfile.TemporaryDirectory() as tmp:
            manifest, npz, arrays, meta = self.fixture(tmp)
            with zipfile.ZipFile(npz) as z:
                members = {i.filename: z.read(i) for i in z.infolist()}
            header = io.BytesIO()
            np.lib.format.write_array_header_1_0(header,
                {'descr': '<f4', 'fortran_order': False, 'shape': (10**12, 52, 10)})
            members['train_x.npy'] = header.getvalue()
            with zipfile.ZipFile(npz, 'w') as z:
                for name, content in members.items():
                    z.writestr(name, content)
            meta['npz_sha256'] = espfi.digest(npz)
            manifest.write_text(json.dumps(meta))
            with self.assertRaisesRegex(ValueError, 'numpy header'):
                espfi.load_dataset(manifest)

    def test_unverified_manifest_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / 'manifest.json'
            p.write_text(json.dumps({'schema_version': 1, 'classes': espfi.CLASSES}))
            with self.assertRaisesRegex(ValueError, 'provenance'):
                espfi.load_dataset(p)


if __name__ == '__main__':
    unittest.main()
