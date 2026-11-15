"""Synthetic mixed-precision evidence; real-data gates are evaluated separately."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import numpy as np
from safetensors.numpy import load, save_file
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_motion_mixed import MixedPrecisionMotionBundle, MIXED_CONTRACT
from csi_training.espfi_motion_runtime import BufferedMotionBundle
from csi_training.espfi_train import write_json
from test_espfi_motion_runtime import make_models


class MixedMotionTests(unittest.TestCase):
    def test_feature_identity_logits_edges_and_memory(self):
        with tempfile.TemporaryDirectory() as tmp:
            parts,original,path=make_models(Path(tmp))
            artifact=(path/'model.safetensors').read_bytes()
            full=BufferedMotionBundle(path);mixed=MixedPrecisionMotionBundle(path)
            self.assertIs(MixedPrecisionMotionBundle._extract,BufferedMotionBundle._extract)
            self.assertEqual(mixed.model_state_bytes,243740)
            self.assertEqual(mixed.model_state_bytes*2,full.model_state_bytes)
            self.assertEqual(mixed.workspace_bytes,165068)
            self.assertTrue(all(v.dtype==np.float32 for v in mixed.state.values()))
            self.assertEqual(mixed._features.dtype,np.float64)
            self.assertEqual(mixed._classifier_features.dtype,np.float32)
            self.assertFalse(MIXED_CONTRACT['bit_exact'])
            rng=np.random.default_rng(39)
            floor=np.zeros((2,52,190),dtype=np.float32)
            floor[...,::2]=np.nextafter(np.float32(1e-6),np.float32(np.inf))
            for x in [parts['validation']['x'],floor,np.zeros((2,52,190)),np.full((2,52,190),1e100),
                      1e50*np.exp(rng.normal(0,1e-9,(2,52,190))),parts['validation']['x'][...,::-1]]:
                for sample in x:
                    np.testing.assert_array_equal(full._extract(sample),mixed._extract(sample))
                actual=mixed.predict(x)
                self.assertEqual(actual.dtype,np.float32)
                for expected in (full.predict(x),original.predict(x)):
                    np.testing.assert_allclose(actual,expected,atol=1e-4,rtol=0)
                    np.testing.assert_array_equal(actual.argmax(1),expected.argmax(1))
                np.testing.assert_array_equal(actual,np.concatenate([mixed.predict(v[None]) for v in x]))
            self.assertEqual(artifact,(path/'model.safetensors').read_bytes())

    def test_concurrency_ownership_and_invalid(self):
        with tempfile.TemporaryDirectory() as tmp:
            parts,_,path=make_models(Path(tmp));bundle=MixedPrecisionMotionBundle(path)
            x=parts['validation']['x'];expected=bundle.predict(x)
            with ThreadPoolExecutor(max_workers=4) as pool:
                outputs=list(pool.map(lambda _:bundle.predict(x),range(8)))
            for output in outputs:
                np.testing.assert_array_equal(output,expected)
                self.assertTrue(output.flags.owndata)
            outputs[0][:]=999
            np.testing.assert_array_equal(outputs[1],expected)
            for bad in [np.ones((0,52,190)),np.ones((1,52,950)),np.full((1,52,190),np.nan),
                        -np.ones((1,52,190)),np.ones((1,52,190),dtype=complex)]:
                with self.assertRaises(ValueError):bundle.predict(bad)
            np.testing.assert_array_equal(bundle.predict(x),expected)

    def test_loader_cast_overflow_corruption_and_no_training_imports(self):
        with tempfile.TemporaryDirectory() as tmp:
            _,_,path=make_models(Path(tmp))
            code="""import sys,numpy as np
from csi_training.espfi_motion_mixed import MixedPrecisionMotionBundle
b=MixedPrecisionMotionBundle(sys.argv[1]);assert b.predict(np.ones((1,52,190))).dtype==np.float32
assert not any(n=='torch' or n.startswith('torch.') or n=='scipy' or n.startswith('scipy.') for n in sys.modules)
assert 'csi_training.espfi_motion_compact' not in sys.modules
assert 'csi_training.espfi_motion' not in sys.modules
"""
            env=dict(os.environ,PYTHONPATH=str(Path(__file__).resolve().parents[1]))
            subprocess.run([sys.executable,'-c',code,str(path)],check=True,env=env,capture_output=True)
            model=path/'model.safetensors';original=model.read_bytes();model.write_bytes(original+b'bad')
            with self.assertRaisesRegex(ValueError,'hash mismatch'):MixedPrecisionMotionBundle(path)
            for overflow in (1e100,-1e100):
                state=load(original);state['weights'][0,0]=overflow;save_file(state,str(model))
                meta=json.loads((path/'metadata.json').read_text());meta['model_sha256']=hashlib.sha256(model.read_bytes()).hexdigest();write_json(path/'metadata.json',meta)
                # Float64 remains finite; only this optional float32 constructor rejects it.
                BufferedMotionBundle(path)
                with self.assertRaisesRegex(ValueError,'finite float32'):MixedPrecisionMotionBundle(path)

if __name__=='__main__':unittest.main()
