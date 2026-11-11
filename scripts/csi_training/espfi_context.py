"""Bounded within-recording order ablation; not inferred room context.

Only source training folds select the candidate. Each recording is independent:
no artificial timeline, empty-room baseline, graph belief or target label enters
the features. The separate Rust probe exercises actual field/graph integration.
"""
import argparse
import hashlib
import json
from pathlib import Path
import time

import numpy as np
from safetensors.numpy import load, save_file

from .espfi_generalize import amplitudes
from .espfi_kernel import fit, predict_features
from .espfi_motion import MotionBundle, _read, _validate, motion_features
from .espfi_train import digest, scores, write_json

WIDTHS = {'motion_acf': 468, 'ordered_mean': 624, 'ordered_trend': 676}
GRID = [dict(family=family, kind='rff', dimensions=128, gamma_multiplier=gamma, alpha=alpha)
        for family in WIDTHS for gamma in (.1, 1.) for alpha in (.01, .1, 1.)]
BASELINE = dict(family='motion_acf', kind='rff', dimensions=128, gamma_multiplier=.1, alpha=.1)
CONTRACT = {
    'seed': 42, 'grid': GRID, 'input': 'nonnegative_amplitude_N52T190',
    'selection': 'minimum training-fold macro F1, then mean, then fewer scalars, then declaration order',
    'ordered_bins': [[0, 63], [63, 127], [127, 190]],
    'trend': 'OLS slope on linspace(-1,1,190) of per-carrier centered log amplitude divided by global RMS',
    'log_floor': 1e-6, 'rms_floor': 1e-6,
    'context': 'within one recording only; no chronology, physical direction or room calibration inferred',
    'retention_gate': {'development_macro_f1_absolute_gain': .03,
                       'accuracy_nonregression': True,
                       'protect_each_previously_nonzero_class': True,
                       'paired_p95_limit_ms': 1.0},
    'promotionAuthorized': False,
}


def context_features(values, family):
    if family not in WIDTHS:
        raise ValueError('Unknown context feature family')
    x = amplitudes(values)
    original = motion_features(x, 'motion_acf')
    if family == 'motion_acf':
        return original
    logged = np.log(np.maximum(x, CONTRACT['log_floor']))
    relative = logged - logged.mean(-1, keepdims=True)
    relative /= np.maximum(np.sqrt(np.mean(relative**2, axis=(1, 2), keepdims=True)),
                           CONTRACT['rms_floor'])
    ordered = np.stack([relative[..., a:b].mean(-1) for a, b in CONTRACT['ordered_bins']], -1)
    chunks = [original, ordered.reshape(len(x), -1)]
    if family == 'ordered_trend':
        time_axis = np.linspace(-1., 1., 190)
        chunks.append((relative * time_axis).sum(-1) / np.dot(time_axis, time_axis))
    result = np.ascontiguousarray(np.concatenate(chunks, axis=1))
    if result.shape != (len(x), WIDTHS[family]) or not np.isfinite(result).all():
        raise ValueError('Invalid context feature values')
    return result


class ContextBundle:
    def __init__(self, directory):
        root = Path(directory)
        status = json.loads(_read(root / 'status.json', 65536))
        report = json.loads(_read(root / 'metrics.json', 16 * 1024**2))
        if (not isinstance(status, dict) or not isinstance(report, dict)
                or status.get('status') != 'complete' or report.get('status') != 'complete'
                or status.get('promotionAuthorized') is not False
                or report.get('promotionAuthorized') is not False
                or report.get('contract') != CONTRACT
                or report.get('task') != 'public_context_ablation'):
            raise ValueError('Invalid completed context artifact')
        self.candidate = report['selected']['candidate']
        if self.candidate not in GRID:
            raise ValueError('Unknown context candidate')
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
            raise ValueError('Invalid context model state')

    def predict(self, values):
        return predict_features(self.state, context_features(values, self.candidate['family']))


def search(parts, provenance, baseline_directory, output, max_seconds=600):
    if not 1 <= max_seconds <= 3600:
        raise ValueError('Invalid time budget')
    fold_ids = _validate(parts)
    baseline = MotionBundle(baseline_directory)
    prior_bytes = _read(Path(baseline_directory) / 'metrics.json', 16 * 1024**2)
    prior = json.loads(prior_bytes)
    if (baseline.candidate != BASELINE or not provenance.get('manifest_sha256')
            or prior.get('provenance', {}).get('manifest_sha256') != provenance['manifest_sha256']):
        raise ValueError('Baseline model or data provenance mismatch')
    root = Path(output)
    root.mkdir(exist_ok=False, parents=True)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    write_json(root / 'search_contract.json', CONTRACT)
    start = time.monotonic()
    try:
        train, dev = parts['train'], parts['validation']
        cache = {family: context_features(train['x'], family) for family in WIDTHS}
        results, selected, best = [], None, None
        for candidate in GRID:
            evaluations = []
            for fit_ids, eval_ids in fold_ids:
                if time.monotonic() - start > max_seconds:
                    raise TimeoutError('Context search budget exceeded')
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
        actual = predict_features(state, context_features(dev['x'], chosen['family']))
        incumbent = scores(dev['y'], baseline.predict(dev['x']).argmax(1), 7)
        measured = scores(dev['y'], actual.argmax(1), 7)
        replay = load(_read(root / 'model.safetensors', 1024**2))
        if not np.array_equal(actual, predict_features(replay, context_features(dev['x'], chosen['family']))):
            raise ValueError('Reload changes logits')
        def candidate_predict(x):
            return predict_features(state, context_features(x, chosen['family']))
        for i in range(20):
            x = dev['x'][i % len(dev['x']):i % len(dev['x']) + 1]
            candidate_predict(x)
            baseline.predict(x)
        timings = {'candidate': [], 'incumbent': []}
        for i in range(1000):
            x = dev['x'][i % len(dev['x']):i % len(dev['x']) + 1]
            calls = [('candidate', candidate_predict), ('incumbent', baseline.predict)]
            if i % 2:
                calls.reverse()
            for name, fn in calls:
                before = time.perf_counter_ns()
                fn(x)
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
            'status': 'complete', 'promotionAuthorized': False, 'task': 'public_context_ablation',
            'contract': CONTRACT, 'selected': selected, 'candidates': results,
            'baseline_control': next(r for r in results if r['candidate'] == BASELINE),
            'baseline_state_exact': True, 'reload_max_abs_logit_error': 0.0,
            'evidence': 'SYNTHETIC' if provenance.get('evidence') == 'SYNTHETIC' else 'MEASURED',
            'provenance': provenance, 'development': measured, 'incumbent_development': incumbent,
            'retention_checks': checks, 'retained_as_research_candidate': all(checks.values()),
            'latency': timing, 'model_bytes': (root / 'model.safetensors').stat().st_size,
            'model_sha256': digest(root / 'model.safetensors'),
            'baseline_model_sha256': prior['model_sha256'],
            'baseline_receipt_sha256': hashlib.sha256(prior_bytes).hexdigest(),
            'source_sha256': {name: digest(Path(__file__).with_name(name)) for name in
                              ('espfi_context.py', 'espfi_motion.py', 'espfi_generalize.py', 'espfi_kernel.py')},
            'elapsed_seconds': time.monotonic() - start,
            'scope': '18 configurations selected by original training folds; adaptive research on reused development data. '
                     '1000 paired batch1 calls include feature extraction/checks/classifier, exclude acquisition/pooling/IO. '
                     'No cross-recording chronology, empty-room calibration, actual graph feature, final test or field accuracy.',
        }
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
