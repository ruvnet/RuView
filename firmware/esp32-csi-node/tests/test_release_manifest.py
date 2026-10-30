"""Release manifest contracts (RuView#2394) on synthetic bundles, no hardware.

The archives built here mirror what firmware-ci.yml publishes: a flat
`tar -C release-staging -czf <variant>.tar.gz .` bundle with renamed images,
version.txt, flasher_args.json and an inner SHA256SUMS.txt. The image bytes are
synthetic; nothing here qualifies firmware or a board.
"""

import base64
import contextlib
import hashlib
import importlib.util
import io
import json
import re
import tarfile
import tempfile
import unittest
from pathlib import Path


FIRMWARE_ROOT = Path(__file__).resolve().parents[1]
REPOSITORY_ROOT = FIRMWARE_ROOT.parents[1]
TOOL_PATH = FIRMWARE_ROOT / "tools" / "release_manifest.py"
SPEC = importlib.util.spec_from_file_location("release_manifest", TOOL_PATH)
release_manifest = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release_manifest)

try:
    import cryptography  # noqa: F401
    HAVE_CRYPTOGRAPHY = True
except ImportError:  # pragma: no cover - CI installs it
    HAVE_CRYPTOGRAPHY = False

TAG = "v1.2.3-esp32"
COMMIT = "a" * 40
RUN_URL = "https://github.com/ruvnet/RuView/actions/runs/1"

VARIANTS = {
    "c6-4mb": {"chip": "esp32c6", "bootloader": 0x0, "app": "esp32-csi-node-c6.bin",
               "pt": "partition-table-c6.bin"},
    "c5-4mb": {"chip": "esp32c5", "bootloader": 0x2000, "app": "esp32-csi-node-c5.bin",
               "pt": "partition-table-c5.bin"},
}


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def bundle_files(variant: str, version: str = "1.2.3", omit_flasher: bool = False) -> dict:
    spec = VARIANTS[variant]
    files = {
        "bootloader.bin": b"BOOT" + variant.encode() * 64,
        spec["pt"]: b"\xaa\x50" + b"\x01" * 3070,
        "ota_data_initial.bin": b"\xff" * 8192,
        spec["app"]: b"\xe9" + variant.encode() * 4000,
        "version.txt": f"{version}\n".encode(),
    }
    if not omit_flasher:
        files["flasher_args.json"] = json.dumps({
            "extra_esptool_args": {"chip": spec["chip"], "after": "hard_reset"},
            "flash_settings": {"flash_mode": "dio", "flash_size": "4MB", "flash_freq": "80m"},
            "flash_files": {
                hex(spec["bootloader"]): "bootloader/bootloader.bin",
                "0x8000": "partition_table/partition-table.bin",
                "0xf000": "ota_data_initial.bin",
                "0x20000": "esp32-csi-node.bin",
            },
        }, indent=4).encode()
    return files


def inner_sums(files: dict) -> bytes:
    return "".join(f"{sha256(data)}  {name}\n" for name, data in sorted(files.items())).encode()


def write_archive(path: Path, members: list) -> None:
    """members: (name, bytes | None for a directory, optional symlink target)."""
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w") as archive:
        for entry in members:
            name, data = entry[0], entry[1]
            info = tarfile.TarInfo(name)
            info.mtime = 0
            if len(entry) == 3:
                info.type = tarfile.SYMTYPE
                info.linkname = entry[2]
                archive.addfile(info)
            elif data is None:
                info.type = tarfile.DIRTYPE
                archive.addfile(info)
            else:
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
    # Fixed gzip mtime so the fixture archive is byte-stable.
    import gzip
    path.write_bytes(gzip.compress(raw.getvalue(), mtime=0))


def make_release(directory: Path, variants=("c5-4mb", "c6-4mb"), mutate=None) -> Path:
    assets = directory / "assets"
    assets.mkdir()
    for variant in variants:
        files = bundle_files(variant)
        sums = inner_sums(files)
        if mutate:
            files, sums = mutate(variant, files, sums)
        members = [("./", None)] + [(f"./{name}", data) for name, data in sorted(files.items())]
        members.append(("./SHA256SUMS.txt", sums))
        write_archive(assets / f"esp32-csi-node-firmware-{variant}.tar.gz", members)
    write_release_sums(assets)
    return assets


def write_release_sums(assets: Path) -> None:
    lines = [f"{sha256(p.read_bytes())}  {p.name}\n" for p in sorted(assets.glob("*.tar.gz"))]
    (assets / "SHA256SUMS.txt").write_text("".join(lines), encoding="ascii")


