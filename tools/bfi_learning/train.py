#!/usr/bin/env python3
"""Bounded local LSTM/GRU baseline; CSI prediction is not person identification.

Input: prepare.py's numeric NPZ (frames, timestamps, sequence) and hashed JSON
metadata. Pretraining predicts the next observed frame, not a fixed-time sample.
Validation chooses the checkpoint; test scoring is a separate opt-in after
that choice. A single session tests temporal continuation,
not generalization to another person, room, device, or session.

All datasets, checkpoints and metrics must remain outside Git worktrees. This
script performs no capture, network access, package installation or promotion.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
import random
import re
import struct
import sys
import tempfile
import time
import zipfile

import numpy as np

MAX_DATA_BYTES = 1024 * 1024 * 1024
MAX_METRICS_BYTES = 8 * 1024 * 1024
MAX_ZIP_DIRECTORY_BYTES = 64 * 1024
MAX_FRAMES = 1_000_000
MAX_FEATURES = 1024
MAX_PARAMETERS = 1_000_000
# Source receipts produced before --encoder existed. These implementations used
# one LSTM layer; a missing architecture never inherits the evaluation CLI value.
LEGACY_LSTM_TRAINERS = frozenset({
    "0b94c5f05b7004fcf54de0888ae1072af6148932e3198bbffa0188e0d16eca0e",
    "55cc2f93af41b00a7bac20dd13a7635e662ee78b22acca314e78afbd77ad25f5",
})
LEGACY_RAW_ORDER_TRAINERS = LEGACY_LSTM_TRAINERS | {
    "f6a352e9dd91875edfb8ff61f0e323c2cfc4405fdffd02a37b452e3e3923c007",
}
SPLITS = ("train", "validation", "test")
IDENTIFIER = re.compile(r"[A-Za-z0-9_.-]{1,80}\Z")
SHA256 = re.compile(r"[a-f0-9]{64}\Z")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, low, high, name):
    require(type(value) is int and low <= value <= high, f"invalid {name}")
    return value


def finite_number(value, low, high, name):
    require(type(value) in (int, float) and low <= value <= high
            and math.isfinite(value), f"invalid {name}")
    return value


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def outside_git(path):
    resolved = Path(path).resolve()
    require(not any((p / ".git").exists() for p in (resolved, *resolved.parents)),
            "datasets and outputs must be outside Git worktrees")
    return resolved


def read_bounded(path, limit):
    require(path.is_file() and path.stat().st_size <= limit, "file missing or too large")
    with path.open("rb") as handle:
        data = handle.read(limit + 1)
    require(len(data) <= limit, "file grew beyond limit")
    return data


def preflight_npz_directory(raw, expected_names, expansion_limit):
    """Bound ZIP metadata before ZipFile allocates any ZipInfo objects.

    Supported NPZs have one disk and a classic, comment-free end record. Normal
    np.savez/np.savez_compressed force ZIP64 *local* headers even for small
    arrays; those remain supported. Archive-level ZIP64 directories/locators
    and ZIP64 central size/offset sentinels are unnecessary for our <=1 GiB,
    <=32-member inputs and fail closed. No central directory is copied here.
    """
    names = {name.encode("ascii") for name in expected_names}
    require(1 <= len(names) <= 32 and len(raw) >= 22, "invalid NPZ directory bounds")
    end = len(raw) - 22
    eocd = struct.unpack_from("<4s4H2IH", raw, end)
    signature, disk, directory_disk, on_disk, total, size, offset, comment = eocd
    require(signature == b"PK\x05\x06" and comment == 0,
            "NPZ needs a classic comment-free ZIP end record")
    require(not (end >= 20 and raw[end - 20:end - 16] == b"PK\x06\x07")
            and on_disk != 0xffff and total != 0xffff
            and size != 0xffffffff and offset != 0xffffffff,
            "archive-level ZIP64 directory unsupported")
    require(disk == 0 and directory_disk == 0 and on_disk == total == len(names),
            "unexpected ZIP disk or entry count")
    require(46 * total <= size <= min(MAX_ZIP_DIRECTORY_BYTES, total * (46 + 128 + 1024 + 1024)),
            "ZIP central directory byte budget exceeded")
    require(offset <= end and offset + size == end, "ZIP directory offset/size mismatch")
    cursor, expanded = offset, 0
    seen, local_offsets = set(), set()
    for _ in range(total):
        require(cursor + 46 <= end, "truncated ZIP central header")
        header = struct.unpack_from("<4s6H3I5H2I", raw, cursor)
        (magic, _, _, flags, compression, _, _, _, compressed, uncompressed,
         name_size, extra_size, comment_size, member_disk, _, _, local_offset) = header
        require(magic == b"PK\x01\x02", "invalid ZIP central signature")
        require(member_disk == 0 and compressed != 0xffffffff
                and uncompressed != 0xffffffff and local_offset != 0xffffffff,
                "ZIP64 central entry or multidisk member unsupported")
        require(1 <= name_size <= 128 and extra_size <= 1024 and comment_size <= 1024,
                "ZIP entry metadata budget exceeded")
        record_end = cursor + 46 + name_size + extra_size + comment_size
        require(record_end <= end, "truncated ZIP central entry")
        name = raw[cursor + 46:cursor + 46 + name_size]
        require(name in names and name not in seen, "unexpected or duplicate ZIP member")
        require(not flags & 1 and compression in (0, 8), "unsupported ZIP encoding")
        require(local_offset not in local_offsets and local_offset + 30 + compressed <= offset
                and raw[local_offset:local_offset + 4] == b"PK\x03\x04",
                "invalid ZIP local member extent")
        expanded += uncompressed
        require(expanded <= expansion_limit, "NPZ expansion exceeds limit")
        seen.add(name)
        local_offsets.add(local_offset)
        cursor = record_end
    require(cursor == end and seen == names, "ZIP central count/size disagreement")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def load_dataset(npz_path, metadata_path):
    """Check ZIP and NPY allocation bounds before NumPy allocates arrays."""
    data = read_bounded(outside_git(npz_path), MAX_DATA_BYTES)
    metadata_bytes = read_bounded(outside_git(metadata_path), 1024 * 1024)
    meta = json.loads(metadata_bytes, object_pairs_hook=unique_object,
                      parse_constant=lambda x: (_ for _ in ()).throw(ValueError("nonfinite JSON")))
    require(isinstance(meta, dict) and type(meta.get("schema_version")) is int
            and meta["schema_version"] == 1, "unsupported metadata schema")
    require(meta.get("provenance") in ("real", "synthetic"), "unknown provenance")
    require(meta.get("source_kind") in ("rac1", "bfi", "ntu_fi_humanid"), "unsupported source kind")
    if meta["source_kind"] == "ntu_fi_humanid":
        require(meta.get("split_protocol") == "archive_directory_custom",
                "NTU inputs require explicit archive_directory_custom protocol; not author-split reproduction")
    require(meta.get("label_status") in ("unlabeled", "identity_labeled"), "invalid label status")
    require(meta.get("dataset_sha256") == sha256(data), "dataset hash mismatch")
    sources = meta.get("source_files")
    require(isinstance(sources, list) and 1 <= len(sources) <= 1024, "missing source provenance")
    for source in sources:
        require(isinstance(source, dict) and isinstance(source.get("sha256"), str)
                and SHA256.fullmatch(source["sha256"]), "invalid source hash")
        integer(source.get("size_bytes"), 1, 1 << 50, "source size")
    n = integer(meta.get("frame_count"), 32, MAX_FRAMES, "frame count")
    f = integer(meta.get("feature_count"), 1, MAX_FEATURES, "feature count")
    require(isinstance(meta.get("format"), dict), "missing format metadata")
    require(meta.get("time_unit", "seconds") in ("seconds", "sample_index"), "invalid time unit")
    expected = {"frames.npy": ((n, f), "f", 4),
                "timestamps.npy": ((n,), "f", 8), "sequence.npy": ((n,), "u", 4)}
    preflight_npz_directory(data, expected, MAX_DATA_BYTES)
    with zipfile.ZipFile(io.BytesIO(data)) as archive:
        infos = archive.infolist()
        require(len(infos) == 3 and {i.filename for i in infos} == set(expected), "unexpected NPZ members")
        require(sum(i.file_size for i in infos) <= MAX_DATA_BYTES, "NPZ expansion exceeds limit")
        for info in infos:
            require(not info.flag_bits & 1 and info.compress_type in (0, 8), "unsupported ZIP encoding")
            with archive.open(info) as handle:
                version = np.lib.format.read_magic(handle)
                require(version in ((1, 0), (2, 0)), "unsupported NPY version")
                reader = (np.lib.format.read_array_header_1_0 if version == (1, 0)
                          else np.lib.format.read_array_header_2_0)
                shape, fortran, dtype = reader(handle, max_header_size=4096)
                shape_expected, kind, itemsize = expected[info.filename]
                require(shape == shape_expected and not fortran and not dtype.hasobject
                        and dtype.kind == kind and dtype.itemsize == itemsize,
                        "invalid tensor shape or numeric dtype")
                require(handle.tell() + math.prod(shape) * dtype.itemsize == info.file_size,
                        "NPY byte count mismatch")
    with np.load(io.BytesIO(data), allow_pickle=False) as loaded:
        frames = np.asarray(loaded["frames"], dtype=np.float32)
        timestamps = np.asarray(loaded["timestamps"], dtype=np.float64)
        sequence = np.asarray(loaded["sequence"], dtype=np.uint32)
    require(np.isfinite(frames).all() and np.max(np.abs(frames)) <= 1e6, "invalid frame values")
    require(np.isfinite(timestamps).all() and (timestamps > 0).all()
            and (timestamps < 1e11).all(), "invalid timestamps")
    sessions = meta.get("sessions")
    require(isinstance(sessions, list) and 1 <= len(sessions) <= 1024, "invalid sessions")
    cursor, seen = 0, set()
    for session in sessions:
        require(isinstance(session, dict), "invalid session")
        sid = session.get("session_id")
        require(isinstance(sid, str) and IDENTIFIER.fullmatch(sid) and sid not in seen, "invalid or duplicate session id")
        seen.add(sid)
        start = integer(session.get("start"), 0, n - 1, "session start")
        end = integer(session.get("end"), 1, n, "session end")
        require(start == cursor and end > start, "sessions must partition all rows without overlap")
        require(np.all(np.diff(timestamps[start:end]) > 0), "timestamps must increase within each session")
        cursor = end
    require(cursor == n, "sessions do not cover every row")
    return {"frames": frames, "timestamps": timestamps, "sequence": sequence,
            "metadata": meta, "metadata_sha256": sha256(metadata_bytes)}


def build_splits(dataset, window, purge, mode="pretrain", max_gap_seconds=1.0,
                 identity_protocol="session", stride=1):
    """Return target indices; no input or target frame crosses a split boundary."""
    integer(window, 2, 256, "window")
    integer(purge, window, 4096, "purge (must be at least window)")
    finite_number(max_gap_seconds, 0.000001, 60, "maximum gap")
    integer(stride, 1, 256, "stride")
    require(mode in ("pretrain", "identity"), "invalid mode")
    meta, times = dataset["metadata"], dataset["timestamps"]
    intervals = {k: [] for k in SPLITS}
    labels = np.full(len(times), -1, dtype=np.int64)
    classes = []
    if mode == "identity":
        require(meta["provenance"] == "real" and meta["label_status"] == "identity_labeled",
                "identity mode requires real, explicitly labeled data")
        require(identity_protocol in ("session", "published_files"), "invalid identity protocol")
        if identity_protocol == "published_files":
            require(meta.get("split_unit") == "source_file"
                    and meta.get("acquisition_session_disjointness") == "unknown"
                    and meta.get("label_origin") == "published_dataset",
                    "published-file protocol requires explicit source and session limitations")
            seen_files = set()
            source_hashes = {s["sha256"] for s in meta["source_files"]}
            for s in meta["sessions"]:
                digest = s.get("source_sha256")
                require(digest in source_hashes and digest not in seen_files,
                        "recording hashes must be unique and in source provenance")
                seen_files.add(digest)
                require(s.get("source_split") in ("train", "test"), "missing published source split")
                require((s["source_split"] == "test") == (s.get("split") == "test"),
                        "declared source test may only be test; validation must come from declared source train")
        else:
            require(meta.get("split_unit") == "acquisition_session"
                    and meta.get("acquisition_session_disjointness") == "verified",
                    "strict identity mode requires verified acquisition-session separation")
            sources = {s["sha256"] for s in meta["source_files"]}
            seen_recordings, acquisition_splits = set(), {}
            for s in meta["sessions"]:
                digest, acquisition = s.get("source_sha256"), s.get("acquisition_session_id")
                require(digest in sources and digest not in seen_recordings,
                        "strict sessions need unique recording hashes linked to provenance")
                require(isinstance(acquisition, str) and IDENTIFIER.fullmatch(acquisition),
                        "missing source-linked acquisition session id")
                seen_recordings.add(digest)
                require(acquisition not in acquisition_splits or acquisition_splits[acquisition] == s.get("split"),
                        "an acquisition session crosses identity splits")
                acquisition_splits[acquisition] = s.get("split")
        for s in meta["sessions"]:
            identity = s.get("identity")
            require(isinstance(identity, str) and IDENTIFIER.fullmatch(identity), "missing identity label")
            require(s.get("split") in SPLITS, "identity sessions need explicit disjoint split assignments")
        classes = sorted({s["identity"] for s in meta["sessions"]})
        require(2 <= len(classes) <= 64, "identity mode requires 2..64 real identities")
        for split in SPLITS:
            require({s["identity"] for s in meta["sessions"] if s["split"] == split} == set(classes),
                    "every identity needs a separate session in each split")
        for s in meta["sessions"]:
            intervals[s["split"]].append((s["start"], s["end"], s["session_id"]))
            labels[s["start"]:s["end"]] = classes.index(s["identity"])
    else:
        require(meta.get("source_kind") != "ntu_fi_humanid"
                and not any("source_split" in s or "split" in s for s in meta["sessions"]),
                "pretraining must not repartition declared or published holdouts")
        for s in meta["sessions"]:
            a, b = s["start"], s["end"]
            first, second = a + (b - a) * 6 // 10, a + (b - a) * 8 // 10
            for split, lo, hi in (("train", a, first), ("validation", first + purge, second),
                                  ("test", second + purge, b)):
                require(hi - lo > window, "session too short for chronological purged splits")
                intervals[split].append((lo, hi, s["session_id"]))
    targets = {}
    for split, ranges in intervals.items():
        arrays = []
        for a, b, _ in ranges:
            candidates = np.arange(a + window, b, stride, dtype=np.int64)
            bad = np.concatenate(([0], np.cumsum(np.diff(times[a:b]) > max_gap_seconds)))
            local = candidates - a
            candidates = candidates[bad[local] == bad[local - window]]
            arrays.append(candidates)
        targets[split] = np.concatenate(arrays) if arrays else np.empty(0, dtype=np.int64)
        require(len(targets[split]) >= 4, f"too few continuous {split} windows")
        if mode == "identity":
            require(set(labels[targets[split]]) == set(range(len(classes))),
                    f"gap filtering removed an identity from {split}")
    return {"targets": targets, "intervals": intervals, "labels": labels, "classes": classes}


def fit_normalization(frames, train_intervals):
    """Fit exclusively on training rows; float64 accumulation uses bounded chunks."""
    total = np.zeros(frames.shape[1], dtype=np.float64)
    squared = np.zeros_like(total)
    count = 0
    for start, end, _ in train_intervals:
        for offset in range(start, end, 4096):
            chunk = frames[offset:min(offset + 4096, end)].astype(np.float64)
            total += chunk.sum(axis=0)
            squared += (chunk * chunk).sum(axis=0)
            count += len(chunk)
    require(count > 0, "no training rows")
    mean = total / count
    scale = np.sqrt(np.maximum(squared / count - mean * mean, 0))
    scale = np.maximum(scale, 1e-5)
    return mean, scale


def transform_recordings(dataset, representation, temporal_order, seed, mode):
    """Offline recording controls. They are never applied to a forecasting task.

    Demeaning uses only that complete recording's samples, without labels or
    other recordings. Shuffling permutes complete feature rows together; it
    does not permute subcarriers separately. Administrative sample coordinates
    stay fixed, so the shuffled input has no claim to chronological sampling.
    """
    require(representation in ("raw", "demean"), "invalid representation")
    require(temporal_order in ("original", "shuffle"), "invalid temporal order")
    integer(seed, 0, (1 << 31) - 1, "transform seed")
    meta, frames = dataset["metadata"], dataset["frames"]
    changed = representation != "raw" or temporal_order != "original"
    if changed:
        require(mode == "identity" and meta.get("source_kind") == "ntu_fi_humanid"
                and meta.get("split_protocol") == "archive_directory_custom"
                and meta.get("split_unit") == "source_file"
                and meta.get("time_unit") == "sample_index",
                "recording controls require public NTU custom identity mode")
    result = frames.copy() if changed else frames
    permutations = hashlib.sha256()
    for session in meta["sessions"] if changed else ():
        start, end = session["start"], session["end"]
        block = result[start:end]
        if representation == "demean":
            center = block.mean(axis=0, dtype=np.float64)
            np.subtract(block, center, out=block)
        if temporal_order == "shuffle":
            # No labels, split names, global RNG state or other recording values
            # enter this seed. PCG64 is explicit and NumPy's version is recorded.
            source = session.get("source_sha256")
            sid = session.get("session_id")
            require(isinstance(source, str) and SHA256.fullmatch(source)
                    and isinstance(sid, str) and IDENTIFIER.fullmatch(sid), "shuffle needs recording provenance")
            material = (b"ruview-recording-shuffle-v1\0" + seed.to_bytes(4, "little")
                        + bytes.fromhex(source) + hashlib.sha256(sid.encode()).digest())
            digest = hashlib.sha256(material).digest()
            generator = np.random.Generator(np.random.PCG64(int.from_bytes(digest[:16], "little")))
            order = generator.permutation(end - start)
            block[:] = block[order]
            permutations.update(digest)
            permutations.update(np.asarray(order, dtype="<u4").tobytes())
    if changed:
        require(np.isfinite(result).all(), "nonfinite transformed features")
    return result, {"representation": representation, "temporal_order": temporal_order,
                    "operation_order": "demean_then_shuffle_then_train_only_normalization",
                    "centering": "complete_source_recording_per_feature_float64_mean_float32_output" if representation == "demean" else None,
                    "shuffle_algorithm": "sha256_seed_source_session_to_pcg64_permutation_v1" if temporal_order == "shuffle" else None,
                    "permutation_sha256": permutations.hexdigest() if temporal_order == "shuffle" else None,
                    "labels_used": False, "cross_recording_statistics": False,
                    "timestamp_sequence_semantics": ("administrative_sample_coordinates_chronology_destroyed"
                                                     if temporal_order == "shuffle" else "unchanged"),
                    "forecasting_supported": not changed}


def bounded_targets(targets, limit, rng=None):
    if len(targets) <= limit:
        return targets.copy()
    if rng is None:
        return targets[np.linspace(0, len(targets) - 1, limit, dtype=np.int64)]
    return np.sort(rng.choice(targets, size=limit, replace=False))


def write_json(path, value):
    raw = json.dumps(value, indent=2, sort_keys=True, allow_nan=False).encode() + b"\n"
    require(len(raw) <= MAX_METRICS_BYTES, "JSON receipt exceeds reader byte budget")
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
        temporary = Path(handle.name)
        handle.write(raw)
    os.replace(temporary, path)


def validation_history_entry(epoch, loss, count, validation):
    """Keep epoch scalars; quadratic confusion matrices belong to selected snapshots."""
    return {"epoch": epoch, "training_loss": loss, "training_windows": count,
            "validation": {k: v for k, v in validation.items()
                           if k not in ("confusion_matrix", "recording_confusion_matrix")}}


def save_checkpoint(path, state, mean, scale):
    # Numeric-only checkpoint: no pickle loader is required by consumers.
    tensors = {name: tensor.detach().cpu().numpy() for name, tensor in state.items()}
    tensors.update(normalization_mean=mean, normalization_scale=scale)
    with tempfile.NamedTemporaryFile(dir=path.parent, delete=False) as handle:
        temporary = Path(handle.name)
        np.savez(handle, **tensors)
    os.replace(temporary, path)
    return sha256(path.read_bytes())


def frozen_configuration(args):
    """Restore a selected run's hyperparameters; never train during final evaluation."""
    if not args.evaluate_checkpoint:
        return None
    directory = outside_git(args.evaluate_checkpoint)
    raw = read_bounded(directory / "metrics.json", MAX_METRICS_BYTES)
    record = json.loads(raw, object_pairs_hook=unique_object)
    require(isinstance(record, dict) and type(record.get("schema_version")) is int
            and record["schema_version"] == 1, "invalid frozen receipt schema")
    require(record.get("status") == "COMPLETED" and "test" in record and record["test"] is None,
            "frozen run must be complete and previously untested")
    require(isinstance(record.get("checkpoint_sha256"), str) and SHA256.fullmatch(record["checkpoint_sha256"])
            and isinstance(record.get("config"), dict), "missing frozen checkpoint contract")
    for name in ("trainer_sha256", "dataset_sha256", "metadata_sha256"):
        require(isinstance(record.get(name), str) and SHA256.fullmatch(record[name]),
                "invalid frozen provenance hash")
    allowed = set(vars(args)) - {"dataset", "metadata", "output", "evaluate_checkpoint"}
    legacy_encoder = False
    legacy_controls = False
    stored = set(record["config"]) - {"evaluate_checkpoint"}
    controls = {"representation", "temporal_order"}
    if stored in (allowed - controls, allowed - controls - {"encoder"}):
        require(record["trainer_sha256"] in LEGACY_RAW_ORDER_TRAINERS,
                "missing controls allowed only for recognized legacy raw/original trainer")
        record["config"] = dict(record["config"], representation="raw", temporal_order="original")
        legacy_controls = True
        stored = set(record["config"]) - {"evaluate_checkpoint"}
    if stored == allowed - {"encoder"}:
        require(record["trainer_sha256"] in LEGACY_LSTM_TRAINERS and legacy_controls,
                "missing encoder allowed only for recognized legacy LSTM trainer")
        record["config"] = dict(record["config"], encoder="lstm")
        legacy_encoder = True
    require(set(record["config"]) - {"evaluate_checkpoint"} == allowed,
            "incomplete or unknown frozen configuration")
    require(record.get("mode") == record["config"].get("mode")
            and record["config"].get("evaluate_test") is False,
            "frozen selection must be a validation-only run")
    integer(record.get("best_epoch"), 1, 1000, "frozen best epoch")
    requested_device, requested_seconds, requested_threads = args.device, args.max_seconds, args.threads
    for name, value in record["config"].items():
        if name != "evaluate_checkpoint":
            setattr(args, name, value)
    args.device, args.max_seconds, args.threads = requested_device, requested_seconds, requested_threads
    args.evaluate_test = True
    return {"directory": directory, "record": record, "metrics_sha256": sha256(raw),
            "legacy_encoder_default_applied": legacy_encoder,
            "legacy_controls_defaults_applied": legacy_controls}


