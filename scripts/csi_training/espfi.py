"""Pinned public ESP-Fi HAR intake. Separate raw-amplitude contract, not v2 features.

No archive paths are extracted. Labels remain numeric because the upstream README
and supplied benchmark class directories disagree about activity semantics.
"""
from __future__ import annotations
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import zipfile
import numpy as np

REVISION = 'c8bf6aa0d680fa02695c85a82b05e79b3ebb10c4'
ARCHIVE_SHA256 = '85f3df633c5b1a48ca4b1423467fce22821b21eeb3eed085a8d9f983dc156660'
ARCHIVE_BYTES = 644939981
SOURCE_URL = f'https://media.githubusercontent.com/media/AutoSmartGroup/ESP-Fi-HAR/{REVISION}/ESP-Fi_dataset.rar'
CLASSES = [f'activity_{i}' for i in range(1, 8)]
IDENTITY = re.compile(r'([1-4])-([1-8])-([1-7])-(10|[1-9])\.mat\Z')
MAX_MEMBER = 2_000_000
MAX_NPZ = 600_000_000


def digest(path):
    with Path(path).open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def parse_identity(name):
    p = PurePosixPath(name)
    if p.is_absolute() or '..' in p.parts or '\\' in name:
        raise ValueError('unsafe archive path')
    match = IDENTITY.fullmatch(p.name)
    if not match:
        raise ValueError('invalid capture identity')
    return tuple(map(int, match.groups()))


def split_identity(identity):
    """Strict joint holdout: every room and participant is disjoint across splits.

    Mixed boundary combinations are excluded, never reassigned. 840/70/70 samples
    retained from complete source; this deliberately sacrifices coverage for an
    honest unseen-room AND unseen-person test. Test is small: 10 trials per class.
    """
    room, subject, _, _ = identity
    if room in (1, 2) and subject <= 6:
        return 'train'
    if room == 3 and subject == 7:
        return 'validation'
    if room == 4 and subject == 8:
        return 'test'
    return None


