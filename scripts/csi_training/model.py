"""Small temporal CSI encoder; not a pose or occupancy classifier."""
import torch
from torch import nn
from torch.nn import functional as F


class CSIEncoder(nn.Module):
    """8 -> 64 -> 128, batch normalization, GELU and unit embeddings."""
    def __init__(self):
        super().__init__()
        self.w1 = nn.Linear(8, 64)
        self.bn1 = nn.BatchNorm1d(64)
        self.w2 = nn.Linear(64, 128)
        self.bn2 = nn.BatchNorm1d(128)

    def forward(self, x):
        return F.normalize(self.bn2(self.w2(F.gelu(self.bn1(self.w1(x))))), dim=-1)


class PreprocessedEncoder:
    """Verified local candidate loader; includes train-fitted normalization.

    Hashes detect accidental artifact corruption. They are not signatures and
    do not authenticate an untrusted directory or authorize model promotion.
    """
    def __init__(self, directory, device='cpu'):
        import hashlib
        import json
        from pathlib import Path
        import numpy as np
        from safetensors.torch import load_file

        root = Path(directory)
        paths = {name: root / name for name in ('status.json', 'metrics.json', 'preprocessing.json', 'model.safetensors')}
        for name, path in paths.items():
            if path.is_symlink() or not path.is_file() or path.stat().st_size > 2 * 1024 * 1024:
                raise ValueError(f'Invalid bounded artifact: {name}')
        status = json.loads(paths['status.json'].read_text())
        if not isinstance(status, dict) or status.get('status') != 'complete' or status.get('promotionAuthorized') is not False:
            raise ValueError('Candidate lifecycle is not complete and proposal-only')
        report = json.loads(paths['metrics.json'].read_text())
        if not isinstance(report, dict):
            raise ValueError('Invalid metrics schema')
        if report.get('architecture') != 'Linear(8,64),BN64,GELU,Linear(64,128),BN128,L2':
            raise ValueError('Unsupported encoder architecture')
        if report.get('status') != 'complete' or report.get('promotionAuthorized') is not False:
            raise ValueError('Candidate is not complete')
        for filename, key in [('model.safetensors', 'model_sha256'), ('preprocessing.json', 'preprocessing_sha256')]:
            if hashlib.sha256(paths[filename].read_bytes()).hexdigest() != report.get(key):
                raise ValueError(f'Artifact hash mismatch: {filename}')
        prep = json.loads(paths['preprocessing.json'].read_text())
        if not isinstance(prep, dict):
            raise ValueError('Invalid preprocessing schema')
        mean, scale = np.asarray(prep['mean'], dtype=np.float64), np.asarray(prep['scale'], dtype=np.float64)
        if (mean.shape != (8,) or scale.shape != (8,) or not np.isfinite(mean).all()
                or not np.isfinite(scale).all() or not (scale > 0).all()
                or prep.get('fit_partition') != 'train' or prep.get('feature_count') != 8):
            raise ValueError('Invalid preprocessing contract')
        self.mean, self.scale, self.device = mean, scale, device
        self.model = CSIEncoder().to(device).eval()
        self.model.load_state_dict(load_file(str(paths['model.safetensors']), device=device), strict=True)
        if any(not torch.isfinite(v).all() for v in self.model.state_dict().values()):
            raise ValueError('Non-finite model weights')

    def encode(self, features):
        import numpy as np
        values = np.asarray(features, dtype=np.float64)
        if values.ndim != 2 or values.shape[1] != 8 or not 0 < len(values) <= 4096 or not np.isfinite(values).all():
            raise ValueError('Expected 1 to 4096 finite eight-feature rows')
        with np.errstate(over='ignore', invalid='ignore'):
            standardized = ((values - self.mean) / self.scale).astype(np.float32)
        if not np.isfinite(standardized).all():
            raise ValueError('Input normalization overflow')
        with torch.no_grad():
            result = self.model(torch.from_numpy(standardized).to(self.device)).cpu().numpy()
        if not np.isfinite(result).all():
            raise ValueError('Non-finite encoder output')
        return result


def load_bundle(directory, device='cpu'):
    """Load a complete local bundle with verified hashes and preprocessing."""
    return PreprocessedEncoder(directory, device=device)
