"""Frozen nonlinear development search with no support-vector retention."""
import argparse
import json
from pathlib import Path
import time

import numpy as np
from safetensors.numpy import load_file, save_file
from .espfi_generalize import features, folds, GeneralizationBundle
from .espfi_train import digest, scores, write_json

FAMILIES = ('relative_spectral', 'relative_combined')
GRID = ([{'family': family, 'kind': 'linear', 'alpha': alpha} for family in FAMILIES for alpha in (1., 10., 100.)]
        + [{'family': family, 'kind': 'centroid'} for family in FAMILIES]
        + [{'family': family, 'kind': 'rff', 'dimensions': dimensions, 'gamma_multiplier': gamma, 'alpha': alpha}
           for family in FAMILIES for dimensions in (128, 512) for gamma in (.1, 1., 10.) for alpha in (.01, .1, 1., 10.)])
CONTRACT = {'seed': 42, 'grid': GRID, 'input': 'nonnegative_amplitude_N52T190',
            'selection': 'mean training-only grouped CV macro F1, then fewer stored scalars, then declaration order',
            'folds': [{'train_room': 1, 'validation_room': 2}, {'train_room': 2, 'validation_room': 1}],
            'train_participants': [1, 2, 3, 4], 'validation_participants': [5, 6],
            'gamma': 'multiplier / 468 after train-only standardization',
            'rff': 'sqrt(2/M) cos(Z @ Gaussian(0,2gamma) + Uniform(0,2pi))',
            'support_vectors': False}


def transformed(state, values):
    values = np.asarray(values, dtype=state['feature_mean'].dtype)
    z = (values - state['feature_mean']) / state['feature_scale']
    if 'projection' in state:
        return np.asarray(np.sqrt(2. / state['phase'].size), dtype=z.dtype) * np.cos(z @ state['projection'] + state['phase'])
    return z


def predict_features(state, values):
    z = transformed(state, values) - state['output_mean']
    result = z @ state['weights'] + state['bias']
    if not np.isfinite(result).all():
        raise ValueError('Nonfinite classifier output')
    return result


def fit(values, labels, candidate):
    mean, scale = values.mean(0), np.maximum(values.std(0), 1e-6)
    state = {'feature_mean': mean, 'feature_scale': scale}
    if candidate['kind'] == 'rff':
        rng = np.random.default_rng(42)
        dimensions = candidate['dimensions']
        gamma = candidate['gamma_multiplier'] / values.shape[1]
        state['projection'] = rng.normal(0, np.sqrt(2 * gamma), (values.shape[1], dimensions))
        state['phase'] = rng.uniform(0, 2 * np.pi, dimensions)
    z = transformed(state, values)
    if candidate['kind'] == 'centroid':
        centroids = np.stack([z[labels == label].mean(0) for label in range(7)])
        state.update({'output_mean': np.zeros(z.shape[1]), 'weights': 2 * centroids.T,
                      'bias': -np.square(centroids).sum(1)})
    else:
        output_mean = z.mean(0)
        z = z - output_mean
        target = np.eye(7)[labels]
        bias = target.mean(0)
        target -= bias
        if len(z) < z.shape[1]:
            gram = z @ z.T
            gram.flat[::len(gram) + 1] += candidate['alpha']
            weights = z.T @ np.linalg.solve(gram, target)
        else:
            gram = z.T @ z
            gram.flat[::len(gram) + 1] += candidate['alpha']
            weights = np.linalg.solve(gram, z.T @ target)
        state.update({'output_mean': output_mean, 'weights': weights, 'bias': bias})
    if any(not np.isfinite(v).all() for v in state.values()):
        raise ValueError('Nonfinite model state')
    # Canonical layout avoids BLAS reduction-order differences after export.
    return {k: np.ascontiguousarray(v) for k, v in state.items()}


