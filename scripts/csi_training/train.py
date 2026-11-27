"""Reproducible group-held-out temporal contrastive training.

No remote code, model upload, provisioning, or automatic promotion. Temporal
triplet accuracy describes embeddings only, never occupancy or pose accuracy.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import random
import time

import numpy as np


def temporal_triplets(partition, seed, limit=4096):
    """One positive 0<dt<=2s and one negative >30s, within each session/node.

    No in-batch negatives: each anchor has exactly its explicitly screened
    negative. This avoids silently labeling nearby windows as negative.
    """
    rng = np.random.default_rng(seed)
    groups = {}
    for i, key in enumerate(zip(partition.session_ids, partition.node_ids)):
        groups.setdefault(key, []).append(i)
    candidates, eligible = [], 0
    for indices in groups.values():
        ids = np.asarray(indices)
        times = np.asarray(partition.timestamps)[ids]
        order = np.argsort(times, kind='stable')
        ids, times = ids[order], times[order]
        # Binary search bounds avoid allocating an NxN distance matrix.
        for j in range(len(ids)):
            t = times[j]
            lo = np.searchsorted(times, t - 2, side='left')
            hi = np.searchsorted(times, t + 2, side='right')
            equal_lo = np.searchsorted(times, t, side='left')
            equal_hi = np.searchsorted(times, t, side='right')
            npos_left = equal_lo - lo
            npos = npos_left + hi - equal_hi
            left = np.searchsorted(times, t - 30, side='left')
            right = np.searchsorted(times, t + 30, side='right')
            nneg = left + len(ids) - right
            if npos > 0 and nneg:
                n = int(rng.integers(nneg))
                neg = ids[n] if n < left else ids[right + n - left]
                draw = int(rng.integers(npos))
                p = lo + draw if draw < npos_left else equal_hi + draw - npos_left
                candidate = (ids[j], ids[p], neg)
                # Reservoir sampling bounds candidate storage globally while
                # giving every eligible anchor the same inclusion probability.
                eligible += 1
                if len(candidates) < limit:
                    candidates.append(candidate)
                else:
                    slot = int(rng.integers(eligible))
                    if slot < limit:
                        candidates[slot] = candidate
    if not candidates:
        raise ValueError('Each split needs same-session/node positive pairs 0<dt<=2s and negatives >30s')
    rng.shuffle(candidates)
    return np.asarray(candidates[:limit], dtype=np.int64)


def triplet_accuracy(embeddings, triplets):
    a, p, n = (embeddings[triplets[:, i]] for i in range(3))
    pos, neg = np.sum(a * p, axis=1), np.sum(a * n, axis=1)
    # Ties earn half credit; constant embeddings cannot score 100%.
    return float(np.mean((pos > neg) + 0.5 * (pos == neg)))


def unit_rows(x):
    return x / np.maximum(np.linalg.norm(x, axis=1, keepdims=True), 1e-12)


def sha256(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + '\n')


def run_training(args):
    # Set before CUDA context creation; deterministic ops fail rather than drift.
    os.environ.setdefault('CUBLAS_WORKSPACE_CONFIG', ':4096:8')
    import torch
    import safetensors
    from safetensors.torch import save_file
    from .data import load_manifest, fit_standardizer
    from .model import CSIEncoder

    started = time.monotonic()
    def check_budget():
        if time.monotonic() - started >= args.max_seconds:
            raise TimeoutError('Training time budget exhausted; candidate is incomplete and cannot promote')

    for name, low, high in [('epochs', 1, 10000), ('batch_size', 2, 4096),
                            ('max_triplets', 2, 100000), ('max_seconds', 1, 86400)]:
        value = getattr(args, name)
        if not math.isfinite(value) or not low <= value <= high:
            raise ValueError(f'{name} must be between {low} and {high}')
    if not 0 < args.learning_rate <= 0.1 or not 0.01 <= args.temperature <= 10:
        raise ValueError('Invalid learning rate or temperature')
    if not 0 <= args.seed < 2**32:
        raise ValueError('seed must be in [0, 2**32)')
    out = Path(args.output)
    if out.exists():
        raise ValueError('Output must not exist; never overwrite a training run')
    dataset = load_manifest(args.manifest, group_by=args.group_by, allow_synthetic=args.allow_synthetic)
    scaler = fit_standardizer(dataset.partitions['train'].features)
    triplets = {name: temporal_triplets(part, args.seed + i, args.max_triplets)
                for i, (name, part) in enumerate(dataset.partitions.items())}
    values = {name: scaler.transform(part.features).astype(np.float32)
              for name, part in dataset.partitions.items()}
    if any(not np.isfinite(x).all() for x in values.values()):
        raise ValueError('Non-finite standardized input')
    if args.device == 'cuda' and not torch.cuda.is_available():
        raise ValueError('CUDA requested but unavailable; no silent CPU fallback')
    random.seed(args.seed)
    np.random.seed(args.seed)
    torch.manual_seed(args.seed)
    torch.set_num_threads(1)
    torch.use_deterministic_algorithms(True)
    torch.backends.cudnn.benchmark = False
    model = CSIEncoder().to(args.device)
    initial = {k: v.detach().cpu().clone() for k, v in model.state_dict().items()}
    tensors = {k: torch.from_numpy(v).to(args.device) for k, v in values.items()}

    def encode_partition(network, tensor):
        chunks = []
        for batch in tensor.split(args.batch_size):
            check_budget()
            chunks.append(network(batch).cpu().numpy())
        return np.concatenate(chunks, axis=0)

    def embed(network):
        network.eval()
        result = {}
        with torch.no_grad():
            for name, tensor in tensors.items():
                check_budget()
                result[name] = encode_partition(network, tensor)
        return result

    # Preserve initialization; evaluate the random control and candidate test
    # embeddings only after checkpoint selection is frozen.
    optimizer = torch.optim.AdamW(model.parameters(), lr=args.learning_rate)
    rng = np.random.default_rng(args.seed)
    history, best, best_epoch, best_state = [], -1.0, 0, None
    out.mkdir(parents=True, exist_ok=False)
    write_json(out / 'status.json', {'status': 'running', 'promotionAuthorized': False})
    try:
        for epoch in range(args.epochs):
            check_budget()
            model.train()
            ordered = triplets['train'][rng.permutation(len(triplets['train']))]
            losses = []
            for start in range(0, len(ordered), args.batch_size):
                check_budget()
                batch = ordered[start:start + args.batch_size]
                # Concatenation gives BN at least three samples even for tail=1.
                flat = torch.as_tensor(batch.reshape(-1), device=args.device)
                z = model(tensors['train'][flat]).reshape(-1, 3, 128)
                logits = torch.stack(((z[:, 0] * z[:, 1]).sum(-1),
                                      (z[:, 0] * z[:, 2]).sum(-1)), dim=1) / args.temperature
                loss = torch.nn.functional.cross_entropy(logits, torch.zeros(len(batch), dtype=torch.long, device=args.device))
                if not torch.isfinite(loss):
                    raise ValueError('Non-finite loss')
                optimizer.zero_grad(set_to_none=True)
                loss.backward()
                torch.nn.utils.clip_grad_norm_(model.parameters(), 5, error_if_nonfinite=True)
                optimizer.step()
                losses.append(float(loss.detach()))
            model.eval()
            with torch.no_grad():
                validation = encode_partition(model, tensors['validation'])
            score = triplet_accuracy(validation, triplets['validation'])
            history.append({'epoch': epoch + 1, 'loss': float(np.mean(losses)), 'validation_temporal_triplet_accuracy': score})
            if score > best:
                best, best_epoch = score, epoch + 1
                best_state = {k: v.detach().cpu().clone() for k, v in model.state_dict().items()}
        check_budget()
        model.load_state_dict(best_state)
        learned = embed(model)
        random_model = CSIEncoder().to(args.device)
        random_model.load_state_dict(initial)
        random_embeddings = embed(random_model)
        if any(not np.isfinite(x).all() for x in learned.values()):
            raise ValueError('Non-finite final embeddings')
        metrics = {name: {'triplets': len(triplets[name]),
                    'trained': triplet_accuracy(learned[name], triplets[name]),
                    'random_encoder': triplet_accuracy(random_embeddings[name], triplets[name]),
                    'standardized_input': triplet_accuracy(unit_rows(values[name]), triplets[name]),
                    'raw_input': triplet_accuracy(unit_rows(dataset.partitions[name].features), triplets[name])}
                   for name in tensors}
        delta = sum(float((best_state[k] - initial[k]).square().sum()) for k, _ in model.named_parameters())
        save_file(best_state, str(out / 'model.safetensors'))
        write_json(out / 'preprocessing.json', {'mean': np.asarray(scaler.mean).tolist(), 'scale': np.asarray(scaler.scale).tolist(), 'fit_partition': 'train', 'feature_count': 8})
        report = {'status': 'complete', 'promotionAuthorized': False,
                  'metric': 'temporal_triplet_accuracy_not_pose_or_presence',
                  'architecture': 'Linear(8,64),BN64,GELU,Linear(64,128),BN128,L2',
                  'parameter_count': sum(p.numel() for p in model.parameters()),
                  'evidence': 'SYNTHETIC' if any(e['evidence'] == 'synthetic' for e in dataset.inventory) else 'MEASURED',
                  'best_epoch': best_epoch, 'history': history, 'metrics': metrics,
                  'trainable_parameter_delta_squared': delta, 'group_by': args.group_by, 'seed': args.seed, 'device': args.device,
                  'config': {k: v for k, v in vars(args).items() if k not in ('manifest', 'output')},
                  'manifest_sha256': dataset.manifest_sha256,
                  'source_sha256': {name: sha256(Path(__file__).parent / name)
                                    for name in ('train.py', 'data.py', 'model.py')},
                  'data_files': [{'sha256': e['sha256'], 'split': e['split'], 'evidence': e['evidence'], 'captured_at': e['captured_at']} for e in dataset.inventory],
                  'model_sha256': sha256(out / 'model.safetensors'),
                  'preprocessing_sha256': sha256(out / 'preprocessing.json'),
                  'versions': {'torch': torch.__version__, 'numpy': np.__version__, 'safetensors': safetensors.__version__},
                  'elapsed_seconds': time.monotonic() - started,
                  'limitations': ['Temporal proxy does not establish task accuracy', 'No INT4 export or hardware RAM/latency claim', 'No automatic promotion or upload', 'Time budget is cooperative between batches, use an external process timeout for a hard deadline', 'Repeated test reuse requires an external frozen holdout policy']}
        write_json(out / 'metrics.json', report)
        write_json(out / 'status.json', {'status': 'complete', 'promotionAuthorized': False})
        return report
    except BaseException:
        write_json(out / 'status.json', {'status': 'failed', 'promotionAuthorized': False})
        raise


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    inspect = sub.add_parser('inspect', help='Validate local manifest without training')
    train = sub.add_parser('train', help='Train a local candidate; never publish or promote')
    for command in (inspect, train):
        command.add_argument('--manifest', required=True)
        command.add_argument('--group-by', choices=['room_id', 'session_id'], default='room_id')
        command.add_argument('--allow-synthetic', action='store_true')
    train.add_argument('--output', required=True)
    train.add_argument('--device', choices=['cpu', 'cuda'], default='cpu')
    train.add_argument('--epochs', type=int, default=20)
    train.add_argument('--batch-size', type=int, default=64)
    train.add_argument('--max-triplets', type=int, default=4096)
    train.add_argument('--seed', type=int, default=42)
    train.add_argument('--max-seconds', type=float, default=600)
    train.add_argument('--learning-rate', type=float, default=0.001)
    train.add_argument('--temperature', type=float, default=0.1)
    args = parser.parse_args(argv)
    try:
        if args.command == 'inspect':
            from .data import load_manifest
            result = load_manifest(args.manifest, group_by=args.group_by, allow_synthetic=args.allow_synthetic).inventory
        else:
            result = run_training(args)
        print(json.dumps(result, indent=2, allow_nan=False))
    except (ValueError, OSError, TimeoutError, ImportError) as exc:
        parser.exit(2, f'error: {exc}\n')
