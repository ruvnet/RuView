#!/usr/bin/env python3
"""Compare full-capture BFI coverage with the sender's recorded traffic interval."""
import argparse
import collections
import hashlib
import io
import json
import math
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'bfi'))
import bfi_decode as decoder
import quality_report as quality

NS = 1000000000
MAX_JSON = 1024 * 1024
CLOCK_TOLERANCE_NS = 1000000
require = decoder.require


def number(value, low, high, reason, integer=False):
    require(type(value) in ((int,) if integer else (int,float)) and low <= value <= high
            and math.isfinite(value), reason)
    return value


def read_json(path):
    path = Path(path)
    require(path.is_file() and path.stat().st_size <= MAX_JSON, 'json_missing_or_too_large')
    with path.open('rb') as stream:
        raw = stream.read(MAX_JSON+1)
    require(len(raw) <= MAX_JSON, 'json_grew_beyond_limit')
    def pairs(items):
        result = {}
        for key,value in items:
            require(key not in result, 'duplicate_json_key')
            result[key] = value
        return result
    try:
        value = json.loads(raw, object_pairs_hook=pairs,
                           parse_constant=lambda _: require(False,'nonfinite_json_constant'))
    except (ValueError,UnicodeError,RecursionError) as error:
        if isinstance(error,decoder.DecodeError):
            raise
        raise decoder.DecodeError('invalid_json') from error
    require(isinstance(value,dict), 'json_object_required')
    return value, hashlib.sha256(raw).hexdigest()


def traffic_window(traffic, manifest):
    start,end,duration = quality.manifest_window(manifest)
    require(type(traffic.get('schema_version')) is int and traffic['schema_version']==1, 'unsupported_traffic_schema')
    require(traffic.get('capture_exit') == 0 and type(traffic.get('capture_exit')) is int
            and traffic.get('restoration_verified') is True
            and not any(traffic.get(k) for k in ('error','cleanup_verification_error','evidence_write_error')),
            'traffic_run_not_successful')
    for stage in ('preflight','before_traffic'):
        check = traffic.get(stage)
        require(isinstance(check,dict) and all(check.get(k) is True for k in
                ('source_verified_on_ethernet','direct_route_verified','expected_neighbor_verified')),
                'traffic_link_evidence_missing')
    capture_seconds = number(traffic.get('capture_seconds'),6,120,'invalid_capture_profile',True)
    requested = number(traffic.get('requested_seconds'),1,capture_seconds-5,'invalid_traffic_duration',True)
    pps = number(traffic.get('maximum_packets_per_second'),1,1000,'invalid_traffic_pps',True)
    require(manifest.get('requested_seconds') == capture_seconds, 'capture_profile_mismatch')
    planned = pps*requested
    require(planned <= 60000 and planned*1200 <= 64*1024*1024, 'traffic_payload_budget')
    require(traffic.get('planned_datagrams') == planned and type(traffic.get('planned_datagrams')) is int,
            'planned_datagrams_mismatch')
    require(type(traffic.get('payload_bytes')) is int and traffic['payload_bytes']==1200
            and type(traffic.get('destination_port')) is int and traffic['destination_port']==9,
            'unsupported_payload_or_port')
    require(traffic.get('profile') == f'udp-downlink-{pps}pps-1200bytes-{requested}s', 'traffic_profile_string_mismatch')
    sent = number(traffic.get('sent_packets'),1,planned,'invalid_sent_packets',True)
    require(type(traffic.get('sent_payload_bytes')) is int and traffic['sent_payload_bytes']==sent*1200,
            'sent_payload_mismatch')
    armed = number(traffic.get('armed_at_unix_ns'),1,(1<<63)-1,'invalid_armed_timestamp',True)
    traffic_start = number(traffic.get('traffic_started_unix_ns'),1,(1<<63)-1,'invalid_traffic_start',True)
    traffic_end = number(traffic.get('traffic_ended_unix_ns'),1,(1<<63)-1,'invalid_traffic_end',True)
    require(start <= armed <= traffic_start < traffic_end <= end, 'traffic_interval_outside_capture')
    elapsed = number(traffic.get('traffic_elapsed_seconds'),requested-.001,requested+2,
                     'short_or_overlong_traffic_interval')
    require(abs(traffic_end-traffic_start-round(elapsed*NS)) <= CLOCK_TOLERANCE_NS,
            'traffic_wall_monotonic_clock_mismatch')
    actual_pps = number(traffic.get('actual_packets_per_second'),0,1001,'invalid_actual_traffic_pps')
    require(math.isclose(actual_pps,sent/elapsed,rel_tol=1e-9,abs_tol=1e-9), 'actual_traffic_rate_mismatch')
    return traffic_start,traffic_end,{'requested_seconds':requested,'requested_pps':pps,'sent_packets':sent,
                                    'payload_bytes':1200,'sent_payload_bytes':sent*1200,
                                    'monotonic_elapsed_seconds':elapsed,'actual_sent_packets_per_second':actual_pps}


