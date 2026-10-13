"""Bounded amplitude HAR experiment, separate from RuView's eight-feature encoder.

Selection uses validation only. The test partition is evaluated once after the
architecture and checkpoint are frozen. No production promotion or upload.
"""
import argparse
import copy
import hashlib
import json
import platform
from pathlib import Path
import time

import numpy as np
import torch
from torch import nn
from safetensors.torch import load_file, save_file


class AmplitudeClassifier(nn.Module):
    def __init__(self, channels, classes, variant='temporal16', fused=False):
        super().__init__()
        if variant == 'statistics':
            self.net = nn.Sequential(nn.Linear(channels * 3, 32), nn.ReLU(), nn.Linear(32, classes))
        elif variant in ('temporal16', 'temporal32'):
            width = int(variant.removeprefix('temporal'))
            self.net = nn.Sequential(
                nn.Conv1d(channels, width, 1, bias=fused),
                nn.Identity() if fused else nn.BatchNorm1d(width), nn.ReLU(),
                nn.Conv1d(width, width, 7, padding=3, groups=width, bias=fused),
                nn.Identity() if fused else nn.BatchNorm1d(width), nn.ReLU(),
                nn.Conv1d(width, width, 1), nn.ReLU(), nn.AdaptiveAvgPool1d(8 if width == 32 else 1),
                nn.Flatten(), nn.Linear(width * (8 if width == 32 else 1), classes))
        else:
            raise ValueError('Unknown architecture')
        self.variant = variant

    def forward(self, x):
        if self.variant == 'statistics':
            x = torch.cat((x.mean(-1), x.std(-1, unbiased=False), (x[..., 1:] - x[..., :-1]).abs().mean(-1)), 1)
        return self.net(x)


def scores(labels, predictions, classes):
    matrix = np.zeros((classes, classes), dtype=np.int64)
    np.add.at(matrix, (labels, predictions), 1)
    tp = np.diag(matrix)
    denominator = matrix.sum(0) + matrix.sum(1)
    f1 = np.divide(2 * tp, denominator, out=np.zeros(classes, dtype=float), where=denominator > 0)
    return {'accuracy': float(tp.sum() / len(labels)), 'macro_f1': float(f1.mean()),
            'per_class_f1': f1.tolist(), 'confusion_matrix': matrix.tolist(), 'samples': len(labels)}


def predict(model, x, batch_size=128):
    model.eval()
    with torch.inference_mode():
        return torch.cat([model(x[i:i + batch_size]) for i in range(0, len(x), batch_size)]).cpu().numpy()


def latency(model, x, repetitions=100):
    """CPU model-only batch-one timing, excluding preprocessing and IO."""
    sample = x[:1]
    model.eval()
    with torch.inference_mode():
        for _ in range(20):
            model(sample)
        elapsed = []
        for _ in range(repetitions):
            start = time.perf_counter_ns()
            model(sample)
            elapsed.append((time.perf_counter_ns() - start) / 1e6)
    return {'p50_ms': float(np.percentile(elapsed, 50)), 'p95_ms': float(np.percentile(elapsed, 95)),
            'batch_size': 1, 'repetitions': repetitions, 'scope': 'CPU model only, excludes preprocessing and IO'}


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2, allow_nan=False) + '\n')


def validate_partitions(partitions, classes):
    shape = None
    for split in ('train', 'validation', 'test'):
        x, y = partitions[split]['x'], partitions[split]['y']
        if not isinstance(x, np.ndarray) or x.ndim != 3 or not 0 < len(x) <= 100000 or not 1 <= x.shape[1] <= 256 or not 2 <= x.shape[2] <= 2048:
            raise ValueError('Expected bounded N,C,T amplitude arrays')
        if x.nbytes > 2 * 1024**3 or not np.isfinite(x).all():
            raise ValueError('Oversized or nonfinite amplitudes')
        if not isinstance(y, np.ndarray) or y.shape != (len(x),) or y.dtype.kind not in 'iu' or np.any(y < 0) or np.any(y >= classes):
            raise ValueError('Invalid class labels')
        if shape is not None and shape != x.shape[1:]:
            raise ValueError('Partitions have inconsistent signal shape')
        shape = x.shape[1:]
    if set(partitions['train']['y'].tolist()) != set(range(classes)):
        raise ValueError('Training partition must contain every class')
    return shape


