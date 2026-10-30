"""CI signing with Google Cloud KMS (Ed25519) over Workload Identity Federation (RuView#2394).

No test here reaches Google: every Cloud KMS call goes through an injected HTTP
transport (`FakeKms`) that holds a local Ed25519 key, so signatures are real and
verify with the `cryptography` package. `gcloud` is an injected runner too.
"""

import base64
import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from test_release_manifest import (
    HAVE_CRYPTOGRAPHY, REPOSITORY_ROOT, TAG, generate, make_release, quiet, release_manifest,
)
from test_release_manifest_signing import ENV_NAMES, FakeRunner, new_key

KEY_VERSION = ("projects/ruview-release/locations/global/keyRings/firmware/"
               "cryptoKeys/manifest-signing/cryptoKeyVersions/1")
TOKEN = "fake-oauth.SENSITIVE-ACCESS-TOKEN-value_0123456789"
KMS_BASE = "https://cloudkms.googleapis.com/v1/"


def crc32c(data: bytes) -> int:
    return release_manifest.crc32c(data)


class FakeKms:
    """Cloud KMS REST API stand-in with a real local Ed25519 key."""

    def __init__(self, name=KEY_VERSION, algorithm="EC_SIGN_ED25519"):
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

        self.name = name
        self.algorithm = algorithm
        self.key = Ed25519PrivateKey.generate()
        self.pem = self.key.public_key().public_bytes(
            serialization.Encoding.PEM, serialization.PublicFormat.SubjectPublicKeyInfo).decode()
        raw = self.key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
        self.public_b64 = base64.b64encode(raw).decode()
        self.requests = []
        self.corrupt_signature_crc = False
        self.unverified_data_crc = False
        self.bad_signature = False
        self.corrupt_pem_crc = False
        self.response_name = name
        self.sign_status = 200

    def __call__(self, method, url, headers, body):
        self.requests.append({"method": method, "url": url, "headers": dict(headers),
                              "body": json.loads(body) if body else None})
        if method == "GET" and url == f"{KMS_BASE}{self.name}/publicKey":
            pem_crc = crc32c(self.pem.encode()) ^ (1 if self.corrupt_pem_crc else 0)
            return 200, json.dumps({"name": self.response_name, "pem": self.pem, "algorithm": self.algorithm,
                                    "pemCrc32c": str(pem_crc), "protectionLevel": "HSM"}).encode()
        if method == "POST" and url == f"{KMS_BASE}{self.name}:asymmetricSign":
            if self.sign_status != 200:
                return self.sign_status, json.dumps({"error": {"code": self.sign_status,
                                                              "message": "Permission denied on resource",
                                                              "status": "PERMISSION_DENIED"}}).encode()
            request = json.loads(body)
            data = base64.b64decode(request["data"])
            signature = self.key.sign(data)
            if self.bad_signature:
                signature = self.key.sign(data + b"tampered")
            signature_crc = crc32c(signature) ^ (1 if self.corrupt_signature_crc else 0)
            return 200, json.dumps({
                "name": self.response_name, "signature": base64.b64encode(signature).decode(),
                "signatureCrc32c": str(signature_crc),
                "verifiedDataCrc32c": not self.unverified_data_crc and "dataCrc32c" in request,
                "protectionLevel": "HSM"}).encode()
        return 404, b'{"error": {"code": 404, "message": "not found"}}'