def report_timestamps(raw, ap, client):
    """Selection only after quality.analyze has validated every packet and timestamp."""
    seen, timestamps = collections.OrderedDict(), []
    for timestamp, packet, _ in decoder.pcap_packets(io.BytesIO(raw)):
        frame = decoder.strip_radiotap(packet)
        fc = int.from_bytes(frame[:2],'little')
        if (fc>>2)&3 or (fc>>4)&15 not in (13,14) or fc&0xc400:
            continue
        if quality.direction(frame[4:10],frame[10:16],frame[16:22],ap,client) != 'expected_client_to_ap' or frame[22]&15:
            continue
        body = frame[24:]
        if body[:2] not in (b'\x15\x00',b'\x1e\x00',b'\x24\x00'):
            continue
        try:
            decoder.decode_report(body)
        except decoder.DecodeError as error:
            if error.reason.startswith('unsupported_'):
                continue
            raise
        key = (frame[22:24],hashlib.sha256(body).digest())
        prior = seen.get(key)
        duplicate = bool(fc&0x0800 and prior is not None and timestamp-prior <= NS)
        seen[key] = timestamp
        seen.move_to_end(key)
        if len(seen)>4096:
            seen.popitem(last=False)
        if not duplicate:
            timestamps.append(timestamp)
    return timestamps


def analyze(raw, manifest, traffic, ap, client):
    traffic_start,traffic_end,profile = traffic_window(traffic,manifest)
    full = quality.analyze(io.BytesIO(raw),manifest,ap,client)
    timestamps = report_timestamps(raw,ap,client)
    require(len(timestamps)==full['feedback']['unique_reports'], 'selection_disagrees_with_quality_report')
    start,end,duration = quality.manifest_window(manifest)
    inside = [t for t in timestamps if traffic_start <= t <= traffic_end]
    before = [t for t in timestamps if t < traffic_start]
    after = [t for t in timestamps if t > traffic_end]
    def window(times,a,b):
        coverage = quality.coverage(times,a,b,(b-a)/NS) if b>a else None
        if coverage is not None:
            coverage['bin_origin'] = 'interval_start'
            coverage['bin_origin_unix_ns'] = a
        return {'start_unix_ns':a,'end_unix_ns':b,'duration_seconds':(b-a)/NS,
                'coverage':coverage}
    return {'schema_version':1,'status':'ANALYZED','full_capture':full,
            'traffic_interval':window(inside,traffic_start,traffic_end),
            'before_traffic':window(before,start,traffic_start),'after_traffic':window(after,traffic_end,end),
            'traffic_profile':profile,
            'endpoint_rule':'Traffic includes both endpoints; before is strictly earlier, after strictly later',
            'clock_tolerance_ns':CLOCK_TOLERANCE_NS,
            'receipt_association':'Caller-supplied traffic receipt validated for interval/profile; original traffic receipt does not contain PCAP hash',
            'limitations':['Traffic timestamps bound sender setup/send loop, not exact first/last over-air datagrams',
                           'UDP sends do not prove client delivery or trigger AP sounding',
                           'Coverage is captured report continuity, not RF packet loss or AP sounding rate',
                           'Hashing binds these input bytes to this analysis, not an authenticated common capture session',
                           'Traffic/full-capture results have different denominators; original full-capture claims are unchanged']}


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pcap',type=Path)
    for name in ('manifest','traffic','ap','client','output'):
        parser.add_argument('--'+name,required=True)
    args=parser.parse_args(argv)
    try:
        manifest,manifest_hash=read_json(args.manifest)
        traffic,traffic_hash=read_json(args.traffic)
        require(args.pcap.is_file() and args.pcap.stat().st_size <= decoder.MAX_FILE,'capture_missing_or_too_large')
        with args.pcap.open('rb') as stream:
            raw=stream.read(decoder.MAX_FILE+1)
        require(len(raw)<=decoder.MAX_FILE,'capture_grew_beyond_limit')
        result=analyze(raw,manifest,traffic,decoder.mac(args.ap),decoder.mac(args.client))
        result['input_sha256']={'pcap':hashlib.sha256(raw).hexdigest(),'manifest':manifest_hash,'traffic':traffic_hash,
                               'analyzer_source':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                               'decoder_source':hashlib.sha256(Path(decoder.__file__).read_bytes()).hexdigest(),
                               'quality_source':hashlib.sha256(Path(quality.__file__).read_bytes()).hexdigest()}
        output=Path(args.output).expanduser().resolve()
        require(not any((p/'.git').exists() for p in (output,*output.parents)),'output_must_be_outside_git')
        require(not output.exists(),'output_must_be_new')
        value=json.dumps(result,indent=2,allow_nan=False)+'\n'
        require(len(value.encode())<=8*1024*1024,'output_byte_limit')
        os.umask(0o077)
        with output.open('x',encoding='utf-8') as stream:
            stream.write(value)
        print(json.dumps({'status':'ANALYZED','full_reports':result['full_capture']['feedback']['unique_reports'],
                          'traffic_reports':result['traffic_interval']['coverage']['unique_reports']}))
        return 0
    except (decoder.DecodeError,OSError) as error:
        print(json.dumps({'status':'INPUT_ERROR','reason':str(error) if isinstance(error,decoder.DecodeError) else 'file_io_error'}))
        return 1


if __name__=='__main__':
    raise SystemExit(main())
