"""Frozen cross-carrier relationship ablation on development-only public amplitudes."""
import argparse
import hashlib
import json
from pathlib import Path
import time

import numpy as np
from safetensors.numpy import load, save_file

from .espfi_kernel import fit, predict_features
from .espfi_motion import MotionBundle, _read, _validate, motion_features
from .espfi_train import digest, scores, write_json

WIDTHS = {'motion_acf': 468, 'cross_corr': 1326, 'motion_cross': 1794}
GRID = [dict(family=family, kind='rff', dimensions=128, gamma_multiplier=gamma, alpha=alpha)
        for family in WIDTHS for gamma in (.1, 1.) for alpha in (.01, .1, 1.)]
BASELINE = dict(family='motion_acf', kind='rff', dimensions=128, gamma_multiplier=.1, alpha=.1)
CONTROL = BASELINE
CONTRACT = {
    'seed': 42, 'grid': GRID, 'input': 'nonnegative_amplitude_N52T190',
    'selection': 'minimum training-fold macro F1, then mean, then fewer stored scalars, then declaration order',
    'log_floor': 1e-6, 'channel_energy_floor': 190e-12, 'transform_batch_size': 32,
    'correlation': 'center log amplitude per channel; divide by sqrt(max(sum(centered**2),190e-12)); dot normalized channels; upper triangle excluding diagonal, row-major carrier indices',
    'coordinates': 'Fixed dataset carrier order only; no physical spacing or calibrated frequency inferred',
    'retention_gate': {'development_macro_f1_absolute_gain': .03,
                       'accuracy_nonregression': True,
                       'protect_each_previously_nonzero_class': True,
                       'paired_p95_limit_ms': 1.0},
    'promotionAuthorized': False,
}


def spatial_features(values, family):
    if family not in WIDTHS:
        raise ValueError('Unknown spatial feature family')
    x = np.asarray(values)
    if (x.dtype.kind not in 'fiu' or x.ndim != 3 or x.shape[1:] != (52, 190)
            or not 0 < len(x) <= 4096 or not np.isfinite(x).all() or (x < 0).any()):
        raise ValueError('Invalid finite nonnegative spatial input shape')
    if family == 'motion_acf':
        return motion_features(x, 'motion_acf')
    rows, upper = [], np.triu_indices(52, k=1)
    for start in range(0, len(x), 32):
        block = x[start:start+32].astype(np.float64, copy=False)
        logged = np.log(np.maximum(block, 1e-6))
        centered = logged - logged.mean(-1, keepdims=True)
        norm = np.sqrt(np.maximum(np.sum(centered**2, axis=-1, keepdims=True), 190e-12))
        normalized = centered / norm
        matrix = normalized @ normalized.transpose(0, 2, 1)
        rows.append(matrix[:, upper[0], upper[1]])
    corr = np.concatenate(rows)
    result = corr if family == 'cross_corr' else np.concatenate((motion_features(x, 'motion_acf'), corr), axis=1)
    if result.shape != (len(x), WIDTHS[family]) or not np.isfinite(result).all():
        raise ValueError('Nonfinite spatial features')
    return np.ascontiguousarray(result)


def validate_parts(parts):
    fold_ids = _validate(parts)
    for part in parts.values():
        x = np.asarray(part['x'])
        if (not 0 < len(x) <= 4096 or x.shape != (len(part['y']), 52, 190)
                or x.dtype.kind not in 'fiu' or not np.isfinite(x).all() or (x < 0).any()):
            raise ValueError('Invalid bounded spatial partition')
    return fold_ids