def generate(assets: Path, out: Path, *extra: str) -> int:
    return release_manifest.main([
        "generate", "--tag", TAG, "--assets", str(assets), "--channel", "prerelease",
        "--source-commit", COMMIT, "--workflow-run", RUN_URL, "--out", str(out), *extra,
    ])


def quiet(function, *args):
    with contextlib.redirect_stderr(io.StringIO()) as err, contextlib.redirect_stdout(io.StringIO()) as out:
        code = function(*args)
    return code, out.getvalue() + err.getvalue()


class ManifestGenerationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)

    def tearDown(self):
        self.temp.cleanup()

    def test_offsets_come_from_each_bundled_flasher_args(self):
        assets = make_release(self.root)
        out = self.root / "firmware-manifest.json"
        self.assertEqual(quiet(generate, assets, out)[0], 0)
        manifest = json.loads(out.read_text())
        self.assertEqual(manifest["schema"], "ruview.firmware.release-manifest.v1")
        self.assertEqual((manifest["version"], manifest["tag"]), ("1.2.3", TAG))
        self.assertEqual(manifest["sourceCommit"], COMMIT)
        self.assertEqual(manifest["workflowRun"], RUN_URL)
        by_id = {variant["id"]: variant for variant in manifest["variants"]}
        self.assertEqual(sorted(by_id), ["c5-4mb", "c6-4mb"])
        for variant_id, spec in VARIANTS.items():
            variant = by_id[variant_id]
            self.assertEqual((variant["chip"], variant["flashSizeMb"], variant["display"]),
                             (spec["chip"], 4, False))
            images = {image["role"]: image for image in variant["images"]}
            self.assertEqual(images["bootloader"]["offset"], hex(spec["bootloader"]))
            self.assertEqual(images["partition-table"]["offset"], "0x8000")
            self.assertEqual(images["partition-table"]["file"], spec["pt"])
            self.assertEqual(images["ota-data"]["offset"], "0xf000")
            self.assertEqual(images["application"]["offset"], "0x20000")
            self.assertEqual(images["application"]["file"], spec["app"])
            files = bundle_files(variant_id)
            for image in variant["images"]:
                self.assertEqual(image["sha256"], sha256(files[image["file"]]))
                self.assertEqual(image["bytes"], len(files[image["file"]]))
            archive = assets / variant["archive"]["name"]
            self.assertEqual(variant["archive"]["sha256"], sha256(archive.read_bytes()))
            self.assertEqual(variant["archive"]["bytes"], archive.stat().st_size)

    def test_c5_defaults_to_experimental_and_others_to_ci_only(self):
        assets = make_release(self.root)
        out = self.root / "m.json"
        quiet(generate, assets, out)
        qualification = {v["id"]: v["qualification"] for v in json.loads(out.read_text())["variants"]}
        self.assertEqual(qualification, {"c5-4mb": "experimental", "c6-4mb": "ci-only"})

    def test_output_is_deterministic_and_sorted(self):
        assets = make_release(self.root)
        first, second = self.root / "1.json", self.root / "2.json"
        quiet(generate, assets, first, "--revokes", "0.9.0,0.8.15")
        quiet(generate, assets, second, "--revokes", "0.8.15,0.9.0")
        self.assertEqual(first.read_bytes(), second.read_bytes())
        manifest = json.loads(first.read_text())
        self.assertEqual(manifest["revokes"], ["0.8.15", "0.9.0"])
        self.assertEqual([v["id"] for v in manifest["variants"]], ["c5-4mb", "c6-4mb"])
        self.assertEqual(first.read_text(), json.dumps(manifest, indent=2, sort_keys=True) + "\n")

    def test_version_txt_must_equal_tag_version(self):
        def mutate(variant, files, sums):
            files["version.txt"] = b"1.2.4\n"
            return files, inner_sums(files)
        code, output = quiet(generate, make_release(self.root, mutate=mutate), self.root / "m.json")
        self.assertNotEqual(code, 0)
        self.assertIn("version", output)
        self.assertFalse((self.root / "m.json").exists())

    def test_tampered_image_is_refused_by_inner_checksums(self):
        def mutate(variant, files, sums):
            files["bootloader.bin"] = b"evil" + files["bootloader.bin"][4:]
            return files, sums
        code, output = quiet(generate, make_release(self.root, mutate=mutate), self.root / "m.json")
        self.assertNotEqual(code, 0)
        self.assertIn("bootloader.bin", output)

    def test_unlisted_file_in_bundle_is_refused(self):
        def mutate(variant, files, sums):
            files["extra.bin"] = b"x"
            return files, sums
        code, _ = quiet(generate, make_release(self.root, mutate=mutate), self.root / "m.json")
        self.assertNotEqual(code, 0)

    def test_missing_flasher_args_is_refused(self):
        def mutate(variant, files, sums):
            files.pop("flasher_args.json", None)
            return files, inner_sums(files)
        code, output = quiet(generate, make_release(self.root, mutate=mutate), self.root / "m.json")
        self.assertNotEqual(code, 0)
        self.assertIn("flasher_args.json", output)

    def test_flasher_args_chip_must_match_variant(self):
        def mutate(variant, files, sums):
            layout = json.loads(files["flasher_args.json"])
            layout["extra_esptool_args"]["chip"] = "esp32s3"
            files["flasher_args.json"] = json.dumps(layout).encode()
            return files, inner_sums(files)
        code, output = quiet(generate, make_release(self.root, mutate=mutate), self.root / "m.json")
        self.assertNotEqual(code, 0)
        self.assertIn("chip", output)

    def test_overlapping_flash_images_are_refused(self):
        def mutate(variant, files, sums):
            layout = json.loads(files["flasher_args.json"])
            layout["flash_files"].pop("0x20000")
            layout["flash_files"]["0x10000"] = "esp32-csi-node.bin"
            files["flasher_args.json"] = json.dumps(layout).encode()
            return files, inner_sums(files)
        # ota_data_initial (8 KiB at 0xf000) now overlaps the application at 0x10000.
        code, output = quiet(generate, make_release(self.root, mutate=mutate), self.root / "m.json")
        self.assertNotEqual(code, 0)
        self.assertIn("overlap", output)

    def test_unsafe_archive_members_are_refused(self):
        cases = {
            "traversal": [("./", None), ("../escape.bin", b"x")],
            "absolute": [("./", None), ("/etc/passwd", b"x")],
            "symlink": [("./", None), ("./bootloader.bin", None, "/etc/passwd")],
            "nested": [("./", None), ("./sub/bootloader.bin", b"x")],
        }
        for label, members in cases.items():
            with self.subTest(label), tempfile.TemporaryDirectory() as directory:
                assets = Path(directory)
                write_archive(assets / "esp32-csi-node-firmware-c6-4mb.tar.gz", members)
                write_release_sums(assets)
                code, output = quiet(generate, assets, assets / "m.json")
                self.assertNotEqual(code, 0)
                self.assertIn("unsafe", output)

    def test_release_checksum_file_must_cover_each_archive(self):
        assets = make_release(self.root)
        (assets / "SHA256SUMS.txt").write_text(f"{'0' * 64}  esp32-csi-node-firmware-c5-4mb.tar.gz\n")
        code, output = quiet(generate, assets, self.root / "m.json")
        self.assertNotEqual(code, 0)
        self.assertIn("SHA256SUMS", output)

    def test_tag_must_be_semver_esp32_and_revokes_cannot_name_itself(self):
        assets = make_release(self.root)
        for tag in ("1.2.3-esp32", "v1.2-esp32", "v1.2.3", "v1.2.3-esp32;rm"):
            with self.subTest(tag=tag):
                code, _ = quiet(release_manifest.main, [
                    "generate", "--tag", tag, "--assets", str(assets), "--channel", "stable",
                    "--source-commit", COMMIT, "--workflow-run", RUN_URL, "--out", str(self.root / "m.json")])
                self.assertNotEqual(code, 0)
        code, _ = quiet(generate, assets, self.root / "m.json", "--revokes", "1.2.3")
        self.assertNotEqual(code, 0)

    def test_unsigned_verify_requires_explicit_opt_in(self):
        assets = make_release(self.root)
        out = assets / "firmware-manifest.json"
        quiet(generate, assets, out)
        self.assertNotEqual(quiet(release_manifest.main, ["verify", "--manifest", str(out), "--assets", str(assets)])[0], 0)
        code, output = quiet(release_manifest.main, [
            "verify", "--manifest", str(out), "--assets", str(assets), "--allow-unsigned"])
        self.assertEqual(code, 0, output)
        self.assertIn("UNSIGNED", output)