class KernelBundle:
    def __init__(self, directory):
        root = Path(directory)
        for name in ('metrics.json', 'status.json', 'model.safetensors'):
            path = root / name
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 4 * 1024**2:
                raise ValueError('Invalid bounded artifact')
        report = json.loads((root / 'metrics.json').read_text())
        status = json.loads((root / 'status.json').read_text())
        if (status.get('status') != 'complete' or status.get('promotionAuthorized') is not False
                or report.get('status') != 'complete' or report.get('promotionAuthorized') is not False
                or report.get('task') != 'public_amplitude_nonlinear_development' or report.get('contract') != CONTRACT):
            raise ValueError('Invalid completed kernel candidate')
        if report.get('model_sha256') != digest(root / 'model.safetensors'):
            raise ValueError('Model hash mismatch')
        self.candidate = report['selected']['candidate']
        if self.candidate not in GRID:
            raise ValueError('Unknown candidate')
        self.state = load_file(str(root / 'model.safetensors'))
        width = self.candidate.get('dimensions', 468)
        expected = {'feature_mean': (468,), 'feature_scale': (468,), 'output_mean': (width,),
                    'weights': (width, 7), 'bias': (7,)}
        if self.candidate['kind'] == 'rff':
            expected.update({'projection': (468, width), 'phase': (width,)})
        if set(self.state) != set(expected) or any(self.state[k].shape != shape or self.state[k].dtype != np.float64 or not np.isfinite(self.state[k]).all() for k, shape in expected.items()) or not (self.state['feature_scale'] > 0).all():
            raise ValueError('Invalid bounded classifier state')

    def predict(self, amplitudes):
        return predict_features(self.state, features(amplitudes, self.candidate['family']))


