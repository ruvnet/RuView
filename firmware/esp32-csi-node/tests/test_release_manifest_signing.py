"""Signing outside GitHub (RuView#2394), mirroring ruflo's sign-helpers.mjs.

Key sources, the pinned public key, and the maintainer-run `sign-release` /
anyone-run `verify-release` commands. Every `gh` and `gcloud` call goes through
an injected runner, so nothing here touches GitHub or a GCP project.
"""

import base64
import io
import json
import os
import shutil
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from test_release_manifest import (
    HAVE_CRYPTOGRAPHY, TAG, generate, inner_sums, make_release, quiet, release_manifest, write_release_sums,
)

SECRET_NAME = "ruview-firmware-signing-key"
REPO = "ruvnet/RuView"
ENV_NAMES = ("RUVIEW_FIRMWARE_SIGNING_SECRET", "RUVIEW_FIRMWARE_SIGNING_PROJECT", "RUVIEW_FIRMWARE_SIGNING_KEY")


class FakeRunner:
    """Stands in for `gh` and `gcloud`. A local directory plays the GitHub release."""

    def __init__(self, release_dir=None, gcloud_stdout=b"", gcloud_code=0, upload_code=0):
        self.release_dir = release_dir
        self.gcloud_stdout = gcloud_stdout
        self.gcloud_code = gcloud_code
        self.upload_code = upload_code
        self.prerelease = True
        self.calls = []

    def verbs(self):
        return [" ".join(call[:3]) for call in self.calls]

    def __call__(self, argv):
        argv = list(argv)
        self.calls.append(argv)
        done = lambda code=0, out=b"", err=b"": subprocess.CompletedProcess(argv, code, out, err)  # noqa: E731
        if argv[0] == "gcloud":
            return done(self.gcloud_code, self.gcloud_stdout if self.gcloud_code == 0 else b"",
                        b"ERROR: SENSITIVE-GCLOUD-DIAGNOSTIC")
        verb = argv[:3]
        if verb == ["gh", "release", "view"]:
            assets = [{"name": p.name} for p in sorted(self.release_dir.iterdir())]
            return done(out=json.dumps({"assets": assets, "isPrerelease": self.prerelease}).encode())
        if verb == ["gh", "release", "download"]:
            target = Path(argv[argv.index("--dir") + 1])
            patterns = [argv[i + 1] for i, item in enumerate(argv) if item == "--pattern"]
            for name in patterns:
                shutil.copy(self.release_dir / name, target / name)
            return done()
        if verb == ["gh", "release", "upload"]:
            if self.upload_code:
                return done(self.upload_code, err=b"HTTP 502")
            source = Path(argv[4])
            if (self.release_dir / source.name).exists() and "--clobber" not in argv:
                return done(1, err=b"asset already exists")
            shutil.copy(source, self.release_dir / source.name)
            return done()
        if verb == ["gh", "release", "edit"]:
            if "--prerelease=false" in argv:
                self.prerelease = False
            return done()
        raise AssertionError(f"unexpected command {argv}")


def new_key():
    from cryptography.hazmat.primitives import serialization
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

    key = Ed25519PrivateKey.generate()
    raw = key.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw,
                            serialization.NoEncryption())
    pem = key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
                            serialization.NoEncryption())
    public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    return base64.b64encode(raw).decode(), pem, base64.b64encode(public).decode()


def piped_stdin(data: bytes):
    return io.TextIOWrapper(io.BytesIO(data), encoding="utf-8")


