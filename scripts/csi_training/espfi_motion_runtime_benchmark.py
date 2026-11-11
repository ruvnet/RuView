"""Frozen development-only buffered inference benchmark; never trains or promotes."""
import argparse
import json
import os
from pathlib import Path
import platform
import time

import numpy as np
from .espfi import load_development
from .espfi_motion import MotionBundle, _read, _validate
from .espfi_motion_compact import BASELINE, CompactMotionBundle
from .espfi_train import digest

ROUNDS, CALLS, WARMUPS, SEED = 3, 1000, 100, 42
SOURCES = ('espfi_motion_runtime_benchmark.py', 'espfi_motion_runtime.py',
           'espfi_motion_compact.py', 'espfi_motion.py', 'espfi_kernel.py',
           'espfi_generalize.py', 'espfi_train.py', 'espfi.py')


def _summary(values):
    return {'min_ms': float(np.min(values)), 'p50_ms': float(np.percentile(values, 50)),
            'p95_ms': float(np.percentile(values, 95)),
            'derived_calls_per_second': float(1000 / np.mean(values))}


def _measure(candidate, reference, dev):
    rounds = []
    for number in range(ROUNDS):
        order = np.random.default_rng(SEED + number).permutation(len(dev))
        calls = (('buffered', candidate.predict), ('folded_reference', reference.predict))
        for i in range(WARMUPS):
            x = dev[order[i % len(order)]:order[i % len(order)] + 1]
            for _, fn in calls[::1 if (i + number) % 2 == 0 else -1]:
                fn(x)
        values = {name: [] for name, _ in calls}
        for i in range(CALLS):
            x = dev[order[i % len(order)]:order[i % len(order)] + 1]
            for name, fn in calls[::1 if (i + number) % 2 == 0 else -1]:
                before = time.perf_counter_ns()
                fn(x)
                values[name].append((time.perf_counter_ns() - before) / 1e6)
        result = {name: _summary(v) for name, v in values.items()}
        result['round'] = number + 1
        result['seed'] = SEED + number
        result['gain_at_least_10percent'] = all(
            result['buffered'][q] <= .9 * result['folded_reference'][q]
            for q in ('p50_ms', 'p95_ms'))
        rounds.append(result)
    return rounds


def _parity(parts, candidate, references):
    result = {}
    for split, part in parts.items():
        checks = {name: {'max_abs_logit_error': 0., 'identical_predictions': True}
                  for name in references}
        # Exercise both the timed batch-one path and bounded batch iteration.
        for size in (1, 32):
            for offset in range(0, len(part['x']), size):
                x = part['x'][offset:offset + size]
                actual = candidate.predict(x)
                for name, reference in references.items():
                    expected = reference.predict(x)
                    if actual.shape != expected.shape or actual.dtype != expected.dtype or not np.isfinite(actual).all():
                        raise ValueError('Invalid buffered scores')
                    error = float(np.max(np.abs(actual - expected)))
                    equal = bool(np.array_equal(actual.argmax(1), expected.argmax(1)))
                    checks[name]['max_abs_logit_error'] = max(checks[name]['max_abs_logit_error'], error)
                    checks[name]['identical_predictions'] &= equal
                    if error > 1e-10 or not equal:
                        raise ValueError('Buffered runtime parity gate failed')
        result[split] = {'samples': len(part['x']), 'batch_sizes': [1, 32], 'references': checks}
    return result