def load_checkpoint(path, state, expected_digest, mean, scale, torch):
    raw = read_bounded(path, 16 * 1024 * 1024)
    require(sha256(raw) == expected_digest, "frozen checkpoint hash mismatch")
    expected = {k: (tuple(v.shape), v.detach().cpu().numpy().dtype) for k, v in state.items()}
    expected.update(normalization_mean=(mean.shape, mean.dtype), normalization_scale=(scale.shape, scale.dtype))
    preflight_npz_directory(raw, {name + ".npy" for name in expected}, 16 * 1024 * 1024)
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        infos = archive.infolist()
        require(len(infos) == len(expected) and {i.filename for i in infos} == {k + ".npy" for k in expected},
                "checkpoint tensor names mismatch")
        require(sum(i.file_size for i in infos) <= 16 * 1024 * 1024, "checkpoint expansion exceeds bound")
        for info in infos:
            with archive.open(info) as handle:
                version = np.lib.format.read_magic(handle)
                require(version in ((1, 0), (2, 0)), "unsupported checkpoint NPY")
                reader = np.lib.format.read_array_header_1_0 if version == (1, 0) else np.lib.format.read_array_header_2_0
                shape, fortran, dtype = reader(handle, max_header_size=4096)
                require((shape, dtype) == expected[info.filename[:-4]] and not fortran and not dtype.hasobject,
                        "checkpoint tensor shape/dtype mismatch")
                require(handle.tell() + math.prod(shape) * dtype.itemsize == info.file_size, "checkpoint byte count mismatch")
    with np.load(io.BytesIO(raw), allow_pickle=False) as loaded:
        require(all(np.isfinite(loaded[k]).all() for k in loaded.files), "nonfinite checkpoint")
        require(np.array_equal(loaded["normalization_mean"], mean)
                and np.array_equal(loaded["normalization_scale"], scale), "frozen train-only normalization mismatch")
        return {k: torch.from_numpy(loaded[k].copy()) for k in state}


