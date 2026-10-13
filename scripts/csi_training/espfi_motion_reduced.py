"""Optional float64 reduction variant; artifact and default runtime unchanged.

Reordered reductions require measured <=1e-10 logit error and unchanged classes.
No accuracy improvement or speed improvement is implied by this implementation.
"""
import numpy as np
from .espfi_motion_runtime import BufferedMotionBundle

REDUCTION_CONTRACT = {'precision':'float64','max_abs_logit_error':1e-10,
                      'identical_predictions_required':True,'bit_exact':False,
                      'energy':'per-channel einsum before global RMS and energy floor',
                      'differences':'sum absolute centered differences / length / global RMS',
                      'promotionAuthorized':False}


class ReducedMotionBundle(BufferedMotionBundle):
    """Inherits bounded artifact loader, locked prediction and owned outputs."""
    def _extract(self,sample):
        np.maximum(sample,1e-6,out=self._centered,dtype=np.float64)
        np.log(self._centered,out=self._centered)
        self._centered.mean(axis=-1,keepdims=True,out=self._mean)
        self._centered -= self._mean
        np.einsum('ij,ij->i',self._centered,self._centered,out=self._energy,optimize=False)
        # Compute the recording RMS from original energy, before channel flooring.
        scale = max(float(np.sqrt(self._energy.sum()/9880)),1e-6)
        np.maximum(self._energy,190e-12,out=self._energy)
        for i,lag in enumerate((1,2,4,8,16,32)):
            np.einsum('ij,ij->i',self._centered[:,lag:],self._centered[:,:-lag],
                      out=self._per_channel[:,i],optimize=False)
            self._per_channel[:,i] /= self._energy
        for i,lag in enumerate((1,4,16),6):
            scratch = self._scratch[:,:190-lag]
            np.subtract(self._centered[:,lag:],self._centered[:,:-lag],out=scratch)
            np.abs(scratch,out=scratch)
            np.add.reduce(scratch,axis=-1,out=self._per_channel[:,i])
            self._per_channel[:,i] /= 190-lag
            self._per_channel[:,i] /= scale
        return self._features
