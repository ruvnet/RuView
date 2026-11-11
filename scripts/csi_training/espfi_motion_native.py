"""Explicit trusted native reductions; never loaded through model metadata.

The caller must supply a separately trusted kernel path and SHA-256. A digest
verifies bytes, not code safety. Native code executes with process authority.
Only Linux local shared libraries are supported. Default runtime is unchanged.
"""
import ctypes
import hashlib
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
import numpy as np
from .espfi_motion_runtime import BufferedMotionBundle


def _verified_kernel(path,expected):
    if not isinstance(expected,str) or re.fullmatch(r'[0-9a-fA-F]{64}',expected) is None:
        raise ValueError('Explicit trusted kernel SHA-256 required')
    if sys.platform != 'linux':
        raise ValueError('Native kernel currently supports Linux only')
    # O_NOFOLLOW rejects leaf symlinks atomically; fstat validates the opened file.
    fd=os.open(Path(path),os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK)
    try:
        info=os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or not 0<info.st_size<=16*1024**2:
            raise ValueError('Expected bounded regular kernel')
        with os.fdopen(fd,'rb',closefd=False) as handle:
            data=handle.read(16*1024**2+1)
    finally:
        os.close(fd)
    if len(data)>16*1024**2 or hashlib.sha256(data).hexdigest()!=expected.lower():
        raise ValueError('Kernel hash or size mismatch')
    return data


class NativeMotionBundle(BufferedMotionBundle):
    """Verified explicit kernel; inherited locked prediction and owned output."""
    def __init__(self,model_directory,kernel_path,expected_kernel_sha256):
        super().__init__(model_directory)
        data=_verified_kernel(kernel_path,expected_kernel_sha256)
        self._kernel_directory=tempfile.TemporaryDirectory(prefix='espfi-native-')
        try:
            private=Path(self._kernel_directory.name)/'kernel.so'
            with private.open('xb') as handle: handle.write(data)
            self._library=ctypes.CDLL(str(private))
            abi=self._library.espfi_motion_abi
            abi.argtypes=[];abi.restype=ctypes.c_uint
            if abi()!=1: raise ValueError('Unsupported native kernel ABI')
            self._native=self._library.espfi_motion_features
            self._native.argtypes=[np.ctypeslib.ndpointer(dtype=np.float64,ndim=2,shape=(52,190),flags='C_CONTIGUOUS'),
                                   ctypes.c_size_t,np.ctypeslib.ndpointer(dtype=np.float64,ndim=2,shape=(1,468),flags=('C_CONTIGUOUS','WRITEABLE')),
                                   ctypes.c_size_t]
            self._native.restype=ctypes.c_int
        except Exception:
            self._kernel_directory.cleanup()
            raise

    def _extract(self,sample):
        np.maximum(sample,1e-6,out=self._centered,dtype=np.float64)
        np.log(self._centered,out=self._centered)
        self._centered.mean(axis=-1,keepdims=True,out=self._mean)
        self._centered -= self._mean
        if self._native(self._centered,9880,self._features,468)!=0:
            raise ValueError('Native centered reduction rejected input')
        return self._features
