#!/usr/bin/env python3
"""Download a pinned public BFI subset; training and validation are the default.

No packet decoding, model execution, installation or hardware access. Failed
partial files remain private and are never accepted as completed downloads.
The per-file deadline is checked between bounded network reads; a read already
in progress may take up to its 20-second timeout after the deadline.
"""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import http.client
import json
import os
from pathlib import Path, PurePosixPath
import re
import signal
import socket
import ssl
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request

MANIFEST_SHA256 = "c2db11f71fb7cef87ff9853bdf7571816347824445855c128caeb4053eef288a"
REVISION = "c3013b3edc5a563a9cf6759f5d464a9d9b6f456a"
REPOSITORY = "foysalhaque/CSI-BFI-HAR-Dataset"
BASE_URL = f"https://huggingface.co/datasets/{REPOSITORY}/resolve/{REVISION}/"
CARD_SHA256 = "0870600f7e9e5f232f515876fadf76b9a4cafee324a5066d7aaa65a262541d0e"
MAX_TOTAL = 4_000_000_000
MAX_FILE = 64 * 1024 * 1024
MAX_MANIFEST = 256 * 1024
MAX_RECEIPT = 1024 * 1024
CHUNK = 64 * 1024
READ_TIMEOUT = 20
FILE_SECONDS = 180
SPLITS = ("train", "validation", "heldout_same_device", "heldout_other_device_view")
TRANSIENT_HTTP = frozenset((408, 429, 500, 502, 503, 504))
SHA256 = re.compile(r"[a-f0-9]{64}\Z")


class DownloadError(ValueError):
    def __init__(self, code, retryable=False):
        super().__init__(code)
        self.code, self.retryable = code, retryable


def require(condition, code):
    if not condition:
        raise DownloadError(code)


def utc_now():
    return datetime.now(timezone.utc).isoformat()


def is_link(path):
    return path.is_symlink() or (hasattr(path, "is_junction") and path.is_junction())


def private_root(value):
    path = Path(value).expanduser().absolute()
    require(not any(is_link(p) for p in (path, *path.parents)), "output_contains_link")
    path = path.resolve()
    require(not any((p / ".git").exists() for p in (path, *path.parents)), "output_must_be_outside_git")
    return path


def destination(root, relative):
    require(isinstance(relative, str) and len(relative) <= 240 and "\\" not in relative,
            "unsafe_relative_path")
    parts = PurePosixPath(relative).parts
    require(parts and all(re.fullmatch(r"[A-Za-z0-9_.-]+", part) and part not in (".", "..")
                          for part in parts), "unsafe_relative_path")
    path = root.joinpath(*parts)
    require(path.is_relative_to(root) and not any(is_link(p) for p in (path, *path.parents)),
            "destination_contains_link")
    require(path.resolve().is_relative_to(root), "destination_escaped_root")
    return path


def read_bounded(path, limit):
    require(path.is_file() and not is_link(path) and path.stat().st_size <= limit,
            "missing_or_oversized_file")
    with path.open("rb") as stream:
        value = stream.read(limit + 1)
    require(len(value) <= limit, "file_grew_beyond_limit")
    return value


def load_manifest(path):
    raw = read_bounded(Path(path), MAX_MANIFEST)
    require(hashlib.sha256(raw).hexdigest() == MANIFEST_SHA256, "manifest_hash_mismatch")
    manifest = json.loads(raw)
    require(manifest.get("revision") == REVISION and manifest.get("source_repository") == REPOSITORY
            and manifest.get("dataset_license") == "gpl-3.0", "manifest_source_mismatch")
    entries, seen = manifest.get("files"), set()
    require(isinstance(entries, list) and len(entries) == 240, "manifest_file_count")
    for entry in entries:
        relative = entry.get("path", "")
        match = re.fullmatch(r"(HAR-[135])/BFI/(M[12])/([A-T])_([135])_(M[12])_(P[123])\.pcapng", relative)
        require(match is not None and relative not in seen, "invalid_manifest_path")
        seen.add(relative)
        expected = {("HAR-1", "M1"): "train", ("HAR-3", "M1"): "validation",
                    ("HAR-5", "M1"): "heldout_same_device", ("HAR-5", "M2"): "heldout_other_device_view"}
        require(entry.get("split") == expected.get((match[1], match[2]))
                and match[1][-1] == match[4] and match[2] == match[5], "invalid_manifest_split")
        require(type(entry.get("bytes")) is int and 0 < entry["bytes"] <= MAX_FILE,
                "invalid_manifest_size")
        require(isinstance(entry.get("sha256"), str) and SHA256.fullmatch(entry["sha256"]),
                "invalid_manifest_digest")
        require(entry.get("url") == BASE_URL + relative, "invalid_manifest_url")
    require(sum(e["bytes"] for e in entries) == 3_364_983_408, "manifest_total_mismatch")
    for split in SPLITS:
        require(sum(e["split"] == split for e in entries) == 60, "manifest_split_count")
    card = manifest.get("license_receipt", {})
    require(card == {"url": BASE_URL + "README.md", "bytes": 6719, "sha256": CARD_SHA256,
                     "declaration": "license: gpl-3.0"}, "license_receipt_mismatch")
    return manifest, raw


