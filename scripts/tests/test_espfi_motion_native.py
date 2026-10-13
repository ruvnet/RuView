"""Explicit C boundary and synthetic parity checks; no sensing claims."""
from concurrent.futures import ThreadPoolExecutor
import ctypes
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_motion_native import NativeMotionBundle, _verified_kernel
from csi_training.espfi_motion_native_build import build
from csi_training.espfi_motion_runtime import BufferedMotionBundle
from csi_training.espfi_motion import motion_features
from test_espfi_motion_runtime import make_models


@unittest.skipUnless(sys.platform=='linux' and shutil.which('cc'),'Requires Linux system cc')
class NativeMotionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp=tempfile.TemporaryDirectory();cls.root=Path(cls.tmp.name)
        cls.receipt=build(cls.root/'build');cls.kernel=cls.root/'build'/'kernel.so'
        cls.parts,cls.original,cls.model=make_models(cls.root)
    @classmethod
    def tearDownClass(cls):cls.tmp.cleanup()
    def bundle(self):return NativeMotionBundle(self.model,self.kernel,self.receipt['library_sha256'])

    def test_parity_boundaries_strides_concurrency_ownership(self):
        native=self.bundle();reference=BufferedMotionBundle(self.model)
        rng=np.random.default_rng(39);floor=np.zeros((2,52,190),np.float32)
        floor[...,::2]=np.nextafter(np.float32(1e-6),np.float32(np.inf))
        sparse=np.ones((2,52,190));sparse[:,0]=np.exp(np.tile([-1.,1.],95)*1e-5)
        wave=np.tile([-1.,1.],95)
        boundaries=[np.broadcast_to(np.exp(wave*a),(2,52,190)).copy()
                    for a in [np.nextafter(1e-6,0),1e-6,np.nextafter(1e-6,np.inf),.999e-6,1.001e-6]]
        for x in [self.parts['validation']['x'],self.parts['validation']['x'][...,::-1],floor,sparse,np.zeros((2,52,190)),
                  np.ones((2,52,190),dtype=np.int64),np.full((2,52,190),1e100),
                  1e50*np.exp(rng.normal(0,1e-9,(2,52,190))),
                  np.exp(rng.normal(size=(2,52,380)))[...,::2]]+boundaries:
            features=motion_features(x,'motion_acf')
            for i,row in enumerate(x):np.testing.assert_allclose(native._extract(row),features[i:i+1],atol=1e-12,rtol=0)
            actual=native.predict(x)
            for expected in [reference.predict(x),self.original.predict(x)]:
                np.testing.assert_allclose(actual,expected,atol=1e-10,rtol=0)
                np.testing.assert_array_equal(actual.argmax(1),expected.argmax(1))
        x=self.parts['validation']['x'];expected=native.predict(x)
        kernel=native._native
        native._native=lambda *args:2
        try:
            with self.assertRaisesRegex(ValueError,'rejected input'):native.predict(x)
        finally:
            native._native=kernel
        np.testing.assert_array_equal(native.predict(x),expected)
        with ThreadPoolExecutor(max_workers=4) as pool:
            values=list(pool.map(lambda _:native.predict(x),range(8)))
        for v in values:np.testing.assert_array_equal(v,expected);self.assertTrue(v.flags.owndata)
        values[0][:]=999;np.testing.assert_array_equal(values[1],expected)
        for invalid in [np.ones((1,52,950)),np.full((1,52,190),np.nan),-np.ones((1,52,190))]:
            with self.assertRaises(ValueError):native.predict(invalid)

    def test_abi_bounds_nonfinite_library_hash_and_private_copy(self):
        bundle=self.bundle()
        library=ctypes.CDLL(str(self.kernel));fn=library.espfi_motion_features
        fn.argtypes=[ctypes.POINTER(ctypes.c_double),ctypes.c_size_t,ctypes.POINTER(ctypes.c_double),ctypes.c_size_t];fn.restype=ctypes.c_int
        x=np.zeros((52,190));out=np.zeros(468);ptr=lambda a:a.ctypes.data_as(ctypes.POINTER(ctypes.c_double))
        self.assertNotEqual(fn(None,9880,ptr(out),468),0)
        self.assertNotEqual(fn(ptr(x),9879,ptr(out),468),0)
        self.assertNotEqual(fn(ptr(x),9880,None,468),0)
        self.assertNotEqual(fn(ptr(x),9880,ptr(out),467),0)
        x[0,0]=np.inf;self.assertNotEqual(fn(ptr(x),9880,ptr(out),468),0)
        with self.assertRaises(ValueError):self.bundle_with_hash('0'*64)
        with self.assertRaises(ValueError):self.bundle_with_hash('not-a-digest')
        link=self.root/'symlink.so';link.symlink_to(self.kernel)
        with self.assertRaises((ValueError,OSError)):_verified_kernel(link,self.receipt['library_sha256'])
        huge=self.root/'oversized.so'
        with huge.open('wb') as h:h.truncate(16*1024**2+1)
        with self.assertRaises(ValueError):_verified_kernel(huge,'0'*64)
        self.assertNotEqual(Path(bundle._library._name),self.kernel)
        self.assertEqual(hashlib.sha256(Path(bundle._library._name).read_bytes()).hexdigest(),self.receipt['library_sha256'])
        # Compile a deliberately incompatible ABI fixture, never a model-selected path.
        wrong=self.root/'wrong.c';wrong.write_text('unsigned int espfi_motion_abi(void){return 2;}')
        target=self.root/'wrong.so';subprocess.run([shutil.which('cc'),'-shared','-fPIC',str(wrong),'-o',str(target)],check=True,capture_output=True)
        with self.assertRaisesRegex(ValueError,'ABI'):
            NativeMotionBundle(self.model,target,hashlib.sha256(target.read_bytes()).hexdigest())
    def bundle_with_hash(self,value):return NativeMotionBundle(self.model,self.kernel,value)

    def test_build_receipt_and_standalone_process(self):
        receipt=json.loads((self.root/'build'/'build.json').read_text())
        self.assertEqual(receipt['source_sha256'],hashlib.sha256((self.root/'build'/'kernel.c').read_bytes()).hexdigest())
        self.assertIn('-fno-fast-math',receipt['flags']);self.assertIn('-ffp-contract=off',receipt['flags'])
        with self.assertRaises(FileExistsError):build(self.root/'build')
        code="""import sys,numpy as np
from csi_training.espfi_motion_native import NativeMotionBundle
b=NativeMotionBundle(*sys.argv[1:]);assert b.predict(np.ones((1,52,190))).shape==(1,7)
assert not any(n=='torch' or n.startswith('torch.') or n=='scipy' or n.startswith('scipy.') for n in sys.modules)
assert 'csi_training.espfi_motion' not in sys.modules
assert 'csi_training.espfi_motion_compact' not in sys.modules
"""
        env=dict(os.environ,PYTHONPATH=str(Path(__file__).resolve().parents[1]))
        subprocess.run([sys.executable,'-c',code,str(self.model),str(self.kernel),receipt['library_sha256']],check=True,env=env,capture_output=True)

if __name__=='__main__':unittest.main()
