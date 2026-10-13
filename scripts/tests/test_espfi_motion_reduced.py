"""Synthetic reordered-reduction parity, boundary and inherited safety checks."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import sys
import tempfile
import unittest
import numpy as np
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from csi_training.espfi_motion_reduced import ReducedMotionBundle
from csi_training.espfi_motion_runtime import BufferedMotionBundle
from csi_training.espfi_motion import motion_features
from test_espfi_motion_runtime import make_models


class ReducedMotionTests(unittest.TestCase):
    def test_reordered_features_logits_and_rounding_boundaries(self):
        with tempfile.TemporaryDirectory() as tmp:
            parts,original,path=make_models(Path(tmp))
            reference=BufferedMotionBundle(path);candidate=ReducedMotionBundle(path)
            self.assertIs(ReducedMotionBundle.__init__,BufferedMotionBundle.__init__)
            self.assertIs(ReducedMotionBundle.predict,BufferedMotionBundle.predict)
            rng=np.random.default_rng(44)
            floor=np.zeros((2,52,190),dtype=np.float32)
            floor[...,::2]=np.nextafter(np.float32(1e-6),np.float32(np.inf))
            # Centered log RMS straddles both the 1e-6 scale floor and 190e-12 energy floor.
            wave=np.tile([-1.,1.],95)
            boundaries=[np.broadcast_to(np.exp(wave*a),(2,52,190)).copy()
                        for a in [np.nextafter(1e-6,0),1e-6,np.nextafter(1e-6,np.inf),.999e-6,1.001e-6]]
            sparse=np.ones((2,52,190),dtype=np.float64)
            sparse[:,0,:]=np.exp(wave*1e-5)
            sparse[:,1,:]=np.exp(wave*1e-9)
            # Constant carriers must not inflate the recording RMS via floored energy.
            inputs=[parts['validation']['x'],floor,sparse,np.zeros((2,52,190)),np.ones((2,52,190)),
                    np.full((2,52,190),1e100),1e50*np.exp(rng.normal(0,1e-9,(2,52,190))),
                    np.exp(rng.normal(size=(2,52,380)))[...,::2],parts['validation']['x'][...,::-1],
                    np.ones((1,52,190),dtype=np.int64)] + boundaries
            for x in inputs:
                features=motion_features(x,'motion_acf')
                for i,sample in enumerate(x):
                    np.testing.assert_allclose(candidate._extract(sample),features[i:i+1],atol=1e-12,rtol=0)
                actual=candidate.predict(x)
                for expected in [reference.predict(x),original.predict(x)]:
                    np.testing.assert_allclose(actual,expected,atol=1e-10,rtol=0)
                    np.testing.assert_array_equal(actual.argmax(1),expected.argmax(1))
                np.testing.assert_array_equal(actual,np.concatenate([candidate.predict(v[None]) for v in x]))
            self.assertEqual(candidate.workspace_bytes,reference.workspace_bytes)
            self.assertEqual(candidate.model_state_bytes,reference.model_state_bytes)

    def test_concurrency_owned_outputs_and_invalid_inputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            parts,_,path=make_models(Path(tmp));bundle=ReducedMotionBundle(path)
            x=parts['validation']['x'];expected=bundle.predict(x)
            with ThreadPoolExecutor(max_workers=4) as pool:
                results=list(pool.map(lambda _:bundle.predict(x),range(8)))
            for actual in results:
                np.testing.assert_array_equal(actual,expected)
                self.assertTrue(actual.flags.owndata)
            results[0][:]=999
            np.testing.assert_array_equal(results[1],expected)
            for bad in [np.ones((0,52,190)),np.ones((1,52,950)),np.full((1,52,190),np.inf),
                        -np.ones((1,52,190)),np.ones((1,52,190),dtype=complex)]:
                with self.assertRaises(ValueError):bundle.predict(bad)
            np.testing.assert_array_equal(bundle.predict(x),expected)

    def test_inherited_verified_artifact_loader(self):
        with tempfile.TemporaryDirectory() as tmp:
            _,_,path=make_models(Path(tmp))
            model=path/'model.safetensors';original=model.read_bytes()
            ReducedMotionBundle(path)
            self.assertEqual(model.read_bytes(),original)
            model.write_bytes(original+b'corruption')
            with self.assertRaisesRegex(ValueError,'hash mismatch'):ReducedMotionBundle(path)
            model.write_bytes(original)
            (path/'status.json').write_text('{"status":"failed","promotionAuthorized":false}')
            with self.assertRaisesRegex(ValueError,'Incomplete'):ReducedMotionBundle(path)

if __name__=='__main__':unittest.main()
