#!/usr/bin/env python3
"""Offline unlabeled BFI features and continuity; no capture, learning or identity labels."""
import argparse
import collections
import hashlib
import io
import json
import math
import os
from pathlib import Path
import sys

import numpy as np

BFI_TOOLS = Path(__file__).resolve().parents[1] / 'bfi'
sys.path.insert(0, str(BFI_TOOLS))
import bfi_decode as decoder
import quality_report as quality

MAX_ELEMENTS = 16 * 1024 * 1024
MAX_GROUPS = 64
MAX_REPORTS = 8192
FORMAT_KEYS = ('standard', 'nr', 'nc', 'bandwidth_mhz', 'grouping', 'codebook', 'tone_count')
require = decoder.require


def sha(data):
    return hashlib.sha256(data).hexdigest()


def extract(raw, manifest, ap, client, gap_seconds=1.0):
    require(math.isfinite(gap_seconds) and 0 < gap_seconds <= 60, 'invalid_gap_threshold')
    # Authoritative frame/control/FCS/window validation happens first. This second
    # pass only selects already validated reports and calls the same strict decoder.
    aggregate = quality.analyze(io.BytesIO(raw), manifest, ap, client)
    start, end, duration = quality.manifest_window(manifest)
    seen, groups = collections.OrderedDict(), {}
    elements = unique = 0
    previous_retained = None
    for timestamp, packet, _ in decoder.pcap_packets(io.BytesIO(raw)):
        frame = decoder.strip_radiotap(packet)
        fc = int.from_bytes(frame[:2], 'little')
        if (fc >> 2) & 3 or (fc >> 4) & 15 not in (13, 14):
            continue
        if quality.direction(frame[4:10], frame[10:16], frame[16:22], ap, client) != 'expected_client_to_ap':
            continue
        if fc & 0xc400 or frame[22] & 15:
            continue
        body = frame[24:]
        if body[:2] not in (b'\x15\x00', b'\x1e\x00', b'\x24\x00'):
            continue
        try:
            report = decoder.decode_report(body)
        except decoder.DecodeError as error:
            if error.reason.startswith('unsupported_'):
                continue
            raise
        retry_key = (frame[22:24], hashlib.sha256(body).digest())
        prior = seen.get(retry_key)
        duplicate = bool(fc & 0x0800 and prior is not None and timestamp-prior <= quality.NS)
        seen[retry_key] = timestamp
        seen.move_to_end(retry_key)
        if len(seen) > 4096:
            seen.popitem(last=False)
        if duplicate:
            continue
        require(previous_retained is None or timestamp > previous_retained, 'nonincreasing_feature_timestamp')
        previous_retained = timestamp
        require(unique < MAX_REPORTS, 'feature_report_limit')
        key = tuple(report[k] for k in FORMAT_KEYS)
        require(key in groups or len(groups) < MAX_GROUPS, 'feature_group_limit')
        features = 3 * report['tone_count']
        elements += features
        require(features <= 1024 and elements <= MAX_ELEMENTS, 'feature_allocation_limit')
        group = groups.setdefault(key, {'format': dict(zip(FORMAT_KEYS,key)), 'rows': [], 'times': [],
                                       'sequences': [], 'segments': [], 'last_ordinal': None})
        if group['times']:
            require(timestamp > group['times'][-1], 'nonincreasing_feature_timestamp')
        boundary = (not group['times'] or timestamp-group['times'][-1] > gap_seconds*quality.NS
                    or group['last_ordinal'] != unique-1)
        if boundary:
            group['segments'].append(len(group['rows']))
        row = [value for _, _, phi, psi in report['angles']
               for value in (math.cos(phi), math.sin(phi), 2*psi/math.pi)]
        group['rows'].append(row)
        group['times'].append(timestamp)
        group['sequences'].append(int.from_bytes(frame[22:24],'little') >> 4)
        group['last_ordinal'] = unique
        unique += 1
    require(unique == aggregate['feedback']['unique_reports'], 'selection_disagrees_with_quality_report')
    for group in groups.values():
        ends = group['segments'][1:] + [len(group['rows'])]
        intervals = list(zip(group['segments'], ends))
        group['window_census'] = {str(w): sum(max(0,b-a-w) for a,b in intervals) for w in (4,8,16,32)}
        group['intervals'] = intervals
        group['continuity'] = quality.coverage(group['times'], start, end, duration)
    return list(groups.values()), aggregate


