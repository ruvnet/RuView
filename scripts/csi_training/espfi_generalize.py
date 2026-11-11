"""Development-only cross-room/participant ridge search, never final-test tuning.

All features are deterministic per recording. Fitted scalers and ridge weights
see fold-training data only. The held-out development partition is consulted
once, after the candidate is selected by two internal training-data folds.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import platform
import time

import numpy as np
import torch
from safetensors.numpy import load_file, save_file

from .espfi_train import AmplitudeBundle, digest, scores, write_json

FAMILIES = ('raw_statistics', 'relative_temporal', 'relative_spectral', 'relative_combined')
ALPHAS = (0.1, 1.0, 10.0, 100.0)
CONTRACT = {'input': 'nonnegative_amplitude_N52T190', 'classes': 7,
            'families': list(FAMILIES), 'alphas': list(ALPHAS),
            'selection': 'mean two-fold macro F1, then fewer features, then declared order',
            'folds': [{'train_room': 1, 'validation_room': 2}, {'train_room': 2, 'validation_room': 1}],
            'training_participants': [1, 2, 3, 4], 'validation_participants': [5, 6],
            'frequency_units': 'normalized cycles/sample; no measured sampling rate inferred',
            'log_floor': 1e-6, 'window_rms_floor': 1e-6}


def amplitudes(values):
    x = np.asarray(values)
    if (x.ndim != 3 or x.shape[1:] != (52, 190) or not 0 < len(x) <= 4096
            or not np.isfinite(x).all() or (x < 0).any()):
        raise ValueError('Expected 1..4096 finite nonnegative amplitude recordings N52T190')
    return x.astype(np.float64, copy=False)


def features(values, family):
    """No learned state, labels, timestamps, participant or room identifiers."""
    x = amplitudes(values)
    if family == 'raw_statistics':
        return np.concatenate((x.mean(-1), x.std(-1), np.abs(np.diff(x, axis=-1)).mean(-1)), axis=1)
    if family not in FAMILIES:
        raise ValueError('Unknown feature family')
    logged = np.log(np.maximum(x, 1e-6))
    centered = logged - logged.mean(-1, keepdims=True)
    scale = np.maximum(np.sqrt(np.mean(centered**2, axis=(1, 2), keepdims=True)), 1e-6)
    relative = centered / scale
    channel_rms = np.sqrt(np.mean(relative**2, axis=-1))
    output = []
    if family in ('relative_temporal', 'relative_combined'):
        bins = 8 if family == 'relative_temporal' else 4
        pooled = np.stack([b.mean(-1) for b in np.array_split(relative, bins, axis=-1)], axis=-1)
        output.append(pooled.reshape(len(x), -1))
    if family in ('relative_spectral', 'relative_combined'):
        power = np.abs(np.fft.rfft(relative, axis=-1))**2 / (190**2)
        edges = (1, 2, 4, 8, 16, 24, 40, 64, 96) if family == 'relative_spectral' else (1, 4, 12, 32, 96)
        bands = np.stack([power[..., a:b].sum(-1) for a, b in zip(edges[:-1], edges[1:])], axis=-1)
        output.append(bands.reshape(len(x), -1))
    output.append(channel_rms)
    result = np.concatenate(output, axis=1)
    if not np.isfinite(result).all():
        raise ValueError('Nonfinite feature transform')
    return result


def fit_ridge(x, y, alpha):
    mean = x.mean(0)
    scale = np.maximum(x.std(0), 1e-6)
    z = (x - mean) / scale
    target = np.eye(7)[y]
    bias = target.mean(0)
    target = target - bias
    if len(x) < x.shape[1]:
        gram = z @ z.T
        gram.flat[::len(gram) + 1] += alpha
        weights = z.T @ np.linalg.solve(gram, target)
    else:
        gram = z.T @ z
        gram.flat[::len(gram) + 1] += alpha
        weights = np.linalg.solve(gram, z.T @ target)
    result = {'mean': mean, 'scale': scale, 'weights': weights, 'bias': bias}
    if any(not np.isfinite(v).all() for v in result.values()):
        raise ValueError('Nonfinite ridge fit')
    return result


def logits(state, x):
    values = np.asarray(x, dtype=state['weights'].dtype)
    return ((values - state['mean']) / state['scale']) @ state['weights'] + state['bias']


def folds(identity, labels):
    identity = np.asarray(identity)
    if identity.shape != (len(labels), 4) or identity.dtype.kind not in 'iu':
        raise ValueError('Invalid recording identities')
    if not np.array_equal(identity[:, 2] - 1, labels):
        raise ValueError('Identity labels differ')
    if not set(identity[:, 0]).issubset({1, 2}) or not set(identity[:, 1]).issubset(set(range(1, 7))):
        raise ValueError('Development search accepts training rooms/participants only')
    result = []
    for room in (1, 2):
        train = np.flatnonzero((identity[:, 0] == room) & np.isin(identity[:, 1], [1, 2, 3, 4]))
        validation = np.flatnonzero((identity[:, 0] == 3 - room) & np.isin(identity[:, 1], [5, 6]))
        if set(labels[train]) != set(range(7)) or set(labels[validation]) != set(range(7)):
            raise ValueError('Every fold must contain all seven classes')
        result.append((train, validation))
    return result


class GeneralizationBundle:
    def __init__(self, directory):
        root = Path(directory)
        for name in ('status.json', 'metrics.json', 'model.safetensors'):
            path = root / name
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 2 * 1024**2:
                raise ValueError('Invalid bounded artifact')
        report = json.loads((root / 'metrics.json').read_text())
        status = json.loads((root / 'status.json').read_text())
        if (status.get('status') != 'complete' or status.get('promotionAuthorized') is not False
                or report.get('status') != 'complete' or report.get('promotionAuthorized') is not False
                or report.get('task') != 'public_amplitude_development_generalization'
                or report.get('contract') != CONTRACT):
            raise ValueError('Invalid complete candidate contract')
        if digest(root / 'model.safetensors') != report.get('model_sha256'):
            raise ValueError('Model hash mismatch')
        self.family = report['selected']['family']
        if self.family not in FAMILIES:
            raise ValueError('Unknown feature family')
        self.state = load_file(str(root / 'model.safetensors'))
        dimension = 156 if self.family == 'raw_statistics' else 468
        dtype_name = report.get('tensor_dtype', 'float64')
        if dtype_name not in ('float64', 'float32'):
            raise ValueError('Unsupported tensor dtype')
        dtype = np.dtype(dtype_name)
        shapes = {'mean': (dimension,), 'scale': (dimension,), 'weights': (dimension, 7), 'bias': (7,)}
        if set(self.state) != set(shapes) or any(self.state[k].shape != shape or self.state[k].dtype != dtype or not np.isfinite(self.state[k]).all() for k, shape in shapes.items()) or not (self.state['scale'] > 0).all():
            raise ValueError('Invalid ridge tensor contract')

    def predict(self, values):
        result = logits(self.state, features(values, self.family))
        if not np.isfinite(result).all():
            raise ValueError('Nonfinite inference')
        return result


def compare_development(parts, provenance, baseline_directory, output, *, max_seconds=600):
    """Internal API accepting ONLY intake-validated training/development arrays."""
    if not 1 <= max_seconds <= 3600 or set(parts) != {'train', 'validation'}:
        raise ValueError('Bounded train/development-only input required')
    started = time.monotonic()
    torch.set_num_threads(1)
    baseline = AmplitudeBundle(baseline_directory)
    original_report = json.loads((Path(baseline_directory) / 'metrics.json').read_text())
    if provenance['manifest_sha256'] != original_report['provenance']['manifest_sha256']:
        raise ValueError('Baseline and new candidate need identical dataset provenance')
    train = parts['train']
    x, y = amplitudes(train['x']), np.asarray(train['y'])
    if y.shape != (len(x),) or y.dtype.kind not in 'iu' or set(y) != set(range(7)):
        raise ValueError('Invalid training labels')
    fold_indices = folds(train['identity'], y)
    development_ids = np.asarray(parts['validation']['identity'])
    if development_ids.shape != (len(parts['validation']['y']), 4) or not np.all(development_ids[:, 0] == 3) or not np.all(development_ids[:, 1] == 7) or not np.array_equal(development_ids[:, 2] - 1, parts['validation']['y']):
        raise ValueError('Expected room3 participant7 development partition')
    root = Path(output)
    root.mkdir(parents=True, exist_ok=False)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    write_json(root / 'search_contract.json', CONTRACT)
    try:
        candidates, selected, best_rank = [], None, None
        for family in FAMILIES:
            transformed = features(x, family)
            for alpha in ALPHAS:
                fold_results = []
                for fit_ids, eval_ids in fold_indices:
                    if time.monotonic() - started > max_seconds:
                        raise TimeoutError('Development search exceeded budget')
                    state = fit_ridge(transformed[fit_ids], y[fit_ids], alpha)
                    predictions = logits(state, transformed[eval_ids]).argmax(1)
                    fold_results.append(scores(y[eval_ids], predictions, 7))
                candidate = {'family': family, 'alpha': alpha, 'features': transformed.shape[1],
                             'folds': fold_results, 'mean_cv_macro_f1': float(np.mean([v['macro_f1'] for v in fold_results]))}
                candidates.append(candidate)
                rank = (candidate['mean_cv_macro_f1'], -candidate['features'])
                if best_rank is None or rank > best_rank:
                    selected, best_rank = candidate, rank
        # Selection is final before computing any development feature or prediction.
        write_json(root / 'selection.json', {'contract': CONTRACT, 'candidates': candidates, 'selected': selected})
        state = fit_ridge(features(x, selected['family']), y, selected['alpha'])
        save_file({k: np.ascontiguousarray(v) for k, v in state.items()}, str(root / 'model.safetensors'))
        validation = amplitudes(parts['validation']['x'])
        dev_features = features(validation, selected['family'])
        raw_logits = logits(state, dev_features)
        reloaded = load_file(str(root / 'model.safetensors'))
        reload_logits = logits(reloaded, dev_features)
        if not np.array_equal(raw_logits, reload_logits):
            raise ValueError('Export/reload logits differ')
        baseline_logits = baseline.predict(validation)
        development = scores(parts['validation']['y'], raw_logits.argmax(1), 7)
        reference = scores(parts['validation']['y'], baseline_logits.argmax(1), 7)
        # Timing includes deterministic feature extraction, fitted scaling, and classification.
        sample = validation[:1]
        for _ in range(20):
            logits(state, features(sample, selected['family']))
        durations = []
        for _ in range(200):
            before = time.perf_counter_ns()
            timed_result = logits(state, features(sample, selected['family']))
            if not np.isfinite(timed_result).all():
                raise ValueError('Nonfinite timed inference')
            durations.append((time.perf_counter_ns() - before) / 1e6)
        report = {'schema_version': 1, 'status': 'complete', 'promotionAuthorized': False,
                  'task': 'public_amplitude_development_generalization', 'contract': CONTRACT,
                  'evidence': 'SYNTHETIC' if provenance.get('evidence') == 'SYNTHETIC' else 'MEASURED',
                  'candidates': candidates, 'selected': selected, 'development': development,
                  'original_development': reference,
                  'development_macro_f1_delta': development['macro_f1'] - reference['macro_f1'],
                  'development_improvement': development['macro_f1'] > reference['macro_f1'],
                  'gpu_used': False, 'incremental_gpu_spend_usd': 0,
                  'test_evaluation': 'Final-test arrays not deserialized or evaluated by this development run',
                  'limitations': ['Development comparison only; room3 participant7 has been observed previously',
                                  'Requires new untouched room and participant before promotion',
                                  'Opaque activity IDs; upstream semantic labels unresolved'],
                  'provenance': provenance, 'implementation_sha256': digest(__file__),
                  'baseline_metrics_sha256': digest(Path(baseline_directory) / 'metrics.json'),
                  'baseline_model_sha256': digest(Path(baseline_directory) / 'model.safetensors'),
                  'model_sha256': digest(root / 'model.safetensors'),
                  'model_bytes': (root / 'model.safetensors').stat().st_size,
                  'parameter_count': state['weights'].size + state['bias'].size,
                  'stored_scalar_count': sum(v.size for v in state.values()), 'reload_max_abs_logit_error': 0.0,
                  'latency': {'p50_ms': float(np.percentile(durations, 50)), 'p95_ms': float(np.percentile(durations, 95)),
                              'repetitions': 200, 'batch_size': 1,
                              'scope': 'Feature extraction, scaling and ridge; excludes IO and temporal pooling'},
                  'runtime': {'numpy': np.__version__, 'platform': platform.platform()},
                  'elapsed_seconds': time.monotonic() - started, 'max_seconds': max_seconds}
        write_json(root / 'metrics.json', report)
        write_json(root / 'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except Exception:
        write_json(root / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise



def optimize_export(source, validation, baseline_directory, output, *, optimized_baseline_directory=None, repetitions=1000):
    """Frozen float32 export with validation equivalence and paired CPU timing."""
    if not 50 <= repetitions <= 2000:
        raise ValueError('Invalid bounded benchmark iterations')
    torch.set_num_threads(1)
    original = GeneralizationBundle(source)
    if original.state['weights'].dtype != np.float64:
        raise ValueError('Export requires original float64 fitted tensors')
    original_baseline = AmplitudeBundle(baseline_directory)
    baseline_report = json.loads((Path(baseline_directory) / 'metrics.json').read_text())
    report = json.loads((Path(source) / 'metrics.json').read_text())
    if baseline_report['provenance']['manifest_sha256'] != report['provenance']['manifest_sha256']:
        raise ValueError('Benchmark baselines require identical dataset provenance')
    expected = original.predict(validation)
    converted = copy.deepcopy(original)
    converted.state = {k: v.astype(np.float32) for k, v in original.state.items()}
    actual = converted.predict(validation)
    error = float(np.max(np.abs(expected - actual)))
    if error > 1e-4 or not np.array_equal(expected.argmax(1), actual.argmax(1)):
        raise ValueError('Float32 export failed frozen validation parity')
    bundles = [('original_baseline', original_baseline), ('generalization_float64', original),
               ('generalization_float32', converted)]
    if optimized_baseline_directory is not None:
        optimized_baseline = AmplitudeBundle(optimized_baseline_directory)
        folded_report = json.loads((Path(optimized_baseline_directory) / 'metrics.json').read_text())
        if folded_report.get('parent_model_sha256') != baseline_report['model_sha256']:
            raise ValueError('Folded baseline is not derived from the original benchmark baseline')
        bundles.insert(1, ('folded_baseline', optimized_baseline))
    root = Path(output)
    root.mkdir(parents=True, exist_ok=False)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    try:
        save_file({k: np.ascontiguousarray(v) for k, v in converted.state.items()}, str(root / 'model.safetensors'))
        converted.state = load_file(str(root / 'model.safetensors'))
        if not np.array_equal(actual, converted.predict(validation)):
            raise ValueError('Float32 export reload changed predictions')
        # Each call includes public input checks, feature preprocessing and output
        # finite checks. Rotate order to reduce clock/load drift bias.
        sample = np.asarray(validation)[:1]
        for _ in range(30):
            for _, bundle in bundles:
                bundle.predict(sample)
        timings = {name: [] for name, _ in bundles}
        for i in range(repetitions):
            order = bundles[i % len(bundles):] + bundles[:i % len(bundles)]
            for name, bundle in order:
                start = time.perf_counter_ns()
                bundle.predict(sample)
                timings[name].append((time.perf_counter_ns() - start) / 1e6)
        timing_report = {name: {'p50_ms': float(np.percentile(values, 50)), 'p95_ms': float(np.percentile(values, 95))}
                         for name, values in timings.items()}
        report.update({'tensor_dtype': 'float32', 'model_sha256': digest(root / 'model.safetensors'),
                       'model_bytes': (root / 'model.safetensors').stat().st_size,
                       'implementation_sha256': digest(__file__),
                       'parent_model_sha256': digest(Path(source) / 'model.safetensors'),
                       'parent_metrics_sha256': digest(Path(source) / 'metrics.json'),
                       'development_metric_origin': 'Inherited from parent, every validation prediction verified unchanged',
                       'optimization': {'operation': 'Frozen float32 tensors and feature-vector boundary',
                                        'max_abs_logit_error': error, 'identical_validation_predictions': True,
                                        'validation_samples': len(validation), 'reload_max_abs_logit_error': 0.0,
                                        'timings': timing_report, 'repetitions': repetitions, 'batch_size': 1,
                                        'threads': 1, 'scope': 'Full public bundle predict including input/output checks and preprocessing; excludes IO and temporal pooling'},
                       'test_evaluation': 'No final-test arrays deserialized or predictions computed during export'})
        # The paired public-API result supersedes the earlier inner-function timing.
        report['latency'] = timing_report['generalization_float32']
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
    parser.add_argument('--optimize-bundle', help='Existing frozen float64 generalization bundle; no training')
    parser.add_argument('--optimized-baseline', help='Previously folded original amplitude baseline for paired timing')
    args = parser.parse_args(argv)
    from .espfi import load_development
    parts, provenance = load_development(args.manifest)
    if args.optimize_bundle:
        source_report = json.loads((Path(args.optimize_bundle) / 'metrics.json').read_text())
        if source_report['provenance']['manifest_sha256'] != provenance['manifest_sha256']:
            raise ValueError('Export validation data differs from fitted data provenance')
        result = optimize_export(args.optimize_bundle, parts['validation']['x'], args.baseline, args.output,
                                 optimized_baseline_directory=args.optimized_baseline)
        print(json.dumps({'model_bytes': result['model_bytes'], 'optimization': result['optimization']}, indent=2))
        return
    result = compare_development(parts, provenance, args.baseline, args.output, max_seconds=args.max_seconds)
    print(json.dumps({key: result[key] for key in ('selected', 'development', 'original_development', 'development_macro_f1_delta', 'latency')}, indent=2))