def validate_config(args, feature_count):
    require(args.encoder in ("lstm", "gru"), "encoder must be lstm or gru")
    require(args.representation in ("raw", "demean"), "representation must be raw or demean")
    require(args.temporal_order in ("original", "shuffle"), "temporal order must be original or shuffle")
    integer(args.window, 2, 256, "window")
    integer(args.purge, args.window, 4096, "purge")
    integer(args.hidden, 4, 128, "hidden width")
    integer(args.batch_size, 1, 128, "batch size")
    integer(args.epochs, 1, 1000, "epochs")
    integer(args.max_steps, 1, 100_000, "max steps")
    integer(args.max_train_windows, 4, 200_000, "training window cap")
    integer(args.max_eval_windows, 4, 50_000, "evaluation window cap")
    integer(args.seed, 0, (1 << 31) - 1, "seed")
    integer(args.threads, 1, 8, "CPU threads")
    integer(args.stride, 1, 256, "stride")
    finite_number(args.max_seconds, 10, 43_200, "time budget")
    finite_number(args.learning_rate, 1e-6, 0.01, "learning rate")
    finite_number(args.max_gap_seconds, 0.000001, 60, "maximum gap")
    require(args.device in ("cpu", "cuda"), "device must be explicit cpu or cuda")
    # Conservative tensor/activation accounting; allocator/workspace RSS varies by backend.
    activation_estimate = args.batch_size * args.window * (feature_count + 12 * args.hidden) * 4
    require(activation_estimate <= 128 * 1024 * 1024, "batch activation estimate exceeds bound")