def prepare(pcap, manifest_path, output, ap, client, provenance, gap_seconds=1.0):
    require(provenance in ('real','synthetic'), 'explicit_provenance_required')
    pcap, manifest_path, output = Path(pcap), Path(manifest_path), Path(output).expanduser().resolve()
    require(pcap.is_file() and pcap.stat().st_size <= decoder.MAX_FILE, 'capture_missing_or_too_large')
    with pcap.open('rb') as stream:
        raw = stream.read(decoder.MAX_FILE+1)
    require(len(raw) <= decoder.MAX_FILE, 'capture_grew_beyond_limit')
    manifest = quality.read_manifest(manifest_path)
    groups, aggregate = extract(raw, manifest, decoder.mac(ap), decoder.mac(client), gap_seconds)
    timestamp_origin_ns, _, _ = quality.manifest_window(manifest)
    require(not any((p/'.git').exists() for p in (output,*output.parents)), 'output_must_be_outside_git')
    require(not output.exists(), 'output_must_be_new')
    os.umask(0o077)
    output.mkdir(mode=0o700, parents=False)
    source = {'sha256':sha(raw), 'size_bytes':len(raw)}
    receipt = {'schema_version':1,'status':'PREPARED' if groups else 'NO_SUPPORTED_FEATURES',
               'provenance':provenance,'label_status':'unlabeled','source':source,'quality':aggregate,
               'gap_threshold_seconds':gap_seconds, 'datasets':[],
               'kernel_drops':None, 'kernel_drop_basis':'not provided by capture manifest; no zero-drop assumption',
               'window_census_basis':'stride1 next-observation windows before chronological purge/split; not training eligibility',
               'limitations':['Provenance is an operator declaration; PCAP is not authenticated',
                              'BFI is quantized steering information, not full channel CSI',
                              'Observation gaps and management sequence jumps do not measure RF loss',
                              'No identity labels, movement inference or training are produced']}
    for number, group in enumerate(groups):
        stem=f'dataset-{number:03}'
        frames=np.asarray(group['rows'],dtype=np.float32)
        # Subtract as integers before float conversion: Unix epochs near 1.8e9
        # seconds cannot represent adjacent nanoseconds in float64.
        timestamps=np.asarray([t-timestamp_origin_ns for t in group['times']],dtype=np.float64)/quality.NS+1.0
        sequence=np.asarray(group['sequences'],dtype=np.uint32)
        require(np.isfinite(frames).all() and np.isfinite(timestamps).all()
                and np.all(timestamps>0) and np.all(np.diff(timestamps)>0), 'invalid_output_numeric_values')
        npz=output/(stem+'.npz')
        with npz.open('xb') as handle:
            np.savez(handle,frames=frames,timestamps=timestamps,sequence=sequence)
        metadata={'schema_version':1,'source_kind':'bfi','provenance':provenance,'label_status':'unlabeled',
                  'source_files':[source], 'dataset_sha256':sha(npz.read_bytes()),
                  'frame_count':len(frames),'feature_count':frames.shape[1], 'time_unit':'seconds',
                  'timestamp_basis':'seconds_relative_to_integer_capture_origin_plus_offset',
                  'timestamp_origin_ns':timestamp_origin_ns, 'timestamp_offset_seconds':1.0,
                  'sequence_basis':'observed_12bit_management_sequence_not_BFI_counter',
                  'feature_encoding':'per_tone_cos_phi_sin_phi_2psi_over_pi', 'format':group['format'],
                  'decoder_sha256':sha(Path(decoder.__file__).read_bytes()),
                  'quality_source_sha256':sha(Path(quality.__file__).read_bytes()),
                  'importer_sha256':sha(Path(__file__).read_bytes()),
                  'capture_manifest_sha256':sha(json.dumps(manifest,sort_keys=True,allow_nan=False).encode()),
                  'capture_manifest_hash_basis':'canonical_parsed_json',
                  'sessions':[{'session_id':f"{source['sha256'][:16]}-g{number}-s{i}",'start':a,'end':b}
                              for i,(a,b) in enumerate(group['intervals'])],
                  'session_basis':'contiguous runs within one capture and one format; gap or intervening format splits runs',
                  'gap_threshold_seconds':gap_seconds,'window_census':group['window_census'],
                  'continuity':group['continuity'],'limitations':receipt['limitations']}
        metadata_path=output/(stem+'.json')
        serialized=json.dumps(metadata,indent=2,allow_nan=False)+'\n'
        require(len(serialized.encode('utf-8')) <= 1024*1024, 'metadata_byte_limit')
        metadata_path.write_text(serialized,encoding='utf-8')
        receipt['datasets'].append({'data':npz.name,'metadata':metadata_path.name,
                                    'data_sha256':metadata['dataset_sha256'],'metadata_sha256':sha(metadata_path.read_bytes()),
                                    'frames':len(frames),'features':frames.shape[1],
                                    'window_census':group['window_census']})
    (output/'receipt.json').write_text(json.dumps(receipt,indent=2,allow_nan=False)+'\n',encoding='utf-8')
    return receipt


def main(argv=None):
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pcap',type=Path)
    for name in ('manifest','output','ap','client'):
        parser.add_argument('--'+name,required=True)
    parser.add_argument('--provenance',choices=('real','synthetic'),required=True)
    parser.add_argument('--gap-seconds',type=float,default=1.0)
    args=parser.parse_args(argv)
    try:
        result=prepare(args.pcap,args.manifest,args.output,args.ap,args.client,args.provenance,args.gap_seconds)
        print(json.dumps({'status':result['status'],'datasets':result['datasets']}))
        return 0
    except (decoder.DecodeError,OSError) as error:
        print(json.dumps({'status':'INPUT_ERROR','reason':str(error) if isinstance(error,decoder.DecodeError) else 'file_io_error'}))
        return 1


if __name__=='__main__':
    raise SystemExit(main())