def decode_amplitude(content, pool=5):
    from scipy.io import loadmat
    if len(content) > MAX_MEMBER or pool not in (1, 2, 5, 10, 19, 25, 50, 95):
        raise ValueError('invalid MAT size or pooling factor')
    mat = loadmat(io.BytesIO(content), variable_names=['CSIamp'])
    x = mat.get('CSIamp')
    if x is None or x.dtype.kind not in 'fiu' or x.shape not in ((950, 52), (52, 950)):
        raise ValueError('expected real CSIamp 950x52')
    if x.shape == (52, 950):
        x = x.T
    if not np.isfinite(x).all() or np.any(x < 0) or np.any(x > 1e12):
        raise ValueError('invalid amplitude values')
    # Averaging before decimation reduces high frequency aliasing. This is not a
    # calibrated resampling operation: source packet timestamps are unavailable.
    return np.ascontiguousarray(x.reshape(950 // pool, pool, 52).mean(1).T, dtype=np.float32)


def prepare(archive, output, pool=5):
    import libarchive
    archive, output = Path(archive), Path(output)
    if archive.stat().st_size != ARCHIVE_BYTES or digest(archive) != ARCHIVE_SHA256:
        raise ValueError('archive differs from pinned upstream LFS content')
    if output.exists():
        raise ValueError('output must not already exist')
    rows = {s: [] for s in ('train', 'validation', 'test')}
    seen, hashes, records = set(), {}, []
    count = 0
    with libarchive.file_reader(str(archive)) as entries:
        for entry in entries:
            if not entry.pathname.endswith('.mat'):
                continue
            count += 1
            if count > 2240 or not entry.isfile or not 0 < entry.size <= MAX_MEMBER:
                raise ValueError('unexpected archive member')
            ident = parse_identity(entry.pathname)
            if ident in seen:
                raise ValueError('duplicate recording identity')
            seen.add(ident)
            content = bytearray()
            for block in entry.get_blocks():
                content.extend(block)
                if len(content) > MAX_MEMBER:
                    raise ValueError('expanded MAT exceeds bound')
            x = decode_amplitude(content, pool)
            # Compare actual signal contents, not MATLAB timestamps/header bytes.
            signal_hash = hashlib.sha256(x.tobytes()).hexdigest()
            hashes[signal_hash] = hashes.get(signal_hash, 0) + 1
            split = split_identity(ident)
            records.append({'path': entry.pathname, 'sha256': hashlib.sha256(content).hexdigest(),
                            'identity': ident, 'split': split, 'signal_sha256': signal_hash})
            if split:
                rows[split].append((ident, x))
    if count != 2240:
        raise ValueError('incomplete upstream archive')
    duplicate_hashes = {h for h, n in hashes.items() if n > 1}
    for record in records:
        if record['signal_sha256'] in duplicate_hashes:
            record['split'] = None
            record['exclusion_reason'] = 'duplicate_signal_group'
    for split in rows:
        rows[split] = [(i, x) for i, x in rows[split]
                       if hashlib.sha256(x.tobytes()).hexdigest() not in duplicate_hashes]
    arrays = {}
    for split, samples in rows.items():
        samples.sort(key=lambda pair: pair[0])
        arrays[split + '_x'] = np.stack([v for _, v in samples])
        arrays[split + '_y'] = np.array([i[2] - 1 for i, _ in samples], dtype=np.int64)
        arrays[split + '_identity'] = np.array([i for i, _ in samples], dtype=np.int64)
    output.mkdir(parents=True)
    target = output / 'amplitudes.npz'
    np.savez_compressed(target, **arrays)
    manifest = {'schema_version': 1, 'dataset': 'ESP-Fi HAR', 'source_url': SOURCE_URL,
                'source_revision': REVISION, 'archive_sha256': ARCHIVE_SHA256,
                'license': 'CC-BY-4.0', 'license_url': f'https://github.com/AutoSmartGroup/ESP-Fi-HAR/blob/{REVISION}/Model%20Code/DATA_LICENSE.txt',
                'attribution': 'Wen et al. ESP-Fi HAR. Ad Hoc Networks 186 (2026) 104192. doi:10.1016/j.adhoc.2026.104192',
                'classes': CLASSES, 'semantic_labels_verified': False,
                'label_warning': 'README numeric activity mapping conflicts with benchmark directory names; use numeric IDs only.',
                'evidence': 'real', 'channels': 52, 'window_samples': 950 // pool,
                'temporal_mean_pool': pool, 'split_policy': 'joint_room_subject_intersection_v1',
                'duplicate_groups': len(duplicate_hashes), 'duplicate_recordings': sum(hashes[h] for h in duplicate_hashes),
                'counts': {s: len(v) for s, v in rows.items()}, 'excluded_count': count - sum(map(len, rows.values())),
                'npz': target.name, 'npz_sha256': digest(target), 'records': records}
    path = output / 'manifest.json'
    path.write_text(json.dumps(manifest, indent=2) + '\n')
    return path


def load_dataset(manifest):
    """Load all partitions for a final frozen evaluation, including test signals."""
    return _load_partitions(manifest, ('train', 'validation', 'test'))


def load_development(manifest):
    """Load only train/validation tensors for model development.

    The entire bundle is hashed and every array header inspected for integrity.
    Test array payloads are never deserialized or returned. Identity columns are
    room, participant, numeric activity, trial, suitable for grouped inner folds.
    """
    return _load_partitions(manifest, ('train', 'validation'))


def _load_partitions(manifest, splits):
    path = Path(manifest).resolve()
    if path.stat().st_size > 3_000_000:
        raise ValueError('manifest too large')
    meta = json.loads(path.read_text())
    if meta.get('schema_version') != 1 or meta.get('classes') != CLASSES:
        raise ValueError('unsupported dataset contract')
    if (meta.get('source_revision') != REVISION or meta.get('archive_sha256') != ARCHIVE_SHA256
            or meta.get('source_url') != SOURCE_URL or meta.get('license') != 'CC-BY-4.0'
            or meta.get('split_policy') != 'joint_room_subject_intersection_v1'
            or meta.get('temporal_mean_pool') not in (1, 2, 5, 10, 19, 25, 50, 95)
            or meta.get('window_samples') != 950 // meta['temporal_mean_pool']):
        raise ValueError('unverified source provenance')
    records = meta.get('records', [])
    expected = {(r, p, a, t) for r in range(1, 5) for p in range(1, 9)
                for a in range(1, 8) for t in range(1, 11)}
    if len(records) != 2240 or {tuple(r['identity']) for r in records} != expected:
        raise ValueError('incomplete source identity population')
    lookup = {tuple(r['identity']): r for r in records}
    occurrences = {}
    for record in records:
        h = record['signal_sha256']
        if not re.fullmatch('[0-9a-f]{64}', h):
            raise ValueError('invalid signal hash')
        occurrences[h] = occurrences.get(h, 0) + 1
    for ident, record in lookup.items():
        duplicate = occurrences[record['signal_sha256']] > 1
        if record['split'] != (None if duplicate else split_identity(ident)):
            raise ValueError('incorrect source split assignment')
    npz = (path.parent / meta['npz']).resolve()
    if npz.parent != path.parent or npz.stat().st_size > MAX_NPZ or digest(npz) != meta['npz_sha256']:
        raise ValueError('unsafe or modified dataset bundle')
    with zipfile.ZipFile(npz) as z:
        infos = z.infolist()
        expected_names = {s + suffix + '.npy' for s in ('train', 'validation', 'test')
                          for suffix in ('_x', '_y', '_identity')}
        if (sum(i.file_size for i in infos) > MAX_NPZ or len(infos) != 9
                or {i.filename for i in infos} != expected_names):
            raise ValueError('oversized or unexpected dataset bundle')
        for info in infos:
            split, suffix = info.filename[:-4].split('_', 1)
            count = sum(r['split'] == split for r in records)
            expected_shape = {'x': (count, 52, meta['window_samples']),
                              'y': (count,), 'identity': (count, 4)}[suffix]
            expected_dtype = np.dtype('float32' if suffix == 'x' else 'int64')
            with z.open(info) as f:
                version = np.lib.format.read_magic(f)
                if version == (1, 0):
                    shape, fortran, dtype = np.lib.format.read_array_header_1_0(f)
                elif version == (2, 0):
                    shape, fortran, dtype = np.lib.format.read_array_header_2_0(f)
                else:
                    raise ValueError('unsupported numpy header')
                if (shape != expected_shape or dtype != expected_dtype or fortran
                        or f.tell() + int(np.prod(shape)) * dtype.itemsize != info.file_size):
                    raise ValueError('invalid numpy header or payload size')
    parts, groups, signals, identities_seen = {}, {}, set(), set()
    with np.load(npz, allow_pickle=False) as data:
        for split in splits:
            x, y, identities = (data[split + suffix] for suffix in ('_x', '_y', '_identity'))
            if (x.dtype != np.float32 or x.ndim != 3 or x.shape[1:] != (52, meta['window_samples'])
                    or not 0 < len(x) <= 2240 or not np.isfinite(x).all()
                    or y.dtype != np.int64 or y.shape != (len(x),)
                    or identities.dtype != np.int64 or identities.shape != (len(x), 4)):
                raise ValueError('invalid tensor schema')
            if set(y.tolist()) != set(range(7)):
                raise ValueError('all seven classes required per split')
            for sample, label, identity in zip(x, y, identities):
                ident = tuple(map(int, identity))
                parse_identity('-'.join(map(str, ident)) + '.mat')
                if split_identity(ident) != split or label != ident[2] - 1:
                    raise ValueError('split or class provenance mismatch')
                h = hashlib.sha256(sample.tobytes()).hexdigest()
                if ident in identities_seen or h != lookup[ident]['signal_sha256'] or lookup[ident]['split'] != split:
                    raise ValueError('recording provenance mismatch')
                identities_seen.add(ident)
                if h in signals:
                    raise ValueError('duplicate signal across dataset')
                signals.add(h)
            groups[split] = (set(identities[:, 0]), set(identities[:, 1]))
            parts[split] = {'x': x, 'y': y, 'identity': identities}
    for a, b in (('train', 'validation'), ('train', 'test'), ('validation', 'test')):
        if a in groups and b in groups and any(groups[a][i] & groups[b][i] for i in (0, 1)):
            raise ValueError('room or participant leakage')
    if identities_seen != {i for i, r in lookup.items() if r['split'] in splits}:
        raise ValueError('incomplete retained population')
    expected_counts = {s: sum(r['split'] == s for r in records)
                       for s in ('train', 'validation', 'test')}
    if meta.get('counts') != expected_counts:
        raise ValueError('split counts mismatch')
    meta['manifest_sha256'] = digest(path)
    meta['loaded_partitions'] = list(splits)
    return parts, meta


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--pool', type=int, default=5)
    args = parser.parse_args()
    print(prepare(args.archive, args.output, args.pool))


if __name__ == '__main__':
    main()
