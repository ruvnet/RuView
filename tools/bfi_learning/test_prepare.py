import io
import struct
import tempfile
import unittest
import zlib
from pathlib import Path
from unittest.mock import patch
import numpy as np
from scipy.io import savemat
import prepare as p


def packet(seq=1, tick=1000, synthetic=False, tones=2):
    b = bytearray(49 + tones * 2)
    b[:5] = b'RAC1\x01'
    struct.pack_into('<HI', b, 5, 49, len(b) + 4)
    b[11:13] = bytes([1, 0])
    struct.pack_into('<II', b, 13, seq, tick)
    b[21:27] = bytes.fromhex('020000000001')
    b[27:33] = bytes.fromhex('020000000002')
    b[33], b[39], b[40], b[41], b[43], b[44] = 6, 16, 1, 200, 1, int(synthetic)
    struct.pack_into('<H', b, 37, tones)
    struct.pack_into('<I', b, 45, tones * 2)
    return bytes(b) + struct.pack('<I', zlib.crc32(b))


def records(packets):
    raw = b''.join(struct.pack('<QI', 1700000000000000000+i*1000, len(b))+b for i,b in enumerate(packets))
    return p.read_records(io.BytesIO(raw))


class PrepareTests(unittest.TestCase):
    def test_crc_and_outer_truncation(self):
        b = bytearray(packet()); b[-1] ^= 1
        with self.assertRaisesRegex(p.ImportError, 'crc'): p.decode_rac1(b)
        with self.assertRaisesRegex(p.ImportError, 'header_truncated'): p.read_records(io.BytesIO(b'abc'))

    def test_reorder_dedup_wrap_and_honest_loss(self):
        r,_ = records([packet(0,2000), packet(0xffffffff,1000), packet(0,2000), packet(2,4000)])
        ordered,m = p.order_link(r)
        self.assertEqual([x['sequence'] for x in ordered], [0xffffffff,0,2])
        self.assertEqual(m['exact_duplicates_removed'],1)
        self.assertEqual(m['missing_sequence_values_within_observed_link_span'],1)
        self.assertTrue(all(a['timestamp_ns'] < b['timestamp_ns'] for a,b in zip(ordered,ordered[1:])))

    def test_synthetic_cannot_be_real(self):
        r,_ = records([packet(synthetic=True)])
        with self.assertRaisesRegex(p.ImportError, 'synthetic_flag'):
            p.prepare_records(r,r[0]['peer'],r[0]['trigger'],None,'real')

    def test_equal_hardware_ticks_rejected(self):
        r,_ = records([packet(1,1000),packet(2,1000)])
        with self.assertRaisesRegex(p.ImportError, 'timestamp'): p.order_link(r)

    def test_host_gap_rejects_hidden_full_hardware_wrap(self):
        r,_ = records([packet(1,1000),packet(2,2000)])
        r[1]['host_ns']=r[0]['host_ns']+((1<<32)+1000)*1000
        with self.assertRaisesRegex(p.ImportError,'ambiguous_wrap'): p.order_link(r)

    def test_short_hardware_wrap_is_valid(self):
        r,_ = records([packet(1,(1<<32)-1000),packet(2,1000)])
        r[1]['host_ns']=r[0]['host_ns']+2000000
        ordered,_=p.order_link(r)
        self.assertEqual(ordered[1]['timestamp_ns']-ordered[0]['timestamp_ns'],2000000)

    def test_trainer_feature_budget_rejected(self):
        r,_ = records([packet(tones=513)])
        with self.assertRaisesRegex(p.ImportError,'trainer_feature_limit'):
            p.prepare_records(r,r[0]['peer'],r[0]['trigger'],None,'real')

    def test_format_separation_and_unlabeled_provenance(self):
        r,s = records([packet(1,1000),packet(2,2000,tones=3)])
        data,filters = p.prepare_records(r,r[0]['peer'],r[0]['trigger'],None,'real')
        self.assertEqual([d['frames'].shape for d in data],[(1,4),(1,6)])
        with tempfile.TemporaryDirectory() as td:
            result=p.write_datasets(Path(td)/'out',data,s,'real',filters)
            import json
            meta=json.loads((Path(td)/'out/dataset-000.json').read_text())
            self.assertEqual(meta['label_status'],'unlabeled')
            self.assertNotIn('identity',meta['sessions'][0])
            self.assertEqual(meta['dataset_sha256'],p.sha_file(Path(td)/'out/dataset-000.npz'))
            with np.load(Path(td)/'out/dataset-000.npz',allow_pickle=False) as loaded:
                self.assertEqual(loaded['frames'].dtype,np.float32)

    def test_public_author_transform(self):
        with tempfile.TemporaryDirectory() as td:
            f=Path(td)/'x.mat'; a=np.arange(342*2000,dtype=np.float64).reshape(342,2000)
            savemat(f,{'CSIamp':a})
            out=p.load_public_mat(f)
            np.testing.assert_array_equal(out,a[:,::4].T.astype(np.float32))
            self.assertEqual(out.shape,(500,342))

    def test_public_rejects_object_shape_nonfinite(self):
        with tempfile.TemporaryDirectory() as td:
            f=Path(td)/'x.mat'
            for a in (np.ones((1,1)),np.array([['text']],dtype=object),np.full((342,2000),np.nan)):
                savemat(f,{'CSIamp':a})
                with self.assertRaises(p.ImportError): p.load_public_mat(f)

    def test_public_metadata_declares_custom_partition(self):
        items=[dict(path=Path('unused'),identity='person1',source_split=s,source_folder=s+'_amp',split=t,sha256=str(i)*64,size_bytes=1024)
               for i,(s,t) in enumerate([('train','train'),('train','validation'),('test','test')],1)]
        with tempfile.TemporaryDirectory() as td, patch.object(p,'public_inventory',return_value=items), patch.object(p,'load_public_mat',side_effect=[np.full((500,342),i,np.float32) for i in range(3)]), patch.object(p,'sha_file',side_effect=[i['sha256'] for i in items]+['a'*64]):
            p.prepare_public('unused',Path(td)/'out')
            import json
            meta=json.loads((Path(td)/'out/dataset-000.json').read_text())
            self.assertEqual(meta['time_unit'],'sample_index')
            self.assertEqual(meta['acquisition_session_disjointness'],'unknown')
            self.assertEqual(meta['sessions'][-1]['source_split'],'test')
            self.assertEqual(meta['sessions'][-1]['split'],'test')
            self.assertEqual(meta['label_origin'],'published_dataset')
            self.assertEqual(meta['split_protocol'],'archive_directory_custom')
            self.assertEqual(meta['author_training_folder'],'test_amp')
            self.assertEqual(meta['author_test_folder'],'train_amp')
            self.assertEqual(meta['source_folder_mapping'],{'train':'train_amp','test':'test_amp'})
            self.assertEqual(meta['sessions'][-1]['source_folder'],'test_amp')

    def test_distinct_mat_files_with_same_float32_features_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td); items=[]
            for index,value in enumerate([1.0,1.0+1e-10]):
                f=root/f'{index}.mat'
                savemat(f,{'CSIamp':np.full((342,2000),value,dtype=np.float64)})
                items.append(dict(path=f,identity='001',source_split='train' if index==0 else 'test',
                                  source_folder='train_amp' if index==0 else 'test_amp',split='train' if index==0 else 'test',
                                  sha256=p.sha_file(f),size_bytes=f.stat().st_size))
            self.assertNotEqual(items[0]['sha256'],items[1]['sha256'])
            with patch.object(p,'public_inventory',return_value=items):
                with self.assertRaisesRegex(p.ImportError,'duplicate_processed_recording'):
                    p.prepare_public(root,root/'out')

    def test_inventory_split_and_duplicate_content_guard(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)
            for split,count in [('train_amp',21),('test_amp',39)]:
                for person in range(14):
                    folder=root/split/f'person{person:02}'
                    folder.mkdir(parents=True)
                    for index in range(count):
                        (folder/f'{index:02}.mat').write_bytes(bytes(128))
            import hashlib
            with patch.object(p,'sha_file',side_effect=lambda f:hashlib.sha256(str(f).encode()).hexdigest()):
                items=p.public_inventory(root)
            self.assertEqual(sum(i['split']=='test' for i in items),546)
            self.assertEqual(sum(i['split']=='validation' for i in items),56)
            self.assertTrue(all(i['source_split']=='train' for i in items if i['split']=='validation'))
            with patch.object(p,'sha_file',return_value='a'*64):
                with self.assertRaisesRegex(p.ImportError,'duplicate_recording_hash'): p.public_inventory(root)

if __name__ == '__main__': unittest.main()
