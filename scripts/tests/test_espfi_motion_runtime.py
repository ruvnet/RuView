"""Buffered inference parity, ownership and standalone dependency boundaries."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import numpy as np
from safetensors.numpy import save_file
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_motion_runtime import BufferedMotionBundle, SPEC, SHAPES, random_map, tensor_hash
from csi_training.espfi_motion_compact import CompactMotionBundle
from csi_training.espfi_motion import MotionBundle, motion_features
from csi_training.espfi_train import write_json
from test_espfi_context import make_baseline
from test_espfi_generalize import fixture


def make_models(root):
    parts,provenance=fixture()
    make_baseline(parts,provenance,root/'baseline')
    baseline=MotionBundle(root/'baseline')
    compact=root/'compact';compact.mkdir()
    save_file({k:baseline.state[k] for k in SHAPES},str(compact/'model.safetensors'))
    write_json(compact/'status.json',{'status':'complete','promotionAuthorized':False})
    write_json(compact/'metadata.json',{'status':'complete','promotionAuthorized':False,'spec':SPEC,
        'model_sha256':hashlib.sha256((compact/'model.safetensors').read_bytes()).hexdigest(),
        'random_map_sha256':{k:tensor_hash(v) for k,v in random_map().items()}})
    return parts,baseline,compact


class BufferedMotionTests(unittest.TestCase):
    def test_features_logits_floors_strides_and_batch(self):
        with tempfile.TemporaryDirectory() as tmp:
            parts,old,path=make_models(Path(tmp))
            bundle=BufferedMotionBundle(path);compact=CompactMotionBundle(path)
            rng=np.random.default_rng(44)
            near=np.full((2,52,190),np.float32(1e-6));near[...,::2]=np.nextafter(near[...,::2],np.float32(0))
            mixed=np.zeros((2,52,190),dtype=np.float32)
            mixed[...,::2]=np.nextafter(np.float32(1e-6),np.float32(np.inf))
            inputs=[parts['validation']['x'],near,mixed,np.zeros((2,52,190)),np.ones((2,52,190)),
                    np.full((2,52,190),1e100),1e50*np.exp(rng.normal(0,1e-9,(2,52,190))),
                    np.exp(rng.normal(size=(2,52,380)))[...,::2],parts['validation']['x'][...,::-1],
                    np.ones((1,52,190),dtype=np.int64)]
            for x in inputs:
                reference=motion_features(x,'motion_acf')
                for i,sample in enumerate(x):
                    actual_features=bundle._extract(sample).reshape(52,9)
                    expected_features=reference[i].reshape(52,9)
                    # Only the six ACF sums change reduction order with einsum.
                    np.testing.assert_allclose(actual_features[:,:6],expected_features[:,:6],atol=1e-12,rtol=0)
                    np.testing.assert_array_equal(actual_features[:,6:],expected_features[:,6:])
                actual=bundle.predict(x)
                np.testing.assert_allclose(actual,compact.predict(x),atol=1e-10,rtol=0)
                np.testing.assert_allclose(actual,old.predict(x),atol=1e-10,rtol=0)
                np.testing.assert_array_equal(actual.argmax(1),old.predict(x).argmax(1))
                np.testing.assert_array_equal(actual,np.concatenate([bundle.predict(v[None]) for v in x]))
            self.assertGreater(bundle.workspace_bytes,0)
            self.assertEqual(bundle.model_state_bytes,sum(v.nbytes for v in bundle.state.values()))
            size=bundle.workspace_bytes
            bundle.predict(parts['train']['x'])
            self.assertEqual(bundle.workspace_bytes,size)

    def test_concurrent_owned_outputs_and_invalid_inputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            parts,_,path=make_models(Path(tmp));bundle=BufferedMotionBundle(path)
            x=parts['validation']['x'];expected=bundle.predict(x)
            with ThreadPoolExecutor(max_workers=4) as pool:
                outputs=list(pool.map(lambda _:bundle.predict(x),range(8)))
            for result in outputs:
                np.testing.assert_array_equal(result,expected)
                self.assertTrue(result.flags.owndata)
            outputs[0][:]=999
            np.testing.assert_array_equal(outputs[1],expected)
            for invalid in (np.ones((1,52,950)),np.ones((0,52,190)),np.ones((4097,52,190),dtype=np.uint8),
                            np.ones((1,52,190),dtype=complex),np.ones((1,52,190),dtype=bool),
                            np.full((1,52,190),np.inf),-np.ones((1,52,190))):
                with self.assertRaises(ValueError):bundle.predict(invalid)
            np.testing.assert_array_equal(bundle.predict(x),expected)

    def test_artifact_bounds_corruption_and_standalone_process(self):
        with tempfile.TemporaryDirectory() as tmp:
            parts,_,path=make_models(Path(tmp))
            script="""import sys,numpy as np
from csi_training.espfi_motion_runtime import BufferedMotionBundle
b=BufferedMotionBundle(sys.argv[1]);assert b.predict(np.ones((1,52,190))).shape==(1,7)
assert not any(n=='torch' or n.startswith('torch.') or n=='scipy' or n.startswith('scipy.') for n in sys.modules)
assert 'csi_training.espfi_motion' not in sys.modules
assert 'csi_training.espfi_motion_compact' not in sys.modules
"""
            import os
            env=dict(os.environ,PYTHONPATH=str(Path(__file__).resolve().parents[1]))
            subprocess.run([sys.executable,'-c',script,str(path)],check=True,env=env,capture_output=True)
            model=path/'model.safetensors';original=model.read_bytes();model.write_bytes(original+b'bad')
            with self.assertRaisesRegex(ValueError,'hash mismatch'):BufferedMotionBundle(path)
            model.write_bytes(original)
            metadata=path/'metadata.json';original_meta=metadata.read_bytes();metadata.write_bytes(b'x'*65537)
            with self.assertRaisesRegex(ValueError,'Oversized'):BufferedMotionBundle(path)
            metadata.write_bytes(original_meta)
            state={k:np.ones(shape,dtype=np.float64) for k,shape in SHAPES.items()};state['feature_scale'][0]=0
            save_file(state,str(model));meta=json.loads(original_meta);meta['model_sha256']=hashlib.sha256(model.read_bytes()).hexdigest();write_json(metadata,meta)
            with self.assertRaisesRegex(ValueError,'Invalid compact'):BufferedMotionBundle(path)
            write_json(path/'status.json',{'status':'failed','promotionAuthorized':False})
            with self.assertRaisesRegex(ValueError,'Incomplete'):BufferedMotionBundle(path)

if __name__=='__main__':unittest.main()
