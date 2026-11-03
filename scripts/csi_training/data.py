"""Bounded, hash verified CSI feature intake and explicit grouped partitions.

Manifest v1 has a ``files`` list. Each entry supplies path, sha256,
captured_at (UTC ISO 8601), room_id, session_id, split, evidence and
use_permitted. Capture dates are declarations, not inferred from file mtimes.
"""
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import re
import stat

import numpy as np

SPLITS = ("train", "validation", "test")
MAX_FILE_BYTES = 256 * 1024 * 1024
MAX_TOTAL_BYTES = 512 * 1024 * 1024
MAX_MANIFEST_BYTES = 1024 * 1024
MAX_LINE_BYTES = 65536
MAX_FRAMES = 500000
MAX_INPUT_LINES = 1000000


class DataError(ValueError):
    """Input does not satisfy the training evidence contract."""


@dataclass
class Partition:
    features: np.ndarray
    timestamps: np.ndarray
    node_ids: list
    groups: list
    session_ids: list


@dataclass
class Dataset:
    partitions: dict
    inventory: list
    manifest_sha256: str
    group_by: str


@dataclass
class Standardizer:
    mean: np.ndarray
    scale: np.ndarray

    def transform(self, features):
        values = np.asarray(features, dtype=np.float64)
        if values.ndim != 2 or values.shape[1] != 8 or not np.isfinite(values).all():
            raise DataError("standardizer requires finite Nx8 features")
        result = (values - self.mean) / self.scale
        if not np.isfinite(result).all():
            raise DataError("standardization overflow")
        return result.astype(np.float32)


def fit_standardizer(train_features):
    values = np.asarray(train_features, dtype=np.float64)
    if values.ndim != 2 or values.shape[1] != 8 or len(values) < 2 or not np.isfinite(values).all():
        raise DataError("fit requires at least two finite training feature rows")
    mean, scale = values.mean(axis=0), values.std(axis=0)
    if not np.isfinite(mean).all() or not np.isfinite(scale).all():
        raise DataError("training statistics overflow")
    return Standardizer(mean, np.where(scale < 1e-8, 1.0, scale))


def _json(text):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise DataError("duplicate JSON key")
            result[key] = value
        return result
    try:
        return json.loads(text, object_pairs_hook=pairs,
                          parse_constant=lambda _: (_ for _ in ()).throw(DataError("nonfinite JSON number")))
    except (ValueError, UnicodeError) as exc:
        raise DataError("invalid JSON input") from exc


def _regular(path, maximum):
    if path.is_symlink():
        raise DataError("symlink input is not permitted")
    try:
        info = path.stat()
    except OSError as exc:
        raise DataError("input file unavailable") from exc
    if not stat.S_ISREG(info.st_mode) or info.st_size > maximum:
        raise DataError("input must be a bounded regular file")
    return info.st_size


def _text(value, name):
    if not isinstance(value, str) or not value.strip() or len(value) > 256 or any(ord(c) < 32 for c in value):
        raise DataError(f"invalid {name}")
    return value


def _manifest(path):
    path = Path(path).absolute()
    _regular(path, MAX_MANIFEST_BYTES)
    raw = path.read_bytes()
    if len(raw) > MAX_MANIFEST_BYTES:
        raise DataError("manifest exceeds limit")
    obj = _json(raw)
    if not isinstance(obj, dict) or type(obj.get("schema_version")) is not int or obj["schema_version"] != 1:
        raise DataError("manifest schema_version must equal 1")
    entries = obj.get("files")
    if not isinstance(entries, list) or not entries or len(entries) > 1024:
        raise DataError("manifest files must contain 1 to 1024 entries")
    checked, digests, total = [], set(), 0
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise DataError("manifest file entry must be an object")
        for key in ("room_id", "session_id", "path"):
            _text(entry.get(key), key)
        digest = entry.get("sha256")
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise DataError("sha256 must be a lowercase hex digest")
        if digest in digests:
            raise DataError("duplicate file hash")
        digests.add(digest)
        if entry.get("split") not in SPLITS or entry.get("evidence") not in ("real", "synthetic"):
            raise DataError("invalid split or evidence")
        if entry.get("use_permitted") is not True:
            raise DataError("explicit use_permitted=true is required")
        captured = _text(entry.get("captured_at"), "captured_at")
        try:
            date = datetime.fromisoformat(captured.replace("Z", "+00:00"))
        except ValueError as exc:
            raise DataError("invalid captured_at") from exc
        if date.tzinfo is None or date.utcoffset() != timezone.utc.utcoffset(date):
            raise DataError("captured_at must specify UTC")
        source = path.parent / entry["path"]
        total += _regular(source, MAX_FILE_BYTES)
        if total > MAX_TOTAL_BYTES:
            raise DataError("total dataset exceeds byte limit")
        checked.append((dict(entry, captured_at=date.isoformat()), source, index))
    return checked, hashlib.sha256(raw).hexdigest()


