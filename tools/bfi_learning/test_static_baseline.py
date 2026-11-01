import unittest
from unittest.mock import patch
import numpy as np
import static_baseline as s


class StaticControlTests(unittest.TestCase):
    def fixture(self):
        sessions=[];frames=[]
        for split in ('train','validation','test'):
            for label in (0,1):
                for repeat in range(2):
                    start=len(frames)*40
                    frames.append(np.tile([label*4+repeat*.01,repeat*.02],(40,1)).astype('float32'))
                    sessions.append({'start':start,'end':start+40,'identity':str(label),'split':split})
        return {'frames':np.concatenate(frames),'metadata':{'sessions':sessions}}

    def test_static_identity_signal_and_no_test_metrics(self):
        d=self.fixture()
        with patch.object(s,'build_splits',return_value={'classes':['0','1']}):r=s.score_static(d)
        self.assertEqual(r['results']['nearest_centroid']['recording_accuracy'],1)
        self.assertEqual(r['results']['ridge_alpha_1']['recording_accuracy'],1)
        self.assertIsNone(r['test'])
        self.assertEqual(r['validation_recordings'],4)

    def test_heldout_values_do_not_change_results(self):
        d=self.fixture()
        with patch.object(s,'build_splits',return_value={'classes':['0','1']}):
            a=s.score_static(d)
            for record in d['metadata']['sessions']:
                if record['split']=='test':d['frames'][record['start']:record['end']]=999999
            b=s.score_static(d)
        self.assertEqual(a,b)


if __name__=='__main__':unittest.main()
