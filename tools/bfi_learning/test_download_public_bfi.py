"""Downloader contract tests use fake HTTP only; no data or network is needed."""
from __future__ import annotations

import hashlib
import io
import json
import os
from pathlib import Path
import socket
import ssl
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

import download_public_bfi as d


def entry(data, path="HAR-1/BFI/M1/A_1_M1_P1.pcapng", split="train"):
    return {"path": path, "url": d.BASE_URL + path, "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(), "split": split}


class FakeClock:
    value = 0.0

    def __call__(self):
        return self.value


class FakeResponse:
    def __init__(self, data, headers=None, status=200, events=None, on_read=None):
        self.data = io.BytesIO(data)
        self.headers = {"Content-Length": str(len(data))} if headers is None else headers
        self.status, self.events, self.on_read = status, list(events or []), on_read
        self.read_sizes, self.closed = [], False

    def read1(self, amount):
        self.read_sizes.append(amount)
        if self.on_read:
            self.on_read()
        if self.events:
            event = self.events.pop(0)
            if isinstance(event, BaseException):
                raise event
            return event
        return self.data.read(amount)

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.closed = True


class FakeOpener:
    def __init__(self, responses):
        self.responses, self.calls = list(responses), []

    def open(self, request, timeout):
        self.calls.append((request.full_url, timeout, dict(request.header_items())))
        if not self.responses:
            raise AssertionError("unexpected network request")
        result = self.responses.pop(0)
        if isinstance(result, BaseException):
            raise result
        return result


def http_error(code):
    return urllib.error.HTTPError(d.BASE_URL, code, "test", {}, None)


class DownloadTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.record, self.snapshots = {}, []
        self.budget = d.Budget()

    def persist(self):
        self.snapshots.append(json.loads(json.dumps(self.record)))

    def download(self, contract, opener, **kwargs):
        return d.download_file(contract, self.root, opener, self.budget, self.record,
                               self.persist, **kwargs)

    def assert_failed_partial(self, contract, code):
        self.assertFalse((self.root / contract["path"]).exists())
        self.assertEqual(self.record["status"], "FAILED")
        attempt = self.record["attempts"][-1]
        self.assertEqual(attempt["failure"], code)
        self.assertTrue(attempt["partial_retained"])
        partial = self.root / attempt["partial_path"]
        raw = partial.read_bytes()
        self.assertEqual(attempt["written_bytes"], len(raw))
        self.assertEqual(attempt["partial_sha256"], hashlib.sha256(raw).hexdigest())
        self.assertEqual(self.snapshots[-1], self.record)
        return raw

    def test_success_bounded_reads_exact_hash_atomic_completion(self):
        raw = b"public-packet-bytes" * 10000
        contract = entry(raw)
        response = FakeResponse(raw)
        opener = FakeOpener([response])
        self.download(contract, opener)
        self.assertEqual((self.root / contract["path"]).read_bytes(), raw)
        self.assertEqual(self.record["status"], "DOWNLOADED")
        self.assertEqual(self.record["attempts"][0]["status"], "COMPLETED")
        self.assertFalse(self.record["attempts"][0]["partial_retained"])
        self.assertFalse(list(self.root.rglob("*.partial")))
        self.assertEqual(self.budget.network_bytes, len(raw))
        self.assertEqual(self.budget.disk_bytes, len(raw))
        self.assertTrue(all(0 < count <= d.CHUNK for count in response.read_sizes))
        self.assertEqual(opener.calls[0][1], 20)
        self.assertEqual(opener.calls[0][2]["Accept-encoding"], "identity")
        self.assertTrue(response.closed)

    def test_hash_failure_retains_evidence_never_publishes(self):
        contract = entry(b"right")
        with self.assertRaisesRegex(d.DownloadError, "download_hash_mismatch"):
            self.download(contract, FakeOpener([FakeResponse(b"wrong")]))
        self.assertEqual(self.assert_failed_partial(contract, "download_hash_mismatch"), b"wrong")

    def test_short_response_never_retries(self):
        contract = entry(b"whole")
        opener = FakeOpener([FakeResponse(b"part", headers={})])
        with self.assertRaisesRegex(d.DownloadError, "short_response"):
            self.download(contract, opener)
        self.assertEqual(self.assert_failed_partial(contract, "short_response"), b"part")
        self.assertEqual(len(opener.calls), 1)

    def test_oversized_response_never_publishes(self):
        contract = entry(b"whole")
        with self.assertRaisesRegex(d.DownloadError, "oversized_response"):
            self.download(contract, FakeOpener([FakeResponse(b"wholeEXTRA", headers={})]))
        self.assert_failed_partial(contract, "oversized_response")
        self.assertEqual(self.record["attempts"][0]["received_bytes"], 6)

    def test_headers_rejected_before_body(self):
        for headers, status, code in [
            ({"Content-Length": "6"}, 200, "content_length_mismatch"),
            ({"Content-Encoding": "gzip"}, 200, "encoded_http_response"),
            ({"Content-Range": "bytes 0-4/5"}, 200, "unexpected_http_response"),
            ({}, 206, "unexpected_http_response"),
        ]:
            with self.subTest(code=code, status=status):
                response = FakeResponse(b"whole", headers=headers, status=status)
                with self.assertRaisesRegex(d.DownloadError, code):
                    self.download(entry(b"whole"), FakeOpener([response]))
                self.assertEqual(response.read_sizes, [])
                self.assert_failed_partial(entry(b"whole"), code)

    def test_transient_http_one_retry_then_success(self):
        contract = entry(b"whole")
        opener = FakeOpener([http_error(503), FakeResponse(b"whole")])
        self.download(contract, opener)
        self.assertEqual(len(opener.calls), 2)
        self.assertEqual([a["status"] for a in self.record["attempts"]], ["FAILED", "COMPLETED"])
        self.assertEqual(self.record["attempts"][0]["failure"], "http_503")

    def test_transient_http_retry_is_limited_to_two_attempts(self):
        opener = FakeOpener([http_error(429), http_error(503), FakeResponse(b"whole")])
        with self.assertRaisesRegex(d.DownloadError, "http_503"):
            self.download(entry(b"whole"), opener)
        self.assertEqual(len(opener.calls), 2)
        self.assertEqual(len(self.record["attempts"]), 2)
        self.assert_failed_partial(entry(b"whole"), "http_503")

    def test_permanent_http_failure_is_not_retried(self):
        opener = FakeOpener([http_error(403), FakeResponse(b"whole")])
        with self.assertRaisesRegex(d.DownloadError, "http_403"):
            self.download(entry(b"whole"), opener)
        self.assertEqual(len(opener.calls), 1)
        self.assert_failed_partial(entry(b"whole"), "http_403")

    def test_partial_timeout_retries_with_new_partial_and_counts_all_bytes(self):
        contract = entry(b"whole")
        response = FakeResponse(b"", headers={}, events=[b"wh", TimeoutError()])
        self.download(contract, FakeOpener([response, FakeResponse(b"whole")]))
        failed, completed = self.record["attempts"]
        self.assertEqual(failed["status"], "FAILED")
        self.assertEqual((self.root / failed["partial_path"]).read_bytes(), b"wh")
        self.assertNotEqual(failed["partial_path"], completed["partial_path"])
        self.assertEqual(self.budget.network_bytes, 7)
        self.assertEqual(self.budget.disk_bytes, 7)

    def test_deadline_after_read_records_bytes_and_stops(self):
        clock = FakeClock()
        response = FakeResponse(b"whole", on_read=lambda: setattr(clock, "value", 181.0))
        opener = FakeOpener([response])
        with self.assertRaisesRegex(d.DownloadError, "file_deadline_exceeded"):
            self.download(entry(b"whole"), opener, clock=clock)
        self.assert_failed_partial(entry(b"whole"), "file_deadline_exceeded")
        self.assertEqual(self.budget.network_bytes, 5)
        self.assertEqual(len(opener.calls), 1)

    def test_network_and_disk_budgets(self):
        self.budget = d.Budget(limit=d.MAX_RECEIPT + 3)
        self.budget.network_bytes = d.MAX_RECEIPT
        with self.assertRaisesRegex(d.DownloadError, "network_byte_budget_exceeded"):
            self.download(entry(b"whole"), FakeOpener([FakeResponse(b"whole")]))
        self.assert_failed_partial(entry(b"whole"), "network_byte_budget_exceeded")
        self.assertEqual(self.budget.network_bytes, d.MAX_RECEIPT + 3)
        self.budget = d.Budget(limit=d.MAX_RECEIPT + 3)
        with self.assertRaisesRegex(d.DownloadError, "disk_byte_budget_exceeded"):
            self.download(entry(b"whole"), FakeOpener([FakeResponse(b"whole")]))
        self.assertEqual(self.assert_failed_partial(entry(b"whole"), "disk_byte_budget_exceeded"), b"")

    def test_resume_hashes_existing_without_network(self):
        contract = entry(b"whole")
        target = self.root / contract["path"]
        target.parent.mkdir(parents=True)
        target.write_bytes(b"whole")
        self.download(contract, FakeOpener([]))
        self.assertEqual(self.record["status"], "VERIFIED_EXISTING")
        target.write_bytes(b"wrong")
        with self.assertRaisesRegex(d.DownloadError, "existing_file_hash_mismatch"):
            self.download(contract, FakeOpener([]))
        self.assertEqual(self.record["status"], "FAILED")
        self.assertEqual(target.read_bytes(), b"wrong")

    def test_exclusive_completion_does_not_overwrite_racing_file(self):
        contract = entry(b"whole")

        def collision(source, target, **kwargs):
            Path(target).write_bytes(b"other-owner")
            raise FileExistsError()

        with patch.object(d.os, "link", side_effect=collision):
            with self.assertRaises(FileExistsError):
                self.download(contract, FakeOpener([FakeResponse(b"whole")]))
        self.assertEqual((self.root / contract["path"]).read_bytes(), b"other-owner")
        self.assertEqual(self.record["status"], "FAILED")
        self.assertTrue(self.record["attempts"][0]["partial_retained"])

    def test_initial_url_and_contract_checked_before_network(self):
        contract = entry(b"whole")
        contract["url"] = "https://example.com/arbitrary"
        with self.assertRaisesRegex(d.DownloadError, "unapproved_initial_url"):
            self.download(contract, FakeOpener([]))
        contract["url"] = d.BASE_URL + contract["path"]
        contract["bytes"] = d.MAX_FILE + 1
        with self.assertRaisesRegex(d.DownloadError, "invalid_download_contract"):
            self.download(contract, FakeOpener([]))


class InputTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()

    def test_actual_pinned_manifest_and_license(self):
        path = Path(d.__file__).parent / "manifests/csi-bfi-har-c3013b3-subset.json"
        manifest, raw = d.load_manifest(path)
        self.assertEqual(hashlib.sha256(raw).hexdigest(), d.MANIFEST_SHA256)
        self.assertEqual(len(manifest["files"]), 240)
        default = [e for e in manifest["files"] if e["split"] in d.SPLITS[:2]]
        self.assertEqual(len(default), 120)
        self.assertEqual(sum(e["bytes"] for e in default), 929_362_200)
        altered = self.root / "manifest.json"
        altered.write_bytes(raw + b" ")
        with self.assertRaisesRegex(d.DownloadError, "manifest_hash_mismatch"):
            d.load_manifest(altered)

    def test_unsafe_paths_and_git_output_rejected(self):
        for relative in ("../outside", "/absolute", "x/../../out", "C:/outside", "x\\evil", ""):
            with self.subTest(relative=relative), self.assertRaises(d.DownloadError):
                d.destination(self.root, relative)
        (self.root / ".git").write_text("gitdir: somewhere")
        with self.assertRaisesRegex(d.DownloadError, "outside_git"):
            d.private_root(self.root / "private-data")

    def test_symlink_output_rejected(self):
        link = self.root / "link"
        try:
            link.symlink_to(self.root, target_is_directory=True)
        except OSError:
            self.skipTest("host does not allow creating symlinks")
        with self.assertRaises(d.DownloadError):
            d.private_root(link / "data")
        with self.assertRaises(d.DownloadError):
            d.destination(self.root, "link/data")

    def test_redirect_allowlist(self):
        for value in (d.BASE_URL + "README.md", "https://cas-bridge.xethub.hf.co/object?token=test"):
            self.assertTrue(d.approved_transport_url(value))
        for value in ("http://huggingface.co/x", "https://huggingface.co.evil.test/x",
                      "https://evil.test/x", "https://huggingface.co@evil.test/x",
                      "https://u:p@huggingface.co/x", "https://huggingface.co:444/x",
                      "file:///tmp/a", "https://huggingface.co/x#fragment",
                      "https://huggingface.co:invalid/x"):
            with self.subTest(value=value):
                self.assertFalse(d.approved_transport_url(value))
                with self.assertRaisesRegex(d.DownloadError, "unapproved_download_redirect"):
                    d.PinnedRedirects().redirect_request(urllib.request.Request(d.BASE_URL), None,
                                                        302, "test", {}, value)

    def test_transport_retry_only_transient(self):
        for error, retry in [(TimeoutError(), True), (ConnectionResetError(), True),
                             (socket.gaierror(socket.EAI_AGAIN, "temporary"), True),
                             (ssl.SSLError("certificate"), False), (OSError("local"), False)]:
            with self.subTest(error=type(error).__name__):
                def fail():
                    raise urllib.error.URLError(error)
                with self.assertRaises(d.DownloadError) as found:
                    d.transport(fail)
                self.assertEqual(found.exception.retryable, retry)

    def test_receipt_size_and_lock_exclusivity(self):
        target = self.root / "receipt.json"
        with self.assertRaisesRegex(d.DownloadError, "receipt_byte_budget_exceeded"):
            d.atomic_json(target, {"value": "x" * d.MAX_RECEIPT})
        self.assertFalse(target.exists())
        with d.output_lock(self.root):
            with self.assertRaises(FileExistsError):
                with d.output_lock(self.root):
                    self.fail("second lock acquired")
        self.assertFalse((self.root / ".download.lock").exists())


class RunTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve() / "public-private"
        self.card = b"license: gpl-3.0\n" + b" " * (6719 - 17)
        self.assertEqual(len(self.card), 6719)
        self.data = [b"train", b"validation", b"test-one", b"test-two"]
        self.entries = [entry(data, path, split) for data, path, split in zip(self.data, (
            "HAR-1/BFI/M1/A_1_M1_P1.pcapng", "HAR-3/BFI/M1/A_3_M1_P1.pcapng",
            "HAR-5/BFI/M1/A_5_M1_P1.pcapng", "HAR-5/BFI/M2/A_5_M2_P1.pcapng"), d.SPLITS)]
        self.manifest = {"files": self.entries}
        self.raw = json.dumps(self.manifest).encode()
        self.args = SimpleNamespace(manifest="fake-manifest", output=str(self.root),
                                    resume=False, include_heldout=False)
        self.patches = [patch.object(d, "load_manifest", return_value=(self.manifest, self.raw)),
                        patch.object(d, "CARD_SHA256", hashlib.sha256(self.card).hexdigest())]
        for item in self.patches:
            item.start()
            self.addCleanup(item.stop)

    def test_default_excludes_holdouts_receipt_provenance_and_verified_resume(self):
        opener = FakeOpener([FakeResponse(raw) for raw in (self.card, *self.data[:2])])
        receipt = d.run(self.args, opener)
        self.assertEqual(receipt["status"], "COMPLETED")
        self.assertEqual(receipt["selected_trace_files"], 2)
        self.assertEqual(receipt["verified_files"], 3)
        self.assertEqual({e["split"] for e in receipt["files"]}, {"license", "train", "validation"})
        self.assertEqual(receipt["source_sha256"], hashlib.sha256(Path(d.__file__).read_bytes()).hexdigest())
        self.assertIn("python_version", receipt)
        self.assertIn("between_bounded_reads", receipt["limits"]["deadline_enforcement"])
        self.assertFalse((self.root / "HAR-5").exists())
        self.assertEqual((self.root / "README.md").read_bytes(), self.card)
        previous = (self.root / "download-receipt.json").read_bytes()
        self.args.resume = True
        resumed = d.run(self.args, FakeOpener([]))
        self.assertEqual(resumed["network_bytes"], 0)
        self.assertTrue(all(r["status"] == "VERIFIED_EXISTING" for r in resumed["files"]))
        self.assertEqual((self.root / resumed["previous_receipt"]["path"]).read_bytes(), previous)
        self.args.include_heldout = True
        opt_in = d.run(self.args, FakeOpener([FakeResponse(raw) for raw in self.data[2:]]))
        self.assertEqual(opt_in["selected_trace_files"], 4)
        self.assertEqual(opt_in["verified_files"], 5)
        self.assertFalse((self.root / ".download.lock").exists())

    def test_interruption_persists_failed_partial_and_releases_lock(self):
        response = FakeResponse(b"", headers={}, events=[b"par", KeyboardInterrupt()])
        with self.assertRaises(KeyboardInterrupt):
            d.run(self.args, FakeOpener([response]))
        receipt = json.loads((self.root / "download-receipt.json").read_bytes())
        self.assertEqual(receipt["status"], "INTERRUPTED")
        self.assertEqual(receipt["verified_files"], 0)
        self.assertEqual(receipt["files"][0]["status"], "FAILED")
        attempt = receipt["files"][0]["attempts"][0]
        self.assertTrue(attempt["partial_retained"])
        self.assertEqual((self.root / attempt["partial_path"]).read_bytes(), b"par")
        self.assertFalse((self.root / "README.md").exists())
        self.assertFalse((self.root / ".download.lock").exists())

    def test_failed_resume_corruption_retains_failure_receipt(self):
        d.run(self.args, FakeOpener([FakeResponse(raw) for raw in (self.card, *self.data[:2])]))
        target = self.root / self.entries[0]["path"]
        target.write_bytes(b"wrong")
        self.args.resume = True
        with self.assertRaisesRegex(d.DownloadError, "existing_file_hash_mismatch"):
            d.run(self.args, FakeOpener([]))
        receipt = json.loads((self.root / "download-receipt.json").read_bytes())
        self.assertEqual(receipt["status"], "FAILED")
        self.assertEqual(receipt["files"][1]["status"], "FAILED")
        self.assertEqual(target.read_bytes(), b"wrong")
        self.assertFalse((self.root / ".download.lock").exists())


if __name__ == "__main__":
    unittest.main()