@unittest.skipUnless(HAVE_CRYPTOGRAPHY, "install cryptography to run Ed25519 sign/verify tests")
class ManifestSignatureTests(unittest.TestCase):
    def setUp(self):
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.assets = make_release(self.root)
        self.manifest = self.assets / "firmware-manifest.json"
        self.signature = self.assets / "firmware-manifest.json.sig"
        key = Ed25519PrivateKey.generate()
        raw = key.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw,
                                serialization.NoEncryption())
        self.private_b64 = base64.b64encode(raw).decode()
        self.public_b64 = base64.b64encode(key.public_key().public_bytes(
            serialization.Encoding.Raw, serialization.PublicFormat.Raw)).decode()
        self.key_file = self.root / "signing.key"
        self.key_file.write_text(self.private_b64 + "\n")
        self.pin = self.root / "pinned-public-key.b64"
        self.pin.write_text(self.public_b64 + "\n")
        self.assertEqual(quiet(generate, self.assets, self.manifest)[0], 0)

    def tearDown(self):
        self.temp.cleanup()

    def sign(self):
        return quiet(release_manifest.main, ["sign", "--manifest", str(self.manifest),
                                             "--key-file", str(self.key_file), "--public-key", str(self.pin),
                                             "--out", str(self.signature)])

    def verify(self, public_key=None):
        return quiet(release_manifest.main, [
            "verify", "--manifest", str(self.manifest), "--sig", str(self.signature),
            "--public-key", public_key or self.public_b64, "--assets", str(self.assets)])

    def test_round_trip_and_key_is_never_printed(self):
        code, output = self.sign()
        self.assertEqual(code, 0, output)
        self.assertNotIn(self.private_b64, output)
        self.assertEqual(len(base64.b64decode(self.signature.read_text().strip())), 64)
        code, output = self.verify()
        self.assertEqual(code, 0, output)
        self.assertIn(release_manifest.fingerprint(base64.b64decode(self.public_b64)), output)

    def test_public_key_may_be_a_file(self):
        self.sign()
        key_file = self.root / "public-key.ed25519.b64"
        key_file.write_text(self.public_b64 + "\n")
        self.assertEqual(self.verify(str(key_file))[0], 0)

    def test_tampered_manifest_is_rejected(self):
        self.sign()
        text = self.manifest.read_text().replace('"0x2000"', '"0x0"', 1)
        self.manifest.write_text(text)
        code, output = self.verify()
        self.assertNotEqual(code, 0)
        self.assertIn("signature", output)

    def test_wrong_public_key_is_rejected(self):
        self.sign()
        other = base64.b64encode(bytes(range(32))).decode()
        self.assertNotEqual(self.verify(other)[0], 0)

    def test_signed_manifest_with_wrong_offset_is_rejected_against_bundle(self):
        manifest = json.loads(self.manifest.read_text())
        manifest["variants"][0]["images"][0]["offset"] = "0x0"  # c5 bootloader is really 0x2000
        self.manifest.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
        self.sign()
        code, output = self.verify()
        self.assertNotEqual(code, 0)
        self.assertIn("c5-4mb", output)

    def test_archive_replaced_after_signing_is_rejected(self):
        self.sign()
        def rebuilt_bootloader(variant, files, sums):
            files["bootloader.bin"] = b"rebuilt"
            return files, inner_sums(files)
        other = self.root / "other"
        other.mkdir()
        replacement = make_release(other, variants=("c6-4mb",), mutate=rebuilt_bootloader)
        target = self.assets / "esp32-csi-node-firmware-c6-4mb.tar.gz"
        target.write_bytes((replacement / target.name).read_bytes())
        write_release_sums(self.assets)
        code, output = self.verify()
        self.assertNotEqual(code, 0)
        self.assertIn("c6-4mb", output)

    def test_missing_signing_key_fails_closed(self):
        self.key_file.unlink()
        code, output = self.sign()
        self.assertNotEqual(code, 0)
        self.assertFalse(self.signature.exists())
        self.assertIn(str(self.key_file), output)