def approved_transport_url(value):
    try:
        parsed = urllib.parse.urlsplit(value)
        port = parsed.port
    except (TypeError, ValueError):
        return False
    return (len(value) <= 16384 and parsed.scheme == "https" and not parsed.username
            and not parsed.password and not parsed.fragment and port in (None, 443)
            and (parsed.hostname == "huggingface.co" or (parsed.hostname or "").endswith(".hf.co")))


class PinnedRedirects(urllib.request.HTTPRedirectHandler):
    max_redirections = 5
    max_repeats = 2

    def redirect_request(self, request, response, code, message, headers, newurl):
        require(approved_transport_url(newurl), "unapproved_download_redirect")
        return super().redirect_request(request, response, code, message, headers, newurl)


def transport(call):
    try:
        return call()
    except urllib.error.HTTPError as error:
        code = error.code
        error.close()
        raise DownloadError(f"http_{code}", code in TRANSIENT_HTTP) from None
    except (urllib.error.URLError, OSError, http.client.HTTPException) as error:
        reason = error.reason if isinstance(error, urllib.error.URLError) else error
        transient = (not isinstance(reason, ssl.SSLError)
                     and (isinstance(reason, (TimeoutError, ConnectionError, http.client.RemoteDisconnected,
                                              http.client.IncompleteRead))
                          or isinstance(reason, socket.gaierror) and reason.errno == socket.EAI_AGAIN))
        raise DownloadError("transient_transport_failure" if transient else "transport_failure", transient) from None


class Budget:
    def __init__(self, disk_bytes=0, limit=MAX_TOTAL):
        self.network_bytes, self.disk_bytes, self.limit = 0, disk_bytes, limit

    def receive(self, count):
        self.network_bytes += count
        require(self.network_bytes <= self.limit, "network_byte_budget_exceeded")

    def write(self, count):
        require(self.disk_bytes + count + MAX_RECEIPT <= self.limit, "disk_byte_budget_exceeded")
        self.disk_bytes += count


def existing_size(root):
    total, count = 0, 0
    for base, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(base) / name
            count += 1
            require(count <= 2048 and not is_link(path), "unsafe_or_excessive_output_entries")
            if path.is_file():
                require(path.stat().st_size <= max(MAX_FILE, MAX_RECEIPT), "oversized_existing_output")
                total += path.stat().st_size
            else:
                require(path.is_dir(), "nonregular_output_entry")
    require(total + MAX_RECEIPT <= MAX_TOTAL, "existing_output_exceeds_budget")
    return total


def verify_file(path, entry):
    require(path.is_file() and not is_link(path) and path.stat().st_size == entry["bytes"],
            "existing_file_size_or_type_mismatch")
    digest, count = hashlib.sha256(), 0
    with path.open("rb") as stream:
        while chunk := stream.read(CHUNK):
            count += len(chunk)
            require(count <= entry["bytes"], "existing_file_grew")
            digest.update(chunk)
    require(count == entry["bytes"] and digest.hexdigest() == entry["sha256"], "existing_file_hash_mismatch")


def atomic_json(path, value):
    raw = (json.dumps(value, indent=2, sort_keys=True, allow_nan=False) + "\n").encode()
    require(len(raw) <= MAX_RECEIPT, "receipt_byte_budget_exceeded")
    require(not is_link(path), "receipt_is_link")
    with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".receipt-", delete=False) as stream:
        temporary = Path(stream.name)
        stream.write(raw)
        stream.flush()
        os.fsync(stream.fileno())
    try:
        os.replace(temporary, path)
    finally:
        if temporary.exists():
            temporary.unlink()


def failure_code(error):
    return error.code if isinstance(error, DownloadError) else type(error).__name__