def run(args):
    frozen = frozen_configuration(args)
    dataset = load_dataset(args.dataset, args.metadata)
    frames, meta = dataset["frames"], dataset["metadata"]
    args.purge = args.window if args.purge is None else args.purge
    validate_config(args, frames.shape[1])
    if frozen:
        require(frozen["record"]["dataset_sha256"] == meta["dataset_sha256"]
                and frozen["record"]["metadata_sha256"] == dataset["metadata_sha256"],
                "frozen run dataset or metadata mismatch")
    require(meta["provenance"] == "real" or args.allow_synthetic,
            "synthetic input requires --allow-synthetic and cannot prove hardware performance")
    splits = build_splits(dataset, args.window, args.purge, args.mode, args.max_gap_seconds,
                          args.identity_protocol, args.stride)
    output = outside_git(args.output)
    require(not output.exists(), "output must be a new private directory")
    feature_frames, input_controls = transform_recordings(dataset, args.representation, args.temporal_order,
                                                         args.seed, args.mode)
    if frozen:
        recorded_controls = frozen["record"].get("input_controls")
        require(recorded_controls == input_controls or
                (recorded_controls is None and frozen["legacy_controls_defaults_applied"]),
                "frozen input controls mismatch")
    os.environ.setdefault("CUBLAS_WORKSPACE_CONFIG", ":4096:8")
    import torch
    require(args.device != "cuda" or torch.cuda.is_available(), "CUDA requested but unavailable")
    random.seed(args.seed)
    np.random.seed(args.seed)
    torch.manual_seed(args.seed)
    torch.set_num_threads(args.threads)
    torch.use_deterministic_algorithms(True)
    if args.device == "cuda":
        torch.cuda.manual_seed_all(args.seed)
        torch.backends.cudnn.benchmark = False
        # Applies only to this process's PyTorch allocator, not other services.
        torch.cuda.set_per_process_memory_fraction(0.20, 0)
        torch.cuda.reset_peak_memory_stats(0)
    rng = np.random.default_rng(args.seed)
    targets = {split: bounded_targets(splits["targets"][split],
               args.max_train_windows if split == "train" else args.max_eval_windows,
               rng if split == "train" else None) for split in SPLITS}
    if args.mode == "identity":
        require(all(set(splits["labels"][v]) == set(range(len(splits["classes"])))
                    for v in targets.values()), "window caps removed an identity; increase caps")
    mean, scale = fit_normalization(feature_frames, splits["intervals"]["train"])
    # A transformed array is already owned by this run; reuse it for normalization.
    normalized = feature_frames.copy() if feature_frames is frames else feature_frames
    np.subtract(normalized, mean, out=normalized)
    np.divide(normalized, scale, out=normalized)
    require(np.isfinite(normalized).all(), "normalization produced nonfinite values")
    output_features = frames.shape[1] if args.mode == "pretrain" else len(splits["classes"])
    architecture = {"encoder": args.encoder, "layers": 1, "input_features": frames.shape[1],
                    "hidden_features": args.hidden, "output_features": output_features}
    if frozen:
        saved_architecture = frozen["record"].get("model_architecture")
        require(saved_architecture == architecture or
                (saved_architecture is None and frozen["legacy_encoder_default_applied"]),
                "frozen model architecture mismatch")

    class Model(torch.nn.Module):
        def __init__(self):
            super().__init__()
            encoder_type = torch.nn.LSTM if args.encoder == "lstm" else torch.nn.GRU
            self.encoder = encoder_type(frames.shape[1], args.hidden, batch_first=True)
            self.head = torch.nn.Linear(args.hidden, output_features)

        def forward(self, x):
            _, state = self.encoder(x)
            hidden = state[0] if args.encoder == "lstm" else state
            return self.head(hidden[-1])

    model = Model().to(args.device)
    parameters = sum(p.numel() for p in model.parameters())
    require(parameters <= MAX_PARAMETERS, "model parameter bound exceeded")
    optimizer = None if frozen else torch.optim.Adam(model.parameters(), lr=args.learning_rate)
    loss_fn = torch.nn.MSELoss() if args.mode == "pretrain" else torch.nn.CrossEntropyLoss()
    offsets = np.arange(args.window, 0, -1)
    train_labels = splits["labels"][targets["train"]]
    majority = int(np.bincount(train_labels).argmax()) if args.mode == "identity" else None
    record_index = np.empty(len(frames), dtype=np.int32)
    record_labels = np.empty(len(meta["sessions"]), dtype=np.int64)
    for i, session in enumerate(meta["sessions"]):
        record_index[session["start"]:session["end"]] = i
        record_labels[i] = splits["labels"][session["start"]]
    majority_recording = (int(np.bincount([splits["labels"][a] for a, _, _ in splits["intervals"]["train"]]).argmax())
                          if args.mode == "identity" else None)
    output.mkdir(parents=True, mode=0o700)
    start = time.monotonic()
    deadline, train_deadline = start + args.max_seconds, start + args.max_seconds * 0.8
    history, best_state, best_score, best_epoch, steps = [], None, float("inf"), None, 0
    stop_reason = "epoch_limit"

    def gpu_metrics():
        if args.device != "cuda":
            return None
        return {"allocator_fraction_limit": 0.20,
                "peak_allocated_bytes": torch.cuda.max_memory_allocated(0),
                "peak_reserved_bytes": torch.cuda.max_memory_reserved(0),
                "scope": "this process PyTorch allocator; excludes CUDA context and external allocations"}

    def batches(indices):
        for offset in range(0, len(indices), args.batch_size):
            chosen = indices[offset:offset + args.batch_size]
            x = torch.from_numpy(normalized[chosen[:, None] - offsets]).to(args.device)
            y = torch.from_numpy(normalized[chosen] if args.mode == "pretrain"
                                 else splits["labels"][chosen]).to(args.device)
            yield chosen, x, y

    def evaluate(indices, split):
        model.eval()
        count, loss_sum, raw_sum, base_sum, base_raw_sum, correct = 0, 0., 0., 0., 0., 0
        confusion = np.zeros((len(splits["classes"]), len(splits["classes"])), dtype=np.int64)
        record_probabilities = np.zeros((len(meta["sessions"]), len(splits["classes"])), dtype=np.float64)
        record_windows = np.zeros(len(meta["sessions"]), dtype=np.int64)
        with torch.no_grad():
            for chosen, x, y in batches(indices):
                if time.monotonic() > deadline:
                    raise TimeoutError("time budget exhausted before complete evaluation")
                prediction = model(x)
                loss = loss_fn(prediction, y)
                require(torch.isfinite(loss).item(), "nonfinite evaluation loss")
                loss_sum += loss.item() * len(chosen)
                count += len(chosen)
                if args.mode == "pretrain":
                    error = (prediction - y).cpu().numpy().astype(np.float64)
                    base = (x[:, -1] - y).cpu().numpy().astype(np.float64)
                    raw_sum += np.square(error * scale).mean(axis=1).sum()
                    base_sum += np.square(base).mean(axis=1).sum()
                    base_raw_sum += np.square(base * scale).mean(axis=1).sum()
                else:
                    predicted = prediction.argmax(dim=1).cpu().numpy()
                    truth = y.cpu().numpy()
                    correct += int((predicted == truth).sum())
                    np.add.at(confusion, (truth, predicted), 1)
                    np.add.at(record_probabilities, record_index[chosen], prediction.softmax(dim=1).cpu().numpy())
                    np.add.at(record_windows, record_index[chosen], 1)
        result = {"windows": count, "loss": loss_sum / count}
        if args.mode == "pretrain":
            baseline = base_sum / count
            result.update(normalized_mse=loss_sum / count, raw_feature_mse=float(raw_sum / count),
                          persistence_normalized_mse=float(baseline), persistence_raw_feature_mse=float(base_raw_sum / count),
                          improvement_over_persistence=(1 - loss_sum / count / baseline) if baseline > 0 else None)
        else:
            present = record_windows > 0
            record_truth = record_labels[present]
            record_prediction = record_probabilities[present].argmax(axis=1)
            record_confusion = np.zeros_like(confusion)
            np.add.at(record_confusion, (record_truth, record_prediction), 1)
            result.update(accuracy=correct / count,
                          accuracy_unit="window",
                          balanced_accuracy=float(np.mean(np.diag(confusion) / confusion.sum(axis=1))),
                          majority_baseline_accuracy=float(np.mean(splits["labels"][indices] == majority)),
                          confusion_matrix=confusion.tolist(), recordings=int(present.sum()),
                          recording_accuracy=float(np.mean(record_prediction == record_truth)),
                          recording_balanced_accuracy=float(np.mean(np.diag(record_confusion) / record_confusion.sum(axis=1))),
                          recording_majority_baseline_accuracy=float(np.mean(record_truth == majority_recording)),
                          recording_confusion_matrix=record_confusion.tolist(),
                          recording_aggregation="mean_softmax_over_selected_windows",
                          available_recordings=int(len(np.unique(record_index[splits["targets"][split]]))),
                          recordings_skipped_by_window_cap=int(len(np.unique(record_index[splits["targets"][split]])) - present.sum()),
                          available_windows=len(splits["targets"][split]),
                          selected_window_fraction=count / len(splits["targets"][split]),
                          minimum_windows_per_evaluated_recording=int(record_windows[present].min()),
                          maximum_windows_per_evaluated_recording=int(record_windows[present].max()))
        return result

    config = {k: v for k, v in vars(args).items() if k not in ("dataset", "metadata", "output", "evaluate_checkpoint")}
    receipt = {"schema_version": 1, "status": "RUNNING", "mode": args.mode,
               "evidence": "MEASURED_LOCAL" if meta["provenance"] == "real" else "SYNTHETIC",
               "dataset_sha256": meta["dataset_sha256"], "metadata_sha256": dataset["metadata_sha256"],
               "trainer_sha256": sha256(Path(__file__).read_bytes()), "config": config,
               "versions": {"python": sys.version.split()[0], "numpy": np.__version__, "torch": torch.__version__},
               "device_name": torch.cuda.get_device_name(0) if args.device == "cuda" else "CPU",
               "cuda_memory": gpu_metrics(),
               "parameters": parameters, "feature_count": frames.shape[1],
               "model_architecture": architecture,
               "input_controls": input_controls,
               "split_method": ("chronological_per_session_with_purge" if args.mode == "pretrain"
                                else ("archive_directory_custom_with_whole_file_validation" if args.identity_protocol == "published_files"
                                      else "explicit_disjoint_acquisition_sessions")),
               "split_protocol": meta.get("split_protocol", "local_chronological" if args.mode == "pretrain" else "declared_acquisition_sessions"),
               "acquisition_session_disjointness": meta.get("acquisition_session_disjointness", "unknown"),
               "time_unit": meta.get("time_unit", "seconds"),
               "split_intervals": splits["intervals"],
               "available_windows": {k: len(v) for k, v in splits["targets"].items()},
               "used_windows": {k: len(v) for k, v in targets.items()},
               "normalization_fit": "training_rows_only", "test_used_for_selection": False,
               "identity_validated": False, "history": history, "test": None,
               "history_detail": "epoch scalars; full matrices in best_validation and latest_validation",
               "test_evaluation_requested": args.evaluate_test,
               "limitations": ["Next observed frame prediction does not prove identity, pose, or motion sensing",
                               "Declared input provenance is hashed, not an authenticated hardware attestation",
                               "Temporal holdout does not establish new session, subject, room, or device generalization",
                               "NTU archive-directory custom split does not reproduce the author's reversed-folder loader protocol",
                               "Repeated test inspection or tuning on its metrics invalidates its held-out role",
                               "Wall-time checks are between batches; a stalled backend needs an external watchdog"]}
    if args.mode == "identity":
        receipt["class_label_sha256"] = [sha256(c.encode()) for c in splits["classes"]]
    write_json(output / "metrics.json", receipt)
    try:
        if frozen:
            saved = frozen["record"]
            loaded = load_checkpoint(frozen["directory"] / "checkpoint.npz", model.state_dict(),
                                     saved["checkpoint_sha256"], mean, scale, torch)
            model.load_state_dict(loaded)
            receipt.update(run_kind="frozen_checkpoint_evaluation", checkpoint_sha256=saved["checkpoint_sha256"],
                           frozen_metrics_sha256=frozen["metrics_sha256"], frozen_trainer_sha256=saved["trainer_sha256"],
                           legacy_encoder_default_applied=frozen["legacy_encoder_default_applied"],
                           legacy_controls_defaults_applied=frozen["legacy_controls_defaults_applied"],
                           best_epoch=saved["best_epoch"], steps=0)
            # Exclusive claim persists even if scoring is interrupted: never silently retry a holdout.
            claim = frozen["directory"] / "holdout-evaluation.json"
            with claim.open("x", encoding="utf-8") as handle:
                json.dump({"status": "STARTED", "checkpoint_sha256": saved["checkpoint_sha256"],
                           "dataset_sha256": meta["dataset_sha256"]}, handle)
            receipt["test"] = evaluate(targets["test"], "test")
            receipt.update(status="COMPLETED", stop_reason="frozen_evaluation", elapsed_seconds=time.monotonic() - start,
                           cuda_memory=gpu_metrics())
            write_json(output / "metrics.json", receipt)
            write_json(claim, {"status": "COMPLETED", "checkpoint_sha256": saved["checkpoint_sha256"],
                               "evaluation_metrics_sha256": sha256((output / "metrics.json").read_bytes())})
            return receipt
        for epoch in range(1, args.epochs + 1):
            model.train()
            epoch_loss, epoch_count = 0., 0
            for chosen, x, y in batches(rng.permutation(targets["train"])):
                if time.monotonic() >= train_deadline or steps >= args.max_steps:
                    stop_reason = "time_limit" if time.monotonic() >= train_deadline else "step_limit"
                    break
                optimizer.zero_grad(set_to_none=True)
                loss = loss_fn(model(x), y)
                require(torch.isfinite(loss).item(), "nonfinite training loss")
                loss.backward()
                torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0, error_if_nonfinite=True)
                optimizer.step()
                steps += 1
                epoch_loss += loss.item() * len(chosen)
                epoch_count += len(chosen)
            if epoch_count:
                validation = evaluate(targets["validation"], "validation")
                history.append(validation_history_entry(epoch, epoch_loss / epoch_count, epoch_count, validation))
                receipt["latest_validation"] = validation
                if validation["loss"] < best_score:
                    best_score, best_epoch = validation["loss"], epoch
                    best_state = {k: v.detach().cpu().clone() for k, v in model.state_dict().items()}
                    receipt["checkpoint_sha256"] = save_checkpoint(output / "checkpoint.npz", best_state, mean, scale)
                    receipt["best_validation"] = validation
                receipt.update(best_epoch=best_epoch, steps=steps, elapsed_seconds=time.monotonic() - start,
                               cuda_memory=gpu_metrics())
                write_json(output / "metrics.json", receipt)
            if stop_reason != "epoch_limit":
                break
        require(best_state is not None, "no completed training/validation checkpoint")
        model.load_state_dict(best_state)
        # Tuning runs leave test metrics absent. Explicit final evaluation is once per run;
        # repeated external runs must not use these metrics to choose a checkpoint.
        if args.evaluate_test:
            receipt["test"] = evaluate(targets["test"], "test")
        receipt.update(status="COMPLETED", stop_reason=stop_reason, best_epoch=best_epoch,
                       steps=steps, elapsed_seconds=time.monotonic() - start, cuda_memory=gpu_metrics())
        write_json(output / "metrics.json", receipt)
    except (ValueError, OSError, RuntimeError, TimeoutError, KeyboardInterrupt) as exc:
        receipt.update(status="INCOMPLETE", error=type(exc).__name__, elapsed_seconds=time.monotonic() - start,
                       stop_reason="error_or_interruption", steps=steps, cuda_memory=gpu_metrics())
        write_json(output / "metrics.json", receipt)
        raise
    return receipt


