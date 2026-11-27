"""Pinned native-resolution intake; preserve the original development population.

Only previously admitted train/validation MAT payloads are decoded. The original
manifest retains quarantine authority. No final test or excluded capture is
reassigned. Raw arrays and manifests remain outside Git.
"""
from __future__ import annotations
import argparse
import hashlib
import io
import json
from pathlib import Path
import zipfile

import numpy as np
from . import espfi
from .espfi_motion import _read

SPLITS = ('train', 'validation')
DTYPES = {np.dtype(t).str for t in ('float32', 'float64', 'int8', 'uint8',
          'int16', 'uint16', 'int32', 'uint32', 'int64', 'uint64')}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def _parent_records(meta):
    if (meta.get('schema_version') != 1 or meta.get('classes') != espfi.CLASSES
            or meta.get('source_revision') != espfi.REVISION
            or meta.get('archive_sha256') != espfi.ARCHIVE_SHA256
            or meta.get('source_url') != espfi.SOURCE_URL
            or meta.get('license') != 'CC-BY-4.0'
            or meta.get('split_policy') != 'joint_room_subject_intersection_v1'
            or meta.get('temporal_mean_pool') != 5 or meta.get('window_samples') != 190):
        raise ValueError('Invalid pooled parent provenance')
    records = meta.get('records', [])
    expected = {(r, p, a, t) for r in range(1, 5) for p in range(1, 9)
                for a in range(1, 8) for t in range(1, 11)}
    lookup = {tuple(r['identity']): r for r in records}
    if len(records) != 2240 or set(lookup) != expected:
        raise ValueError('Incomplete parent population')
    counts = {}
    for record in records:
        for key in ('signal_sha256', 'sha256'):
            if not isinstance(record.get(key), str) or not espfi.re.fullmatch('[0-9a-f]{64}', record[key]):
                raise ValueError('Invalid parent digest')
        h = record['signal_sha256']
        counts[h] = counts.get(h, 0) + 1
    for ident, record in lookup.items():
        if (espfi.parse_identity(record['path']) != ident
                or record['split'] != (None if counts[record['signal_sha256']] > 1
                                       else espfi.split_identity(ident))):
            raise ValueError('Invalid parent quarantine or split')
    if meta.get('counts') != {s: sum(r['split'] == s for r in records)
                              for s in (*SPLITS, 'test')}:
        raise ValueError('Invalid parent counts')
    return lookup


def pool_native(raw, source_dtype):
    """Reproduce original MAT dtype, orientation, averaging and final cast."""
    if source_dtype not in DTYPES or raw.shape != (52, 950):
        raise ValueError('Unsupported native schema')
    source = raw.T.astype(np.dtype(source_dtype))
    return np.ascontiguousarray(source.reshape(190, 5, 52).mean(1).T, dtype=np.float32)


def decode_native(content):
    from scipy.io import loadmat
    if not 0 < len(content) <= espfi.MAX_MEMBER:
        raise ValueError('Invalid MAT size')
    x = loadmat(io.BytesIO(content), variable_names=['CSIamp']).get('CSIamp')
    if (x is None or x.dtype.str not in DTYPES or x.shape not in ((950, 52), (52, 950))
            or not np.isfinite(x).all() or (x < 0).any() or (x > 1e12).any()):
        raise ValueError('Invalid native amplitude')
    dtype = x.dtype.str
    if x.shape == (950, 52):
        x = x.T
    raw = np.ascontiguousarray(x, dtype=np.float64)
    # Require lossless conversion, including supported integer source types.
    if not np.array_equal(raw.astype(x.dtype), x):
        raise ValueError('Native conversion loses information')
    pooled = espfi.decode_amplitude(content, pool=5)
    if not np.array_equal(pool_native(raw, dtype), pooled):
        raise ValueError('Original pooling does not reproduce exactly')
    return raw, pooled, dtype


