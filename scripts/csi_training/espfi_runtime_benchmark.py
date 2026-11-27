"""Frozen development parity and repeated paired CPU inference benchmark.

Run with OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1. Never loads test arrays or
trains a model. The float32 feature probe is diagnostic and cannot be exported.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import time
import numpy as np
from scipy.fft import rfft
from .espfi import load_development
from .espfi_generalize import features
from .espfi_kernel import predict_features
from .espfi_runtime import BufferedKernelBundle, SPEC, load_state
from .espfi_train import digest, scores


def float32_probe(values):
    """Rejected unless both real and stress parity pass; no deployment API."""
    x = np.asarray(values, dtype=np.float32)
    logged = np.log(np.maximum(x, np.float32(1e-6)))
    centered = logged - logged.mean(-1, keepdims=True)
    relative = centered / np.maximum(np.sqrt(np.mean(centered**2, axis=(1, 2), keepdims=True)), 1e-6)
    temporal = np.stack([v.mean(-1) for v in np.array_split(relative, 4, axis=-1)], axis=-1)
    power = np.abs(rfft(relative, axis=-1))**2 / (190**2)
    bands = np.stack([power[..., a:b].sum(-1) for a, b in
                      ((1, 4), (4, 12), (12, 32), (32, 96))], axis=-1)
    return np.concatenate((temporal.reshape(len(x), -1), bands.reshape(len(x), -1),
                           np.sqrt(np.mean(relative**2, axis=-1))), axis=1)


def benchmark(manifest, model, repetitions=1000, rounds=3):
    if not 100 <= repetitions <= 10000 or not 3 <= rounds <= 10:
        raise ValueError('Expected 100..10000 pairs and 3..10 rounds')
    if any(os.environ.get(k) != '1' for k in ('OPENBLAS_NUM_THREADS', 'OMP_NUM_THREADS')):
        raise ValueError('Set OPENBLAS_NUM_THREADS=1 OMP_NUM_THREADS=1 before launch')
    parts, provenance = load_development(manifest)
    x, y = parts['validation']['x'], parts['validation']['y']
    state = load_state(model)
    reference = lambda v: predict_features(state, features(v, SPEC['family']))
    models = {name: BufferedKernelBundle(model, fuse=fuse) for name, fuse in
              (('buffered', False), ('fused', True))}
    calls = {'reference': reference, **{k: v.predict for k, v in models.items()}}
    expected = reference(x)
    parity = {}
    for name, candidate in models.items():
        actual = candidate.predict(x)
        error = float(np.abs(expected-actual).max())
        equal = bool(np.array_equal(expected.argmax(1), actual.argmax(1)))
        if error > 1e-4 or not equal:
            raise ValueError(f'{name} failed parity: {error}, predictions_equal={equal}')
        parity[name] = {'max_abs_logit_error': error, 'predictions_equal': equal,
                        'retained_model_bytes': sum(v.nbytes for v in candidate.state.values()),
                        'workspace_bytes': candidate.workspace_bytes}
    for i in range(30):
        for call in calls.values():
            call(x[i % len(x):i % len(x)+1])
    timings = []
    names = list(calls)
    for round_index in range(rounds):
        elapsed = {name: [] for name in names}
        for i in range(repetitions):
            sample = x[i % len(x):i % len(x)+1]
            order = names[(i+round_index) % 3:] + names[:(i+round_index) % 3]
            if i % 2:
                order = order[::-1]
            for name in order:
                start = time.perf_counter_ns()
                calls[name](sample)
                elapsed[name].append((time.perf_counter_ns()-start)/1e6)
        timings.append({name: {'p50_ms': float(np.percentile(v, 50)),
                               'p95_ms': float(np.percentile(v, 95))} for name, v in elapsed.items()})
    for name in models:
        parity[name]['speed_gate_passed'] = all(
            r[name]['p50_ms'] <= .9*r['reference']['p50_ms'] and
            r[name]['p95_ms'] <= r['reference']['p95_ms'] for r in timings)
    # This stress signal deliberately exposes cancellation in single precision.
    rng = np.random.default_rng(7)
    stress = np.stack((np.ones((52, 190)), np.full((52, 190), 1e8),
                       1e8 * (1+rng.normal(0, 1e-8, (52, 190)))))
    probe_error = float(np.abs(reference(stress)-predict_features(state, float32_probe(stress))).max())
    probe_dev = predict_features(state, float32_probe(x))
    report = {'evidence': 'MEASURED', 'promotionAuthorized': False,
              'prediction_count': len(x), 'tolerance': 1e-4, 'parity': parity,
              'rounds': timings, 'pairs_per_round': repetitions,
              'development': scores(y, expected.argmax(1), 7),
              'float32_feature_probe': {'evidence': 'SYNTHETIC stress plus development parity',
                  'stress_max_abs_logit_error': probe_error,
                  'development_max_abs_logit_error': float(np.abs(expected-probe_dev).max()),
                  'development_predictions_equal': bool(np.array_equal(expected.argmax(1), probe_dev.argmax(1))),
                  'stress_gate_passed': probe_error <= 1e-4, 'exported': False},
              'model_sha256': digest(Path(model)/'model.safetensors'),
              'metadata_sha256': digest(Path(model)/'metadata.json'),
              'source_sha256': {name: digest(Path(__file__).with_name(name)) for name in
                  ('espfi_runtime.py', 'espfi_runtime_benchmark.py', 'espfi_generalize.py', 'espfi_kernel.py')},
              'runtime': {'python': platform.python_version(), 'numpy': np.__version__,
                          'machine': platform.machine(), 'threads': 1},
              'provenance': provenance,
              'scope': 'Rotating paired batch1 calls cycling development windows; includes checks, '
                       'features and classifier; excludes acquisition, pooling, IO. Shared CPU, '
                       'not edge hardware. Retained tensors/workspace are not total process RAM. '
                       'No training or final-test evaluation. Repeatedly observed development set.'}
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', required=True)
    parser.add_argument('--model', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--repetitions', type=int, default=1000)
    parser.add_argument('--rounds', type=int, default=3)
    args = parser.parse_args()
    output = Path(args.output)
    # Reserve without overwriting a prior receipt; failed runs stay incomplete.
    with output.open('x') as handle:
        handle.write('{"status":"running","promotionAuthorized":false}\n')
    report = benchmark(args.manifest, args.model, args.repetitions, args.rounds)
    report['status'] = 'complete'
    output.write_text(json.dumps(report, indent=2, allow_nan=False)+'\n')
    print(json.dumps({k: v for k, v in report.items() if k not in ('provenance', 'source_sha256', 'development')}, indent=2))


if __name__ == '__main__':
    main()
