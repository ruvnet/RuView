"""Bounded CSI inference without training dependencies.

The reference feature precision remains float64. One locked workspace is reused
per instance; returned scores own their storage. Models are research candidates.
"""
import hashlib
import json
from pathlib import Path
from threading import Lock
import numpy as np
from safetensors.numpy import load

SPEC = {'family': 'relative_combined', 'dimensions': 128, 'gamma_multiplier': .1,
        'alpha': .01, 'seed': 42, 'input_features': 468}
SHAPES = {'feature_mean': (468,), 'feature_scale': (468,), 'output_mean': (128,),
          'weights': (128, 7), 'bias': (7,)}
def random_map():
    rng = np.random.default_rng(42)
    return {'projection': rng.normal(0, np.sqrt(2*.1/468), (468, 128)),
            'phase': rng.uniform(0, 2*np.pi, 128)}
def tensor_hash(value):
    return hashlib.sha256(np.ascontiguousarray(value).tobytes()).hexdigest()
def validate(state, dtype=np.float64):
    if set(state) != set(SHAPES) or any(
        state[k].shape != shape or state[k].dtype != dtype or
        not np.isfinite(state[k]).all() for k, shape in SHAPES.items()
    ) or not (state['feature_scale'] > 0).all():
        raise ValueError('Invalid compact state')


def load_state(directory):
    root = Path(directory)
    contents = {}
    for name in ('metadata.json', 'model.safetensors'):
        p = root/name
        if p.is_symlink() or not p.is_file() or p.stat().st_size > 65536:
            raise ValueError('Invalid bounded compact artifact')
        with p.open('rb') as handle:
            contents[name] = handle.read(65537)
        if len(contents[name]) > 65536:
            raise ValueError('Invalid bounded compact artifact')
    report = json.loads(contents['metadata.json'])
    if (not isinstance(report, dict) or report.get('status') != 'complete'
            or report.get('promotionAuthorized') is not False or report.get('spec') != SPEC):
        raise ValueError('Invalid compact contract')
    if hashlib.sha256(contents['model.safetensors']).hexdigest() != report.get('model_sha256'):
        raise ValueError('Compact model hash mismatch')
    precision = report.get('precision', 'float64')
    if precision not in ('float64', 'float32'):
        raise ValueError('Unsupported precision')
    dtype = np.dtype(precision)
    # Deserialize precisely the bounded bytes whose digest was verified.
    state = load(contents['model.safetensors'])
    validate(state, dtype)
    generated = random_map()
    if {k:tensor_hash(v) for k,v in generated.items()} != report.get('random_map_sha256'):
        raise ValueError('Runtime random generator differs from export')
    state.update({k: np.ascontiguousarray(v, dtype=dtype) for k, v in generated.items()})
    return state


class BufferedKernelBundle:
    """Single-window workspace with bounded batch iteration and owned outputs.

    Inference calls on one instance serialize. Use an instance per worker for
    concurrent streams. Float64 features preserve the reference's near-constant
    signal behavior; fitted model tensors retain their exported precision.
    """
    def __init__(self, directory, *, fuse=True):
        state = load_state(directory)
        dtype = state['weights'].dtype
        self._lock = Lock()
        self._fuse = bool(fuse)
        if fuse:
            # Fold in double precision before one cast to the deployed dtype.
            projection = state['projection'].astype(np.float64)
            mean = state['feature_mean'].astype(np.float64)
            scale = state['feature_scale'].astype(np.float64)
            self.state = {
                'projection': np.ascontiguousarray(projection / scale[:, None], dtype=dtype),
                'phase': np.asarray(state['phase'] - (mean / scale) @ projection, dtype=dtype),
                'weights': state['weights'],
                'bias': np.asarray(state['bias'] - state['output_mean'].astype(np.float64)
                                   @ state['weights'].astype(np.float64), dtype=dtype),
            }
        else:
            self.state = state
        if any(not np.isfinite(value).all() for value in self.state.values()):
            raise ValueError('Nonfinite folded model state')
        self._relative = np.empty((52, 190), dtype=np.float64)
        self._square = np.empty_like(self._relative)
        self._channel_mean = np.empty((52, 1), dtype=np.float64)
        self._power = np.empty((52, 96), dtype=np.float64)
        self._features = np.empty((1, 468), dtype=np.float64)
        self._temporal = self._features[0, :208].reshape(52, 4)
        self._bands = self._features[0, 208:416].reshape(52, 4)
        self._rms = self._features[0, 416:]
        self._temporal_views = [self._relative[:, a:b] for a, b in
                                ((0, 48), (48, 96), (96, 143), (143, 190))]
        self._band_views = [self._power[:, a:b] for a, b in
                            ((1, 4), (4, 12), (12, 32), (32, 96))]
        self._z = np.empty((1, 468), dtype=dtype)
        self._hidden = np.empty((1, 128), dtype=dtype)
        self._normalizer = np.asarray(np.sqrt(2. / 128), dtype=dtype)

    @property
    def workspace_bytes(self):
        """Retained scratch only; excludes transient FFT/output and process RAM."""
        return sum(getattr(self, name).nbytes for name in
                   ('_relative', '_square', '_channel_mean', '_power', '_features', '_z', '_hidden'))

    def _extract(self, sample):
        np.maximum(sample, 1e-6, out=self._relative, dtype=np.float64)
        np.log(self._relative, out=self._relative)
        self._relative.mean(axis=-1, keepdims=True, out=self._channel_mean)
        self._relative -= self._channel_mean
        np.square(self._relative, out=self._square)
        scale = max(float(np.sqrt(self._square.mean())), 1e-6)
        self._relative /= scale
        np.square(self._relative, out=self._square)
        self._square.mean(axis=-1, out=self._rms)
        np.sqrt(self._rms, out=self._rms)
        for i, view in enumerate(self._temporal_views):
            view.mean(axis=-1, out=self._temporal[:, i])
        # Keep the frozen 190-point transform and band endpoints unchanged.
        spectrum = np.fft.rfft(self._relative, axis=-1)
        np.abs(spectrum, out=self._power)
        np.square(self._power, out=self._power)
        self._power /= 190**2
        for i, view in enumerate(self._band_views):
            view.sum(axis=-1, out=self._bands[:, i])
        np.copyto(self._z, self._features, casting='unsafe')

    def predict(self, values):
        x = np.asarray(values)
        if (x.ndim != 3 or x.shape[1:] != (52, 190) or not 0 < len(x) <= 4096
                or x.dtype.kind not in 'fiu' or not np.isfinite(x).all() or (x < 0).any()):
            raise ValueError('Expected 1..4096 finite nonnegative amplitude recordings N52T190')
        result = np.empty((len(x), 7), dtype=self._hidden.dtype)
        with self._lock:
            for i, sample in enumerate(x):
                self._extract(sample)
                if not self._fuse:
                    self._z -= self.state['feature_mean']
                    self._z /= self.state['feature_scale']
                np.matmul(self._z, self.state['projection'], out=self._hidden)
                self._hidden += self.state['phase']
                np.cos(self._hidden, out=self._hidden)
                self._hidden *= self._normalizer
                if not self._fuse:
                    self._hidden -= self.state['output_mean']
                np.matmul(self._hidden, self.state['weights'], out=result[i:i+1])
            result += self.state['bias']
        if not np.isfinite(result).all():
            raise ValueError('Nonfinite classifier output')
        return result