def prepare(archive, parent_manifest, output):
    import libarchive
    archive, output = Path(archive), Path(output)
    if (archive.is_symlink() or not archive.is_file()
            or archive.stat().st_size != espfi.ARCHIVE_BYTES
            or espfi.digest(archive) != espfi.ARCHIVE_SHA256):
        raise ValueError('Archive differs from pinned source')
    parent_bytes = _read(parent_manifest, 3_000_000)
    parent = json.loads(parent_bytes)
    lookup = _parent_records(parent)
    prior, checked = espfi.load_development(parent_manifest)
    if checked['manifest_sha256'] != sha(parent_bytes):
        raise ValueError('Parent changed during intake')
    admitted = {i: r for i, r in lookup.items() if r['split'] in SPLITS}
    expected_pooled = {tuple(i): x for s in SPLITS
                       for i, x in zip(prior[s]['identity'], prior[s]['x'])}
    output.mkdir(parents=True, exist_ok=False)
    def status(value):
        (output / 'status.json').write_text(json.dumps({'status': value, 'promotionAuthorized': False}))
    status('running')
    try:
        seen, found, rows, records = set(), set(), {s: [] for s in SPLITS}, []
        with libarchive.file_reader(str(archive)) as entries:
            for entry in entries:
                if not entry.pathname.endswith('.mat'):
                    continue
                ident = espfi.parse_identity(entry.pathname)
                if (ident in seen or not entry.isfile or not 0 < entry.size <= espfi.MAX_MEMBER
                        or entry.pathname != lookup[ident]['path']):
                    raise ValueError('Unexpected source member')
                seen.add(ident)
                if ident not in admitted:
                    continue  # No get_blocks/loadmat for test or excluded records.
                content = bytearray()
                for block in entry.get_blocks():
                    content.extend(block)
                    if len(content) > espfi.MAX_MEMBER:
                        raise ValueError('Expanded member exceeds bound')
                record = admitted[ident]
                if sha(content) != record['sha256']:
                    raise ValueError('Source MAT digest mismatch')
                raw, pooled, dtype = decode_native(content)
                if (sha(pooled.tobytes()) != record['signal_sha256']
                        or not np.array_equal(pooled, expected_pooled[ident])):
                    raise ValueError('Pooled control differs from parent')
                found.add(ident)
                rows[record['split']].append((ident, raw, pooled))
                records.append({'identity': list(ident), 'split': record['split'],
                                'source_dtype': dtype, 'source_sha256': record['sha256'],
                                'raw_sha256': sha(raw.tobytes()),
                                'pooled_sha256': record['signal_sha256']})
        if set(lookup) != seen or set(admitted) != found:
            raise ValueError('Incomplete source population')
        arrays = {}
        for split in SPLITS:
            samples = sorted(rows[split], key=lambda row: row[0])
            arrays[split + '_x'] = np.stack([x for _, x, _ in samples])
            arrays[split + '_pooled'] = np.stack([x for _, _, x in samples])
            arrays[split + '_identity'] = np.array([i for i, _, _ in samples], dtype=np.int64)
            arrays[split + '_y'] = arrays[split + '_identity'][:, 2] - 1
        target = output / 'amplitudes.npz'
        np.savez_compressed(target, **arrays)
        (output / 'parent_manifest.json').write_bytes(parent_bytes)
        meta = {'schema_version': 1, 'task': 'espfi_native_resolution',
                'source_revision': espfi.REVISION, 'archive_sha256': espfi.ARCHIVE_SHA256,
                'classes': espfi.CLASSES, 'evidence': 'real', 'license': 'CC-BY-4.0',
                'parent_manifest_sha256': sha(parent_bytes), 'pooled_exact': True,
                'raw_dtype': 'float64', 'test_payload_decoded': False,
                'npz': target.name, 'npz_sha256': espfi.digest(target),
                'counts': {s: len(rows[s]) for s in SPLITS},
                'records': sorted(records, key=lambda row: row['identity'])}
        (output / 'manifest.json').write_text(json.dumps(meta, indent=2) + '\n')
        status('complete')
        return output / 'manifest.json'
    except Exception:
        status('failed')
        raise


