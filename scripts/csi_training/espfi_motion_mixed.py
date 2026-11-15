"""Optional mixed ACF inference: float64 features, float32 folded classifier.

This is a separate numerical experiment, not bit-exact inference. Acceptance
requires measured <=1e-4 absolute logits and unchanged classes on admitted data;
it does not relax the default float64 runtime's <=1e-10 contract. No artifacts
are written or retrained. Scratch retains transformed last-window values.
"""
import numpy as np
from .espfi_motion_runtime import BufferedMotionBundle

MIXED_CONTRACT = {'feature_precision':'float64', 'fold_precision':'float64',
                  'classifier_precision':'float32', 'max_abs_logit_error':1e-4,
                  'identical_predictions_required':True, 'bit_exact':False,
                  'promotionAuthorized':False}


class MixedPrecisionMotionBundle(BufferedMotionBundle):
    """One locked fixed workspace; caller receives independently owned scores."""
    def __init__(self,directory):
        super().__init__(directory)
        with np.errstate(over='ignore',invalid='ignore'):
            converted = {k:np.ascontiguousarray(v,dtype=np.float32) for k,v in self.state.items()}
        if any(not np.isfinite(v).all() for v in converted.values()):
            raise ValueError('Folded state cannot be represented as finite float32')
        # Replacing the dictionary releases all instance references to float64 state.
        self.state = converted
        self._classifier_features = np.empty((1,468),dtype=np.float32)
        self._hidden = np.empty((1,128),dtype=np.float32)
        self._scores = np.empty((1,7),dtype=np.float32)
        self._normalizer = np.float32(np.sqrt(2./128))

    @property
    def workspace_bytes(self):
        return super().workspace_bytes + self._classifier_features.nbytes

    def predict(self,values):
        x = np.asarray(values)
        if (x.dtype.kind not in 'fiu' or x.ndim != 3 or x.shape[1:] != (52,190)
                or not 0 < len(x) <= 4096):
            raise ValueError('Expected 1..4096 real amplitude windows N52T190')
        result = np.empty((len(x),7),dtype=np.float32)
        with self._lock:
            for i,sample in enumerate(x):
                if not np.isfinite(sample).all() or (sample < 0).any():
                    raise ValueError('Expected finite nonnegative amplitudes')
                self._extract(sample)
                np.copyto(self._classifier_features,self._features,casting='unsafe')
                np.matmul(self._classifier_features,self.state['projection'],out=self._hidden)
                self._hidden += self.state['phase']
                np.cos(self._hidden,out=self._hidden)
                self._hidden *= self._normalizer
                np.matmul(self._hidden,self.state['weights'],out=self._scores)
                self._scores += self.state['bias']
                if not np.isfinite(self._scores).all():
                    raise ValueError('Nonfinite mixed runtime prediction')
                result[i] = self._scores[0]
        return result