def download_file(entry, root, opener, budget, record, persist, clock=time.monotonic):
    target = destination(root, entry["path"])
    if target.exists():
        try:
            verify_file(target, entry)
        except BaseException as error:
            record.update(status="FAILED", failure=failure_code(error))
            persist()
            raise
        record["status"] = "VERIFIED_EXISTING"
        persist()
        return
    require(entry["url"] == BASE_URL + entry["path"], "unapproved_initial_url")
    require(type(entry["bytes"]) is int and 0 < entry["bytes"] <= MAX_FILE
            and SHA256.fullmatch(entry["sha256"]), "invalid_download_contract")
    target.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    destination(root, entry["path"])
    deadline = clock() + FILE_SECONDS
    record["attempts"] = []
    for attempt_number in (1, 2):
        partial = None
        received, written = 0, 0
        digest = hashlib.sha256()
        attempt = {"number": attempt_number, "status": "RUNNING", "received_bytes": 0}
        record["attempts"].append(attempt)
        try:
            require(clock() < deadline, "file_deadline_exceeded")
            descriptor, name = tempfile.mkstemp(prefix="." + target.name + ".", suffix=".partial", dir=target.parent)
            partial = Path(name)
            with os.fdopen(descriptor, "wb") as output:
                attempt["partial_path"] = partial.relative_to(root).as_posix()
                record["status"] = "DOWNLOADING"
                persist()
                request = urllib.request.Request(entry["url"], headers={"Accept-Encoding": "identity",
                                                                         "User-Agent": "RuView-public-data/1"})
                response = transport(lambda: opener.open(request, timeout=READ_TIMEOUT))
                with response:
                    require(clock() < deadline, "file_deadline_exceeded")
                    require(response.status == 200 and response.headers.get("Content-Range") is None,
                            "unexpected_http_response")
                    require(response.headers.get("Content-Encoding", "identity").lower() == "identity",
                            "encoded_http_response")
                    length = response.headers.get("Content-Length")
                    require(length is None or length == str(entry["bytes"]), "content_length_mismatch")
                    require(callable(getattr(response, "read1", None)), "bounded_read1_unavailable")
                    while True:
                        require(clock() < deadline, "file_deadline_exceeded")
                        require(budget.network_bytes < budget.limit, "network_byte_budget_exceeded")
                        amount = min(CHUNK, entry["bytes"] - received + 1,
                                     budget.limit - budget.network_bytes)
                        chunk = transport(lambda: response.read1(amount))
                        require(isinstance(chunk, bytes) and len(chunk) <= amount, "invalid_response_chunk")
                        received += len(chunk)
                        budget.receive(len(chunk))
                        require(clock() < deadline, "file_deadline_exceeded")
                        if not chunk:
                            break
                        require(received <= entry["bytes"], "oversized_response")
                        budget.write(len(chunk))
                        output.write(chunk)
                        written += len(chunk)
                        digest.update(chunk)
                require(received == entry["bytes"], "short_response")
                require(digest.hexdigest() == entry["sha256"], "download_hash_mismatch")
                output.flush()
                os.fsync(output.fileno())
            destination(root, entry["path"])
            # Hard-link creation is atomic and fails if the completed name exists.
            # os.replace would silently overwrite an existing completed file.
            os.link(partial, target, follow_symlinks=False)
            partial.unlink()
            attempt.update(status="COMPLETED", received_bytes=received, written_bytes=written,
                           partial_retained=False, partial_sha256=digest.hexdigest())
            record["status"] = "DOWNLOADED"
            persist()
            return
        except BaseException as error:
            # Include only bytes that actually reached the partial file, including
            # any successful buffered flush before an I/O or transport failure.
            partial_digest = hashlib.sha256()
            if partial is not None and partial.is_file() and not is_link(partial):
                with partial.open("rb") as stream:
                    written = 0
                    while chunk := stream.read(CHUNK):
                        written += len(chunk)
                        require(written <= MAX_FILE, "partial_file_exceeds_limit")
                        partial_digest.update(chunk)
            attempt.update(status="FAILED", failure=failure_code(error), received_bytes=received,
                           written_bytes=written, partial_sha256=partial_digest.hexdigest(),
                           partial_retained=partial is not None and partial.exists())
            record["status"] = "FAILED"
            persist()
            if isinstance(error, DownloadError) and error.retryable and attempt_number == 1 and clock() < deadline:
                continue
            raise


