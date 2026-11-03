"""Frozen source-only motion ablation inspired by published ACF and RuView STFT.

Independent feature experiment, not a reproduction of an external model.
Only grouped training folds select a candidate; final test arrays are absent.
"""
import argparse
import json
from pathlib import Path
import time
import numpy as np
from safetensors.numpy import load, save_file
from .espfi_generalize import amplitudes, features, folds
from .espfi_kernel import fit, predict_features
from .espfi_runtime import BufferedKernelBundle
from .espfi_train import digest, scores, write_json

WIDTHS = {'relative_combined': 468, 'motion_acf': 468,
          'motion_invariant': 45, 'local_spectral': 468}
GRID = [dict(family=family, kind='rff', dimensions=128, gamma_multiplier=gamma, alpha=alpha)
        for family in WIDTHS for gamma in (.1, 1.) for alpha in (.01, .1, 1.)]
CONTRACT = {'seed': 42, 'grid': GRID, 'input': 'nonnegative_amplitude_N52T190',
            'selection': 'mean training-only grouped CV macro F1, then fewer scalars, then declaration order',
            'acf_lags': [1, 2, 4, 8, 16, 32], 'difference_lags': [1, 4, 16],
            'carrier_quantiles': [.1, .25, .5, .75, .9],
            'acf': 'centered log-amplitude biased autocorrelation divided by full channel energy floored at 190e-12',
            'stft': {'window': 32, 'starts': list(range(0, 159, 16))+[158],
                     'taper': 'symmetric Hann', 'bands': [[1, 5], [5, 17]],
                     'pooling': '4 consecutive frame groups via array_split; channel RMS appended'},
            'frequency_units': 'normalized cycles/sample; no measured rate inferred',
            'retention_gate': {'development_macro_f1_absolute_gain': .03,
                               'accuracy_nonregression': True, 'no_additional_zero_f1_classes': True,
                               'paired_p95_limit_ms': 1.0},
            'promotionAuthorized': False}
ROBUST_CONTRACT = dict(CONTRACT, selection='minimum training-fold macro F1, then mean, then fewer scalars, then declaration order')


def motion_features(values, family):
    if family not in WIDTHS:
        raise ValueError('Unknown motion family')
    if family == 'relative_combined':
        return features(values, family)
    x = amplitudes(values)
    logged = np.log(np.maximum(x, 1e-6))
    centered = logged-logged.mean(-1, keepdims=True)
    scale = np.maximum(np.sqrt(np.mean(centered**2, axis=(1, 2), keepdims=True)), 1e-6)
    relative = centered/scale
    if family == 'local_spectral':
        rows = []
        for offset in range(0, len(x), 32):
            part = relative[offset:offset+32]
            windows = np.lib.stride_tricks.sliding_window_view(part, 32, axis=-1)
            windows = windows[..., CONTRACT['stft']['starts'], :] * np.hanning(32)
            power = np.abs(np.fft.rfft(windows, axis=-1))**2 / 32**2
            bands = np.stack((power[..., 1:5].sum(-1), power[..., 5:17].sum(-1)), axis=-1)
            local = np.stack([b.mean(axis=-2) for b in np.array_split(bands, 4, axis=-2)], axis=-2)
            rows.append(np.concatenate((local.reshape(len(part), -1),
                                        np.sqrt(np.mean(part**2, axis=-1))), axis=1))
        result = np.concatenate(rows)
    else:
        energy = np.maximum(np.sum(centered**2, axis=-1), 190e-12)
        acf = [np.sum(centered[..., lag:]*centered[..., :-lag], axis=-1)/energy
               for lag in CONTRACT['acf_lags']]
        changes = [np.mean(np.abs(relative[..., lag:]-relative[..., :-lag]), axis=-1)
                   for lag in CONTRACT['difference_lags']]
        per_channel = np.stack(acf+changes, axis=-1)
        if family == 'motion_invariant':
            pooled = np.quantile(per_channel, CONTRACT['carrier_quantiles'], axis=1)
            result = pooled.transpose(1, 0, 2).reshape(len(x), -1)
        else:
            result = per_channel.reshape(len(x), -1)
    if result.shape != (len(x), WIDTHS[family]) or not np.isfinite(result).all():
        raise ValueError('Invalid motion features')
    return np.ascontiguousarray(result)


def _read(path, limit):
    path = Path(path)
    if path.is_symlink() or not path.is_file():
        raise ValueError('Expected regular bounded artifact')
    with path.open('rb') as handle:
        data = handle.read(limit+1)
    if len(data) > limit:
        raise ValueError('Oversized artifact')
    return data


