"""Frozen native-resolution ablation on development-only public amplitudes."""
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

WIDTHS = {'pooled190': 468, 'raw950_matched': 468, 'raw950_native': 468}
LAGS = {'pooled190': ([1, 2, 4, 8, 16, 32], [1, 4, 16]),
        'raw950_matched': ([5, 10, 20, 40, 80, 160], [5, 20, 80]),
        'raw950_native': ([1, 2, 4, 8, 16, 32], [1, 4, 16])}
GRID = [dict(family=family, kind='rff', dimensions=128, gamma_multiplier=gamma, alpha=alpha)
        for family in WIDTHS for gamma in (.1, 1.) for alpha in (.01, .1, 1.)]
BASELINE = dict(family='motion_acf', kind='rff', dimensions=128, gamma_multiplier=.1, alpha=.1)
CONTROL = dict(BASELINE, family='pooled190')
CONTRACT = {
    'seed': 42, 'grid': GRID, 'input': {'pooled190': [52, 190], 'raw950_matched': [52, 950], 'raw950_native': [52, 950]},
    'selection': 'minimum training-fold macro F1, then mean, then declaration order',
    'lags': {name: {'acf': acf, 'difference': diff} for name, (acf, diff) in LAGS.items()},
    'log_floor': 1e-6, 'rms_floor': 1e-6, 'energy_floor': 'T * 1e-12',
    'raw_transform_batch_size': 32,
    'units': 'Original sample-index spans only; no measured sampling rate or physical time inferred',
    'retention_gate': {'development_macro_f1_absolute_gain': .03,
                       'accuracy_nonregression': True,
                       'protect_each_previously_nonzero_class': True,
                       'paired_p95_limit_ms': 1.0},
    'promotionAuthorized': False,
}


def resolution_features(values, family):
    if family not in WIDTHS:
        raise ValueError('Unknown resolution feature family')
    x = np.asarray(values)
    shape = tuple(CONTRACT['input'][family])
    if (x.dtype.kind not in 'fiu' or x.ndim != 3 or x.shape[1:] != shape
            or not 0 < len(x) <= 4096 or not np.isfinite(x).all() or (x < 0).any()):
        raise ValueError('Invalid finite nonnegative resolution input shape')
    if family == 'pooled190':
        return motion_features(x, 'motion_acf')
    # All reductions are within one recording; bounded chunks preserve arithmetic.
    return np.ascontiguousarray(np.concatenate([_raw_features(x[start:start+32], family)
                                                for start in range(0, len(x), 32)]))


def _raw_features(values, family):
    x = values.astype(np.float64, copy=False)
    logged = np.log(np.maximum(x, 1e-6))
    centered = logged - logged.mean(-1, keepdims=True)
    scale = np.maximum(np.sqrt(np.mean(centered**2, axis=(1, 2), keepdims=True)), 1e-6)
    relative = centered / scale
    energy = np.maximum(np.sum(centered**2, axis=-1), x.shape[-1] * 1e-12)
    acf_lags, diff_lags = LAGS[family]
    acf = [np.sum(centered[..., lag:] * centered[..., :-lag], axis=-1) / energy for lag in acf_lags]
    changes = [np.mean(np.abs(relative[..., lag:] - relative[..., :-lag]), axis=-1) for lag in diff_lags]
    result = np.ascontiguousarray(np.stack(acf + changes, axis=-1).reshape(len(x), 468))
    if not np.isfinite(result).all():
        raise ValueError('Nonfinite resolution features')
    return result


def model_input(part, family):
    return part['pooled'] if family == 'pooled190' else part['x']


def validate_parts(parts):
    fold_ids = _validate(parts)
    for part in parts.values():
        n = len(part['y'])
        if not 0 < n <= 4096:
            raise ValueError('Resolution partition exceeds recording bound')
        for name, shape, dtype in [('x', (n, 52, 950), np.float64), ('pooled', (n, 52, 190), np.float32)]:
            value = np.asarray(part[name])
            if (value.shape != shape or value.dtype != dtype or not np.isfinite(value).all() or (value < 0).any()):
                raise ValueError('Invalid resolution partition precision or shape')
    return fold_ids


class ResolutionBundle:
    def __init__(self, directory):
        root = Path(directory)
        status = json.loads(_read(root / 'status.json', 65536))
        if not isinstance(status, dict) or status.get('status') != 'complete':
            raise ValueError('Invalid completed resolution artifact')
        report = json.loads(_read(root / 'metrics.json', 16 * 1024**2))
        if (not isinstance(status, dict) or not isinstance(report, dict)
                or status.get('status') != 'complete' or report.get('status') != 'complete'
                or status.get('promotionAuthorized') is not False
                or report.get('promotionAuthorized') is not False
                or report.get('contract') != CONTRACT
                or report.get('task') != 'public_resolution_ablation'):
            raise ValueError('Invalid completed resolution artifact')
        self.candidate = report['selected']['candidate']
        if self.candidate not in GRID:
            raise ValueError('Unknown resolution candidate')
        data = _read(root / 'model.safetensors', 1024**2)
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
            raise ValueError('Invalid resolution model state')

    def predict(self, values):
        return predict_features(self.state, resolution_features(values, self.candidate['family']))


