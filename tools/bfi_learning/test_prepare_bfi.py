"""Synthetic offline packets only; no capture or training."""
import datetime
import hashlib
import io
import json
import math
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import numpy as np
import prepare_bfi as p

AP,CLIENT=bytes.fromhex('020000000001'),bytes.fromhex('020000000002')


def body(width=20):
    control,tones={20:(0x04008009,64),40:(0x08808049,122),80:(0x12008089,250)}[width]
    length=(tones*6+7)//8
    return b'\x1e\x00'+control.to_bytes(5,'little')+bytes(2)+b'\x65\x06'+bytes(length-2)


def packet(width=20,seq=1,retry=False,content=None,other=False):
    header=struct.pack('<HH',0xe0|(0x800 if retry else 0),0)+AP+(bytes.fromhex('020000000003') if other else CLIENT)+AP+struct.pack('<H',seq<<4)
    return bytes.fromhex('0000080000000000')+header+(body(width) if content is None else content)


def capture(items,duration=10):
    raw=struct.pack('<IHHIIII',0xa1b2c3d4,2,4,0,0,65535,127)
    for offset,data in items:
        micro=round((1000+offset)*1000000); sec,frac=divmod(micro,1000000)
        raw+=struct.pack('<IIII',sec,frac,len(data),len(data))+data
    iso=lambda sec:datetime.datetime.fromtimestamp(sec,datetime.timezone.utc).isoformat()
    manifest={'schema':1,'capture_started_at':iso(1000),'capture_ended_at':iso(1000+duration),
              'capture_seconds':duration,'capture_exit':124,'sha256':hashlib.sha256(raw).hexdigest()}
    return raw,manifest