def benchmark(manifest, model, baseline, output):
    """Reserve a new receipt before loading inputs; failure remains explicit."""
    output, manifest, model, baseline = map(Path, (output, manifest, model, baseline))
    # Exclusive creation never overwrites an existing receipt, including symlinks.
    with output.open('x') as handle:
        json.dump({'status': 'running', 'promotionAuthorized': False}, handle)
    def record(value):
        with output.open('w') as handle:
            json.dump(value, handle, indent=2, allow_nan=False)
            handle.write('\n')
    started = time.monotonic()
    try:
        from .espfi_motion_runtime import BufferedMotionBundle
        paths = {'manifest': manifest, 'compact_artifact': model/'model.safetensors',
                 'compact_metadata': model/'metadata.json', 'compact_status': model/'status.json',
                 'original_artifact': baseline/'model.safetensors',
                 'original_receipt': baseline/'metrics.json', 'original_status': baseline/'status.json'}
        hashes = {name: digest(path) for name, path in paths.items()}
        source_hashes = {name: digest(Path(__file__).with_name(name)) for name in SOURCES}
        parts, provenance = load_development(manifest)
        _validate(parts)
        metadata = json.loads(_read(model/'metadata.json', 65536))
        prior = json.loads(_read(baseline/'metrics.json', 16*1024**2))
        if (provenance.get('manifest_sha256') != hashes['manifest']
                or metadata.get('provenance', {}).get('manifest_sha256') != hashes['manifest']
                or prior.get('provenance', {}).get('manifest_sha256') != hashes['manifest']
                or metadata.get('baseline_model_sha256') != hashes['original_artifact']
                or metadata.get('baseline_receipt_sha256') != hashes['original_receipt']):
            raise ValueError('Input provenance mismatch')
        if provenance.get('evidence') != 'SYNTHETIC' and {k: len(v['x']) for k, v in parts.items()} != {'train': 837, 'validation': 70}:
            raise ValueError('Unexpected admitted development population')
        candidate = BufferedMotionBundle(model)
        reference = CompactMotionBundle(model)
        original = MotionBundle(baseline)
        if original.candidate != BASELINE:
            raise ValueError('Unexpected original candidate')
        parity = _parity(parts, candidate, {'folded_reference': reference, 'original': original})
        rounds = _measure(candidate, reference, parts['validation']['x'])
        if any(digest(path) != hashes[name] for name, path in paths.items()):
            raise ValueError('Inputs changed during benchmark')
        if any(digest(Path(__file__).with_name(name)) != value for name, value in source_hashes.items()):
            raise ValueError('Sources changed during benchmark')
        report = {'status': 'complete', 'promotionAuthorized': False,
                  'evidence': 'SYNTHETIC' if provenance.get('evidence') == 'SYNTHETIC' else 'MEASURED',
                  'task': 'buffered_motion_inference_benchmark', 'hashes': hashes,
                  'source_sha256': source_hashes, 'provenance': provenance, 'parity': parity,
                  'rounds': rounds, 'retained_runtime': all(r['gain_at_least_10percent'] for r in rounds),
                  'protocol': {'rounds': ROUNDS, 'calls_per_runtime_per_round': CALLS,
                               'warmups_per_runtime_per_round': WARMUPS, 'seed': SEED,
                               'batch_size': 1, 'alternating_runtime_order': True,
                               'rotating_seeded_development_order': True,
                               'primary_reference': 'CompactMotionBundle default folded',
                               'original_reference': 'parity only'},
                  'memory_bytes': {'buffered_model_arrays': int(candidate.model_state_bytes),
                                   'buffered_workspace': int(candidate.workspace_bytes),
                                   'folded_reference_arrays': sum(v.nbytes for v in vars(reference).values() if isinstance(v, np.ndarray))},
                  'environment': {'python': platform.python_version(), 'numpy': np.__version__,
                                  'platform': platform.platform(), 'machine': platform.machine(),
                                  'thread_environment': {k: os.environ.get(k) for k in ('OPENBLAS_NUM_THREADS', 'OMP_NUM_THREADS', 'MKL_NUM_THREADS')}},
                  'elapsed_seconds': time.monotonic() - started,
                  'scope': 'Full predict includes input checks, features and classifier. Excludes model load, pooling, acquisition and IO. Throughput is derived from mean batch-one latency, not raw CSI throughput. No training or final-test array access; reused development data, not independent accuracy confirmation.'}
        record(report)
        return report
    except Exception as exc:
        record({'status': 'failed', 'promotionAuthorized': False, 'error_type': type(exc).__name__})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest', 'model', 'baseline', 'output'):
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args()
    print(json.dumps(benchmark(args.manifest, args.model, args.baseline, args.output), indent=2))


if __name__ == '__main__':
    main()
