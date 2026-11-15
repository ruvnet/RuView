"""Synthetic native intake tests; no captured signal fixture is committed."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

import numpy as np
from scipy.io import savemat
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from csi_training import espfi
from csi_training import espfi_resolution_data as native


class ResolutionIntakeTests(unittest.TestCase):
    @staticmethod
    def mat(values):
        out = io.BytesIO()
        savemat(out, {'CSIamp': values})
        return out.getvalue()

    def test_source_precision_and_orientation(self):
        rng = np.random.default_rng(73)
        for dtype in (np.float32, np.float64, np.int32):
            x = (rng.random((950, 52)) * 1000).astype(dtype)
            for values in (x, x.T):
                content = self.mat(values)
                raw, pooled, source_dtype = native.decode_native(content)
                self.assertEqual(raw.dtype, np.float64)
                np.testing.assert_array_equal(raw, x.T)
                np.testing.assert_array_equal(pooled, espfi.decode_amplitude(content))
                np.testing.assert_array_equal(native.pool_native(raw, source_dtype), pooled)
        # Premature float32 rounding is detectably different on sensitive input.
        x = 1 + rng.uniform(0, 3e-7, size=(950, 52))
        raw, pooled, _ = native.decode_native(self.mat(x))
        self.assertFalse(np.array_equal(native.pool_native(raw, np.dtype('float32').str), pooled))

    def test_native_rejects_bad_values(self):
        for x in (np.full((950, 52), np.nan), np.full((950, 52), np.inf),
                  -np.ones((950, 52)), np.ones((950, 52)) * 1e13,
                  np.ones((950, 52), dtype=complex), np.ones((10, 52))):
            with self.assertRaises(ValueError):
                native.decode_native(self.mat(x))
        with self.assertRaises(ValueError):
            native.decode_native(b'x' * (espfi.MAX_MEMBER + 1))

    def parent(self):
        records = []
        for r in range(1, 5):
            for p in range(1, 9):
                for a in range(1, 8):
                    for t in range(1, 11):
                        i = (r, p, a, t)
                        h = native.sha(str(i).encode())
                        records.append(dict(identity=i, path='-'.join(map(str, i))+'.mat',
                            split=espfi.split_identity(i), sha256=h, signal_sha256=h))
        return dict(schema_version=1, classes=espfi.CLASSES, source_revision=espfi.REVISION,
                    archive_sha256=espfi.ARCHIVE_SHA256, source_url=espfi.SOURCE_URL,
                    license='CC-BY-4.0', split_policy='joint_room_subject_intersection_v1',
                    temporal_mean_pool=5, window_samples=190, records=records,
                    counts={s: sum(r['split'] == s for r in records) for s in ('train', 'validation', 'test')})

    def test_parent_population_and_quarantine(self):
        parent = self.parent()
        self.assertEqual(len(native._parent_records(parent)), 2240)
        parent['records'][1]['signal_sha256'] = parent['records'][0]['signal_sha256']
        with self.assertRaisesRegex(ValueError, 'quarantine'):
            native._parent_records(parent)
        for r in parent['records'][:2]:
            r['split'] = None
        parent['counts']['train'] -= 2
        self.assertEqual(len(native._parent_records(parent)), 2240)
        parent['records'].pop()
        with self.assertRaisesRegex(ValueError, 'population'):
            native._parent_records(parent)

    def test_prepare_skip_payloads_reload_tamper_and_failure(self):
        # Small synthetic parent isolates streaming behavior; full population
        # validation is exercised independently above and never disabled in CLI.
        identities = [(1, 1, 1, 1), (3, 7, 2, 1), (4, 8, 3, 1), (1, 7, 4, 1)]
        contents, lookup, prior, accessed = {}, {}, {}, []
        for offset, ident in enumerate(identities):
            content = self.mat(np.full((950, 52), 1.25 + offset, dtype=np.float64))
            name = '-'.join(map(str, ident)) + '.mat'
            contents[ident] = content
            pooled = espfi.decode_amplitude(content)
            split = espfi.split_identity(ident)
            lookup[ident] = dict(identity=ident, path=name, split=split,
                                 sha256=native.sha(content), signal_sha256=native.sha(pooled.tobytes()))
            if split in native.SPLITS:
                prior[split] = dict(x=pooled[None], y=np.array([ident[2]-1]), identity=np.array([ident]))
        class Entry:
            isfile = True
            def __init__(self, ident):
                self.ident, self.pathname = ident, lookup[ident]['path']
                self.size = len(contents[ident])
            def get_blocks(self):
                accessed.append(self.ident)
                if self.ident not in identities[:2]:
                    raise AssertionError('Test or excluded payload opened')
                yield contents[self.ident]
        @contextlib.contextmanager
        def archive_reader(_):
            yield iter(Entry(i) for i in identities)
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive = root/'source.rar'
            archive.write_bytes(b'rar')
            parent = root/'original.json'
            parent.write_text('{}')
            provenance = {'manifest_sha256': native.sha(parent.read_bytes())}
            with patch.object(espfi, 'ARCHIVE_BYTES', 3), patch.object(espfi, 'ARCHIVE_SHA256', native.sha(b'rar')), \
                 patch.object(native, '_parent_records', return_value=lookup), \
                 patch.object(espfi, 'load_development', return_value=(prior, provenance)), \
                 patch.dict(sys.modules, {'libarchive': types.SimpleNamespace(file_reader=archive_reader)}):
                target = native.prepare(archive, parent, root/'prepared')
                self.assertEqual(accessed, identities[:2])
                parts, meta = native.load_resolution(target)
                self.assertFalse(meta['test_payload_decoded'])
                self.assertEqual(set(parts), {'train', 'validation'})
                np.testing.assert_array_equal(parts['train']['pooled'], prior['train']['x'])
                bundle = target.parent/'amplitudes.npz'
                original = bundle.read_bytes()
                bundle.write_bytes(original + b'bad')
                with self.assertRaisesRegex(ValueError, 'digest'):
                    native.load_resolution(target)
                bundle.write_bytes(original)
                status = target.parent/'status.json'
                status.write_text('{"status":"failed","promotionAuthorized":false}')
                with self.assertRaisesRegex(ValueError, 'completed'):
                    native.load_resolution(target)
                with patch.object(native, 'decode_native', side_effect=ValueError('decoder failure')):
                    with self.assertRaisesRegex(ValueError, 'decoder failure'):
                        native.prepare(archive, parent, root/'failed')
                self.assertEqual(json.loads((root/'failed/status.json').read_text())['status'], 'failed')


if __name__ == '__main__':
    unittest.main()