@contextmanager
def output_lock(root):
    lock = root / ".download.lock"
    descriptor = os.open(lock, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    try:
        with os.fdopen(descriptor, "w") as stream:
            json.dump({"pid": os.getpid(), "created_utc": utc_now()}, stream)
        yield
    finally:
        lock.unlink()


def run(args, opener=None, clock=time.monotonic):
    manifest, raw_manifest = load_manifest(args.manifest)
    root = private_root(args.output)
    require(not root.exists() or args.resume, "existing_output_requires_resume")
    selected = [e for e in manifest["files"] if args.include_heldout or e["split"] in SPLITS[:2]]
    selected = [{"path": "README.md", "url": BASE_URL + "README.md", "bytes": 6719,
                 "sha256": CARD_SHA256, "split": "license"}] + selected
    require(sum(e["bytes"] for e in selected) + len(raw_manifest) + MAX_RECEIPT < MAX_TOTAL,
            "selection_exceeds_budget")
    previous, prior = None, None
    if root.exists():
        saved = read_bounded(root / "source-manifest.json", MAX_MANIFEST)
        require(saved == raw_manifest, "resume_manifest_mismatch")
        prior = read_bounded(root / "download-receipt.json", MAX_RECEIPT)
        previous = json.loads(prior)
        require(isinstance(previous, dict) and previous.get("manifest_sha256") == MANIFEST_SHA256,
                "resume_receipt_mismatch")
        previous = {"sha256": hashlib.sha256(prior).hexdigest(), "status": previous.get("status")}
    else:
        root.mkdir(parents=True, mode=0o700)
    with output_lock(root):
        if previous is not None:
            history = destination(root, "receipt-history")
            history.mkdir(mode=0o700, exist_ok=True)
            archived = destination(root, f"receipt-history/{previous['sha256']}.json")
            if archived.exists():
                require(read_bounded(archived, MAX_RECEIPT) == prior, "receipt_history_mismatch")
            else:
                with archived.open("xb") as stream:
                    stream.write(prior)
            previous["path"] = archived.relative_to(root).as_posix()
        source = root / "source-manifest.json"
        if not source.exists():
            with source.open("xb") as stream:
                stream.write(raw_manifest)
        budget = Budget(existing_size(root))
        started = clock()
        receipt = {"schema_version": 1, "status": "RUNNING", "manifest_sha256": MANIFEST_SHA256,
                   "source_sha256": hashlib.sha256(read_bounded(Path(__file__), MAX_MANIFEST)).hexdigest(),
                   "python_version": sys.version,
                   "revision": REVISION, "dataset_license": "gpl-3.0", "started_utc": utc_now(),
                   "limits": {"total_bytes": MAX_TOTAL, "per_file_bytes": MAX_FILE,
                              "network_read_timeout_seconds": READ_TIMEOUT,
                              "per_file_deadline_seconds": FILE_SECONDS,
                              "deadline_enforcement": "checked_between_bounded_reads_not_instantaneous_cancellation",
                              "chunk_bytes": CHUNK, "max_attempts": 2},
                   "include_heldout": args.include_heldout, "previous_receipt": previous,
                   "selected_trace_files": len(selected) - 1,
                   "files": [{"path": e["path"], "bytes": e["bytes"], "sha256": e["sha256"],
                              "split": e["split"], "status": "PENDING"} for e in selected]}

        def persist():
            receipt.update(updated_utc=utc_now(), elapsed_seconds=clock() - started,
                           network_bytes=budget.network_bytes, accounted_disk_bytes=budget.disk_bytes,
                           verified_files=sum(r["status"] in ("DOWNLOADED", "VERIFIED_EXISTING") for r in receipt["files"]))
            atomic_json(root / "download-receipt.json", receipt)

        persist()
        try:
            opener = opener or urllib.request.build_opener(PinnedRedirects())
            for entry, record in zip(selected, receipt["files"]):
                download_file(entry, root, opener, budget, record, persist, clock)
            receipt["status"] = "COMPLETED"
            persist()
        except BaseException as error:
            receipt.update(status="INTERRUPTED" if isinstance(error, (KeyboardInterrupt, InterruptedError)) else "FAILED",
                           failure=failure_code(error))
            persist()
            raise
    return receipt


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", default=str(Path(__file__).parent / "manifests/csi-bfi-har-c3013b3-subset.json"))
    parser.add_argument("--output", required=True, help="Private directory outside Git")
    parser.add_argument("--resume", action="store_true", help="Verify and reuse completed files from this exact manifest")
    parser.add_argument("--include-heldout", action="store_true", help="Also download both held-out day-5 views; never trains on them")
    args = parser.parse_args(argv)

    def interrupt(signum, frame):
        raise InterruptedError("download_interrupted")

    previous = signal.signal(signal.SIGTERM, interrupt)
    try:
        receipt = run(args)
        print(json.dumps({k: receipt[k] for k in ("status", "selected_trace_files", "verified_files", "network_bytes")}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        print(json.dumps({"status": "INTERRUPTED" if isinstance(error, (KeyboardInterrupt, InterruptedError)) else "FAILED",
                          "failure": failure_code(error)}))
        return 1
    finally:
        signal.signal(signal.SIGTERM, previous)


if __name__ == "__main__":
    raise SystemExit(main())