def load_resolution(manifest):
    path = Path(manifest)
    data = _read(path, 3_000_000)
    meta = json.loads(data)
    lifecycle = json.loads(_read(path.parent / 'status.json', 65536))
    if (lifecycle != {'status': 'complete', 'promotionAuthorized': False}
            or meta.get('schema_version') != 1 or meta.get('task') != 'espfi_native_resolution'
            or meta.get('source_revision') != espfi.REVISION
            or meta.get('archive_sha256') != espfi.ARCHIVE_SHA256
            or meta.get('classes') != espfi.CLASSES or meta.get('license') != 'CC-BY-4.0'
            or meta.get('evidence') != 'real' or meta.get('raw_dtype') != 'float64'
            or meta.get('pooled_exact') is not True or meta.get('test_payload_decoded') is not False
            or meta.get('npz') != 'amplitudes.npz'):
        raise ValueError('Invalid completed resolution intake')
    parent = _read(path.parent / 'parent_manifest.json', 3_000_000)
    if sha(parent) != meta.get('parent_manifest_sha256'):
        raise ValueError('Parent manifest digest mismatch')
    lookup = _parent_records(json.loads(parent))
    admitted = {i: r for i, r in lookup.items() if r['split'] in SPLITS}
    records = meta.get('records', [])
    rows = {tuple(r['identity']): r for r in records}
    if len(records) != len(admitted) or set(rows) != set(admitted):
        raise ValueError('Resolution population differs from parent')
    counts = {s: sum(r['split'] == s for r in admitted.values()) for s in SPLITS}
    if meta.get('counts') != counts:
        raise ValueError('Resolution counts differ')
    for ident, r in rows.items():
        original = admitted[ident]
        if (r['split'] != original['split'] or r['source_sha256'] != original['sha256']
                or r['pooled_sha256'] != original['signal_sha256'] or r['source_dtype'] not in DTYPES):
            raise ValueError('Resolution record binding mismatch')
    payload = _read(path.parent / 'amplitudes.npz', espfi.MAX_NPZ)
    if sha(payload) != meta.get('npz_sha256'):
        raise ValueError('Resolution bundle digest mismatch')
    with zipfile.ZipFile(io.BytesIO(payload)) as z:
        expected = {s + '_' + k + '.npy': (shape, dtype) for s in SPLITS for k, shape, dtype in
                    [('x', (counts[s], 52, 950), np.dtype('float64')),
                     ('pooled', (counts[s], 52, 190), np.dtype('float32')),
                     ('y', (counts[s],), np.dtype('int64')),
                     ('identity', (counts[s], 4), np.dtype('int64'))]}
        infos = z.infolist()
        if (len(infos) != 8 or {i.filename for i in infos} != set(expected)
                or sum(i.file_size for i in infos) > espfi.MAX_NPZ):
            raise ValueError('Unexpected resolution array population')
        for info in infos:
            with z.open(info) as f:
                version = np.lib.format.read_magic(f)
                if version not in ((1, 0), (2, 0)):
                    raise ValueError('Unsupported numpy version')
                reader = (np.lib.format.read_array_header_1_0 if version == (1, 0)
                          else np.lib.format.read_array_header_2_0)
                shape, fortran, dtype = reader(f)
                if ((shape, dtype) != expected[info.filename] or fortran
                        or f.tell() + int(np.prod(shape)) * dtype.itemsize != info.file_size):
                    raise ValueError('Unexpected resolution array header')
    parts, seen = {}, set()
    with np.load(io.BytesIO(payload), allow_pickle=False) as npz:
        for split in SPLITS:
            part = {key: npz[split + '_' + key] for key in ('x', 'pooled', 'y', 'identity')}
            for raw, pooled, label, identity in zip(part['x'], part['pooled'], part['y'], part['identity']):
                ident = tuple(map(int, identity))
                r = rows.get(ident)
                if (r is None or ident in seen or r['split'] != split or label != ident[2] - 1
                        or not np.isfinite(raw).all() or (raw < 0).any() or (raw > 1e12).any()
                        or sha(raw.tobytes()) != r['raw_sha256']
                        or sha(pooled.tobytes()) != r['pooled_sha256']
                        or not np.array_equal(pool_native(raw, r['source_dtype']), pooled)):
                    raise ValueError('Resolution array provenance mismatch')
                seen.add(ident)
            parts[split] = part
    if seen != set(admitted):
        raise ValueError('Incomplete loaded resolution population')
    meta['manifest_sha256'] = sha(data)
    return parts, meta


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ('archive', 'parent-manifest', 'output'):
        parser.add_argument('--' + arg, required=True)
    args = parser.parse_args()
    print(prepare(args.archive, args.parent_manifest, args.output))


if __name__ == '__main__':
    main()