def discover(manifest):
    """Inventory declared capture dates; load_manifest verifies hashes and frames."""
    entries, _ = _manifest(manifest)
    return sorted([dict(captured_at=e["captured_at"], sha256=e["sha256"],
                        split=e["split"], evidence=e["evidence"],
                        room_id=e["room_id"], session_id=e["session_id"])
                   for e, _, _ in entries], key=lambda e: datetime.fromisoformat(e["captured_at"]), reverse=True)


def _number(value):
    if type(value) not in (int, float) or abs(value) > 1e20 or not math.isfinite(value):
        raise DataError("numeric values must be finite and bounded")
    return 0.0 if value == 0 else float(value)


def load_manifest(manifest, group_by="room_id", allow_synthetic=False):
    """Validate partitions without deriving labels or creating raw CSI features."""
    if group_by not in ("room_id", "session_id"):
        raise DataError("group_by must be room_id or session_id")
    entries, manifest_digest = _manifest(manifest)
    owners, session_owners, fingerprints, rows = {}, {}, {}, {s: [] for s in SPLITS}
    seen_observations = set()
    frame_count = 0
    input_lines = 0
    for entry, source, index in entries:
        split, group = entry["split"], entry[group_by]
        if entry["evidence"] == "synthetic" and not allow_synthetic:
            raise DataError("synthetic data requires explicit allow_synthetic")
        for mapping, key in ((owners, group), (session_owners, entry["session_id"])):
            if key in mapping and mapping[key] != split:
                raise DataError("group or session appears across splits")
            mapping[key] = split
        digest, last, count, size = hashlib.sha256(), {}, 0, 0
        with source.open("rb") as stream:
            while True:
                line = stream.readline(MAX_LINE_BYTES + 1)
                if not line:
                    break
                input_lines += 1
                if input_lines > MAX_INPUT_LINES:
                    raise DataError("dataset exceeds input line limit")
                size += len(line)
                if len(line) > MAX_LINE_BYTES or size > MAX_FILE_BYTES:
                    raise DataError("capture line or file exceeds limit")
                digest.update(line)
                if not line.strip():
                    continue
                row = _json(line)
                if not isinstance(row, dict):
                    raise DataError("capture row must be an object")
                # Mixed legacy recordings are supported only for explicit types.
                if row.get("type") in ("vitals", "raw_csi"):
                    continue
                if row.get("type") not in (None, "feature", "features"):
                    raise DataError("unsupported capture row type")
                features = row.get("features")
                node = row.get("node_id")
                if isinstance(features, dict):
                    # CsiRecorder.write_frame removes the packet type and places
                    # explicit packet features/node_id in this nested envelope.
                    # Outer timestamp is already seconds; timestamp_us is metadata.
                    if row.get("type") is not None or features.get("type") not in (None, "feature", "features"):
                        raise DataError("unsupported collector envelope")
                    node = features.get("node_id")
                    features = features.get("features")
                if not isinstance(features, list) or len(features) != 8:
                    raise DataError("feature rows require eight explicit features")
                features = [_number(v) for v in features]
                timestamp = _number(row.get("timestamp"))
                if type(node) is int:
                    if node < 0 or node > 2**32 - 1:
                        raise DataError("invalid node_id")
                    node = str(node)
                else:
                    node = _text(node, "node_id")
                observation = (entry["session_id"], node, timestamp)
                if observation in seen_observations:
                    raise DataError("duplicate session/node/timestamp observation")
                seen_observations.add(observation)
                if node in last and timestamp < last[node]:
                    raise DataError("timestamps must be monotonic per node")
                last[node] = timestamp
                # Bounded copy heuristic, not proof of absence of leakage.
                # Idle/quantized vectors can legitimately recur at distinct times.
                signature = json.dumps([timestamp, node, features], separators=(",", ":")).encode()
                fingerprint = hashlib.sha256(signature).digest()
                if fingerprint in fingerprints and fingerprints[fingerprint] != split:
                    raise DataError("duplicate feature frame across splits")
                fingerprints[fingerprint] = split
                rows[split].append((features, timestamp, node, group, entry["session_id"]))
                count += 1
                frame_count += 1
                if frame_count > MAX_FRAMES:
                    raise DataError("dataset exceeds frame limit")
        if digest.hexdigest() != entry["sha256"]:
            raise DataError(f"file hash mismatch at manifest entry {index}")
        if count == 0:
            raise DataError("capture has no explicit feature rows; raw CSI requires a reviewed adapter")
    partitions = {}
    for split, values in rows.items():
        if len(values) < 2:
            raise DataError(f"partition {split} requires at least two feature rows")
        features, timestamps, nodes, groups, sessions = zip(*values)
        partitions[split] = Partition(np.asarray(features, dtype=np.float64),
                                      np.asarray(timestamps, dtype=np.float64),
                                      list(nodes), list(groups), list(sessions))
    inventory = sorted([{k: e[k] for k in ("captured_at", "sha256", "split", "evidence", "room_id", "session_id")}
                        for e, _, _ in entries], key=lambda e: datetime.fromisoformat(e["captured_at"]), reverse=True)
    return Dataset(partitions, inventory, manifest_digest, group_by)
