#!/usr/bin/env python3
"""Run bounded serial public-data GPU jobs on Linux; never score final test."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import threading
import time
import uuid

SEEDS = (1729, 1730, 1731)
CHILD_TIMEOUT = 1020
MAX_LOG_BYTES = 8 * 1024 * 1024
MAX_METRICS_BYTES = 8 * 1024 * 1024
SUPPORTED_HOST = sys.platform == 'linux'


def require(ok, message):
    if not ok:
        raise ValueError(message)


def private_path(value):
    path = Path(value).expanduser().resolve()
    require(not any((p / '.git').exists() for p in (path, *path.parents)), 'private path must be outside Git')
    return path


def digest(path, limit=1024 ** 3):
    h = hashlib.sha256()
    total = 0
    with path.open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            total += len(block)
            require(total <= limit, 'file exceeded hash byte budget')
            h.update(block)
    return h.hexdigest()


def save(path, state):
    value = json.dumps(state, indent=2, allow_nan=False)
    require(len(value) < 256 * 1024, 'state exceeds budget')
    temporary = path.with_suffix('.tmp')
    temporary.write_text(value + '\n', encoding='utf-8')
    temporary.replace(path)


def gpu_free():
    result = subprocess.run(['nvidia-smi', '--id=0', '--query-gpu=memory.free', '--format=csv,noheader,nounits'],
                            capture_output=True, text=True, timeout=15, check=True)
    text = result.stdout.strip()
    require(len(text) <= 16 and text.isdigit(), 'invalid GPU memory response')
    available = int(text)
    require(4096 <= available <= 1024 * 1024, 'GPU 0 needs at least 4096 MiB free')
    return available


def command(args, seed, output):
    require(seed in SEEDS, 'unsupported seed')
    experiment = experiment_config(args)
    encoder = experiment['encoder']
    argv = [sys.executable, str(args.source_root / 'train.py'),
            '--dataset', str(args.dataset), '--metadata', str(args.metadata), '--output', str(output),
            '--mode', 'identity', '--identity-protocol', 'published_files', '--device', 'cuda',
            '--seed', str(seed), '--hidden', '64', '--window', '128', '--stride', '8',
            '--batch-size', '64', '--epochs', str(experiment['epochs']), '--max-steps', '100000', '--max-seconds', '900',
            '--max-train-windows', '40000', '--max-eval-windows', '50000', '--threads', '2']
    if encoder is not None:
        argv.extend(['--encoder', encoder])
    for name in ('representation', 'temporal_order'):
        if experiment[name] is not None:
            argv.extend(['--' + name.replace('_', '-'), experiment[name]])
    return argv


def experiment_config(args):
    result = {name: getattr(args, name, None) for name in ('encoder', 'representation', 'temporal_order')}
    result['epochs'] = getattr(args, 'epochs', 200)
    require(result['encoder'] in (None, 'lstm', 'gru'), 'unsupported encoder')
    require(result['representation'] in (None, 'raw', 'demean'), 'unsupported representation')
    require(result['temporal_order'] in (None, 'original', 'shuffle'), 'unsupported temporal order')
    require(type(result['epochs']) is int and 1 <= result['epochs'] <= 200, 'epochs must be 1 through 200')
    return result


def stop_child(child, process_group=False):
    previous = {}
    for name in ('SIGINT', 'SIGTERM', 'SIGHUP'):
        sig = getattr(signal, name, None)
        if sig is not None:
            previous[sig] = signal.signal(sig, signal.SIG_IGN)
    try:
        def send(sig):
            if process_group:
                try:
                    os.killpg(child.pid, sig)
                except ProcessLookupError:
                    pass
            elif sig == signal.SIGTERM:
                child.terminate()
            else:
                child.kill()
        if child.poll() is None:
            send(signal.SIGTERM)
            try:
                child.wait(timeout=20)
            except subprocess.TimeoutExpired:
                send(signal.SIGKILL if process_group else None)
                child.wait(timeout=20)
        else:
            child.wait(timeout=1)
        if process_group:
            # The leader may have exited while descendants retain the output pipe.
            send(signal.SIGTERM)
            send(signal.SIGKILL)
    finally:
        for sig, handler in previous.items():
            signal.signal(sig, handler)


def drain_log(pipe, log, errors, limit=MAX_LOG_BYTES):
    written = 0
    try:
        while block := pipe.read(65536):
            remaining = limit - written
            log.write(block[:remaining])
            written += min(len(block), remaining)
            if len(block) > remaining:
                errors.append('child log limit exceeded')
                return
    except (OSError, ValueError):
        errors.append('child log write failed')


def monitor_child(child, log, process_group):
    errors = []
    reader = threading.Thread(target=drain_log, args=(child.stdout, log, errors), daemon=True)
    reader.start()
    deadline = time.monotonic() + CHILD_TIMEOUT
    try:
        while child.poll() is None:
            require(not errors, errors[0] if errors else '')
            if time.monotonic() >= deadline:
                raise subprocess.TimeoutExpired('training child', CHILD_TIMEOUT)
            time.sleep(0.2)
        returncode = child.wait(timeout=1)
        # Clean up any descendants before waiting for pipe EOF.
        stop_child(child, process_group)
        reader.join(timeout=5)
        require(not reader.is_alive(), 'child log reader failed to stop')
        require(not errors, errors[0] if errors else '')
        return returncode
    finally:
        stop_child(child, process_group)
        reader.join(timeout=5)
        child.stdout.close()


def verify_inputs(args, state):
    for name, path in (('trainer', args.source_root / 'train.py'),
                       ('dataset', args.dataset), ('metadata', args.metadata)):
        require(digest(path, 1024 ** (3 if name == 'dataset' else 2)) == state[name + '_sha256'],
                name + ' changed during batch')


def verify_metrics(output, state, seed):
    path = output / 'metrics.json'
    require(path.is_file() and path.stat().st_size <= MAX_METRICS_BYTES, 'missing or oversized child metrics')
    with path.open('rb') as stream:
        raw = stream.read(MAX_METRICS_BYTES + 1)
    require(len(raw) <= MAX_METRICS_BYTES, 'child metrics grew beyond limit')
    metrics = json.loads(raw)
    require(isinstance(metrics, dict) and metrics.get('status') == 'COMPLETED', 'child metrics incomplete')
    for name in ('trainer', 'dataset', 'metadata'):
        key = name + '_sha256'
        require(metrics.get(key) == state[key], 'child metrics ' + key + ' mismatch')
    require('test' in metrics and metrics['test'] is None, 'missing or unexpected final test evaluation')
    require(metrics.get('mode') == 'identity' and metrics.get('test_evaluation_requested') is False,
            'child must be validation-only identity training')
    expected_config = {'seed': seed, 'mode': 'identity', 'identity_protocol': 'published_files',
                       'evaluate_test': False, 'device': 'cuda', 'hidden': 64, 'window': 128,
                       'stride': 8, 'batch_size': 64, 'max_steps': 100000, 'max_seconds': 900.0,
                       'max_train_windows': 40000, 'max_eval_windows': 50000, 'threads': 2}
    config = metrics.get('config')
    require(isinstance(config, dict), 'missing child configuration')
    for key, expected in expected_config.items():
        require(key in config and type(config[key]) is type(expected)
                and config[key] == expected, 'child configuration mismatch: ' + key)
    for key, expected in state.get('experiment', {}).items():
        if expected is not None:
            require(key in config and type(config[key]) is type(expected)
                    and config[key] == expected, 'child configuration mismatch: ' + key)
    return hashlib.sha256(raw).hexdigest()


def run(args):
    # Process groups bound descendants and inherited pipes. The Windows fallback
    # in the cleanup helper is not equivalent; do not launch GPU batches there.
    require(SUPPORTED_HOST, 'GPU batch supervision requires Linux process groups')
    seeds = getattr(args, 'seeds', SEEDS)
    require(isinstance(seeds, (list, tuple)) and 1 <= len(seeds) <= len(SEEDS)
            and all(type(seed) is int and seed in SEEDS for seed in seeds)
            and len(set(seeds)) == len(seeds), 'seeds must be a nonempty unique subset of the fixed seeds')
    experiment = experiment_config(args)
    encoder = experiment['encoder']
    args.dataset, args.metadata = private_path(args.dataset), private_path(args.metadata)
    args.run_root = private_path(args.run_root)
    args.source_root = Path(args.source_root).expanduser().resolve()
    require(args.dataset.is_file() and args.dataset.stat().st_size <= 1024 ** 3, 'invalid dataset file')
    require(args.metadata.is_file() and args.metadata.stat().st_size <= 1024 ** 2, 'invalid metadata file')
    trainer = args.source_root / 'train.py'
    require(trainer.is_file() and trainer.stat().st_size <= 1024 ** 2, 'invalid trainer source')
    os.umask(0o077)
    args.run_root.mkdir(mode=0o700, parents=True, exist_ok=True)
    lock = args.run_root / '.batch.lock'
    state = {'status': 'STARTING', 'pid': os.getpid(), 'jobs': [], 'test_evaluation': False,
             'requested_seeds': list(seeds),
             'encoder_requested': encoder,
             'experiment': experiment,
             'trainer_sha256': digest(trainer), 'launcher_sha256': digest(Path(__file__)),
             'dataset_sha256': digest(args.dataset), 'metadata_sha256': digest(args.metadata)}
    lock.mkdir(mode=0o700)  # Atomic and exclusive; a stale lock requires operator review.
    path = args.run_root / ('batch-' + uuid.uuid4().hex)
    state_path = path / 'batch-state.json'
    old_signals = {}
    def interrupted(signum, frame):
        raise KeyboardInterrupt('batch interrupted')
    try:
        path.mkdir(mode=0o700)
        (lock / 'owner.json').write_text(json.dumps({'pid': os.getpid(), 'batch': path.name}), encoding='utf-8')
        for name in ('SIGINT', 'SIGTERM', 'SIGHUP'):
            sig = getattr(signal, name, None)
            if sig is not None:
                old_signals[sig] = signal.signal(sig, interrupted)
        save(state_path, state)
        env = dict(os.environ, CUDA_VISIBLE_DEVICES='0', OMP_NUM_THREADS='2', MKL_NUM_THREADS='2', PYTHONUNBUFFERED='1')
        for seed in seeds:
            verify_inputs(args, state)
            free = gpu_free()
            output = path / f'seed-{seed}'
            require(not output.exists(), 'job output already exists')
            argv = command(args, seed, output)
            job = {'seed': seed, 'command': argv, 'status': 'RUNNING', 'gpu_free_before_mib': free,
                   'started_unix': time.time(), 'external_timeout_seconds': CHILD_TIMEOUT,
                   'peak_vram_mib': None, 'peak_vram_note': 'Not sampled by launcher'}
            state['jobs'].append(job)
            state['status'] = 'RUNNING'
            save(state_path, state)
            try:
                with (path / f'seed-{seed}.log').open('xb') as log:
                    # Store PID immediately after spawn so supervision does not wait for job completion.
                    process_group = os.name == 'posix'
                    child = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                             env=env, shell=False, start_new_session=process_group)
                    job['pid'] = child.pid
                    try:
                        save(state_path, state)
                        job['returncode'] = monitor_child(child, log, process_group)
                    finally:
                        stop_child(child, process_group)
                job['status'] = 'COMPLETED' if job['returncode'] == 0 else 'FAILED'
                require(job['returncode'] == 0, 'training child failed')
                verify_inputs(args, state)
                job['metrics_sha256'] = verify_metrics(output, state, seed)
            except BaseException as error:
                job['status'] = 'FAILED'
                job['error_type'] = type(error).__name__
                raise
            finally:
                job['ended_unix'] = time.time()
                save(state_path, state)
        state['status'] = 'COMPLETED'
        return state
    except BaseException as error:
        state['status'] = 'INCOMPLETE'
        state['error_type'] = type(error).__name__
        raise
    finally:
        for sig, previous in old_signals.items():
            signal.signal(sig, previous)
        if path.exists():
            save(state_path, state)
        owner = lock / 'owner.json'
        if owner.exists():
            owner.unlink()
        lock.rmdir()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('dataset', 'metadata', 'source-root', 'run-root'):
        parser.add_argument('--' + name, required=True)
    parser.add_argument('--seeds', nargs='+', type=int, choices=SEEDS, default=list(SEEDS),
                        help='unique subset of the fixed seeds; default runs all three')
    parser.add_argument('--encoder', choices=('lstm', 'gru'), default=None,
                        help='append an explicit encoder only when supplied; omission supports legacy trainers')
    parser.add_argument('--representation', choices=('raw', 'demean'), default=None)
    parser.add_argument('--temporal-order', choices=('original', 'shuffle'), default=None)
    parser.add_argument('--epochs', type=int, default=200, help='bounded comparison budget, 1 through 200')
    args = parser.parse_args(argv)
    try:
        run(args)
        return 0
    except (ValueError, OSError, subprocess.SubprocessError, KeyboardInterrupt) as error:
        print(json.dumps({'status': 'INCOMPLETE', 'error_type': type(error).__name__}), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
