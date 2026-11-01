"""Synthetic receipts and packet bytes only; no network or radio access."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
import traffic_coverage as t
from test_prepare_bfi import capture,packet,AP,CLIENT


def inputs(records=None):
    raw,manifest=capture(records or [],45)
    manifest['requested_seconds']=45
    evidence={'schema_version':1,'capture_exit':0,'restoration_verified':True,
              'capture_seconds':45,'requested_seconds':30,'maximum_packets_per_second':250,
              'planned_datagrams':7500,'payload_bytes':1200,'destination_port':9,
              'profile':'udp-downlink-250pps-1200bytes-30s','sent_packets':7400,'sent_payload_bytes':8880000,
              'armed_at_unix_ns':1001*t.NS,'traffic_started_unix_ns':1002*t.NS,
              'traffic_ended_unix_ns':1032*t.NS,'traffic_elapsed_seconds':30.0,
              'actual_packets_per_second':7400/30}
    check={'source_verified_on_ethernet':True,'direct_route_verified':True,'expected_neighbor_verified':True}
    evidence.update(preflight=check,before_traffic=check)
    return raw,manifest,evidence


class TrafficCoverageTests(unittest.TestCase):
    def test_full_capture_and_actual_traffic_differ(self):
        raw,m,e=inputs([(2,packet(seq=1)),(3,packet(seq=2)),(31,packet(seq=3)),(32,packet(seq=4))])
        result=t.analyze(raw,m,e,AP,CLIENT)
        self.assertEqual(result['full_capture']['feedback']['trailing_no_report_seconds'],13)
        self.assertEqual(result['traffic_interval']['coverage']['trailing_no_report_seconds'],0)
        self.assertEqual(result['traffic_interval']['coverage']['maximum_no_report_gap_seconds'],28)
        self.assertEqual(result['after_traffic']['coverage']['maximum_no_report_gap_seconds'],13)
        self.assertEqual(result['full_capture']['feedback']['reports_per_second'],4/45)
        self.assertEqual(result['traffic_interval']['coverage']['reports_per_second'],4/30)

    def test_boundary_reports_count_once_and_retry_dedup_across_boundary(self):
        raw,m,e=inputs([(1,packet(seq=1)),(2,packet(seq=1,retry=True)),(2,packet(seq=2)),(32,packet(seq=3)),(33,packet(seq=4))])
        r=t.analyze(raw,m,e,AP,CLIENT)
        counts=[r[k]['coverage']['unique_reports'] for k in ('before_traffic','traffic_interval','after_traffic')]
        self.assertEqual(counts,[1,2,1])
        self.assertEqual(sum(counts),r['full_capture']['feedback']['unique_reports'])

    def test_zero_reports_retains_both_observation_windows(self):
        raw,m,e=inputs()
        r=t.analyze(raw,m,e,AP,CLIENT)
        self.assertEqual(r['full_capture']['feedback']['maximum_no_report_gap_seconds'],45)
        self.assertEqual(r['traffic_interval']['coverage']['maximum_no_report_gap_seconds'],30)

    def test_each_interval_declares_its_exact_bin_origin(self):
        raw,m,e=inputs([(3,packet())])
        r=t.analyze(raw,m,e,AP,CLIENT)
        for name in ('before_traffic','traffic_interval','after_traffic'):
            with self.subTest(name=name):
                interval=r[name]
                self.assertEqual(interval['coverage']['bin_origin'],'interval_start')
                self.assertEqual(interval['coverage']['bin_origin_unix_ns'],interval['start_unix_ns'])
        self.assertEqual(r['full_capture']['feedback']['bin_origin'],'manifest_capture_start')
        self.assertNotIn('bin_origin_unix_ns',r['full_capture']['feedback'])

    def test_invalid_or_short_intervals_fail(self):
        raw,m,e=inputs()
        for key,value in [('traffic_started_unix_ns',None),('traffic_started_unix_ns',1002.0*t.NS),
                          ('traffic_ended_unix_ns',1050*t.NS),('traffic_elapsed_seconds',.2),
                          ('traffic_elapsed_seconds',30.1),('traffic_ended_unix_ns',1003*t.NS),
                          ('armed_at_unix_ns',1003*t.NS)]:
            bad=dict(e);bad[key]=value
            with self.subTest(key=key,value=value),self.assertRaises(t.decoder.DecodeError):
                t.analyze(raw,m,bad,AP,CLIENT)

    def test_profile_payload_rate_and_link_evidence_fail_closed(self):
        raw,m,e=inputs()
        for key,value in [('sent_packets',0),('sent_payload_bytes',123),('planned_datagrams',8000),
                          ('maximum_packets_per_second',1001),('profile','guess'),('capture_exit',True),
                          ('payload_bytes',1199),('actual_packets_per_second',250),('before_traffic',{}),
                          ('restoration_verified',False),('error','interrupted')]:
            bad=dict(e);bad[key]=value
            with self.subTest(key=key),self.assertRaises(t.decoder.DecodeError): t.analyze(raw,m,bad,AP,CLIENT)

    def test_pcap_hash_mismatch_fails(self):
        raw,m,e=inputs();m['sha256']='0'*64
        with self.assertRaisesRegex(t.decoder.DecodeError,'hash_mismatch'):t.analyze(raw,m,e,AP,CLIENT)

    def test_json_duplicate_and_nonfinite_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            f=Path(td)/'in.json'
            for value in ['{"a":1,"a":2}','{"a":NaN}','[]']:
                f.write_text(value)
                with self.assertRaises(t.decoder.DecodeError): t.read_json(f)

    def test_cli_binds_input_hashes_and_redacts_addresses(self):
        raw,m,e=inputs([(3,packet())])
        with tempfile.TemporaryDirectory() as td:
            root=Path(td)
            (root/'c.pcap').write_bytes(raw);(root/'m.json').write_text(json.dumps(m));(root/'t.json').write_text(json.dumps(e))
            args=[str(root/'c.pcap'),'--manifest',str(root/'m.json'),'--traffic',str(root/'t.json'),
                  '--ap','02:00:00:00:00:01','--client','02:00:00:00:00:02','--output',str(root/'result.json')]
            self.assertEqual(t.main(args),0)
            result=(root/'result.json').read_text()
            self.assertNotIn('02:00:',result)
            parsed=json.loads(result)
            self.assertEqual(parsed['input_sha256']['pcap'],m['sha256'])
            self.assertEqual(t.main(args),1)

if __name__=='__main__':unittest.main()