def parser():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--dataset", required=True)
    p.add_argument("--metadata", required=True)
    p.add_argument("--output", required=True, help="new private directory outside any Git checkout")
    p.add_argument("--mode", choices=("pretrain", "identity"), default="pretrain")
    p.add_argument("--encoder", choices=("lstm", "gru"), default="lstm")
    p.add_argument("--representation", choices=("raw", "demean"), default="raw")
    p.add_argument("--temporal-order", choices=("original", "shuffle"), default="original")
    p.add_argument("--identity-protocol", choices=("session", "published_files"), default="session")
    p.add_argument("--evaluate-test", action="store_true", help="final evaluation only; leave off during tuning")
    p.add_argument("--evaluate-checkpoint", metavar="FROZEN_RUN_DIRECTORY",
                   help="score the selected run without training; one-use holdout receipt prevents repeats")
    p.add_argument("--device", choices=("cpu", "cuda"), default="cpu")
    p.add_argument("--allow-synthetic", action="store_true")
    p.add_argument("--window", type=int, default=32)
    p.add_argument("--stride", type=int, default=1)
    p.add_argument("--purge", type=int, default=None)
    p.add_argument("--hidden", type=int, default=32)
    p.add_argument("--batch-size", type=int, default=64)
    p.add_argument("--epochs", type=int, default=100)
    p.add_argument("--max-steps", type=int, default=100_000)
    p.add_argument("--max-seconds", type=float, default=21_600)
    p.add_argument("--max-train-windows", type=int, default=100_000)
    p.add_argument("--max-eval-windows", type=int, default=10_000)
    p.add_argument("--max-gap", "--max-gap-seconds", dest="max_gap_seconds", type=float, default=1.0,
                   help="maximum adjacent gap in metadata time_unit (seconds or sample_index)")
    p.add_argument("--learning-rate", type=float, default=0.001)
    p.add_argument("--seed", type=int, default=1729)
    p.add_argument("--threads", type=int, default=2)
    return p


def main():
    try:
        result = run(parser().parse_args())
        print(json.dumps({"status": result["status"], "mode": result["mode"],
                          "evidence": result["evidence"], "best_epoch": result["best_epoch"],
                          "test": result["test"], "identity_validated": False}, allow_nan=False))
        return 0
    except (ValueError, OSError, RuntimeError, TimeoutError, zipfile.BadZipFile, EOFError) as exc:
        print(json.dumps({"status": "FAILED", "error": str(exc)[:300]}), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