class MotionBundle:
    def __init__(self, directory):
        root = Path(directory)
        status = json.loads(_read(root/'status.json', 65536))
        report = json.loads(_read(root/'metrics.json', 16*1024**2))
        if (not isinstance(status, dict) or not isinstance(report, dict)
                or status.get('status') != 'complete' or status.get('promotionAuthorized') is not False
                or report.get('status') != 'complete' or report.get('promotionAuthorized') is not False
                or report.get('contract') not in (CONTRACT, ROBUST_CONTRACT)
                or report.get('task') != 'public_motion_ablation'):
            raise ValueError('Invalid completed motion artifact')
        self.candidate = report['selected']['candidate']
        if self.candidate not in GRID:
            raise ValueError('Unknown candidate')
        data = _read(root/'model.safetensors', 1024**2)
        import hashlib
        if hashlib.sha256(data).hexdigest() != report.get('model_sha256'):
            raise ValueError('Model hash mismatch')
        self.state = load(data)
        width = WIDTHS[self.candidate['family']]
        expected = {'feature_mean': (width,), 'feature_scale': (width,),
                    'projection': (width, 128), 'phase': (128,), 'output_mean': (128,),
                    'weights': (128, 7), 'bias': (7,)}
        if (set(self.state) != set(expected) or any(self.state[k].shape != shape or
                self.state[k].dtype != np.float64 or not np.isfinite(self.state[k]).all()
                for k, shape in expected.items()) or not (self.state['feature_scale'] > 0).all()):
            raise ValueError('Invalid motion model state')

    def predict(self, values):
        return predict_features(self.state, motion_features(values, self.candidate['family']))


def _validate(parts):
    if set(parts) != {'train', 'validation'}:
        raise ValueError('Requires development-only partitions')
    for name, part in parts.items():
        ids, y = np.asarray(part['identity']), np.asarray(part['y'])
        if (y.dtype.kind not in 'iu' or y.ndim != 1 or len(y) != len(part['x'])
                or not len(y) or set(y) != set(range(7)) or ids.dtype.kind not in 'iu'
                or ids.shape != (len(y), 4) or len(np.unique(ids, axis=0)) != len(y)
                or not np.array_equal(ids[:, 2]-1, y) or not np.all((ids[:, 3] >= 1) & (ids[:, 3] <= 10))):
            raise ValueError('Invalid recording identities or labels')
        if name == 'validation' and (not np.all(ids[:, 0] == 3) or not np.all(ids[:, 1] == 7)):
            raise ValueError('Invalid development identities')
    return folds(parts['train']['identity'], parts['train']['y'])