class SpatialBundle:
    def __init__(self, directory):
        root = Path(directory)
        status = json.loads(_read(root / 'status.json', 65536))
        if not isinstance(status, dict) or status.get('status') != 'complete':
            raise ValueError('Invalid completed spatial artifact')
        report = json.loads(_read(root / 'metrics.json', 16 * 1024**2))
        if (not isinstance(status, dict) or not isinstance(report, dict)
                or status.get('status') != 'complete' or report.get('status') != 'complete'
                or status.get('promotionAuthorized') is not False
                or report.get('promotionAuthorized') is not False
                or report.get('contract') != CONTRACT
                or report.get('task') != 'public_spatial_ablation'):
            raise ValueError('Invalid completed spatial artifact')
        self.candidate = report['selected']['candidate']
        if self.candidate not in GRID:
            raise ValueError('Unknown spatial candidate')
        data = _read(root / 'model.safetensors', 4 * 1024**2)
        if hashlib.sha256(data).hexdigest() != report.get('model_sha256'):
            raise ValueError('Model hash mismatch')
        self.state = load(data)
        width = WIDTHS[self.candidate['family']]
        shapes = {'feature_mean': (width,), 'feature_scale': (width,),
                  'projection': (width, 128), 'phase': (128,), 'output_mean': (128,),
                  'weights': (128, 7), 'bias': (7,)}
        if (set(self.state) != set(shapes) or any(self.state[k].shape != shape
                or self.state[k].dtype != np.float64 or not np.isfinite(self.state[k]).all()
                for k, shape in shapes.items()) or not (self.state['feature_scale'] > 0).all()):
            raise ValueError('Invalid spatial model state')

    def predict(self, values):
        return predict_features(self.state, spatial_features(values, self.candidate['family']))