def optimize(partitions, provenance, output, *, epochs=20, seed=42, batch_size=64,
             variants=('statistics', 'temporal16', 'temporal32'), learning_rate=0.003, max_seconds=600):
    # Internal API: partitions must come from the provenance-validating intake.
    if not 1 <= max_seconds <= 3600:
        raise ValueError('Invalid time budget')
    started = time.monotonic()
    if not 1 <= epochs <= 200 or not 1 <= batch_size <= 1024 or not 0 < learning_rate <= 0.1:
        raise ValueError('Invalid bounded training parameters')
    if not variants or len(set(variants)) != len(variants) or any(v not in ('statistics', 'temporal16', 'temporal32') for v in variants):
        raise ValueError('Invalid architecture search')
    classes = len(provenance['classes'])
    if not 2 <= classes <= 100:
        raise ValueError('Invalid classes')
    channels, samples = validate_partitions(partitions, classes)
    root = Path(output)
    root.mkdir(parents=True, exist_ok=False)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    try:
        torch.set_num_threads(1)
        torch.use_deterministic_algorithms(True)
        # Streaming channel statistics avoid a second full float64 dataset copy.
        train_x = partitions['train']['x']
        count = len(train_x) * samples
        total = np.zeros(channels, dtype=np.float64)
        squares = np.zeros(channels, dtype=np.float64)
        for start in range(0, len(train_x), batch_size):
            block = train_x[start:start + batch_size].astype(np.float64)
            total += block.sum(axis=(0, 2))
            squares += np.square(block).sum(axis=(0, 2))
        mean = total / count
        scale = np.sqrt(np.maximum(squares / count - mean**2, 1e-12))
        tensors = {}
        for split, part in partitions.items():
            normalized = ((part['x'] - mean[None, :, None]) / scale[None, :, None]).astype(np.float32)
            if not np.isfinite(normalized).all():
                raise ValueError('Normalization overflow')
            tensors[split] = torch.from_numpy(normalized)
        labels = torch.from_numpy(partitions['train']['y'].astype(np.int64))
        weights = len(labels) / (classes * np.bincount(labels.numpy(), minlength=classes))
        loss_fn = nn.CrossEntropyLoss(weight=torch.tensor(weights, dtype=torch.float32))
        records, winner, winner_score = [], None, -1.0
        for variant in variants:
            torch.manual_seed(seed)
            rng = np.random.default_rng(seed)
            model = AmplitudeClassifier(channels, classes, variant)
            optimizer = torch.optim.AdamW(model.parameters(), lr=learning_rate, weight_decay=1e-4)
            scheduler = torch.optim.lr_scheduler.CosineAnnealingLR(optimizer, T_max=epochs)
            best, state, history, best_epoch = -1.0, None, [], None
            for epoch in range(epochs):
                model.train()
                order = rng.permutation(len(labels))
                for start in range(0, len(order), batch_size):
                    if time.monotonic() - started > max_seconds:
                        raise TimeoutError('Training time budget exceeded; no complete candidate')
                    ids = order[start:start + batch_size]
                    optimizer.zero_grad(set_to_none=True)
                    loss = loss_fn(model(tensors['train'][ids]), labels[ids])
                    if not torch.isfinite(loss):
                        raise ValueError('Nonfinite training loss')
                    loss.backward()
                    nn.utils.clip_grad_norm_(model.parameters(), 1.0, error_if_nonfinite=True)
                    optimizer.step()
                scheduler.step()
                result = scores(partitions['validation']['y'], predict(model, tensors['validation']).argmax(1), classes)
                history.append({'epoch': epoch + 1, 'validation_macro_f1': result['macro_f1']})
                if result['macro_f1'] > best:
                    best, state, best_epoch = result['macro_f1'], copy.deepcopy(model.state_dict()), epoch + 1
                if epoch + 1 - best_epoch >= 5:
                    break
            model.load_state_dict(state)
            model.eval()
            record = {'variant': variant, 'best_epoch': best_epoch, 'validation_macro_f1': best,
                      'parameters': sum(p.numel() for p in model.parameters()), 'history': history,
                      'latency': latency(model, tensors['validation'])}
            records.append(record)
            # Ties keep the earlier predeclared candidate.
            if best > winner_score:
                winner, winner_score = copy.deepcopy(model), best
        original = predict(winner, tensors['validation'])
        fused = copy.deepcopy(winner)
        if fused.variant != 'statistics':
            for conv, bn in ((0, 1), (3, 4)):
                fused.net[conv] = torch.nn.utils.fusion.fuse_conv_bn_eval(fused.net[conv], fused.net[bn])
                fused.net[bn] = nn.Identity()
        difference = float(np.max(np.abs(original - predict(fused, tensors['validation']))))
        if difference > 1e-4:
            raise ValueError('Fused inference exceeded logit equivalence tolerance')
        # Architecture and checkpoint are frozen before first test predictions.
        test_metrics = scores(partitions['test']['y'], predict(fused, tensors['test']).argmax(1), classes)
        majority = int(np.bincount(labels.numpy(), minlength=classes).argmax())
        majority_metrics = scores(partitions['test']['y'], np.full(len(partitions['test']['y']), majority), classes)
        prep = {'mean': mean.tolist(), 'scale': scale.tolist(), 'fit_partition': 'train',
                'channels': channels, 'samples': samples, 'input': 'amplitude_NCT', 'classes': provenance['classes']}
        write_json(root / 'preprocessing.json', prep)
        save_file({k: v.contiguous() for k, v in fused.state_dict().items()}, str(root / 'model.safetensors'))
        reloaded = AmplitudeClassifier(channels, classes, winner.variant, winner.variant != 'statistics').eval()
        reloaded.load_state_dict(load_file(str(root / 'model.safetensors')), strict=True)
        reload_error = float(np.max(np.abs(predict(fused, tensors['validation']) - predict(reloaded, tensors['validation']))))
        if reload_error != 0.0:
            raise ValueError('Exported inference failed exact reload equivalence')
        report = {'schema_version': 1, 'status': 'complete', 'promotionAuthorized': False,
                  'task': 'public_amplitude_activity_classification', 'variant': winner.variant,
                  'fused': winner.variant != 'statistics', 'seed': seed, 'epochs': epochs,
                  'batch_size': batch_size, 'learning_rate': learning_rate, 'early_stopping_patience': 5, 'selection': 'validation macro F1 only; first candidate wins ties',
                  'candidates': records, 'test': test_metrics, 'majority_test': majority_metrics,
                  'fused_max_abs_logit_error': difference, 'reload_max_abs_logit_error': reload_error, 'latency': latency(fused, tensors['validation']),
                  'parameters': sum(p.numel() for p in fused.parameters()), 'model_bytes': (root / 'model.safetensors').stat().st_size,
                  'elapsed_seconds': time.monotonic() - started, 'max_seconds': max_seconds, 'provenance': provenance,
                  'evidence': 'SYNTHETIC' if provenance.get('evidence') == 'SYNTHETIC' else 'MEASURED',
                  'implementation_sha256': digest(__file__),
                  'runtime': {'torch': torch.__version__, 'numpy': np.__version__, 'platform': platform.platform(), 'threads': 1, 'device': 'cpu'},
                  'model_sha256': digest(root / 'model.safetensors'), 'preprocessing_sha256': digest(root / 'preprocessing.json')}
        write_json(root / 'metrics.json', report)
        write_json(root / 'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except Exception:
        write_json(root / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise


class AmplitudeBundle:
    def __init__(self, directory):
        root = Path(directory)
        for name in ('status.json', 'metrics.json', 'preprocessing.json', 'model.safetensors'):
            path = root / name
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 10 * 1024**2:
                raise ValueError('Invalid bounded bundle artifact')
        status = json.loads((root / 'status.json').read_text())
        report = json.loads((root / 'metrics.json').read_text())
        if status.get('status') != 'complete' or status.get('promotionAuthorized') is not False or report.get('status') != 'complete' or report.get('promotionAuthorized') is not False or report.get('task') != 'public_amplitude_activity_classification':
            raise ValueError('Not a complete proposal-only amplitude bundle')
        for name in ('model', 'preprocessing'):
            filename = name + ('.safetensors' if name == 'model' else '.json')
            if digest(root / filename) != report.get(name + '_sha256'):
                raise ValueError('Bundle hash mismatch')
        prep = json.loads((root / 'preprocessing.json').read_text())
        self.channels, self.samples = prep['channels'], prep['samples']
        self.mean, self.scale = np.asarray(prep['mean']), np.asarray(prep['scale'])
        if not 1 <= self.channels <= 256 or not 2 <= self.samples <= 2048 or self.mean.shape != (self.channels,) or self.scale.shape != self.mean.shape or not np.isfinite(self.mean).all() or not np.isfinite(self.scale).all() or not (self.scale > 0).all() or prep.get('fit_partition') != 'train':
            raise ValueError('Invalid preprocessing')
        if not 2 <= len(prep['classes']) <= 100:
            raise ValueError('Invalid class count')
        self.preprocessing_folded = report.get('preprocessing_folded', False)
        if not isinstance(self.preprocessing_folded, bool) or (self.preprocessing_folded and report['variant'] != 'statistics'):
            raise ValueError('Invalid folded preprocessing architecture')
        self.model = AmplitudeClassifier(self.channels, len(prep['classes']), report['variant'], report['fused']).eval()
        self.model.load_state_dict(load_file(str(root / 'model.safetensors')), strict=True)
        if any(not torch.isfinite(v).all() for v in self.model.state_dict().values()):
            raise ValueError('Nonfinite weights')

    def predict(self, amplitudes):
        x = np.asarray(amplitudes)
        if x.ndim != 3 or x.shape[1:] != (self.channels, self.samples) or not 0 < len(x) <= 4096 or not np.isfinite(x).all():
            raise ValueError('Expected finite bounded N,C,T amplitudes')
        if self.preprocessing_folded:
            x = x.astype(np.float32, copy=False)
        else:
            x = ((x - self.mean[None, :, None]) / self.scale[None, :, None]).astype(np.float32)
        if not np.isfinite(x).all():
            raise ValueError('Normalization overflow')
        return predict(self.model, torch.from_numpy(x))



def optimize_bundle(source, validation, output, *, repetitions=1000):
    """Fold statistics preprocessing without training or recomputing test predictions.

    For positive channel scale s, mean((x-m)/s)=(mean(x)-m)/s,
    std((x-m)/s)=std(x)/s and absdiff((x-m)/s)=absdiff(x)/s.
    Therefore the affine transform folds into the first linear layer exactly
    in real arithmetic. Floating point equivalence is checked on validation.
    """
    if not 50 <= repetitions <= 2000:
        raise ValueError('Invalid benchmark repetitions')
    torch.set_num_threads(1)
    original = AmplitudeBundle(source)
    if original.model.variant != 'statistics' or original.preprocessing_folded:
        raise ValueError('Requires an unfused statistics bundle')
    # Validate signal contract before creating outputs.
    original.predict(validation[:1])
    if len(validation) > 4096:
        raise ValueError('Validation population exceeds bound')
    expected = original.predict(validation)
    folded = copy.deepcopy(original.model)
    linear = folded.net[0]
    weight = linear.weight.detach().double()
    scale = torch.from_numpy(np.tile(original.scale, 3))
    offset = torch.from_numpy(original.mean / original.scale)
    with torch.no_grad():
        linear.bias.copy_((linear.bias.double() - weight[:, :original.channels] @ offset).float())
        linear.weight.copy_((weight / scale[None, :]).float())
    if any(not torch.isfinite(v).all() for v in folded.state_dict().values()):
        raise ValueError('Folded weights are not finite')
    actual = predict(folded, torch.from_numpy(np.asarray(validation, dtype=np.float32)))
    max_error = float(np.max(np.abs(expected - actual)))
    if not np.allclose(expected, actual, rtol=1e-4, atol=1e-4) or not np.array_equal(expected.argmax(1), actual.argmax(1)):
        raise ValueError('Folded preprocessing failed logit or prediction equivalence')
    root = Path(output)
    root.mkdir(parents=True, exist_ok=False)
    write_json(root / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    try:
        source = Path(source)
        report = json.loads((source / 'metrics.json').read_text())
        # Historical test evidence stays in the parent bundle, not a new score.
        for key in ('test', 'majority_test', 'latency'):
            report.pop(key, None)
        (root / 'preprocessing.json').write_bytes((source / 'preprocessing.json').read_bytes())
        save_file({k: v.contiguous() for k, v in folded.state_dict().items()}, str(root / 'model.safetensors'))
        report.update({'preprocessing_folded': True,
                       'model_sha256': digest(root / 'model.safetensors'),
                       'model_bytes': (root / 'model.safetensors').stat().st_size,
                       'implementation_sha256': digest(__file__),
                       'parent_metrics_sha256': digest(source / 'metrics.json'),
                       'parent_model_sha256': digest(source / 'model.safetensors'),
                       'test_evaluation': 'Test predictions and metrics not recomputed; transformation checked on validation',
                       'optimization': {'operation': 'Fold channel normalization into statistics linear layer',
                                        'validation_samples': len(validation), 'max_abs_logit_error': max_error,
                                        'identical_validation_predictions': True}})
        write_json(root / 'metrics.json', report)
        # Validate the reloaded tensors using the same public prediction path,
        # while status remains running until verification and benchmarking end.
        optimized = copy.deepcopy(original)
        optimized.preprocessing_folded = True
        optimized.model.load_state_dict(load_file(str(root / 'model.safetensors')), strict=True)
        reloaded = optimized.predict(validation)
        if not np.array_equal(actual, reloaded):
            raise ValueError('Folded bundle reload changed logits')
        sample = validation[:1]
        for _ in range(30):
            original.predict(sample)
            optimized.predict(sample)
        times = {'original': [], 'optimized': []}
        # Interleave and alternate order to reduce clock/load drift bias.
        for i in range(repetitions):
            order = [('original', original), ('optimized', optimized)]
            if i % 2:
                order.reverse()
            for name, bundle in order:
                started = time.perf_counter_ns()
                bundle.predict(sample)
                times[name].append((time.perf_counter_ns() - started) / 1e6)
        timings = {name: {'p50_ms': float(np.percentile(values, 50)),
                          'p95_ms': float(np.percentile(values, 95))} for name, values in times.items()}
        report['optimization'].update({'batch_size': 1, 'repetitions': repetitions, 'threads': 1,
                                       'scope': 'CPU bundle predict including input checks and normalization; excludes IO and temporal pooling',
                                       'timings': timings, 'reload_max_abs_logit_error': 0.0,
                                       'p50_speedup': timings['original']['p50_ms'] / timings['optimized']['p50_ms']})
        write_json(root / 'metrics.json', report)
        write_json(root / 'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except Exception:
        write_json(root / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', required=True)
    parser.add_argument('--output', required=True)
    parser.add_argument('--epochs', type=int, default=20)
    parser.add_argument('--batch-size', type=int, default=64)
    parser.add_argument('--seed', type=int, default=42)
    parser.add_argument('--max-seconds', type=int, default=600)
    parser.add_argument('--optimize-bundle', help='Existing statistics bundle; inference only, no training or test evaluation')
    args = parser.parse_args(argv)
    from .espfi import load_dataset
    partitions, provenance = load_dataset(args.manifest)
    if args.optimize_bundle:
        parent = json.loads((Path(args.optimize_bundle) / 'metrics.json').read_text())
        if provenance['manifest_sha256'] != parent['provenance']['manifest_sha256']:
            raise ValueError('Optimization dataset differs from training provenance')
        report = optimize_bundle(args.optimize_bundle, partitions['validation']['x'], args.output)
        print(json.dumps(report['optimization'], indent=2))
        return
    report = optimize(partitions, provenance, args.output, epochs=args.epochs, batch_size=args.batch_size, seed=args.seed, max_seconds=args.max_seconds)
    print(json.dumps({'variant': report['variant'], 'test': report['test'], 'latency': report['latency'], 'promotionAuthorized': False}, indent=2))