def search(parts, provenance, baseline_directory, output, max_seconds=600):
    if set(parts) != {'train', 'validation'} or not 1 <= max_seconds <= 3600:
        raise ValueError('Requires bounded development-only partitions')
    start = time.monotonic()
    baseline = GeneralizationBundle(baseline_directory)
    baseline_report = json.loads((Path(baseline_directory) / 'metrics.json').read_text())
    if baseline_report['provenance']['manifest_sha256'] != provenance['manifest_sha256']:
        raise ValueError('Data provenance differs from incumbent')
    train, validation = parts['train'], parts['validation']
    y = np.asarray(train['y'])
    if y.dtype.kind not in 'iu' or y.shape != (len(train['x']),) or set(y) != set(range(7)):
        raise ValueError('Invalid training labels')
    fold_ids = folds(train['identity'], y)
    ids = np.asarray(validation['identity'])
    if ids.shape != (len(validation['y']), 4) or not np.all(ids[:, 0] == 3) or not np.all(ids[:, 1] == 7) or not np.array_equal(ids[:, 2] - 1, validation['y']):
        raise ValueError('Invalid development identities')
    root = Path(output)
    root.mkdir(exist_ok=False, parents=True)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    write_json(root / 'search_contract.json', CONTRACT)
    try:
        cache = {family: features(train['x'], family) for family in FAMILIES}
        results, selected, best = [], None, None
        for candidate in GRID:
            x = cache[candidate['family']]
            evaluations = []
            for fit_ids, eval_ids in fold_ids:
                if time.monotonic() - start > max_seconds:
                    raise TimeoutError('Nonlinear search budget exceeded')
                state = fit(x[fit_ids], y[fit_ids], candidate)
                evaluations.append(scores(y[eval_ids], predict_features(state, x[eval_ids]).argmax(1), 7))
            result = {'candidate': candidate, 'folds': evaluations,
                      'mean_cv_macro_f1': float(np.mean([e['macro_f1'] for e in evaluations])),
                      'stored_scalar_count': sum(v.size for v in state.values())}
            results.append(result)
            rank = (result['mean_cv_macro_f1'], -result['stored_scalar_count'])
            if best is None or rank > best:
                selected, best = result, rank
        # Freeze selection before touching external development values or labels.
        write_json(root / 'selection.json', {'selected': selected, 'candidates': results, 'contract': CONTRACT})
        chosen = selected['candidate']
        state = fit(cache[chosen['family']], y, chosen)
        save_file({k: np.ascontiguousarray(v) for k, v in state.items()}, str(root / 'model.safetensors'))
        dev_features = features(validation['x'], chosen['family'])
        prediction = predict_features(state, dev_features)
        reloaded = load_file(str(root / 'model.safetensors'))
        if not np.array_equal(prediction, predict_features(reloaded, dev_features)):
            raise ValueError('Export changed logits')
        incumbent = scores(validation['y'], baseline.predict(validation['x']).argmax(1), 7)
        measured = scores(validation['y'], prediction.argmax(1), 7)
        # Full same-method inference including feature extraction and input/output checks.
        sample = validation['x'][:1]
        def inference():
            return predict_features(state, features(sample, chosen['family']))
        for _ in range(20):
            inference()
            baseline.predict(sample)
        timings = {'candidate': [], 'incumbent': []}
        for i in range(500):
            pairs = [('candidate', inference), ('incumbent', lambda: baseline.predict(sample))]
            if i % 2:
                pairs.reverse()
            for name, fn in pairs:
                before = time.perf_counter_ns()
                fn()
                timings[name].append((time.perf_counter_ns() - before) / 1e6)
        report = {'schema_version': 1, 'task': 'public_amplitude_nonlinear_development', 'status': 'complete',
                  'promotionAuthorized': False, 'contract': CONTRACT, 'selected': selected, 'candidates': results,
                  'evidence': 'SYNTHETIC' if provenance.get('evidence') == 'SYNTHETIC' else 'MEASURED',
                  'development': measured, 'incumbent_development': incumbent,
                  'development_macro_f1_delta': measured['macro_f1'] - incumbent['macro_f1'],
                  'development_improvement': measured['macro_f1'] > incumbent['macro_f1'],
                  'provenance': provenance, 'implementation_sha256': digest(__file__),
                  'feature_implementation_sha256': digest(Path(__file__).with_name('espfi_generalize.py')),
                  'baseline_metrics_sha256': digest(Path(baseline_directory) / 'metrics.json'),
                  'baseline_model_sha256': digest(Path(baseline_directory) / 'model.safetensors'),
                  'model_sha256': digest(root / 'model.safetensors'), 'model_bytes': (root / 'model.safetensors').stat().st_size,
                  'stored_scalar_count': sum(v.size for v in state.values()),
                  'classifier_scalar_count': state['weights'].size + state['bias'].size,
                  'random_projection_scalar_count': sum(state[k].size for k in ('projection', 'phase') if k in state),
                  'reload_max_abs_logit_error': 0.0,
                  'latency': {name: {'p50_ms': float(np.percentile(t, 50)), 'p95_ms': float(np.percentile(t, 95))} for name, t in timings.items()},
                  'timing_scope': '500 paired CPU batch1 calls, input/output checks plus features/classifier; excludes IO and temporal pooling',
                  'test_evaluation': 'Final-test arrays not deserialized or predictions computed',
                  'limitations': ['Adaptive development result, not independent generalization evidence', 'No comparable SOTA claim; new untouched room/participant required'],
                  'elapsed_seconds': time.monotonic() - start, 'max_seconds': max_seconds,
                  'gpu_used': False, 'incremental_gpu_spend_usd': 0}
        write_json(root / 'metrics.json', report)
        write_json(root / 'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except Exception:
        write_json(root / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', required=True)
    parser.add_argument('--baseline', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--max-seconds', type=int, default=600)
    args = parser.parse_args(argv)
    from .espfi import load_development
    parts, metadata = load_development(args.manifest)
    report = search(parts, metadata, args.baseline, args.output, args.max_seconds)
    print(json.dumps({k: report[k] for k in ('selected', 'development', 'incumbent_development', 'development_macro_f1_delta', 'latency', 'model_bytes')}, indent=2))