class KmsCase(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        home = self.root / "home"
        home.mkdir()
        patcher = mock.patch.dict(os.environ, {"HOME": str(home)})
        patcher.start()
        self.addCleanup(patcher.stop)
        for name in (*ENV_NAMES, "GOOGLE_OAUTH_ACCESS_TOKEN"):
            os.environ.pop(name, None)
        self.kms = FakeKms()
        self.pin = self.root / "pin.b64"
        self.pin.write_text(self.kms.public_b64 + "\n")
        self.assets = make_release(self.root)
        self.manifest = self.assets / "firmware-manifest.json"
        self.signature = self.assets / "firmware-manifest.json.sig"
        self.assertEqual(quiet(generate, self.assets, self.manifest)[0], 0)

    def run_tool(self, argv, runner=None, http=None):
        return quiet(lambda: release_manifest.main(argv, runner=runner, http=http or self.kms))

    def sign(self, *extra, key=KEY_VERSION, runner=None, token=TOKEN):
        if token is not None:
            os.environ["GOOGLE_OAUTH_ACCESS_TOKEN"] = token
        return self.run_tool(["sign", "--manifest", str(self.manifest), "--out", str(self.signature),
                              "--kms-key", key, "--public-key", str(self.pin), *extra], runner=runner)

    def assert_refused(self, result, *fragments):
        code, output = result
        self.assertNotEqual(code, 0, output)
        self.assertFalse(self.signature.exists(), "no signature may be written on failure")
        self.assertNotIn(TOKEN, output)
        for fragment in fragments:
            self.assertIn(fragment, output)


class Crc32cTests(unittest.TestCase):
    def test_known_vectors(self):
        # RFC 3720 B.4 and the common check value for "123456789".
        self.assertEqual(release_manifest.crc32c(b""), 0)
        self.assertEqual(release_manifest.crc32c(b"123456789"), 0xE3069283)
        self.assertEqual(release_manifest.crc32c(b"\x00" * 32), 0x8A9136AA)
        self.assertEqual(release_manifest.crc32c(b"\xff" * 32), 0x62A8AB43)
        self.assertEqual(release_manifest.crc32c(bytes(range(32))), 0x46DD794E)
        self.assertEqual(release_manifest.crc32c(bytes(range(31, -1, -1))), 0x113FDB5C)


class KmsKeyNameTests(unittest.TestCase):
    def test_accepts_full_key_version_names(self):
        for name in (KEY_VERSION,
                     "projects/my-proj-123/locations/europe-west3/keyRings/r_1/cryptoKeys/k-2/cryptoKeyVersions/42"):
            with self.subTest(name=name):
                self.assertEqual(release_manifest.validate_kms_key_version(name), name)

    def test_rejects_anything_but_a_single_key_version(self):
        for name in ("", "projects/p/locations/global/keyRings/r/cryptoKeys/k",
                     KEY_VERSION.replace("/1", "/0"), KEY_VERSION + "/",
                     KEY_VERSION + ":asymmetricSign", "/" + KEY_VERSION,
                     KEY_VERSION.replace("ruview-release", "Bad_Project"),
                     KEY_VERSION.replace("firmware", "../firmware"),
                     KEY_VERSION.replace("manifest-signing", "a" * 64),
                     KEY_VERSION + "\n", KEY_VERSION + "?alt=json",
                     "https://cloudkms.googleapis.com/v1/" + KEY_VERSION):
            with self.subTest(name=name):
                with self.assertRaises(release_manifest.ManifestError):
                    release_manifest.validate_kms_key_version(name)


@unittest.skipUnless(HAVE_CRYPTOGRAPHY, "install cryptography to run Ed25519 signing tests")
class KmsSignTests(KmsCase):
    def test_signs_raw_manifest_bytes_and_the_signature_verifies(self):
        code, output = self.sign()
        self.assertEqual(code, 0, output)
        self.assertNotIn(TOKEN, output)
        self.assertIn(release_manifest.fingerprint(base64.b64decode(self.kms.public_b64)), output)
        verify = quiet(release_manifest.main, [
            "verify", "--manifest", str(self.manifest), "--assets", str(self.assets),
            "--sig", str(self.signature), "--public-key", str(self.pin)])
        self.assertEqual(verify[0], 0, verify[1])

    def test_requests_public_key_then_asymmetric_sign_with_raw_data_and_crc(self):
        self.assertEqual(self.sign()[0], 0)
        get, post = self.kms.requests
        self.assertEqual((get["method"], get["url"]), ("GET", f"{KMS_BASE}{KEY_VERSION}/publicKey"))
        self.assertEqual((post["method"], post["url"]), ("POST", f"{KMS_BASE}{KEY_VERSION}:asymmetricSign"))
        manifest_bytes = self.manifest.read_bytes()
        # Ed25519 (EC_SIGN_ED25519) signs the raw data, never a digest.
        self.assertEqual(set(post["body"]), {"data", "dataCrc32c"})
        self.assertEqual(base64.b64decode(post["body"]["data"]), manifest_bytes)
        self.assertEqual(int(post["body"]["dataCrc32c"]), crc32c(manifest_bytes))
        for request in (get, post):
            self.assertEqual(request["headers"]["Authorization"], f"Bearer {TOKEN}")

    def test_refuses_a_key_that_is_not_ed25519_before_signing(self):
        self.kms.algorithm = "EC_SIGN_P256_SHA256"
        self.assert_refused(self.sign(), "EC_SIGN_ED25519")
        self.assertEqual([r["method"] for r in self.kms.requests], ["GET"])

    def test_refuses_a_kms_key_that_differs_from_the_pinned_key(self):
        _, _, other_public = new_key()
        self.pin.write_text(other_public + "\n")
        self.assert_refused(self.sign(), "not the pinned key")
        self.assertEqual([r["method"] for r in self.kms.requests], ["GET"])

    def test_missing_pin_requires_allow_unpinned(self):
        self.pin.unlink()
        self.assert_refused(self.sign(), "no pinned public key")
        code, output = self.sign("--allow-unpinned")
        self.assertEqual(code, 0, output)
        self.assertIn("--allow-unpinned", output)

    def test_refuses_a_public_key_with_a_bad_crc(self):
        self.kms.corrupt_pem_crc = True
        self.assert_refused(self.sign(), "CRC32C")

    def test_refuses_a_response_with_a_bad_signature_crc(self):
        self.kms.corrupt_signature_crc = True
        self.assert_refused(self.sign(), "CRC32C")

    def test_refuses_when_kms_did_not_verify_the_data_crc(self):
        self.kms.unverified_data_crc = True
        self.assert_refused(self.sign(), "CRC32C")

    def test_refuses_a_response_for_another_key_version(self):
        self.kms.response_name = KEY_VERSION.replace("/1", "/2")
        self.assert_refused(self.sign(), "key version")

    def test_refuses_a_signature_that_does_not_verify_with_the_pinned_key(self):
        self.kms.bad_signature = True
        self.assert_refused(self.sign(), "does not verify")

    def test_http_error_fails_closed_without_the_token(self):
        self.kms.sign_status = 403
        self.assert_refused(self.sign(), "403")

    def test_invalid_key_name_is_refused_before_any_request(self):
        self.assert_refused(self.sign(key=KEY_VERSION.replace("/1", "/x")), "cryptoKeyVersions")
        self.assertEqual(self.kms.requests, [])

    def test_kms_key_cannot_be_combined_with_a_local_key_source(self):
        key_file = self.root / "k.b64"
        key_file.write_text(new_key()[0])
        self.assert_refused(self.sign("--key-file", str(key_file)), "--kms-key")
        self.assertEqual(self.kms.requests, [])

    def test_token_from_gcloud_when_environment_is_unset(self):
        runner = FakeRunner(gcloud_stdout=(TOKEN + "\n").encode())
        code, output = self.sign(runner=runner, token=None)
        self.assertEqual(code, 0, output)
        self.assertEqual(runner.calls, [["gcloud", "auth", "print-access-token"]])
        self.assertEqual(self.kms.requests[0]["headers"]["Authorization"], f"Bearer {TOKEN}")
        self.assertNotIn(TOKEN, output)

    def test_environment_token_wins_and_gcloud_is_not_called(self):
        runner = FakeRunner(gcloud_stdout=b"other-token")
        self.assertEqual(self.sign(runner=runner)[0], 0)
        self.assertEqual(runner.calls, [])

    def test_gcloud_failure_fails_closed_without_forwarding_its_output(self):
        runner = FakeRunner(gcloud_code=1)
        result = self.sign(runner=runner, token=None)
        self.assert_refused(result, "gcloud")
        self.assertNotIn("SENSITIVE-GCLOUD-DIAGNOSTIC", result[1])
        self.assertEqual(self.kms.requests, [])

    def test_malformed_token_is_refused_without_echo(self):
        self.assert_refused(self.sign(token=TOKEN + "\r\nX-Injected: 1"), "access token")
        self.assertEqual(self.kms.requests, [])

    def test_token_never_appears_in_exception_chain(self):
        os.environ["GOOGLE_OAUTH_ACCESS_TOKEN"] = TOKEN
        self.kms.sign_status = 500
        args = release_manifest.build_parser().parse_args([
            "sign", "--manifest", str(self.manifest), "--out", str(self.signature),
            "--kms-key", KEY_VERSION, "--public-key", str(self.pin)])
        args.runner, args.http = FakeRunner(), self.kms
        with self.assertRaises(release_manifest.ManifestError) as caught:
            release_manifest.command_sign(args)
        error = caught.exception
        while error is not None:
            self.assertNotIn(TOKEN, repr(error))
            self.assertNotIn(TOKEN, repr(getattr(error, "args", "")))
            error = error.__cause__ or error.__context__


@unittest.skipUnless(HAVE_CRYPTOGRAPHY, "install cryptography to run Ed25519 signing tests")
class KmsReleaseTests(KmsCase):
    def test_sign_release_with_kms_then_verify_release_end_to_end(self):
        release = self.root / "release"
        release.mkdir()
        for path in self.assets.iterdir():
            (release / path.name).write_bytes(path.read_bytes())
        runner = FakeRunner(release_dir=release)
        os.environ["GOOGLE_OAUTH_ACCESS_TOKEN"] = TOKEN
        code, output = self.run_tool(["sign-release", "--tag", TAG, "--kms-key", KEY_VERSION,
                                      "--public-key", str(self.pin)], runner=runner)
        self.assertEqual(code, 0, output)
        self.assertNotIn(TOKEN, output)
        self.assertTrue((release / "firmware-manifest.json.sig").is_file())
        code, output = self.run_tool(["verify-release", "--tag", TAG, "--public-key", str(self.pin)],
                                     runner=runner)
        self.assertEqual(code, 0, output)
        self.assertIn("signed and verified", output)

    def test_kms_public_key_prints_raw_base64_and_fingerprint(self):
        os.environ["GOOGLE_OAUTH_ACCESS_TOKEN"] = TOKEN
        out = self.root / "public.b64"
        code, output = self.run_tool(["kms-public-key", "--kms-key", KEY_VERSION, "--out", str(out)])
        self.assertEqual(code, 0, output)
        self.assertEqual(out.read_text().strip(), self.kms.public_b64)
        self.assertIn(self.kms.public_b64, output)
        self.assertIn(release_manifest.fingerprint(base64.b64decode(self.kms.public_b64)), output)
        self.assertNotIn(TOKEN, output)

    def test_public_key_from_pem_converts_the_gcloud_export(self):
        pem = self.root / "kms.pem"
        pem.write_text(self.kms.pem)
        code, output = self.run_tool(["public-key-from-pem", "--pem", str(pem)])
        self.assertEqual(code, 0, output)
        self.assertIn(self.kms.public_b64, output)

    def test_public_key_from_pem_refuses_non_ed25519(self):
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric import ec

        pem = self.root / "p256.pem"
        pem.write_bytes(ec.generate_private_key(ec.SECP256R1()).public_key().public_bytes(
            serialization.Encoding.PEM, serialization.PublicFormat.SubjectPublicKeyInfo))
        code, output = self.run_tool(["public-key-from-pem", "--pem", str(pem)])
        self.assertNotEqual(code, 0)
        self.assertIn("Ed25519", output)


class KmsTransportTests(unittest.TestCase):
    def test_default_transport_uses_https_kms_endpoint_with_timeout(self):
        captured = {}

        class Response:
            status = 200

            def read(self):
                return b"{}"

            def __enter__(self):
                return self

            def __exit__(self, *exc):
                return False

        def fake_urlopen(request, timeout):
            captured.update(url=request.full_url, method=request.get_method(), timeout=timeout,
                            auth=request.get_header("Authorization"))
            return Response()

        with mock.patch.object(release_manifest.urllib.request, "urlopen", fake_urlopen):
            status, body = release_manifest.urllib_transport(
                "GET", f"{KMS_BASE}{KEY_VERSION}/publicKey", {"Authorization": "Bearer t"}, None)
        self.assertEqual((status, body), (200, b"{}"))
        self.assertEqual(captured["method"], "GET")
        self.assertTrue(captured["url"].startswith("https://cloudkms.googleapis.com/v1/projects/"))
        self.assertGreater(captured["timeout"], 0)


class KmsWorkflowContractTests(unittest.TestCase):
    workflow = (REPOSITORY_ROOT / ".github/workflows/firmware-ci.yml").read_text(encoding="utf-8")
    AUTH = "google-github-actions/auth@c200f3691d83b41bf9bbd8638997a462592937ed"

    def publish_job(self) -> str:
        return self.workflow.split("publish-release-assets:", 1)[1]

    def steps(self) -> dict:
        steps = {}
        for chunk in self.publish_job().split("\n      - ")[1:]:
            match = re.match(r"(?:name|uses): (.+)", chunk)
            steps[match.group(1).strip() if match else chunk[:40]] = chunk
        return steps

    def step_with(self, needle: str) -> str:
        matches = [chunk for chunk in self.steps().values() if needle in chunk]
        self.assertEqual(len(matches), 1, f"expected one publish step containing {needle!r}")
        return matches[0]

    def test_auth_step_is_pinned_gated_and_uses_repository_variables(self):
        auth = self.step_with("google-github-actions/auth@")
        self.assertIn(self.AUTH, auth)
        self.assertIn("if: vars.FIRMWARE_SIGNING_KMS_KEY != ''", auth)
        self.assertIn("workload_identity_provider: ${{ vars.FIRMWARE_SIGNING_WIF_PROVIDER }}", auth)
        self.assertIn("service_account: ${{ vars.FIRMWARE_SIGNING_SERVICE_ACCOUNT }}", auth)
        # access_token needs a service account; direct WIF uses the federated auth_token output.
        self.assertIn("token_format: ${{ vars.FIRMWARE_SIGNING_SERVICE_ACCOUNT != '' && 'access_token' || '' }}",
                      auth)
        self.assertIn("create_credentials_file: false", auth)
        self.assertIn("export_environment_variables: false", auth)

    def test_sign_step_is_gated_signs_with_kms_and_verifies_with_the_signature(self):
        sign = self.step_with("release_manifest.py sign ")
        self.assertIn("if: vars.FIRMWARE_SIGNING_KMS_KEY != ''", sign)
        self.assertIn('--kms-key "$KMS_KEY"', sign)
        self.assertIn("KMS_KEY: ${{ vars.FIRMWARE_SIGNING_KMS_KEY }}", sign)
        self.assertIn("GOOGLE_OAUTH_ACCESS_TOKEN: ${{ steps.gcp-auth.outputs.access_token || "
                      "steps.gcp-auth.outputs.auth_token }}", sign)
        self.assertRegex(sign, r"release_manifest\.py verify[^\n]*\\\n[^\n]*--sig release-assets/"
                               r"firmware-manifest\.json\.sig")
        self.assertIn("--public-key firmware/esp32-csi-node/release-signing/public-key.ed25519.b64", sign)
        self.assertIn('echo "signed=true" >> "$GITHUB_OUTPUT"', sign)
        self.assertNotIn("--allow-unpinned", self.publish_job())
        for forbidden in ("--stdin-key", "--key-file", "RUVIEW_FIRMWARE_SIGNING_SECRET", "RUVIEW_FIRMWARE_SIGNING_KEY"):
            self.assertNotIn(forbidden, self.publish_job())

    def test_signing_configuration_check_fails_closed(self):
        check = self.step_with("FIRMWARE_SIGNING_WIF_PROVIDER is not set")
        self.assertIn("if: vars.FIRMWARE_SIGNING_KMS_KEY != ''", check)
        self.assertIn("release-signing/public-key.ed25519.b64", check)
        self.assertGreaterEqual(check.count("exit 1"), 2)

    def test_order_generate_verify_auth_sign_attest_publish(self):
        publish = self.publish_job()
        order = [publish.index(step) for step in (
            "release_manifest.py generate", "--allow-unsigned", "FIRMWARE_SIGNING_WIF_PROVIDER is not set",
            self.AUTH, "release_manifest.py sign ", "--sig release-assets/firmware-manifest.json.sig",
            "actions/attest-build-provenance@", "softprops/action-gh-release@")]
        self.assertEqual(order, sorted(order))

    def test_publish_permissions_and_no_secrets_or_environment(self):
        publish = self.publish_job()
        permissions = publish.split("permissions:", 1)[1].split("steps:", 1)[0]
        for scope in ("contents: write", "id-token: write", "attestations: write"):
            self.assertIn(scope, permissions)
        self.assertNotIn("environment:", publish)
        self.assertNotIn("secrets.", publish)
        self.assertNotIn("FIRMWARE_SIGNING", self.workflow.split("publish-release-assets:", 1)[0])

    def test_prerelease_unless_stable_and_signed_in_ci(self):
        release = self.step_with("softprops/action-gh-release@")
        self.assertIn("prerelease: ${{ !(steps.channel.outputs.channel == 'stable' && "
                      "steps.sign.outputs.signed == 'true') }}", release)
        self.assertIn("id: sign", self.step_with("release_manifest.py sign "))
        self.assertIn("make_latest: false", release)

    def test_release_body_states_signed_or_points_to_the_fallback(self):
        body = self.step_with("softprops/action-gh-release@").split("body: |", 1)[1]
        self.assertIn("steps.sign.outputs.signed == 'true'", body)
        self.assertIn("Cloud KMS", body)
        self.assertIn("UNSIGNED", body)
        self.assertIn("sign-release", body)
        self.assertIn("verify-release", body)

    def test_tokens_are_never_echoed(self):
        for line in self.publish_job().splitlines():
            if "TOKEN" in line and re.search(r"\b(echo|printf|cat)\b", line):
                self.fail(f"publish job prints a token: {line.strip()}")

    def test_readme_documents_kms_first_and_the_tag_ruleset(self):
        readme = (REPOSITORY_ROOT / "firmware/esp32-csi-node/release-signing/README.md").read_text("utf-8")
        kms = readme.index("## CI signing with Cloud KMS (recommended)")
        fallback = readme.index("## Fallback: maintainer-run sign-release")
        self.assertLess(kms, fallback)
        for needle in ("--default-algorithm=ec-sign-ed25519", "--purpose=asymmetric-signing",
                       "assertion.repository == 'ruvnet/RuView'", "assertion.ref_type == 'tag'",
                       "ruvnet/RuView/.github/workflows/firmware-ci.yml@refs/tags/",
                       "roles/cloudkms.signerVerifier", "FIRMWARE_SIGNING_KMS_KEY",
                       "FIRMWARE_SIGNING_WIF_PROVIDER", "FIRMWARE_SIGNING_SERVICE_ACCOUNT", "ruleset"):
            with self.subTest(needle=needle):
                self.assertIn(needle, readme)


if __name__ == "__main__":
    unittest.main()