class BfiPrepareTests(unittest.TestCase):
    def test_golden_circular_angles_and_grid_partition(self):
        raw,manifest=capture([(1,packet(20)),(2,packet(80,2)),(3,packet(20,3))])
        groups,q=p.extract(raw,manifest,AP,CLIENT)
        self.assertEqual([len(g['rows'][0]) for g in groups],[192,750])
        np.testing.assert_allclose(groups[0]['rows'][0][:3],[math.cos(11*math.pi/16),math.sin(11*math.pi/16),5/8],atol=1e-12)
        self.assertEqual(groups[0]['intervals'],[(0,1),(1,2)])
        self.assertEqual(q['feedback']['unique_reports'],3)

    def test_retry_dedup_and_wrong_client(self):
        raw,m=capture([(1,packet()),(1.1,packet(retry=True)),(2,packet(seq=2,other=True))])
        groups,q=p.extract(raw,m,AP,CLIENT)
        self.assertEqual(len(groups[0]['rows']),1)
        self.assertEqual(q['expected_retry_duplicates'],1)

    def test_manifest_mismatch_and_unsupported(self):
        raw,m=capture([(1,packet(content=b'\x24\x00'))])
        groups,q=p.extract(raw,m,AP,CLIENT)
        self.assertEqual(groups,[])
        self.assertEqual(q['unsupported_actions']['unsupported_eht_feedback'],1)
        m['sha256']='0'*64
        with self.assertRaisesRegex(p.decoder.DecodeError,'hash_mismatch'): p.extract(raw,m,AP,CLIENT)

    def test_gap_census_full_observation(self):
        raw,m=capture([(i*.1,packet(seq=i)) for i in range(6)]+[(5,packet(seq=20))])
        groups,q=p.extract(raw,m,AP,CLIENT)
        self.assertEqual(groups[0]['window_census'],{'4':2,'8':0,'16':0,'32':0})
        self.assertEqual(groups[0]['intervals'],[(0,6),(6,7)])
        self.assertEqual(groups[0]['continuity']['trailing_no_report_seconds'],5)
        self.assertEqual(groups[0]['continuity']['reports_per_second'],.7)

    def test_equal_timestamp_rejected(self):
        raw,m=capture([(1,packet()),(1,packet(seq=2))])
        with self.assertRaisesRegex(p.decoder.DecodeError,'nonincreasing'): p.extract(raw,m,AP,CLIENT)

    def test_bounds_and_malformed_supported(self):
        raw,m=capture([(1,packet(content=body()[:-1]))])
        with self.assertRaisesRegex(p.decoder.DecodeError,'length_mismatch'): p.extract(raw,m,AP,CLIENT)
        raw,m=capture([(1,packet())])
        with patch.object(p,'MAX_ELEMENTS',191):
            with self.assertRaisesRegex(p.decoder.DecodeError,'allocation'): p.extract(raw,m,AP,CLIENT)

    def test_safe_numeric_output_and_no_identity_label(self):
        raw,m=capture([(i*.1,packet(seq=i)) for i in range(40)])
        with tempfile.TemporaryDirectory() as td:
            root=Path(td); (root/'input.pcap').write_bytes(raw); (root/'manifest.json').write_text(json.dumps(m))
            r=p.prepare(root/'input.pcap',root/'manifest.json',root/'out','02:00:00:00:00:01','02:00:00:00:00:02','synthetic')
            meta=json.loads((root/'out/dataset-000.json').read_text())
            self.assertEqual(meta['label_status'],'unlabeled')
            self.assertEqual(meta['provenance'],'synthetic')
            self.assertNotIn('identity',meta['sessions'][0])
            with np.load(root/'out/dataset-000.npz',allow_pickle=False) as loaded:
                self.assertEqual(loaded['frames'].shape,(40,192))
                self.assertEqual(loaded['frames'].dtype,np.float32)
                self.assertEqual(loaded['sequence'].dtype,np.uint32)
            self.assertEqual(meta['dataset_sha256'],p.sha((root/'out/dataset-000.npz').read_bytes()))
            self.assertEqual(r['quality']['feedback']['unique_reports'],40)

    def test_no_reports_writes_receipt_without_fake_dataset(self):
        raw,m=capture([])
        with tempfile.TemporaryDirectory() as td:
            root=Path(td); (root/'input.pcap').write_bytes(raw); (root/'manifest.json').write_text(json.dumps(m))
            r=p.prepare(root/'input.pcap',root/'manifest.json',root/'out','02:00:00:00:00:01','02:00:00:00:00:02','synthetic')
            self.assertEqual(r['datasets'],[])
            self.assertEqual(r['quality']['feedback']['maximum_no_report_gap_seconds'],10)

    def test_modern_unix_nanoseconds_remain_distinct_and_reconstruct(self):
        seconds=1800000000
        raw=struct.pack('<IHHIIII',0xa1b23c4d,2,4,0,0,65535,127)
        for fraction,seq in [(1,1),(2,2)]:
            data=packet(seq=seq)
            raw+=struct.pack('<IIII',seconds,fraction,len(data),len(data))+data
        iso=lambda value:datetime.datetime.fromtimestamp(value,datetime.timezone.utc).isoformat()
        manifest={'schema':1,'capture_started_at':iso(seconds),'capture_ended_at':iso(seconds+1),
                  'capture_seconds':1,'capture_exit':124,'sha256':hashlib.sha256(raw).hexdigest()}
        with tempfile.TemporaryDirectory() as td:
            root=Path(td); (root/'in.pcap').write_bytes(raw); (root/'m.json').write_text(json.dumps(manifest))
            receipt=p.prepare(root/'in.pcap',root/'m.json',root/'out','02:00:00:00:00:01','02:00:00:00:00:02','synthetic')
            meta=json.loads((root/'out/dataset-000.json').read_text())
            with np.load(root/'out/dataset-000.npz',allow_pickle=False) as data:
                times=data['timestamps']
                self.assertGreater(times[1],times[0])
                reconstructed=[meta['timestamp_origin_ns']+round((float(t)-meta['timestamp_offset_seconds'])*p.quality.NS) for t in times]
            self.assertEqual(reconstructed,[seconds*1000000000+1,seconds*1000000000+2])
            self.assertEqual(meta['timestamp_offset_seconds'],1.0)
            self.assertEqual(receipt['quality']['capture_duration_seconds'],1)

if __name__=='__main__': unittest.main()