def search(parts, provenance, baseline_directory, output, max_seconds=600):
    if not 1 <= max_seconds <= 3600:
        raise ValueError('Invalid time budget')
    fold_ids = validate_parts(parts)
    baseline = MotionBundle(baseline_directory)
    prior_bytes = _read(Path(baseline_directory) / 'metrics.json', 16 * 1024**2)
    prior = json.loads(prior_bytes)
    if (baseline.candidate != BASELINE or not provenance.get('manifest_sha256')
            or prior.get('provenance', {}).get('manifest_sha256') != provenance.get('manifest_sha256')):
        raise ValueError('Baseline model or data provenance mismatch')
    root = Path(output)
    root.mkdir(exist_ok=False, parents=True)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    write_json(root / 'search_contract.json', CONTRACT)
    start = time.monotonic()
    try:
        train, dev = parts['train'], parts['validation']
        cache = {family: spatial_features(train['x'], family) for family in WIDTHS}
        results, selected, best = [], None, None
        for candidate in GRID:
            evaluations = []
            for fit_ids, eval_ids in fold_ids:
                if time.monotonic() - start > max_seconds:
                    raise TimeoutError('Spatial search budget exceeded')
                state = fit(cache[candidate['family']][fit_ids], train['y'][fit_ids], candidate)
                result = predict_features(state, cache[candidate['family']][eval_ids])
                evaluations.append(scores(train['y'][eval_ids], result.argmax(1), 7))
            row = {'candidate': candidate, 'folds': evaluations,
                   'minimum_cv_macro_f1': min(v['macro_f1'] for v in evaluations),
                   'mean_cv_macro_f1': float(np.mean([v['macro_f1'] for v in evaluations])),
                   'scalar_count': sum(v.size for v in state.values())}
            results.append(row)
            rank = (row['minimum_cv_macro_f1'], row['mean_cv_macro_f1'], -row['scalar_count'])
            if best is None or rank > best:
                best, selected = rank, row
        # Persist the complete train-only choice before any development feature extraction.
        write_json(root / 'selection.json', {'selected': selected, 'candidates': results, 'contract': CONTRACT})
        chosen = selected['candidate']
        state = fit(cache[chosen['family']], train['y'], chosen)
        # Require the unchanged control to reproduce the exact existing learned state.
        control = fit(cache['motion_acf'], train['y'], BASELINE)
        if any(not np.array_equal(control[k], baseline.state[k]) for k in control):
            raise ValueError('Unchanged baseline state did not reproduce')
        save_file(state, str(root / 'model.safetensors'))
        actual = predict_features(state, spatial_features(dev['x'], chosen['family']))
        incumbent = scores(dev['y'], baseline.predict(dev['x']).argmax(1), 7)
        measured = scores(dev['y'], actual.argmax(1), 7)
        replay = load(_read(root / 'model.safetensors', 4 * 1024**2))
        if not np.array_equal(actual, predict_features(replay, spatial_features(dev['x'], chosen['family']))):
            raise ValueError('Reload changes logits')
        def candidate_predict(x):
            return predict_features(state, spatial_features(x, chosen['family']))
        for i in range(20):
            j = i % len(dev['y'])
            candidate_predict(dev['x'][j:j+1])
            baseline.predict(dev['x'][j:j+1])
        timings = {'candidate': [], 'incumbent': []}
        for i in range(1000):
            if time.monotonic() - start > max_seconds:
                raise TimeoutError('Spatial search budget exceeded')
            j = i % len(dev['y'])
            calls = [('candidate', candidate_predict, dev['x'][j:j+1]),
                     ('incumbent', baseline.predict, dev['x'][j:j+1])]
            if i % 2:
                calls.reverse()
            for name, fn, sample in calls:
                before = time.perf_counter_ns()
                fn(sample)
                timings[name].append((time.perf_counter_ns() - before) / 1e6)
        timing = {k: {'p50_ms': float(np.percentile(v, 50)), 'p95_ms': float(np.percentile(v, 95))}
                  for k, v in timings.items()}
        gate = CONTRACT['retention_gate']
        checks = {
            'macro_f1_gain': measured['macro_f1'] >= incumbent['macro_f1'] + gate['development_macro_f1_absolute_gain'],
            'accuracy_nonregression': measured['accuracy'] >= incumbent['accuracy'],
            'protect_each_previously_nonzero_class': all(b == 0 or a > 0 for a, b in
                                                       zip(measured['per_class_f1'], incumbent['per_class_f1'])),
            'p95_under_1ms': timing['candidate']['p95_ms'] < gate['paired_p95_limit_ms'],
        }
        report = {
            'status': 'complete', 'promotionAuthorized': False, 'task': 'public_spatial_ablation',
            'contract': CONTRACT, 'selected': selected, 'candidates': results,
            'baseline_control': next(r for r in results if r['candidate'] == CONTROL),
            'baseline_state_exact': True, 'reload_max_abs_logit_error': 0.0,
            'evidence': 'SYNTHETIC' if provenance.get('evidence') == 'SYNTHETIC' else 'MEASURED',
            'provenance': provenance, 'development': measured, 'incumbent_development': incumbent,
            'retention_checks': checks, 'retained_as_research_candidate': all(checks.values()),
            'latency': timing, 'model_bytes': (root / 'model.safetensors').stat().st_size,
            'model_sha256': digest(root / 'model.safetensors'),
            'baseline_model_sha256': prior['model_sha256'],
            'baseline_receipt_sha256': hashlib.sha256(prior_bytes).hexdigest(),
            'source_sha256': {name: digest(Path(__file__).with_name(name)) for name in
                              ('espfi_spatial.py', 'espfi.py', 'espfi_motion.py', 'espfi_generalize.py', 'espfi_kernel.py', 'espfi_train.py')},
            'elapsed_seconds': time.monotonic() - start,
            'scope': '18 configurations selected by original training folds; adaptive research on reused development data. '
                     '1000 paired batch1 calls include feature extraction/checks/classifier, exclude acquisition/pooling/IO. '
                     'Same pooledN52T190 inputs; no acquisition/pooling timing or end-to-end speed claim. No physical geometry, final-test or hardware accuracy claim.',
        }
        if time.monotonic() - start > max_seconds:
            raise TimeoutError('Spatial search budget exceeded')
        write_json(root / 'metrics.json', report)
        write_json(root / 'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except Exception:
        write_json(root / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest', 'baseline', 'output'):
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--max-seconds', type=int, default=600)
    args = parser.parse_args()
    from .espfi import load_development
    parts, provenance = load_development(args.manifest)
    result = search(parts, provenance, args.baseline, args.output, args.max_seconds)
    print(json.dumps({k: result[k] for k in ('selected', 'development', 'incumbent_development',
                     'retention_checks', 'retained_as_research_candidate', 'latency', 'model_bytes')}, indent=2))


if __name__ == '__main__':
    main()
