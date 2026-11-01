#!/usr/bin/env python3
"""Import private framed RAC1 captures into numeric, unlabeled training data.

Offline only. Record framing is <QI host_timestamp_ns, datagram_length>.
RAC1 v1 layout follows ADR-323 and realtek_csi.rs; CRC is integrity, not
authentication. Different links/formats never share a feature array.
"""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import sys
import zlib

import numpy as np

MAX_FILE = 128 * 1024 * 1024
MAX_RECORDS = 200000
MAX_GROUPS = 128
MAX_ELEMENTS = 32 * 1024 * 1024
MAX_SPAN_US = 86400 * 1000000
HOST_CLOCK_TOLERANCE_US = 5 * 1000000
MASK32 = (1 << 32) - 1


class ImportError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ImportError(message)


def mac(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-fA-F]{2}(?::[0-9a-fA-F]{2}){5}", value),
            "invalid_source_mac")
    result = bytes.fromhex(value.replace(":", ""))
    require(result != bytes(6) and not result[0] & 1, "source_mac_must_be_unicast")
    return result


def decode_rac1(data):
    require(53 <= len(data) <= 65507, "invalid_datagram_length")
    u16 = lambda offset: struct.unpack_from("<H", data, offset)[0]
    u32 = lambda offset: struct.unpack_from("<I", data, offset)[0]
    require(data[:4] == b"RAC1" and data[4] == 1 and u16(5) == 49, "unsupported_rac1_header")
    require(u32(7) == len(data), "rac1_frame_length_mismatch")
    require(zlib.crc32(data[:-4]) == u32(len(data) - 4), "rac1_crc_mismatch")
    tones, bits, payload_length = u16(37), data[39], u32(45)
    require(1 <= tones <= 4096 and bits in (16, 32), "invalid_tone_format")
    require(payload_length == tones * (bits // 8) and len(data) == 53 + payload_length,
            "rac1_payload_length_mismatch")
    require(data[12] in (0, 1, 2) and data[34] in (0, 1) and data[36] in (0, 1, 2, 3)
            and data[40] in (1, 2, 4, 8), "unsupported_rac1_metadata")
    require(1 <= data[33] <= 196, "invalid_channel")
    require(data[43] in (0, 1) and data[44] & ~1 == 0, "invalid_validity_or_flags")
    require(struct.unpack_from("b", data, 41)[0] <= 0, "invalid_rssi")
    return {"sequence": u32(13), "hardware_us": u32(17), "node_id": data[11],
            "peer": data[21:27], "trigger": data[27:33], "valid": bool(data[43]),
            "synthetic": bool(data[44] & 1), "wire": data,
            "format": {"csi_mode": data[12], "channel": data[33], "bandwidth_mhz": (20, 40)[data[34]],
                       "rx_rate_code": data[35], "protocol_mode": data[36], "tone_count": tones,
                       "bits_per_tone": bits, "decimation": data[40], "rxsc": data[42]}}


def read_records(stream):
    digest = hashlib.sha256()
    records = []
    size = 0
    while header := stream.read(12):
        require(len(header) == 12, "record_header_truncated")
        host_ns, length = struct.unpack("<QI", header)
        require(0 < host_ns < 1 << 63, "invalid_host_timestamp")
        require(53 <= length <= 65507, "invalid_record_length")
        size += 12 + length
        require(size <= MAX_FILE and len(records) < MAX_RECORDS, "capture_budget_exceeded")
        data = stream.read(length)
        require(len(data) == length, "record_payload_truncated")
        record = decode_rac1(data)
        record["host_ns"] = host_ns
        records.append(record)
        digest.update(header)
        digest.update(data)
    require(records, "empty_capture")
    require(max(r["host_ns"] for r in records) - min(r["host_ns"] for r in records)
            <= MAX_SPAN_US * 1000, "host_capture_span_exceeded")
    return records, {"sha256": digest.hexdigest(), "size_bytes": size}


def order_link(records):
    """Unwrap one bounded sequence span and reject resets/conflicting reuse."""
    anchor = records[0]["sequence"]
    unique = {}
    duplicates = reorders = 0
    previous = None
    for record in records:
        value = (record["sequence"] - anchor) & MASK32
        relative = value if value < 1 << 31 else value - (1 << 32)
        if previous is not None and relative < previous:
            reorders += 1
        previous = relative
        if relative in unique:
            require(unique[relative]["wire"] == record["wire"], "conflicting_sequence_reuse")
            duplicates += 1
            continue
        unique[relative] = record
    indices = sorted(unique)
    require(indices[-1] - indices[0] < 1 << 31, "ambiguous_sequence_span")
    ordered = [unique[index] for index in indices]
    elapsed_us = 0
    for index, record in enumerate(ordered):
        if index:
            delta = (record["hardware_us"] - ordered[index - 1]["hardware_us"]) & MASK32
            require(0 < delta < 1 << 31, "hardware_timestamp_duplicate_regression_or_reset")
            host_delta_us = (record["host_ns"] - ordered[index - 1]["host_ns"]) / 1000
            require(abs(host_delta_us - delta) <= HOST_CLOCK_TOLERANCE_US,
                    "hardware_host_gap_disagreement_or_ambiguous_wrap")
            elapsed_us += delta
        require(elapsed_us <= MAX_SPAN_US, "hardware_capture_span_exceeded")
        record["timestamp_ns"] = ordered[0]["host_ns"] + elapsed_us * 1000
    metrics = {"received_records": len(records), "unique_sequences": len(ordered),
               "hardware_host_gap_tolerance_us": HOST_CLOCK_TOLERANCE_US,
               "exact_duplicates_removed": duplicates, "arrival_sequence_reversals": reorders,
               "sequence_span": indices[-1] - indices[0] + 1,
               "missing_sequence_values_within_observed_link_span": indices[-1] - indices[0] + 1 - len(ordered),
               "host_arrival_timestamp_regressions": sum(b["host_ns"] < a["host_ns"]
                                                         for a, b in zip(records, records[1:])),
               "loss_interpretation": "Unobserved sequence values do not identify the loss layer; no edge loss estimate"}
    return ordered, metrics


def prepare_records(records, peer, trigger, node_id, provenance):
    require(provenance in ("real", "synthetic"), "explicit_provenance_required")
    selected = [r for r in records if r["peer"] == peer and r["trigger"] == trigger
                and (node_id is None or r["node_id"] == node_id)]
    require(selected, "source_filter_matched_no_records")
    if provenance == "real":
        require(not any(r["synthetic"] for r in selected), "synthetic_flag_cannot_be_promoted_to_real")
    links = collections.defaultdict(list)
    for record in selected:
        links[record["node_id"]].append(record)
    datasets = []
    total_elements = 0
    for link_index, (_, link_records) in enumerate(sorted(links.items())):
        ordered, metrics = order_link(link_records)
        groups = collections.defaultdict(list)
        for record in ordered:
            if record["valid"]:
                # The source filter fixes MACs; node IDs still identify distinct
                # receivers, never people. Keep each format/grid separate.
                groups[tuple(record["format"].items())].append(record)
        for group_records in groups.values():
            require(len(datasets) < MAX_GROUPS, "format_group_limit_exceeded")
            fmt = group_records[0]["format"]
            require(fmt["tone_count"] <= 512, "tone_count_exceeds_trainer_feature_limit")
            count, features = len(group_records), fmt["tone_count"] * 2
            total_elements += count * features
            require(total_elements <= MAX_ELEMENTS, "feature_element_budget_exceeded")
            dtype, scale = ("i1", 128) if fmt["bits_per_tone"] == 16 else ("<i2", 32768)
            frames = np.stack([np.frombuffer(r["wire"][49:-4], dtype=dtype).astype(np.float32) / scale
                               for r in group_records]).astype(np.float32, copy=False)
            timestamps = np.array([r["timestamp_ns"] / 1e9 for r in group_records], dtype=np.float64)
            sequences = np.array([r["sequence"] for r in group_records], dtype=np.uint32)
            require(np.isfinite(frames).all() and np.isfinite(timestamps).all()
                    and np.all(np.diff(timestamps) > 0), "invalid_numeric_features_or_timestamp_precision")
            datasets.append({"frames": frames, "timestamps": timestamps, "sequence": sequences,
                             "format": fmt, "link_id": f"link-{link_index + 1}",
                             "metrics": {**metrics, "invalid_csi_records_excluded": sum(not r["valid"] for r in ordered),
                                         "records_in_this_format": count,
                                         "other_format_records": sum(r["valid"] for r in ordered) - count}})
    require(datasets, "no_valid_csi_records")
    return datasets, {"source_records": len(records), "selected_records": len(selected),
                      "filtered_other_source_records": len(records) - len(selected)}


def write_datasets(output, datasets, source, provenance, filters):
    output = Path(output).expanduser().resolve()
    require(not any((parent / ".git").exists() for parent in (output, *output.parents)),
            "private_dataset_must_be_outside_git")
    require(not output.exists(), "output_must_be_new")
    os.umask(0o077)
    output.mkdir(mode=0o700, parents=False)
    index = {"schema_version": 1, "provenance": provenance, "label_status": "unlabeled", "datasets": []}
    for number, data in enumerate(datasets):
        stem = f"dataset-{number:03d}"
        npz_path = output / (stem + ".npz")
        with npz_path.open("xb") as stream:
            np.savez(stream, frames=data["frames"], timestamps=data["timestamps"], sequence=data["sequence"])
        metadata = {"schema_version": 1, "provenance": provenance, "source_kind": "rac1",
                    "label_status": "unlabeled", "source_files": [source],
                    "dataset_sha256": hashlib.sha256(npz_path.read_bytes()).hexdigest(),
                    "frame_count": len(data["frames"]), "feature_count": data["frames"].shape[1],
                    "feature_encoding": "interleaved_iq_signed_unit_scale",
                    "timestamp_basis": "sequence_ordered_unwrapped_hardware_us_anchored_to_first_receive_utc",
                    "format": data["format"], "link_id": data["link_id"],
                    "source_filter": {"explicit_peer_and_trigger": True, **filters},
                    "quality": data["metrics"],
                    "sessions": [{"session_id": source["sha256"][:24], "start": 0, "end": len(data["frames"])}],
                    "limitations": ["Real provenance is the operator's declaration, not authenticated by CRC",
                                    "Hardware time is anchored to host arrival; this is not cross-device clock synchronization",
                                    "One capture is one session; no identity labels or movement ground truth were supplied",
                                    "Sequence gaps cannot distinguish sender, network or recorder loss",
                                    "Format groups must not be concatenated without an explicit feature alignment"]}
        json_path = output / (stem + ".json")
        json_path.write_text(json.dumps(metadata, indent=2, allow_nan=False) + "\n", encoding="utf-8")
        index["datasets"].append({"data": npz_path.name, "metadata": json_path.name,
                                  "data_sha256": metadata["dataset_sha256"],
                                  "metadata_sha256": hashlib.sha256(json_path.read_bytes()).hexdigest(),
                                  "frames": metadata["frame_count"], "features": metadata["feature_count"]})
    (output / "index.json").write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
    return index


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path, nargs="?")
    parser.add_argument("--ntu-humanid", type=Path, help="Extracted author dataset containing train_amp and test_amp")
    parser.add_argument("--peer-mac")
    parser.add_argument("--trigger-mac")
    parser.add_argument("--node-id", type=int)
    parser.add_argument("--provenance", choices=("real", "synthetic"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        if args.ntu_humanid:
            require(args.capture is None and args.peer_mac is None and args.trigger_mac is None
                    and args.node_id is None and args.provenance is None, "public_and_private_options_are_exclusive")
            result = prepare_public(args.ntu_humanid, args.output)
            print(json.dumps({"status": "PREPARED", "label_status": "identity_labeled", **result}))
            return 0
        require(args.capture is not None, "capture_or_public_dataset_required")
        require(args.node_id is None or 0 <= args.node_id <= 255, "invalid_node_id")
        peer, trigger = mac(args.peer_mac), mac(args.trigger_mac)
        require(args.capture.is_file() and args.capture.stat().st_size <= MAX_FILE, "capture_missing_or_too_large")
        with args.capture.open("rb") as stream:
            records, source = read_records(stream)
        datasets, filters = prepare_records(records, peer, trigger, args.node_id, args.provenance)
        index = write_datasets(args.output, datasets, source, args.provenance, filters)
        print(json.dumps({"status": "PREPARED", "datasets": index["datasets"], "label_status": "unlabeled"}))
        return 0
    except (ImportError, OSError) as error:
        print(json.dumps({"status": "INPUT_ERROR", "reason": str(error) if isinstance(error, ImportError) else "file_io_error"}))
        return 1


def sha_file(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def public_inventory(root):
    """Published directory labels only; no inference from network identifiers."""
    root = Path(root).resolve()
    items = []
    for split, dirname in (("train", "train_amp"), ("test", "test_amp")):
        directory = root / dirname
        require(directory.is_dir() and not directory.is_symlink(), "missing_public_split")
        classes = sorted(directory.iterdir())
        require(len(classes) == 14, "expected_14_published_identities")
        for folder in classes:
            require(folder.is_dir() and not folder.is_symlink()
                    and re.fullmatch(r"[A-Za-z0-9_.-]{1,80}", folder.name), "invalid_identity_directory")
            files = sorted(folder.iterdir())
            require(2 <= len(files) <= 100, "invalid_recording_count")
            for path in files:
                require(path.is_file() and not path.is_symlink() and path.suffix.lower() == ".mat"
                        and 128 <= path.stat().st_size <= 16 * 1024 * 1024,
                        "invalid_or_oversized_mat_file")
                items.append({"path": path, "identity": folder.name, "source_split": split, "source_folder": dirname,
                              "sha256": sha_file(path), "size_bytes": path.stat().st_size})
    require(sum(i["source_split"] == "train" for i in items) == 294
            and sum(i["source_split"] == "test" for i in items) == 546, "published_file_counts_mismatch")
    require({i["identity"] for i in items if i["source_split"] == "train"}
            == {i["identity"] for i in items if i["source_split"] == "test"}, "identity_mapping_mismatch")
    require(len({i["sha256"] for i in items}) == len(items), "duplicate_recording_hash")
    for identity in sorted({i["identity"] for i in items}):
        candidates = sorted((i for i in items if i["identity"] == identity and i["source_split"] == "train"),
                            key=lambda i: i["sha256"])
        validation = max(1, len(candidates) // 5)
        for number, item in enumerate(candidates):
            item["split"] = "validation" if number < validation else "train"
    for item in items:
        if item["source_split"] == "test":
            item["split"] = "test"
    return items


def load_public_mat(path):
    """SciPy numeric MAT reader; never execute MATLAB, pickle, cells or structs."""
    from scipy.io import loadmat, whosmat
    try:
        fields = whosmat(path)
        require(len(fields) <= 16, "mat_variable_limit")
        matches = [(shape, kind) for name, shape, kind in fields if name == "CSIamp"]
        require(len(matches) == 1 and matches[0][0] == (342, 2000)
                and matches[0][1] in ("single", "double"), "unsupported_CSIamp_shape_or_class")
        value = loadmat(path, variable_names=["CSIamp"], verify_compressed_data_integrity=True)["CSIamp"]
    except (OSError, ValueError, TypeError, KeyError, NotImplementedError) as error:
        raise ImportError("invalid_numeric_mat") from error
    require(value.shape == (342, 2000) and value.dtype.kind == "f"
            and not value.dtype.hasobject and np.isfinite(value).all()
            and np.min(value) >= 0 and np.max(value) <= 1e6, "invalid_amplitude_values")
    # Exact author loader decimation: x[:, ::4].reshape(3, 114, 500).
    return np.ascontiguousarray(value[:, ::4].T, dtype=np.float32)


def prepare_public(root, output):
    items = public_inventory(root)
    output = Path(output).expanduser().resolve()
    require(not any((p / ".git").exists() for p in (output, *output.parents)), "private_dataset_must_be_outside_git")
    require(not output.exists(), "output_must_be_new")
    os.umask(0o077)
    output.mkdir(mode=0o700, parents=False)
    # One allocation avoids holding every original double-precision MAT file.
    frames = np.empty((len(items) * 500, 342), dtype=np.float32)
    sessions, sources, processed_hashes = [], [], set()
    for index, item in enumerate(items):
        processed = load_public_mat(item["path"])
        processed_hash = hashlib.sha256(processed.tobytes(order="C")).hexdigest()
        require(processed_hash not in processed_hashes, "duplicate_processed_recording")
        processed_hashes.add(processed_hash)
        frames[index * 500:(index + 1) * 500] = processed
        require(sha_file(item["path"]) == item["sha256"], "source_changed_during_import")
        sources.append({k: item[k] for k in ("sha256", "size_bytes")})
        sessions.append({"session_id": item["sha256"], "source_sha256": item["sha256"],
                         "source_split": item["source_split"], "split": item["split"],
                         "source_folder": item["source_folder"],
                         "identity": item["identity"], "start": index * 500, "end": (index + 1) * 500})
    npz = output / "dataset-000.npz"
    with npz.open("xb") as stream:
        np.savez(stream, frames=frames, timestamps=np.tile(np.arange(1, 501, dtype=np.float64), len(items)),
                 sequence=np.tile(np.arange(500, dtype=np.uint32), len(items)))
    metadata = {"schema_version": 1, "provenance": "real", "source_kind": "ntu_fi_humanid",
                "label_status": "identity_labeled", "label_origin": "published_dataset",
                "split_unit": "source_file", "acquisition_session_disjointness": "unknown",
                "split_protocol": "archive_directory_custom",
                "author_training_folder": "test_amp", "author_test_folder": "train_amp",
                "source_folder_mapping": {"train": "train_amp", "test": "test_amp"},
                "source_split_semantics": "Declared custom archive-directory partition, not the author benchmark partition",
                "source_files": sources, "dataset_sha256": sha_file(npz), "frame_count": len(frames),
                "feature_count": 342, "time_unit": "sample_index", "sessions": sessions,
                "format": {"kind": "CSI_amplitude", "antennas": 3, "subcarriers": 114,
                           "samples_per_file": 500, "author_temporal_decimation": 4},
                "feature_encoding": "antenna_then_subcarrier_amplitude_no_normalization",
                "split_policy": "First floor(20%) by SHA256 per identity from train_amp become validation; test_amp is reserved for custom test",
                "citation": "https://github.com/xyanchen/WiFi-CSI-Sensing-Benchmark",
                "dataset_doi": "10.17632/dzvgyxkx2f.1", "dataset_license": "CC-BY-4.0",
                "limitations": ["This custom archive-directory partition does not reproduce the author's opposite train/test folder mapping",
                                "Published recording labels are used; acquisition sessions are unknown",
                                "File-disjoint evaluation does not establish session-disjoint generalization",
                                "Sample indices are not seconds; no sample rate is invented",
                                "Public gait identity results do not validate identity from this user's BFI hardware"]}
    meta_path = output / "dataset-000.json"
    meta_path.write_text(json.dumps(metadata, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    return {"data": npz.name, "metadata": meta_path.name, "frame_count": len(frames),
            "dataset_sha256": metadata["dataset_sha256"], "source_files": len(items)}


if __name__ == "__main__":
    sys.exit(main())