def search(parts, provenance, baseline_directory, output, max_seconds=600):
    if not 1 <= max_seconds <= 3600:
        raise ValueError('Invalid time budget')
    fold_ids = validate_parts(parts)
    baseline = MotionBundle(baseline_directory)
    prior_bytes = _read(Path(baseline_directory) / 'metrics.json', 16 * 1024**2)
    prior = json.loads(prior_bytes)
    if (baseline.candidate != BASELINE or not provenance.get('manifest_sha256') or provenance.get('pooled_exact') is not True
            or provenance.get('test_payload_decoded') is not False
            or prior.get('provenance', {}).get('manifest_sha256') != provenance.get('parent_manifest_sha256')):
        raise ValueError('Baseline model or data provenance mismatch')
    root = Path(output)
    root.mkdir(exist_ok=False, parents=True)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    write_json(root / 'search_contract.json', CONTRACT)
    start = time.monotonic()
    try:
        train, dev = parts['train'], parts['validation']
        cache = {family: resolution_features(model_input(train, family), family) for family in WIDTHS}
        results, selected, best = [], None, None
        for candidate in GRID:
            evaluations = []
            for fit_ids, eval_ids in fold_ids:
                if time.monotonic() - start > max_seconds:
                    raise TimeoutError('Resolution search budget exceeded')
                state = fit(cache[candidate['family']][fit_ids], train['y'][fit_ids], candidate)
                result = predict_features(state, cache[candidate['family']][eval_ids])
                evaluations.append(scores(train['y'][eval_ids], result.argmax(1), 7))
            row = {'candidate': candidate, 'folds': evaluations,
                   'minimum_cv_macro_f1': min(v['macro_f1'] for v in evaluations),
                   'mean_cv_macro_f1': float(np.mean([v['macro_f1'] for v in evaluations])),
                   'scalar_count': sum(v.size for v in state.values())}
            results.append(row)
            rank = (row['minimum_cv_macro_f1'], row['mean_cv_macro_f1'])
            if best is None or rank > best:
                best, selected = rank, row
        # Persist the complete train-only choice before any development feature extraction.
        write_json(root / 'selection.json', {'selected': selected, 'candidates': results, 'contract': CONTRACT})
        chosen = selected['candidate']
        state = fit(cache[chosen['family']], train['y'], chosen)
        # Require the unchanged control to reproduce the exact existing learned state.
        control = fit(cache['pooled190'], train['y'], BASELINE)
        if any(not np.array_equal(control[k], baseline.state[k]) for k in control):
            raise ValueError('Unchanged baseline state did not reproduce')
        save_file(state, str(root / 'model.safetensors'))
        actual = predict_features(state, resolution_features(model_input(dev, chosen['family']), chosen['family']))
        incumbent = scores(dev['y'], baseline.predict(dev['pooled']).argmax(1), 7)
        measured = scores(dev['y'], actual.argmax(1), 7)
        replay = load(_read(root / 'model.safetensors', 1024**2))
        if not np.array_equal(actual, predict_features(replay, resolution_features(model_input(dev, chosen['family']), chosen['family']))):
            raise ValueError('Reload changes logits')
        def candidate_predict(x):
            return predict_features(state, resolution_features(x, chosen['family']))
        for i in range(20):
            j = i % len(dev['y'])
            candidate_predict(model_input(dev, chosen['family'])[j:j+1])
            baseline.predict(dev['pooled'][j:j+1])
        timings = {'candidate': [], 'incumbent': []}
        for i in range(1000):
            if time.monotonic() - start > max_seconds:
                raise TimeoutError('Resolution search budget exceeded')
            j = i % len(dev['y'])
            calls = [('candidate', candidate_predict, model_input(dev, chosen['family'])[j:j+1]),
                     ('incumbent', baseline.predict, dev['pooled'][j:j+1])]
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
            'status': 'complete', 'promotionAuthorized': False, 'task': 'public_resolution_ablation',
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
                              ('espfi_resolution.py', 'espfi_resolution_data.py', 'espfi_motion.py', 'espfi_generalize.py', 'espfi_kernel.py', 'espfi_train.py')},
            'elapsed_seconds': time.monotonic() - start,
            'scope': '18 configurations selected by original training folds; adaptive research on reused development data. '
                     '1000 paired batch1 calls include feature extraction/checks/classifier, exclude acquisition/pooling/IO. '
                     'Respective input arrays: selected family190/950, baseline190; no acquisition/pooling timing or end-to-end speed claim. No physical-time, final-test or hardware accuracy claim.',
        }
        if time.monotonic() - start > max_seconds:
            raise TimeoutError('Resolution search budget exceeded')
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
    from .espfi_resolution_data import load_resolution
    parts, provenance = load_resolution(args.manifest)
    result = search(parts, provenance, args.baseline, args.output, args.max_seconds)
    print(json.dumps({k: result[k] for k in ('selected', 'development', 'incumbent_development',
                     'retention_checks', 'retained_as_research_candidate', 'latency', 'model_bytes')}, indent=2))


if __name__ == '__main__':
    main()
