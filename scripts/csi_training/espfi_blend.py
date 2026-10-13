"""Frozen five-weight CSI ensemble experiment. Development evidence only."""
import argparse
import json
from pathlib import Path
import time
import numpy as np
from safetensors.numpy import save_file, load_file
from .espfi import load_development
from .espfi_generalize import features, folds
from .espfi_kernel import fit, predict_features
from .espfi_train import scores, digest, write_json

WEIGHTS = (0., .25, .5, .75, 1.)
COMPONENTS = (
    {'family': 'relative_spectral', 'kind': 'linear', 'alpha': 10.},
    {'family': 'relative_combined', 'kind': 'rff', 'dimensions': 128,
     'gamma_multiplier': .1, 'alpha': .01})
CONTRACT = {'weights': list(WEIGHTS), 'components': list(COMPONENTS),
            'seed': 42, 'selection': 'mean training-fold macro F1 then smaller RFF weight',
            'mix': '(1-w)*spectral_scores + w*RFF_scores; no softmax',
            'test_access': False, 'promotionAuthorized': False}

def select_weight(fold_predictions, labels):
    results = []
    for weight in WEIGHTS:
        evaluations = [scores(y, ((1-weight)*a + weight*b).argmax(1), 7)
                       for (a, b), y in zip(fold_predictions, labels)]
        results.append({'weight': weight, 'folds': evaluations,
                        'mean_macro_f1': float(np.mean([e['macro_f1'] for e in evaluations]))})
    return max(results, key=lambda r: (r['mean_macro_f1'], -r['weight'])), results

def infer(states, weight, amplitudes):
    result = None
    for index, coefficient in ((0, 1-weight), (1, weight)):
        if coefficient:
            value = coefficient * predict_features(states[index], features(amplitudes, COMPONENTS[index]['family']))
            result = value if result is None else result + value
    if result is None or not np.isfinite(result).all():
        raise ValueError('Invalid ensemble output')
    return result

def run(parts, provenance, output):
    if set(parts) != {'train', 'validation'}:
        raise ValueError('Development partitions only')
    root = Path(output)
    root.mkdir(parents=True, exist_ok=False)
    write_json(root/'status.json', {'status': 'running', 'promotionAuthorized': False})
    write_json(root/'contract.json', CONTRACT)
    start = time.monotonic()
    try:
        train, dev = parts['train'], parts['validation']
        y = train['y']
        fold_ids = folds(train['identity'], y)
        cache = [features(train['x'], c['family']) for c in COMPONENTS]
        predictions, labels = [], []
        for fit_ids, eval_ids in fold_ids:
            pair = []
            for x, candidate in zip(cache, COMPONENTS):
                state = fit(x[fit_ids], y[fit_ids], candidate)
                pair.append(predict_features(state, x[eval_ids]))
            predictions.append(pair)
            labels.append(y[eval_ids])
        selected, results = select_weight(predictions, labels)
        write_json(root/'selection.json', {'selected': selected, 'candidates': results})
        # Selection is frozen before using any external development signal.
        weight = selected['weight']
        states = [fit(x, y, c) for x, c in zip(cache, COMPONENTS)]
        active = [i for i, coefficient in ((0, 1-weight), (1, weight)) if coefficient]
        tensors = {str(i)+'__'+k: np.ascontiguousarray(v)
                   for i in active for k, v in states[i].items()}
        save_file(tensors, str(root/'model.safetensors'))
        restored = load_file(str(root/'model.safetensors'))
        restored_states = [{k.split('__', 1)[1]: v for k, v in restored.items()
                            if k.startswith(str(i)+'__')} for i in range(2)]
        before = infer(states, weight, dev['x'])
        after = infer(restored_states, weight, dev['x'])
        if not np.array_equal(before, after):
            raise ValueError('Export parity failed')
        metrics = {name: scores(dev['y'], infer(states, w, dev['x']).argmax(1), 7)
                   for name, w in (('spectral_control', 0.), ('rff_control', 1.), ('selected', weight))}
        timings = {name: [] for name in metrics}
        choices = [('spectral_control', 0.), ('rff_control', 1.), ('selected', weight)]
        for _ in range(20):
            for _, w in choices:
                infer(states, w, dev['x'][:1])
        for repeat in range(500):
            order = choices[repeat % 3:] + choices[:repeat % 3]
            for name, w in order:
                tick = time.perf_counter_ns()
                infer(states, w, dev['x'][:1])
                timings[name].append((time.perf_counter_ns()-tick)/1e6)
        report = {'status': 'complete', 'promotionAuthorized': False,
                  'evidence': provenance.get('evidence', 'MEASURED'),
                  'contract': CONTRACT, 'selected': selected, 'candidates': results,
                  'development': metrics, 'provenance': provenance,
                  'model_sha256': digest(root/'model.safetensors'),
                  'model_bytes': (root/'model.safetensors').stat().st_size,
                  'implementation_sha256': digest(__file__),
                  'dependency_sha256': {name: digest(Path(__file__).with_name(name))
                      for name in ('espfi_kernel.py', 'espfi_generalize.py', 'espfi.py', 'espfi_train.py')},
                  'reload_exact': True, 'active_components': active,
                  'latency': {name: {'p50_ms': float(np.percentile(t, 50)),
                                    'p95_ms': float(np.percentile(t, 95))} for name, t in timings.items()},
                  'timing_scope': '500 rotated CPU batch1 calls including feature extraction and checks; excludes intake and temporal pooling',
                  'elapsed_seconds': time.monotonic()-start,
                  'limitations': ['Repeatedly used development set; not independent confirmation',
                                  'No final test evaluation or SOTA evidence',
                                  'Controls reproduced in float64 on this executor; not prior float32 deployment timing']}
        write_json(root/'metrics.json', report)
        write_json(root/'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except Exception:
        write_json(root/'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', required=True)
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    parts, provenance = load_development(args.manifest)
    report = run(parts, provenance, args.output)
    print(json.dumps({k: report[k] for k in ('selected', 'development', 'model_bytes', 'latency')}, indent=2))