class IsolatedCase(unittest.TestCase):
    """No real HOME, no inherited signing environment."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.home = self.root / "home"
        self.home.mkdir()
        patcher = mock.patch.dict(os.environ, {"HOME": str(self.home)})
        patcher.start()
        self.addCleanup(patcher.stop)
        for name in ENV_NAMES:
            os.environ.pop(name, None)
        self.addCleanup(self.temp.cleanup)

    def run_tool(self, argv, runner=None, stdin=None):
        def call():
            return release_manifest.main(argv, runner=runner)
        if stdin is None:
            return quiet(call)
        with mock.patch("sys.stdin", stdin):
            return quiet(call)


@unittest.skipUnless(HAVE_CRYPTOGRAPHY, "install cryptography to run Ed25519 signing tests")
class SigningKeySourceTests(IsolatedCase):
    def setUp(self):
        super().setUp()
        self.private_b64, self.pem, self.public_b64 = new_key()
        self.assets = make_release(self.root)
        self.manifest = self.assets / "firmware-manifest.json"
        self.signature = self.root / "out.sig"
        self.pin = self.root / "pin.b64"
        self.pin.write_text(self.public_b64 + "\n")
        self.assertEqual(quiet(generate, self.assets, self.manifest)[0], 0)

    def sign(self, *extra, runner=None, stdin=None, pin=True):
        argv = ["sign", "--manifest", str(self.manifest), "--out", str(self.signature), *extra]
        if pin:
            argv += ["--public-key", str(self.pin)]
        return self.run_tool(argv, runner=runner, stdin=stdin)

    def assert_signed(self, result):
        code, output = result
        self.assertEqual(code, 0, output)
        self.assertNotIn(self.private_b64, output)
        self.assertNotIn(self.pem.decode().splitlines()[1], output)
        verify = quiet(release_manifest.main, [
            "verify", "--manifest", str(self.manifest), "--assets", str(self.assets),
            "--sig", str(self.signature), "--public-key", self.public_b64])
        self.assertEqual(verify[0], 0, verify[1])

    def test_default_pin_is_the_committed_public_key(self):
        self.assertEqual(release_manifest.PINNED_PUBLIC_KEY,
                         Path(release_manifest.__file__).resolve().parents[1]
                         / "release-signing" / "public-key.ed25519.b64")

    def test_stdin_key_accepts_pem_and_raw(self):
        self.assert_signed(self.sign("--stdin-key", stdin=piped_stdin(self.pem)))
        self.signature.unlink()
        self.assert_signed(self.sign("--stdin-key", stdin=piped_stdin(self.private_b64.encode() + b"\n")))

    def test_stdin_key_refuses_a_terminal(self):
        terminal = mock.Mock()
        terminal.isatty.return_value = True
        code, output = self.sign("--stdin-key", stdin=terminal)
        self.assertNotEqual(code, 0)
        self.assertIn("pipe", output)
        self.assertFalse(self.signature.exists())

    def test_key_file_accepts_pem_and_raw(self):
        for label, content in (("pem", self.pem), ("raw", self.private_b64.encode())):
            with self.subTest(label):
                key_file = self.root / f"key.{label}"
                key_file.write_bytes(content)
                self.assert_signed(self.sign("--key-file", str(key_file)))
                self.signature.unlink()

    def test_secret_manager_key_is_fetched_with_gcloud_and_never_echoed(self):
        runner = FakeRunner(gcloud_stdout=self.pem)
        os.environ["RUVIEW_FIRMWARE_SIGNING_SECRET"] = SECRET_NAME
        os.environ["RUVIEW_FIRMWARE_SIGNING_PROJECT"] = "example-project"
        before = set(self.root.rglob("*"))
        self.assert_signed(self.sign(runner=runner))
        self.assertEqual(runner.calls, [["gcloud", "secrets", "versions", "access", "latest",
                                         "--secret", SECRET_NAME, "--project", "example-project"]])
        written = set(self.root.rglob("*")) - before
        self.assertEqual(written, {self.signature})

    def test_secret_project_flag_overrides_environment_and_is_optional(self):
        runner = FakeRunner(gcloud_stdout=self.pem)
        os.environ["RUVIEW_FIRMWARE_SIGNING_SECRET"] = SECRET_NAME
        os.environ["RUVIEW_FIRMWARE_SIGNING_PROJECT"] = "ignored"
        self.assert_signed(self.sign("--secret-project", "chosen", runner=runner))
        self.assertEqual(runner.calls[0][-2:], ["--project", "chosen"])
        self.signature.unlink()
        os.environ.pop("RUVIEW_FIRMWARE_SIGNING_PROJECT")
        runner = FakeRunner(gcloud_stdout=self.pem)
        self.assert_signed(self.sign(runner=runner))
        self.assertNotIn("--project", runner.calls[0])

    def test_gcloud_failure_fails_closed_without_forwarding_its_output(self):
        runner = FakeRunner(gcloud_code=1)
        os.environ["RUVIEW_FIRMWARE_SIGNING_SECRET"] = SECRET_NAME
        code, output = self.sign(runner=runner)
        self.assertNotEqual(code, 0)
        self.assertIn(SECRET_NAME, output)
        self.assertNotIn("SENSITIVE-GCLOUD-DIAGNOSTIC", output)
        self.assertFalse(self.signature.exists())

    def test_environment_key_accepts_base64_raw(self):
        os.environ["RUVIEW_FIRMWARE_SIGNING_KEY"] = self.private_b64
        self.assert_signed(self.sign())

    def test_default_key_path_is_under_home(self):
        key_path = self.home / ".ruview" / "firmware-signing.key"
        key_path.parent.mkdir()
        key_path.write_bytes(self.pem)
        self.assert_signed(self.sign())

    def test_sources_resolve_in_documented_order(self):
        _, other_pem, _ = new_key()  # signing with this one would fail the pin
        other_file, key_file = self.root / "other.pem", self.root / "key.pem"
        other_file.write_bytes(other_pem)
        key_file.write_bytes(self.pem)
        os.environ["RUVIEW_FIRMWARE_SIGNING_SECRET"] = SECRET_NAME
        os.environ["RUVIEW_FIRMWARE_SIGNING_KEY"] = "not-a-key"
        # 1. --stdin-key beats --key-file.
        runner = FakeRunner(gcloud_stdout=other_pem)
        self.assert_signed(self.sign("--stdin-key", "--key-file", str(other_file),
                                     runner=runner, stdin=piped_stdin(self.pem)))
        self.signature.unlink()
        # 2. --key-file beats Secret Manager and the environment key.
        self.assert_signed(self.sign("--key-file", str(key_file), runner=runner))
        self.assertEqual(runner.calls, [])
        self.signature.unlink()
        # 3. Secret Manager beats the environment key.
        self.assert_signed(self.sign(runner=FakeRunner(gcloud_stdout=self.pem)))

    def test_missing_key_everywhere_fails_closed(self):
        code, output = self.sign()
        self.assertNotEqual(code, 0)
        self.assertIn("RUVIEW_FIRMWARE_SIGNING_SECRET", output)
        self.assertFalse(self.signature.exists())

    def test_invalid_or_non_ed25519_key_is_refused_without_echo(self):
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric import ec

        ec_pem = ec.generate_private_key(ec.SECP256R1()).private_bytes(
            serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8, serialization.NoEncryption())
        for label, content in (("ec", ec_pem), ("short", base64.b64encode(b"x" * 16)), ("junk", b"%%%")):
            with self.subTest(label):
                key_file = self.root / f"bad.{label}"
                key_file.write_bytes(content)
                code, output = self.sign("--key-file", str(key_file))
                self.assertNotEqual(code, 0)
                self.assertIn("Ed25519", output)
                self.assertNotIn(content.decode().splitlines()[0 if label != "ec" else 1], output)
                self.assertFalse(self.signature.exists())

    def test_key_that_does_not_match_the_pin_is_refused(self):
        _, _, other_public = new_key()
        self.pin.write_text(other_public + "\n")
        os.environ["RUVIEW_FIRMWARE_SIGNING_KEY"] = self.private_b64
        code, output = self.sign()
        self.assertNotEqual(code, 0)
        self.assertIn("pinned", output)
        self.assertFalse(self.signature.exists())
        code, _ = self.sign("--allow-unpinned")
        self.assertNotEqual(code, 0, "--allow-unpinned must not override a mismatched pin")

    def test_missing_pin_requires_loud_allow_unpinned(self):
        os.environ["RUVIEW_FIRMWARE_SIGNING_KEY"] = self.private_b64
        self.pin.unlink()
        code, output = self.sign()
        self.assertNotEqual(code, 0)
        self.assertIn("--allow-unpinned", output)
        self.assertFalse(self.signature.exists())
        code, output = self.sign("--allow-unpinned")
        self.assertEqual(code, 0, output)
        self.assertIn("WARNING", output)

    def test_keygen_writes_private_key_0600_and_prints_public_and_fingerprint(self):
        for fmt in ("pem", "raw"):
            with self.subTest(fmt):
                private, public = self.root / f"k.{fmt}", self.root / f"p.{fmt}.b64"
                code, output = quiet(release_manifest.main, [
                    "keygen", "--format", fmt, "--private-out", str(private), "--public-out", str(public)])
                self.assertEqual(code, 0, output)
                self.assertEqual(stat.S_IMODE(private.stat().st_mode), 0o600)
                public_b64 = public.read_text().strip()
                self.assertIn(public_b64, output)
                self.assertIn(release_manifest.fingerprint(base64.b64decode(public_b64)), output)
                self.assertIn("Secret Manager", output)
                content = private.read_bytes()
                self.assertEqual(content.startswith(b"-----BEGIN PRIVATE KEY-----"), fmt == "pem")
                self.assertNotIn(content.decode().strip().splitlines()[-2 if fmt == "pem" else 0], output)
                key_file_sig = self.root / f"sig.{fmt}"
                code, output = self.run_tool(["sign", "--manifest", str(self.manifest), "--key-file", str(private),
                                              "--public-key", str(public), "--out", str(key_file_sig)])
                self.assertEqual(code, 0, output)


@unittest.skipUnless(HAVE_CRYPTOGRAPHY, "install cryptography to run Ed25519 signing tests")
class ReleaseCommandTests(IsolatedCase):
    def setUp(self):
        super().setUp()
        self.private_b64, self.pem, self.public_b64 = new_key()
        self.pin = self.root / "pin.b64"
        self.pin.write_text(self.public_b64 + "\n")
        self.key_file = self.root / "key.pem"
        self.key_file.write_bytes(self.pem)

    def publish(self, channel="prerelease"):
        """What CI publishes: archives, SHA256SUMS.txt and an unsigned manifest."""
        self.release = make_release(self.root)
        manifest = self.release / "firmware-manifest.json"
        code, output = quiet(release_manifest.main, [
            "generate", "--tag", TAG, "--assets", str(self.release), "--channel", channel,
            "--source-commit", "a" * 40, "--workflow-run", "https://github.com/ruvnet/RuView/actions/runs/1",
            "--out", str(manifest)])
        self.assertEqual(code, 0, output)
        self.runner = FakeRunner(self.release)
        return self.runner

    def sign_release(self, *extra):
        return self.run_tool(["sign-release", "--tag", TAG, "--repo", REPO, "--key-file", str(self.key_file),
                              "--public-key", str(self.pin), *extra], runner=self.runner)

    def verify_release(self, public_key=None):
        return self.run_tool(["verify-release", "--tag", TAG, "--repo", REPO,
                              "--public-key", str(public_key or self.pin)], runner=self.runner)

    def signature(self):
        return self.release / "firmware-manifest.json.sig"

    def test_sign_release_prerelease_downloads_verifies_signs_and_uploads_only(self):
        self.publish("prerelease")
        code, output = self.sign_release()
        self.assertEqual(code, 0, output)
        self.assertEqual(self.runner.verbs(), ["gh release view", "gh release download", "gh release upload"])
        for call in self.runner.calls:
            self.assertEqual(call[3], TAG)
            self.assertEqual(call[call.index("--repo") + 1], REPO)
        download = self.runner.calls[1]
        patterns = sorted(download[i + 1] for i, item in enumerate(download) if item == "--pattern")
        self.assertEqual(patterns, ["SHA256SUMS.txt", "esp32-csi-node-firmware-c5-4mb.tar.gz",
                                    "esp32-csi-node-firmware-c6-4mb.tar.gz", "firmware-manifest.json"])
        self.assertNotIn("--clobber", self.runner.calls[2])
        self.assertTrue(self.signature().exists())
        self.assertTrue(self.runner.prerelease)
        self.assertNotIn(self.private_b64, output)
        code, output = self.verify_release()
        self.assertEqual(code, 0, output)
        self.assertIn("signed and verified", output)

    def test_sign_release_stable_promotes_only_after_upload(self):
        self.publish("stable")
        code, output = self.sign_release()
        self.assertEqual(code, 0, output)
        self.assertEqual(self.runner.verbs(), ["gh release view", "gh release download", "gh release upload",
                                               "gh release edit"])
        self.assertIn("--prerelease=false", self.runner.calls[3])
        self.assertFalse(self.runner.prerelease)

    def test_sign_release_refuses_existing_signature_without_replace(self):
        self.publish("stable")
        self.signature().write_text("old\n")
        code, output = self.sign_release()
        self.assertNotEqual(code, 0)
        self.assertIn("--replace", output)
        self.assertEqual(self.runner.verbs(), ["gh release view"])
        self.assertEqual(self.signature().read_text(), "old\n")
        code, output = self.sign_release("--replace")
        self.assertEqual(code, 0, output)
        self.assertIn("--clobber", self.runner.calls[-2])
        self.assertEqual(self.verify_release()[0], 0)

    def test_sign_release_refuses_a_tampered_archive_before_signing(self):
        self.publish("stable")
        def rebuilt(variant, files, sums):
            files["bootloader.bin"] = b"rebuilt"
            return files, inner_sums(files)
        other = self.root / "other"
        other.mkdir()
        replacement = make_release(other, variants=("c6-4mb",), mutate=rebuilt)
        target = self.release / "esp32-csi-node-firmware-c6-4mb.tar.gz"
        target.write_bytes((replacement / target.name).read_bytes())
        write_release_sums(self.release)
        code, output = self.sign_release()
        self.assertNotEqual(code, 0)
        self.assertIn("c6-4mb", output)
        self.assertEqual(self.runner.verbs(), ["gh release view", "gh release download"])
        self.assertFalse(self.signature().exists())
        self.assertTrue(self.runner.prerelease)

    def test_sign_release_does_not_promote_when_upload_fails(self):
        self.publish("stable")
        self.runner.upload_code = 1
        code, output = self.sign_release()
        self.assertNotEqual(code, 0)
        self.assertIn("upload", output)
        self.assertNotIn("gh release edit", self.runner.verbs())
        self.assertTrue(self.runner.prerelease)

    def test_sign_release_refuses_a_key_that_does_not_match_the_pin(self):
        self.publish("stable")
        _, _, other_public = new_key()
        self.pin.write_text(other_public + "\n")
        code, output = self.sign_release()
        self.assertNotEqual(code, 0)
        self.assertIn("pinned", output)
        self.assertNotIn("gh release upload", self.runner.verbs())

    def test_sign_release_refuses_manifest_for_another_tag(self):
        self.publish("stable")
        manifest = self.release / "firmware-manifest.json"
        manifest.write_text(manifest.read_text().replace(TAG, "v9.9.9-esp32").replace('"1.2.3"', '"9.9.9"'))
        code, output = self.sign_release()
        self.assertNotEqual(code, 0)
        self.assertNotIn("gh release upload", self.runner.verbs())

    def test_verify_release_reports_unsigned_prerelease_as_integrity_only(self):
        self.publish("prerelease")
        code, output = self.verify_release()
        self.assertEqual(code, 0, output)
        self.assertIn("unsigned (integrity only)", output)
        self.assertEqual(self.runner.verbs(), ["gh release view", "gh release download"])

    def test_verify_release_fails_a_stable_unsigned_release(self):
        self.publish("stable")
        code, output = self.verify_release()
        self.assertNotEqual(code, 0)
        self.assertIn("stable", output)

    def test_verify_release_fails_a_signature_from_another_key(self):
        self.publish("stable")
        self.assertEqual(self.sign_release()[0], 0)
        _, _, other_public = new_key()
        other_pin = self.root / "other.b64"
        other_pin.write_text(other_public + "\n")
        code, output = self.verify_release(other_pin)
        self.assertNotEqual(code, 0)
        self.assertIn("signature", output)

    def test_verify_release_fails_signed_release_with_replaced_archive(self):
        self.publish("stable")
        self.assertEqual(self.sign_release()[0], 0)
        target = self.release / "esp32-csi-node-firmware-c5-4mb.tar.gz"
        target.write_bytes(target.read_bytes() + b"\0")
        write_release_sums(self.release)
        self.assertNotEqual(self.verify_release()[0], 0)

    def test_release_commands_validate_tag_before_calling_gh(self):
        self.publish()
        for tag in ("v1.2.3", "v1.2.3-esp32 --repo evil/repo", "-v1.2.3-esp32", "v1.2.3-esp32\n"):
            for argv in (["verify-release", f"--tag={tag}", "--repo", REPO],
                         ["sign-release", f"--tag={tag}", "--repo", REPO, "--key-file", str(self.key_file)]):
                with self.subTest(argv=argv):
                    code, _ = self.run_tool(argv, runner=self.runner)
                    self.assertNotEqual(code, 0)
        self.assertEqual(self.runner.calls, [])

    def test_release_commands_reject_malformed_repo(self):
        self.publish()
        code, _ = self.run_tool(["verify-release", "--tag", TAG, "--repo", "not a repo"], runner=self.runner)
        self.assertNotEqual(code, 0)
        self.assertEqual(self.runner.calls, [])


if __name__ == "__main__":
    unittest.main()