def search(parts, provenance, baseline_directory, baseline_receipt, output, max_seconds=600, selection='mean'):
    if not 1 <= max_seconds <= 3600 or selection not in ('mean', 'worst'):
        raise ValueError('Invalid time budget')
    contract = CONTRACT if selection == 'mean' else ROBUST_CONTRACT
    fold_ids = _validate(parts)
    baseline = BufferedKernelBundle(baseline_directory)
    prior = json.loads(_read(baseline_receipt, 16*1024**2))
    if (not isinstance(prior, dict) or prior.get('provenance', {}).get('manifest_sha256') != provenance.get('manifest_sha256')
            or not provenance.get('manifest_sha256') or prior.get('model_sha256') != digest(Path(baseline_directory)/'model.safetensors')):
        raise ValueError('Baseline data or model provenance mismatch')
    root = Path(output)
    root.mkdir(exist_ok=False, parents=True)
    write_json(root/'status.json', {'status': 'running', 'promotionAuthorized': False})
    write_json(root/'search_contract.json', contract)
    start = time.monotonic()
    try:
        train, dev = parts['train'], parts['validation']
        cache = {family: motion_features(train['x'], family) for family in WIDTHS}
        results, selected, rank = [], None, None
        for candidate in GRID:
            evaluations = []
            for fit_ids, eval_ids in fold_ids:
                if time.monotonic()-start > max_seconds:
                    raise TimeoutError('Motion ablation budget exceeded')
                state = fit(cache[candidate['family']][fit_ids], train['y'][fit_ids], candidate)
                result = predict_features(state, cache[candidate['family']][eval_ids])
                evaluations.append(scores(train['y'][eval_ids], result.argmax(1), 7))
            row = {'candidate': candidate, 'folds': evaluations,
                   'mean_cv_macro_f1': float(np.mean([v['macro_f1'] for v in evaluations])),
                   'scalar_count': sum(v.size for v in state.values())}
            results.append(row)
            value = ((min(v['macro_f1'] for v in evaluations) if selection == 'worst'
                      else row['mean_cv_macro_f1']), row['mean_cv_macro_f1'], -row['scalar_count'])
            if rank is None or value > rank:
                selected, rank = row, value
        write_json(root/'selection.json', {'selected': selected, 'candidates': results, 'contract': contract})
        chosen = selected['candidate']
        state = fit(cache[chosen['family']], train['y'], chosen)
        save_file(state, str(root/'model.safetensors'))
        actual = predict_features(state, motion_features(dev['x'], chosen['family']))
        measured = scores(dev['y'], actual.argmax(1), 7)
        incumbent = scores(dev['y'], baseline.predict(dev['x']).argmax(1), 7)
        replay = load(_read(root/'model.safetensors', 1024**2))
        if not np.array_equal(actual, predict_features(replay, motion_features(dev['x'], chosen['family']))):
            raise ValueError('Reload changes logits')
        timings = {'candidate': [], 'incumbent': []}
        def predict(x):
            return predict_features(state, motion_features(x, chosen['family']))
        for i in range(20):
            sample = dev['x'][i % len(dev['x']):i % len(dev['x'])+1]
            predict(sample); baseline.predict(sample)
        for i in range(1000):
            sample = dev['x'][i % len(dev['x']):i % len(dev['x'])+1]
            calls = [('candidate', predict), ('incumbent', baseline.predict)]
            if i % 2:
                calls.reverse()
            for name, fn in calls:
                before = time.perf_counter_ns(); fn(sample)
                timings[name].append((time.perf_counter_ns()-before)/1e6)
        timing = {k: {'p50_ms': float(np.percentile(v, 50)), 'p95_ms': float(np.percentile(v, 95))} for k, v in timings.items()}
        checks = {'macro_f1_gain': measured['macro_f1'] >= incumbent['macro_f1']+.03,
                  'accuracy_nonregression': measured['accuracy'] >= incumbent['accuracy'],
                  'no_additional_zero_f1_classes': all(b == 0 or a > 0 for a, b in zip(measured['per_class_f1'], incumbent['per_class_f1'])),
                  'p95_under_1ms': timing['candidate']['p95_ms'] < 1.0}
        report = {'status': 'complete', 'promotionAuthorized': False, 'task': 'public_motion_ablation',
                  'contract': contract, 'selected': selected, 'candidates': results,
                  'evidence': 'SYNTHETIC' if provenance.get('evidence') == 'SYNTHETIC' else 'MEASURED',
                  'provenance': provenance, 'development': measured, 'incumbent_development': incumbent,
                  'retention_checks': checks, 'retained_as_research_candidate': all(checks.values()),
                  'latency': timing, 'reload_max_abs_logit_error': 0.0,
                  'model_bytes': (root/'model.safetensors').stat().st_size,
                  'model_sha256': digest(root/'model.safetensors'),
                  'baseline_model_sha256': prior['model_sha256'], 'baseline_receipt_sha256': digest(baseline_receipt),
                  'source_sha256': {name: digest(Path(__file__).with_name(name)) for name in
                      ('espfi_motion.py', 'espfi_generalize.py', 'espfi_kernel.py', 'espfi_runtime.py')},
                  'elapsed_seconds': time.monotonic()-start,
                  'scope': '24 configurations selected only on two training folds; development seen once after selection; '
                           '1000 alternating paired batch1 calls include features/checks/classifier, exclude pooling and IO. '
                           'Candidate is float64 with explicit map, incumbent uses seeded float32 folded runtime; '
                           'storage comparison includes different encodings. No test, hardware, pose or SOTA claim.'}
        write_json(root/'metrics.json', report)
        write_json(root/'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except Exception:
        write_json(root/'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ('manifest', 'baseline', 'baseline-receipt', 'output'):
        parser.add_argument('--'+arg, required=True)
    parser.add_argument('--max-seconds', type=int, default=600)
    parser.add_argument('--selection', choices=('mean', 'worst'), default='mean')
    args = parser.parse_args()
    from .espfi import load_development
    parts, provenance = load_development(args.manifest)
    report = search(parts, provenance, args.baseline, args.baseline_receipt, args.output, args.max_seconds, args.selection)
    print(json.dumps({k: report[k] for k in ('selected', 'development', 'incumbent_development',
                     'retention_checks', 'retained_as_research_candidate', 'latency', 'model_bytes')}, indent=2))


if __name__ == '__main__':
    main()