class ReleaseWorkflowContractTests(unittest.TestCase):
    workflow = (REPOSITORY_ROOT / ".github/workflows/firmware-ci.yml").read_text(encoding="utf-8")

    def test_bundles_carry_flasher_args_in_inner_checksums(self):
        self.assertTrue("cp build/flasher_args.json" in self.workflow, "staging must copy flasher_args.json")
        staging = self.workflow.split("Stage release binaries", 1)[1].split("- name:", 1)[0]
        self.assertRegex(staging, r"sha256sum[^>]*flasher_args\.json[^>]*> SHA256SUMS\.txt")

    def test_every_build_variant_has_a_manifest_profile(self):
        variants = set(re.findall(r"^\s+- variant: (\S+)$", self.workflow, re.MULTILINE))
        self.assertTrue(variants)
        self.assertEqual(variants, set(release_manifest.VARIANT_PROFILES))

    def publish_job(self) -> str:
        return self.workflow.split("publish-release-assets:", 1)[1]

    def test_publish_job_generates_verifies_and_attests_before_upload(self):
        publish = self.publish_job()
        order = [publish.index(step) for step in (
            "release_manifest.py generate", "release_manifest.py verify",
            "actions/attest-build-provenance@", "softprops/action-gh-release@")]
        self.assertEqual(order, sorted(order))
        self.assertIn("actions/attest-build-provenance@4d101475d8b20a2381f78447822ac1eab6504dd8", publish)
        self.assertIn("id-token: write", publish)
        self.assertIn("attestations: write", publish)
        self.assertIn("--allow-unsigned", publish)

    def test_publish_job_never_holds_private_key_material(self):
        # CI may sign only through Cloud KMS (test_release_manifest_kms.py); it never holds a key.
        publish = self.publish_job()
        self.assertNotIn("environment:", publish)
        self.assertNotIn("secrets.", publish)
        self.assertNotIn("SIGNING_KEY", publish)
        for match in re.finditer(r"release_manifest\.py sign(?![-\w])", publish):
            command = publish[match.start():].split("\n          python", 1)[0].split("\n\n", 1)[0]
            self.assertIn('--kms-key "$KMS_KEY"', command)
        self.assertNotRegex(publish.split("body: |", 1)[0].replace("`release_manifest.py sign-release`", ""),
                            r"sign-release")

    def test_publish_job_is_a_prerelease_unless_stable_and_signed_in_ci(self):
        publish = self.publish_job()
        self.assertNotRegex(publish, r"\n\s+prerelease: (true|false)\n")
        self.assertIn("prerelease: ${{ !(steps.channel.outputs.channel == 'stable' && "
                      "steps.sign.outputs.signed == 'true') }}", publish)

    def test_release_body_explains_unsigned_manifest_and_verification(self):
        body = self.publish_job().split("body: |", 1)[1]
        self.assertIn("UNSIGNED", body)
        self.assertIn("sign-release", body)
        self.assertIn("verify-release", body)
        self.assertIn("gh attestation verify", body)

    def test_firmware_workflows_pin_every_action_by_sha(self):
        for name in ("firmware-ci.yml", "firmware-release-verify.yml"):
            text = (REPOSITORY_ROOT / ".github/workflows" / name).read_text(encoding="utf-8")
            uses = re.findall(r"uses:\s*(\S+)", text)
            self.assertTrue(uses, name)
            for action in uses:
                with self.subTest(workflow=name, action=action):
                    self.assertRegex(action, r"^[\w.-]+/[\w./-]+@[0-9a-f]{40}$")

    def test_release_verify_workflow_is_dispatch_only_and_read_only(self):
        text = (REPOSITORY_ROOT / ".github/workflows/firmware-release-verify.yml").read_text(encoding="utf-8")
        triggers = text.split("\non:", 1)[1].split("\npermissions:", 1)[0]
        self.assertIn("workflow_dispatch:", triggers)
        for event in ("push:", "pull_request", "schedule:", "workflow_run:", "release:"):
            self.assertNotIn(event, triggers)
        permissions = re.findall(r"^\s*([a-z-]+): (read|write|none)$", text, re.MULTILINE)
        self.assertTrue(permissions)
        self.assertEqual({scope for scope, _ in permissions}, {"contents"})
        self.assertEqual({level for _, level in permissions}, {"read"})
        self.assertNotIn("secrets.", text)
        self.assertNotIn("environment:", text)
        self.assertIn("release_manifest.py verify-release", text)
        self.assertIn("persist-credentials: false", text)
        for block in text.split("run: |")[1:]:
            self.assertNotIn("${{", block.split("- name:", 1)[0], "inputs must reach run: via env")

    def test_revoked_versions_file_is_valid_and_wired_into_publish(self):
        text = (FIRMWARE_ROOT / "REVOKED_VERSIONS").read_text(encoding="utf-8")
        entries = [line.split("#", 1)[0].strip() for line in text.splitlines()]
        for entry in filter(None, entries):
            self.assertRegex(entry, r"^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$")
        workflow = (REPOSITORY_ROOT / ".github/workflows/firmware-ci.yml").read_text(encoding="utf-8")
        self.assertIn("firmware/esp32-csi-node/REVOKED_VERSIONS", workflow)
        self.assertIn('--revokes "$REVOKES"', workflow)

    def test_release_channel_file_defaults_to_prerelease(self):
        channel = (FIRMWARE_ROOT / "RELEASE_CHANNEL").read_text(encoding="utf-8").strip()
        self.assertIn(channel, release_manifest.CHANNELS)


if __name__ == "__main__":
    unittest.main()
