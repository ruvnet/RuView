"""Seed encoding with optional measured float32 conversion for fixed CSI RFF."""
from pathlib import Path
import numpy as np
from safetensors.numpy import save_file
from .espfi_runtime import SPEC, SHAPES, random_map, tensor_hash, validate, load_state

def export_seeded(state, output, precision='float64'):
    from .espfi_train import digest, write_json
    if precision not in ('float64', 'float32'):
        raise ValueError('Unsupported precision')
    dtype = np.dtype(precision)
    generated = random_map()
    if set(state) != set(SHAPES) | set(generated) or any(
        not np.array_equal(state[k], generated[k]) for k in generated
    ):
        raise ValueError('Projection does not match frozen seed recipe')
    compact = {k: np.ascontiguousarray(state[k], dtype=dtype) for k in SHAPES}
    validate(compact, dtype)
    root = Path(output)
    root.mkdir(exist_ok=False, parents=True)
    save_file(compact, str(root/'model.safetensors'))
    report = {'status': 'complete', 'promotionAuthorized': False, 'spec': SPEC,
              'precision': precision, 'model_sha256': digest(root/'model.safetensors'),
              'model_bytes': (root/'model.safetensors').stat().st_size,
              'random_map_sha256': {k: tensor_hash(v) for k,v in generated.items()},
              'numpy_version': np.__version__, 'implementation_sha256': digest(__file__),
              'feature_implementation_sha256': digest(Path(__file__).with_name('espfi_generalize.py')),
              'stored_scalars': sum(v.size for v in compact.values()),
              'runtime_scalars': sum(v.size for v in state.values()),
              'runtime_state_bytes': sum(v.size for v in state.values()) * dtype.itemsize,
              'kernel_implementation_sha256': digest(Path(__file__).with_name('espfi_kernel.py')),
              'runtime_implementation_sha256': digest(Path(__file__).with_name('espfi_runtime.py')),
              'scope': 'Seed encoded map; float32 requires measured parity, float64 preserves original state'}
    write_json(root/'metadata.json', report)
    return report

class SeededKernelBundle:
    def __init__(self, directory):
        self.state = load_state(directory)

    def predict(self, amplitudes):
        from .espfi_generalize import features
        from .espfi_kernel import predict_features
        return predict_features(self.state, features(amplitudes, SPEC['family']))
