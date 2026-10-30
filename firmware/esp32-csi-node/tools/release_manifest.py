#!/usr/bin/env python3
"""Generate, sign and verify the ESP32 firmware release manifest (RuView#2394).

The manifest binds every published variant archive to its images, their
SHA-256 digests and their flash offsets. Offsets are read from the
`flasher_args.json` that ESP-IDF wrote for that exact build and that the
bundle carries; they are never hand-written here.

Signing (see release-signing/README.md):
- Main path: CI signs with a Google Cloud KMS Ed25519 key (`--kms-key`),
  authenticated by Workload Identity Federation. The private key never leaves
  KMS; CI holds only a short-lived token allowed to call asymmetricSign.
- Fallback: a maintainer runs `sign-release` (same `--kms-key`, or a key from
  GCP Secret Manager / a local file), which verifies the published release,
  signs, uploads the signature and, for the stable channel, clears the
  prerelease flag.

generate/verify use only the standard library. Signing and signature checks
need the `cryptography` package for Ed25519 and import it lazily.

Signature format: `firmware-manifest.json.sig` holds the base64 Ed25519
signature (64 bytes) over the exact bytes of `firmware-manifest.json`.
Public keys are base64 of the raw 32 bytes. Private keys are PEM PKCS8
Ed25519 or base64 of the raw 32 bytes.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
from pathlib import Path, PurePosixPath

SCHEMA = "ruview.firmware.release-manifest.v1"
CHANNELS = ("stable", "prerelease")
QUALIFICATIONS = ("experimental", "ci-only", "hardware-qualified")
ARCHIVE_PREFIX = "esp32-csi-node-firmware-"
TAG_PATTERN = re.compile(r"^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z][0-9A-Za-z.]*)?)-esp32$")
VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z][0-9A-Za-z.]*)?$")
MAX_MEMBER_BYTES = 16 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 64
MANIFEST_NAME = "firmware-manifest.json"
SIGNATURE_NAME = MANIFEST_NAME + ".sig"
RELEASE_SUMS_NAME = "SHA256SUMS.txt"
RELEASE_ARCHIVE_PATTERN = re.compile(r"^esp32-csi-node-firmware-[0-9a-z-]+\.tar\.gz$")
DEFAULT_REPO = "ruvnet/RuView"
REPO_PATTERN = re.compile(r"^[A-Za-z0-9][A-Za-z0-9-]*/[A-Za-z0-9._-]+$")

# Signing key sources (see read_signing_key_bytes for the order).
PINNED_PUBLIC_KEY = Path(__file__).resolve().parents[1] / "release-signing" / "public-key.ed25519.b64"
SECRET_ENV = "RUVIEW_FIRMWARE_SIGNING_SECRET"
PROJECT_ENV = "RUVIEW_FIRMWARE_SIGNING_PROJECT"
KEY_ENV = "RUVIEW_FIRMWARE_SIGNING_KEY"
DEFAULT_KEY_RELATIVE = Path(".ruview") / "firmware-signing.key"
DEFAULT_SECRET_NAME = "ruview-firmware-signing-key"
SECRET_NAME_PATTERN = re.compile(r"^[A-Za-z0-9_-]{1,255}$")
PROJECT_PATTERN = re.compile(r"^[a-z][a-z0-9.:-]{1,98}[a-z0-9]$")

# Cloud KMS (main CI signing path). Only a single key VERSION is accepted.
KMS_ENDPOINT = "https://cloudkms.googleapis.com/v1/"
KMS_KEY_PATTERN = re.compile(
    r"projects/(?:[a-z][a-z0-9.-]{0,61}[a-z0-9]:)?[a-z][a-z0-9-]{4,28}[a-z0-9]"
    r"/locations/[a-z][a-z0-9-]{0,62}"
    r"/keyRings/[A-Za-z0-9_-]{1,63}"
    r"/cryptoKeys/[A-Za-z0-9_-]{1,63}"
    r"/cryptoKeyVersions/[1-9][0-9]{0,18}")
KMS_ALGORITHM = "EC_SIGN_ED25519"
TOKEN_ENV = "GOOGLE_OAUTH_ACCESS_TOKEN"
TOKEN_PATTERN = re.compile(r"[A-Za-z0-9._~+/=-]{16,4096}")
HTTP_TIMEOUT_SECONDS = 30

# One entry per firmware-ci.yml build matrix `variant` (a test keeps them in sync).
VARIANT_PROFILES = {
    "8mb": {"chip": "esp32s3", "display": True, "qualification": "ci-only"},
    "4mb": {"chip": "esp32s3", "display": False, "qualification": "ci-only"},
    "c6-4mb": {"chip": "esp32c6", "display": False, "qualification": "ci-only"},
    "c5-4mb": {"chip": "esp32c5", "display": False, "qualification": "experimental"},
}

# flasher_args.json `flash_files` basename -> manifest role and bundled file pattern.
ROLES = {
    "bootloader.bin": ("bootloader", re.compile(r"^bootloader\.bin$")),
    "partition-table.bin": ("partition-table", re.compile(r"^partition-table(-[0-9a-z-]+)?\.bin$")),
    "ota_data_initial.bin": ("ota-data", re.compile(r"^ota_data_initial\.bin$")),
    "esp32-csi-node.bin": ("application", re.compile(r"^esp32-csi-node(-[0-9a-z-]+)?\.bin$")),
}


class ManifestError(Exception):
    """Any condition under which a release must not be published or flashed."""


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def parse_sums(text: str, source: str) -> dict:
    sums = {}
    for line in text.splitlines():
        if not line.strip():
            continue
        match = re.fullmatch(r"([0-9a-f]{64}) [ *]([^/\\\s]+)", line.strip())
        if not match or match.group(2) in sums:
            raise ManifestError(f"{source}: malformed or duplicate line {line!r}")
        sums[match.group(2)] = match.group(1)
    return sums


def read_bundle(path: Path) -> dict:
    """Read a flat variant archive into memory, refusing anything but plain files."""
    files = {}
    try:
        with tarfile.open(path, mode="r:gz") as archive:
            members = archive.getmembers()
            if len(members) > MAX_ARCHIVE_MEMBERS:
                raise ManifestError(f"{path.name}: unsafe archive with {len(members)} members")
            for member in members:
                name = member.name
                parts = PurePosixPath(name).parts
                if name.startswith("/") or ".." in parts:
                    raise ManifestError(f"{path.name}: unsafe member path {name!r}")
                if member.isdir() and name in (".", "./"):
                    continue
                if not member.isfile():
                    raise ManifestError(f"{path.name}: unsafe member type for {name!r}")
                relative = [part for part in parts if part != "."]
                if len(relative) != 1:
                    raise ManifestError(f"{path.name}: unsafe nested member {name!r}")
                if relative[0] in files:
                    raise ManifestError(f"{path.name}: duplicate member {relative[0]!r}")
                if member.size > MAX_MEMBER_BYTES:
                    raise ManifestError(f"{path.name}: unsafe oversized member {name!r}")
                stream = archive.extractfile(member)
                files[relative[0]] = stream.read() if stream else b""
    except (tarfile.TarError, OSError, EOFError) as error:
        raise ManifestError(f"{path.name}: unreadable archive ({error})") from error
    return files


def verify_inner_sums(name: str, files: dict) -> None:
    if "SHA256SUMS.txt" not in files:
        raise ManifestError(f"{name}: missing inner SHA256SUMS.txt")
    sums = parse_sums(files["SHA256SUMS.txt"].decode("ascii", "replace"), f"{name}/SHA256SUMS.txt")
    listed, present = set(sums), set(files) - {"SHA256SUMS.txt"}
    if listed != present:
        raise ManifestError(f"{name}: SHA256SUMS.txt lists {sorted(listed)} but bundle has {sorted(present)}")
    for filename, digest in sums.items():
        if sha256_hex(files[filename]) != digest:
            raise ManifestError(f"{name}: {filename} does not match its inner SHA256SUMS.txt entry")


def flash_size_mb(value) -> int:
    match = re.fullmatch(r"(\d+)MB", str(value))
    if not match:
        raise ManifestError(f"flash_settings.flash_size {value!r} is not an explicit size")
    return int(match.group(1))


def describe_variant(archive: Path, version: str) -> dict:
    """Inspect one archive and return its manifest entry, or raise ManifestError."""
    name = archive.name
    if not name.startswith(ARCHIVE_PREFIX) or not name.endswith(".tar.gz"):
        raise ManifestError(f"{name}: unexpected archive name")
    variant_id = name[len(ARCHIVE_PREFIX):-len(".tar.gz")]
    files = read_bundle(archive)
    profile = VARIANT_PROFILES.get(variant_id)
    if profile is None:
        raise ManifestError(f"{name}: unknown variant {variant_id!r}; add it to VARIANT_PROFILES")
    verify_inner_sums(name, files)
    bundled_version = files.get("version.txt", b"").decode("utf-8", "replace").strip()
    if bundled_version != version:
        raise ManifestError(f"{name}: version.txt {bundled_version!r} does not match tag version {version!r}")
    if "flasher_args.json" not in files:
        raise ManifestError(f"{name}: missing flasher_args.json; offsets cannot be derived")
    try:
        layout = json.loads(files["flasher_args.json"])
    except ValueError as error:
        raise ManifestError(f"{name}: flasher_args.json is not JSON ({error})") from error
    chip = (layout.get("extra_esptool_args") or {}).get("chip")
    if chip != profile["chip"]:
        raise ManifestError(f"{name}: flasher_args.json chip {chip!r} does not match {profile['chip']!r}")
    images = []
    for offset_text, built_path in sorted((layout.get("flash_files") or {}).items()):
        basename = PurePosixPath(str(built_path)).name
        if basename not in ROLES:
            raise ManifestError(f"{name}: flasher_args.json has unknown flash file {built_path!r}")
        role, pattern = ROLES[basename]
        candidates = [f for f in files if pattern.match(f)]
        if len(candidates) != 1:
            raise ManifestError(f"{name}: expected one bundled {role} image, found {candidates}")
        try:
            offset = int(str(offset_text), 0)
        except ValueError as error:
            raise ManifestError(f"{name}: invalid flash offset {offset_text!r}") from error
        data = files[candidates[0]]
        images.append({"role": role, "file": candidates[0], "offset": offset,
                       "sha256": sha256_hex(data), "bytes": len(data)})
    roles = sorted(image["role"] for image in images)
    if roles != sorted(role for role, _ in ROLES.values()):
        raise ManifestError(f"{name}: flasher_args.json roles {roles} are incomplete or duplicated")
    images.sort(key=lambda image: image["offset"])
    size_mb = flash_size_mb((layout.get("flash_settings") or {}).get("flash_size"))
    end = 0
    for image in images:
        if image["offset"] % 0x1000 or image["offset"] < end:
            raise ManifestError(f"{name}: {image['role']} at {hex(image['offset'])} is unaligned or overlaps")
        end = image["offset"] + image["bytes"]
    if end > size_mb * 1024 * 1024:
        raise ManifestError(f"{name}: images overlap the end of {size_mb} MB flash")
    for image in images:
        image["offset"] = hex(image["offset"])
    data = archive.read_bytes()
    return {
        "id": variant_id,
        "chip": chip,
        "flashSizeMb": size_mb,
        "display": profile["display"],
        "archive": {"name": name, "sha256": sha256_hex(data), "bytes": len(data)},
        "images": images,
        "qualification": profile["qualification"],
    }


def collect_variants(assets: Path, version: str) -> list:
    archives = sorted(assets.glob("*.tar.gz"))
    if not archives:
        raise ManifestError(f"{assets}: no *.tar.gz archives")
    sums_path = assets / "SHA256SUMS.txt"
    if not sums_path.is_file():
        raise ManifestError(f"{assets}: missing release SHA256SUMS.txt")
    sums = parse_sums(sums_path.read_text(encoding="ascii", errors="replace"), "SHA256SUMS.txt")
    for archive in archives:
        if sums.get(archive.name) != sha256_hex(archive.read_bytes()):
            raise ManifestError(f"release SHA256SUMS.txt does not match {archive.name}")
    if set(sums) != {archive.name for archive in archives}:
        raise ManifestError("release SHA256SUMS.txt lists archives that are not present")
    return sorted((describe_variant(archive, version) for archive in archives), key=lambda v: v["id"])


def tag_version(tag: str) -> str:
    match = TAG_PATTERN.fullmatch(tag)
    if not match:
        raise ManifestError(f"tag {tag!r} is not v<semver>-esp32")
    return match.group(1)


def render(manifest: dict) -> bytes:
    return (json.dumps(manifest, indent=2, sort_keys=True) + "\n").encode("utf-8")


def key_value_pairs(values, label: str, allowed=None) -> dict:
    result = {}
    for item in values or []:
        key, sep, value = item.partition("=")
        if not sep or not key or not value or (allowed and value not in allowed):
            raise ManifestError(f"--{label} expects KEY=VALUE, got {item!r}")
        result[key] = value
    return result


def command_generate(args) -> int:
    version = tag_version(args.tag)
    if not re.fullmatch(r"[0-9a-f]{40}", args.source_commit):
        raise ManifestError("--source-commit must be a full 40-character lowercase commit SHA")
    if not args.workflow_run.startswith("https://"):
        raise ManifestError("--workflow-run must be an https URL")
    revokes = sorted({item.strip() for item in (args.revokes or "").split(",") if item.strip()})
    for revoked in revokes:
        if not VERSION_PATTERN.fullmatch(revoked) or revoked == version:
            raise ManifestError(f"--revokes entry {revoked!r} is invalid or names this release")
    variants = collect_variants(Path(args.assets), version)
    overrides = key_value_pairs(args.qualification, "qualification", QUALIFICATIONS)
    for variant in variants:
        variant["qualification"] = overrides.pop(variant["id"], variant["qualification"])
    if overrides:
        raise ManifestError(f"--qualification names unknown variants {sorted(overrides)}")
    manifest = {
        "schema": SCHEMA,
        "version": version,
        "tag": args.tag,
        "channel": args.channel,
        "sourceCommit": args.source_commit,
        "workflowRun": args.workflow_run,
        "minClient": key_value_pairs(args.min_client, "min-client"),
        "revokes": revokes,
        "variants": variants,
    }
    if args.next_public_key:
        manifest["nextPublicKey"] = base64.b64encode(load_public_key(args.next_public_key)).decode()
    Path(args.out).write_bytes(render(manifest))
    print(f"Wrote {args.out}: {args.tag} channel={args.channel} variants={[v['id'] for v in variants]}")
    return 0


def ed25519():
    try:
        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives.asymmetric import ed25519 as module
    except ImportError as error:
        raise ManifestError("Ed25519 needs the 'cryptography' package: pip install cryptography") from error
    return module, InvalidSignature


def raw_public_bytes(public_key) -> bytes:
    from cryptography.hazmat.primitives import serialization
    return public_key.public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)


def decode_key(text: str, label: str) -> bytes:
    try:
        raw = base64.b64decode(text.strip(), validate=True)
    except ValueError as error:
        raise ManifestError(f"{label} is not valid base64") from error
    if len(raw) != 32:
        raise ManifestError(f"{label} must decode to 32 bytes")
    return raw


def load_public_key(value: str) -> bytes:
    path = Path(value)
    text = path.read_text(encoding="ascii") if path.is_file() else value
    return decode_key(text, "public key")


def fingerprint(public_raw: bytes) -> str:
    return "SHA256:" + sha256_hex(public_raw)


# --- Private key sources --------------------------------------------------
# Mirrors ruflo's v3/@claude-flow/cli/scripts/sign-helpers.mjs: the private key
# lives outside GitHub (GCP Secret Manager or a maintainer file) and CI only
# verifies. Key material is never printed, logged, passed in argv, or written
# to disk by this tool, and diagnostics never echo it.

def run_command(argv):
    """Default runner for gh/gcloud: no shell, no stdin, output captured (never inherited)."""
    return subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, check=False)


def parse_private_key(data: bytes, source: str):
    """Accept a PEM PKCS8 Ed25519 key or base64 of the raw 32-byte private key."""
    module, _ = ed25519()
    from cryptography.hazmat.primitives import serialization

    text = data.strip()
    if not text:
        raise ManifestError(f"signing key from {source} is empty")
    invalid = ManifestError(f"signing key from {source} is not a valid Ed25519 private key "
                            "(expected PEM PKCS8 or base64 of the raw 32 bytes)")
    try:
        if text.startswith(b"-----BEGIN"):
            key = serialization.load_pem_private_key(text, password=None)
        else:
            raw = base64.b64decode(text, validate=True)
            if len(raw) != 32:
                raise invalid
            key = module.Ed25519PrivateKey.from_private_bytes(raw)
    except ManifestError:
        raise
    except Exception:  # noqa: BLE001 - parser errors may quote their input; never chain them
        raise invalid from None
    if not isinstance(key, module.Ed25519PrivateKey):
        raise invalid
    return key


def read_secret_manager_key(secret: str, project: str, runner) -> bytes:
    if not SECRET_NAME_PATTERN.fullmatch(secret):
        raise ManifestError(f"${SECRET_ENV} {secret!r} is not a Secret Manager secret name")
    argv = ["gcloud", "secrets", "versions", "access", "latest", "--secret", secret]
    if project:
        if not PROJECT_PATTERN.fullmatch(project):
            raise ManifestError(f"GCP project {project!r} is not a project id")
        argv += ["--project", project]
    try:
        result = runner(argv)
    except OSError:
        raise ManifestError(f"could not run gcloud to read secret {secret!r}; "
                            "install the Google Cloud SDK or use --key-file/--stdin-key") from None
    if result.returncode != 0 or not result.stdout:
        # gcloud's own stderr is deliberately not forwarded.
        raise ManifestError(f"failed to read GCP secret {secret!r} (gcloud exit {result.returncode}); "
                            "is gcloud authenticated (gcloud auth login) with access to it?")
    return result.stdout


def read_signing_key_bytes(args, runner):
    """Resolve the private key: --stdin-key, --key-file, Secret Manager, env key, default file."""
    if args.stdin_key:
        if sys.stdin is None or sys.stdin.isatty():
            raise ManifestError("--stdin-key needs piped input, for example "
                                "`gcloud secrets versions access latest --secret=NAME | ... --stdin-key`; "
                                "refusing interactive key entry")
        return sys.stdin.buffer.read(), "--stdin-key"
    if args.key_file:
        path = Path(args.key_file).expanduser()
        if not path.is_file():
            raise ManifestError(f"--key-file {path} does not exist")
        return path.read_bytes(), str(path)
    secret = os.environ.get(SECRET_ENV, "").strip()
    if secret:
        project = (args.secret_project or os.environ.get(PROJECT_ENV, "")).strip()
        return read_secret_manager_key(secret, project, runner), f"Secret Manager secret {secret!r}"
    env_key = os.environ.get(KEY_ENV, "")
    if env_key.strip():
        return env_key.encode("ascii", "replace"), f"${KEY_ENV}"
    default = Path.home() / DEFAULT_KEY_RELATIVE
    if default.is_file():
        return default.read_bytes(), str(default)
    raise ManifestError(f"no signing key: pass --stdin-key or --key-file PATH, set {SECRET_ENV}=<secret name> "
                        f"(GCP Secret Manager, optional {PROJECT_ENV}), set {KEY_ENV}=<base64 raw key>, "
                        f"or create {default}")


# --- Cloud KMS signing (main path) ----------------------------------------
# The private key is generated in and never leaves Cloud KMS. Requests carry
# CRC32C checksums end to end, the KMS public key must equal the committed
# pin, and every signature is verified locally before it is written. The
# OAuth access token is only ever placed in the Authorization header: it is
# never printed, logged, put in argv, or included in an error message.

def _crc32c_table() -> list:
    table = []
    for byte in range(256):
        crc = byte
        for _ in range(8):
            crc = (crc >> 1) ^ 0x82F63B78 if crc & 1 else crc >> 1
        table.append(crc)
    return table


CRC32C_TABLE = _crc32c_table()


def crc32c(data: bytes) -> int:
    """CRC-32C (Castagnoli), as Cloud KMS uses for request/response integrity."""
    crc = 0xFFFFFFFF
    for byte in data:
        crc = CRC32C_TABLE[(crc ^ byte) & 0xFF] ^ (crc >> 8)
    return crc ^ 0xFFFFFFFF


def validate_kms_key_version(name: str) -> str:
    if not isinstance(name, str) or not KMS_KEY_PATTERN.fullmatch(name):
        raise ManifestError("--kms-key must be a full key version name: projects/P/locations/L/"
                            "keyRings/R/cryptoKeys/K/cryptoKeyVersions/N")
    return name


def urllib_transport(method: str, url: str, headers: dict, body):
    """Default HTTPS transport: returns (status, body bytes); never raises for HTTP status."""
    request = urllib.request.Request(url, data=body, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=HTTP_TIMEOUT_SECONDS) as response:
            return response.status, response.read()
    except urllib.error.HTTPError as error:
        return error.code, error.read()
    except (urllib.error.URLError, OSError) as error:
        reason = getattr(error, "reason", None) or type(error).__name__
        raise ManifestError(f"could not reach Cloud KMS ({reason})") from None


def kms_access_token(runner) -> str:
    """$GOOGLE_OAUTH_ACCESS_TOKEN (set from the auth action in CI), else `gcloud auth print-access-token`."""
    token = os.environ.get(TOKEN_ENV, "").strip()
    if not token:
        try:
            result = runner(["gcloud", "auth", "print-access-token"])
        except OSError:
            raise ManifestError(f"no ${TOKEN_ENV} and could not run gcloud; install the Google Cloud SDK "
                                "and run `gcloud auth login`") from None
        if result.returncode != 0 or not result.stdout:
            # gcloud's own stderr is deliberately not forwarded.
            raise ManifestError(f"gcloud auth print-access-token failed (exit {result.returncode}); "
                                "is gcloud authenticated (gcloud auth login)?")
        token = result.stdout.decode("ascii", "replace").strip()
    if not TOKEN_PATTERN.fullmatch(token):
        raise ManifestError("the Google Cloud access token is malformed; refusing to send it")
    return token


def kms_call(http, method: str, url: str, token: str, payload=None) -> dict:
    headers = {"Authorization": f"Bearer {token}", "Accept": "application/json"}
    body = None
    if payload is not None:
        headers["Content-Type"] = "application/json"
        body = json.dumps(payload).encode("utf-8")
    action = url.rsplit("/", 1)[-1].split(":")[-1]
    status, raw = http(method, url, headers, body)
    if status != 200:
        message = ""
        try:
            message = str(json.loads(raw)["error"]["message"])[:300]
        except (ValueError, KeyError, TypeError):
            pass
        raise ManifestError(f"Cloud KMS {action} failed (HTTP {status}){': ' + message if message else ''}; "
                            "does this identity hold roles/cloudkms.signerVerifier on the key?")
    try:
        data = json.loads(raw)
    except ValueError:
        raise ManifestError(f"Cloud KMS {action} returned a non-JSON response") from None
    if not isinstance(data, dict):
        raise ManifestError(f"Cloud KMS {action} returned an unexpected response")
    return data


def crc_matches(value, data: bytes) -> bool:
    try:
        return int(str(value)) == crc32c(data)
    except ValueError:
        return False


def kms_public_key(http, token: str, name: str) -> bytes:
    """Fetch the key version's public key; require Ed25519 and return the raw 32 bytes."""
    data = kms_call(http, "GET", f"{KMS_ENDPOINT}{name}/publicKey", token)
    if data.get("name") != name:
        raise ManifestError("Cloud KMS returned the public key of another key version")
    if data.get("algorithm") != KMS_ALGORITHM:
        raise ManifestError(f"Cloud KMS key algorithm is {data.get('algorithm')!r}, expected {KMS_ALGORITHM}")
    pem = str(data.get("pem", "")).encode("ascii", "replace")
    if not crc_matches(data.get("pemCrc32c"), pem):
        raise ManifestError("Cloud KMS public key failed its CRC32C check; retry")
    return public_key_from_pem_bytes(pem, "Cloud KMS")


def kms_sign(http, token: str, name: str, manifest_bytes: bytes) -> bytes:
    """asymmetricSign over the raw bytes (Ed25519 signs data, not a digest), with CRC32C both ways."""
    data = kms_call(http, "POST", f"{KMS_ENDPOINT}{name}:asymmetricSign", token, {
        "data": base64.b64encode(manifest_bytes).decode("ascii"),
        "dataCrc32c": str(crc32c(manifest_bytes)),
    })
    if data.get("name") != name:
        raise ManifestError("Cloud KMS signed with another key version than requested")
    if data.get("verifiedDataCrc32c") is not True:
        raise ManifestError("Cloud KMS did not verify the request CRC32C; the data may be corrupted in transit")
    try:
        signature = base64.b64decode(str(data.get("signature", "")), validate=True)
    except ValueError:
        raise ManifestError("Cloud KMS returned a signature that is not base64") from None
    if not crc_matches(data.get("signatureCrc32c"), signature):
        raise ManifestError("Cloud KMS signature failed its CRC32C check; retry")
    if len(signature) != 64:
        raise ManifestError("Cloud KMS returned a signature that is not 64-byte Ed25519")
    return signature


def public_key_from_pem_bytes(pem: bytes, source: str) -> bytes:
    module, _ = ed25519()
    from cryptography.hazmat.primitives import serialization
    try:
        key = serialization.load_pem_public_key(pem)
    except Exception:  # noqa: BLE001
        raise ManifestError(f"public key from {source} is not a valid PEM public key") from None
    if not isinstance(key, module.Ed25519PublicKey):
        raise ManifestError(f"public key from {source} is not Ed25519")
    return raw_public_bytes(key)


def expected_public_key(public_raw: bytes, pin: Path, allow_unpinned: bool) -> bytes:
    """Refuse any key other than the committed one; unpinned signing is first-time setup only."""
    if pin.is_file():
        pinned = load_public_key(str(pin))
        if pinned != public_raw:
            raise ManifestError(f"signing key {fingerprint(public_raw)} is not the pinned key "
                                f"{fingerprint(pinned)} in {pin}; refusing to sign")
        return pinned
    if not allow_unpinned:
        raise ManifestError(f"no pinned public key at {pin}; commit it first, "
                            "or pass --allow-unpinned for first-time setup only")
    print("WARNING: " + "!" * 60 + f"\nWARNING: --allow-unpinned: {pin} does not exist, so nothing checks that "
          f"this is the project key.\nWARNING: signing with {fingerprint(public_raw)}. Commit its public key "
          "before publishing anything signed.\nWARNING: " + "!" * 60, file=sys.stderr)
    return public_raw


def sign_bytes(args, manifest_bytes: bytes) -> tuple:
    """Sign and self-verify against the pinned key. Returns (signature, public key, fingerprint), base64."""
    module, invalid = ed25519()
    if args.kms_key:
        if args.stdin_key or args.key_file:
            raise ManifestError("--kms-key cannot be combined with --stdin-key or --key-file")
        name = validate_kms_key_version(args.kms_key)
        token = kms_access_token(args.runner)
        public_raw = expected_public_key(kms_public_key(args.http, token, name), Path(args.public_key),
                                         args.allow_unpinned)
        signature = kms_sign(args.http, token, name, manifest_bytes)
        del token
    else:
        data, source = read_signing_key_bytes(args, args.runner)
        key = parse_private_key(data, source)
        del data
        public_raw = expected_public_key(raw_public_bytes(key.public_key()), Path(args.public_key),
                                         args.allow_unpinned)
        signature = key.sign(manifest_bytes)
    try:
        module.Ed25519PublicKey.from_public_bytes(public_raw).verify(signature, manifest_bytes)
    except invalid:
        raise ManifestError("signature does not verify against the pinned key; nothing written") from None
    return base64.b64encode(signature).decode(), base64.b64encode(public_raw).decode(), fingerprint(public_raw)


def command_sign(args) -> int:
    signature, _, key_fingerprint = sign_bytes(args, Path(args.manifest).read_bytes())
    Path(args.out).write_text(signature + "\n", encoding="ascii")
    print(f"Signed {args.manifest} -> {args.out} with key {key_fingerprint}")
    return 0


def verify_signature(manifest_bytes: bytes, signature_path: Path, public_value: str) -> str:
    module, invalid = ed25519()
    public_raw = load_public_key(public_value)
    try:
        signature = base64.b64decode(signature_path.read_text(encoding="ascii").strip(), validate=True)
        module.Ed25519PublicKey.from_public_bytes(public_raw).verify(signature, manifest_bytes)
    except (invalid, ValueError, OSError) as error:
        raise ManifestError(f"manifest signature is invalid for key {fingerprint(public_raw)}") from error
    return fingerprint(public_raw)


def verify_manifest(manifest_bytes: bytes, assets: Path) -> dict:
    """Check the manifest against the archives: hashes, inner sums, offsets, version."""
    try:
        manifest = json.loads(manifest_bytes)
    except ValueError as error:
        raise ManifestError(f"manifest is not JSON ({error})") from error
    if not isinstance(manifest, dict) or manifest.get("schema") != SCHEMA or manifest.get("channel") not in CHANNELS:
        raise ManifestError("manifest schema or channel is not recognised")
    version = tag_version(str(manifest.get("tag")))
    if manifest.get("version") != version or version in manifest.get("revokes", []):
        raise ManifestError("manifest version does not match its tag, or revokes itself")
    actual = {variant["id"]: variant for variant in collect_variants(assets, version)}
    claimed = {variant.get("id"): variant for variant in manifest.get("variants", [])}
    if set(actual) != set(claimed):
        raise ManifestError(f"manifest variants {sorted(claimed)} do not match archives {sorted(actual)}")
    for variant_id, entry in actual.items():
        for field in ("chip", "flashSizeMb", "display", "archive", "images"):
            if claimed[variant_id].get(field) != entry[field]:
                raise ManifestError(f"{variant_id}: manifest {field} does not match the bundle")
        if claimed[variant_id].get("qualification") not in QUALIFICATIONS:
            raise ManifestError(f"{variant_id}: unknown qualification")
    return manifest


def command_verify(args) -> int:
    manifest_bytes = Path(args.manifest).read_bytes()
    if args.sig and args.public_key:
        status = f"signature OK ({verify_signature(manifest_bytes, Path(args.sig), args.public_key)})"
    elif args.sig or args.public_key:
        raise ManifestError("--sig and --public-key must be given together")
    elif args.allow_unsigned:
        status = "UNSIGNED (integrity and offsets only; origin not proven)"
    else:
        raise ManifestError("no signature given; pass --sig and --public-key, or --allow-unsigned")
    manifest = verify_manifest(manifest_bytes, Path(args.assets))
    variants = sorted(variant["id"] for variant in manifest["variants"])
    print(f"PASS {manifest['tag']} channel={manifest['channel']} variants={variants}: {status}")
    return 0


# --- GitHub release commands (maintainer machine or read-only CI) ---------

def gh(runner, argv: list, action: str):
    try:
        result = runner(argv)
    except OSError as error:
        raise ManifestError(f"could not run gh for {action} ({error}); install the GitHub CLI") from None
    if result.returncode != 0:
        detail = (result.stderr or b"").decode("utf-8", "replace").strip()[-400:]
        raise ManifestError(f"gh {action} failed (exit {result.returncode}): {detail}")
    return result


def release_target(args) -> tuple:
    tag_version(args.tag)
    if not REPO_PATTERN.fullmatch(args.repo):
        raise ManifestError(f"--repo {args.repo!r} is not OWNER/NAME")
    return args.tag, args.repo


def release_assets(runner, tag: str, repo: str) -> tuple:
    result = gh(runner, ["gh", "release", "view", tag, "--repo", repo, "--json", "assets,isPrerelease"],
                "release view")
    try:
        data = json.loads(result.stdout)
        names = {str(asset["name"]) for asset in data.get("assets", [])}
    except (ValueError, KeyError, TypeError, AttributeError) as error:
        raise ManifestError(f"gh release view returned unexpected output ({error})") from error
    return names, bool(data.get("isPrerelease"))


def download_release(runner, tag: str, repo: str, names: set, directory: Path, with_signature: bool) -> None:
    for required in (RELEASE_SUMS_NAME, MANIFEST_NAME):
        if required not in names:
            raise ManifestError(f"{tag} has no {required}")
    wanted = sorted(name for name in names if RELEASE_ARCHIVE_PATTERN.fullmatch(name))
    if not wanted:
        raise ManifestError(f"{tag} has no {ARCHIVE_PREFIX}*.tar.gz archives")
    wanted += [RELEASE_SUMS_NAME, MANIFEST_NAME]
    if with_signature and SIGNATURE_NAME in names:
        wanted.append(SIGNATURE_NAME)
    argv = ["gh", "release", "download", tag, "--repo", repo, "--dir", str(directory)]
    for name in wanted:
        argv += ["--pattern", name]
    gh(runner, argv, "release download")
    missing = [name for name in wanted if not (directory / name).is_file()]
    if missing:
        raise ManifestError(f"gh release download did not produce {missing}")


def checked_release_manifest(directory: Path, tag: str) -> tuple:
    manifest_bytes = (directory / MANIFEST_NAME).read_bytes()
    manifest = verify_manifest(manifest_bytes, directory)
    if manifest["tag"] != tag:
        raise ManifestError(f"release {tag} carries a manifest for {manifest['tag']}")
    return manifest_bytes, manifest


def command_sign_release(args) -> int:
    tag, repo = release_target(args)
    names, _ = release_assets(args.runner, tag, repo)
    if SIGNATURE_NAME in names and not args.replace:
        raise ManifestError(f"{tag} already has {SIGNATURE_NAME}; pass --replace to overwrite it")
    with tempfile.TemporaryDirectory(prefix="ruview-sign-release-") as temporary:
        directory = Path(temporary)
        download_release(args.runner, tag, repo, names, directory, with_signature=False)
        manifest_bytes, manifest = checked_release_manifest(directory, tag)
        signature, public_b64, key_fingerprint = sign_bytes(args, manifest_bytes)
        signature_path = directory / SIGNATURE_NAME
        signature_path.write_text(signature + "\n", encoding="ascii")
        # Re-read the exact file that will be uploaded and check it against the expected key.
        verify_signature(manifest_bytes, signature_path, public_b64)
        upload = ["gh", "release", "upload", tag, str(signature_path), "--repo", repo]
        gh(args.runner, upload + (["--clobber"] if args.replace else []), "release upload")
    print(f"Signed {tag} manifest with {key_fingerprint} and uploaded {SIGNATURE_NAME}")
    if manifest["channel"] == "stable":
        gh(args.runner, ["gh", "release", "edit", tag, "--repo", repo, "--prerelease=false"], "release edit")
        print(f"{tag}: channel stable, marked as a full (non-prerelease) release")
    else:
        print(f"{tag}: channel {manifest['channel']}, left as a prerelease")
    return 0


def command_verify_release(args) -> int:
    tag, repo = release_target(args)
    names, prerelease = release_assets(args.runner, tag, repo)
    with tempfile.TemporaryDirectory(prefix="ruview-verify-release-") as temporary:
        directory = Path(temporary)
        download_release(args.runner, tag, repo, names, directory, with_signature=True)
        signature_path = directory / SIGNATURE_NAME
        signed_by = None
        if signature_path.is_file():
            if not Path(args.public_key).is_file():
                raise ManifestError(f"{tag} is signed but there is no public key at {args.public_key}")
            signed_by = verify_signature((directory / MANIFEST_NAME).read_bytes(), signature_path,
                                         args.public_key)
        _, manifest = checked_release_manifest(directory, tag)
    channel = manifest["channel"]
    variants = sorted(variant["id"] for variant in manifest["variants"])
    if signed_by:
        print(f"PASS {tag} channel={channel} prerelease={prerelease} variants={variants}: "
              f"signed and verified ({signed_by})")
        if channel == "stable" and prerelease:
            print(f"NOTE: {tag} is signed for stable but still marked prerelease; "
                  "the maintainer's sign-release run did not finish promotion")
        return 0
    if channel == "stable" or not prerelease:
        raise ManifestError(f"{tag} is unsigned but published as stable (channel={channel}, "
                            f"prerelease={prerelease}); a stable release must carry a valid signature")
    print(f"PASS {tag} channel={channel} prerelease={prerelease} variants={variants}: "
          "unsigned (integrity only); origin is not proven until the maintainer runs sign-release")
    return 0


def print_public_key(public_raw: bytes, out) -> None:
    public_b64 = base64.b64encode(public_raw).decode()
    if out:
        Path(out).write_text(public_b64 + "\n", encoding="ascii")
    print(f"Public key: {public_b64}\nFingerprint: {fingerprint(public_raw)}"
          + (f"\nWrote {out}" if out else "")
          + "\nCommit it as firmware/esp32-csi-node/release-signing/public-key.ed25519.b64")


def command_kms_public_key(args) -> int:
    name = validate_kms_key_version(args.kms_key)
    print_public_key(kms_public_key(args.http, kms_access_token(args.runner), name), args.out)
    return 0


def command_public_key_from_pem(args) -> int:
    print_public_key(public_key_from_pem_bytes(Path(args.pem).read_bytes(), args.pem), args.out)
    return 0


def command_keygen(args) -> int:
    from cryptography.hazmat.primitives import serialization
    module, _ = ed25519()
    key = module.Ed25519PrivateKey.generate()
    if args.format == "pem":
        private = key.private_bytes(serialization.Encoding.PEM, serialization.PrivateFormat.PKCS8,
                                    serialization.NoEncryption())
    else:
        raw = key.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw,
                                serialization.NoEncryption())
        private = base64.b64encode(raw) + b"\n"
    private_path, public_path = Path(args.private_out), Path(args.public_out)
    descriptor = os.open(private_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as stream:
        stream.write(private)
    public_raw = raw_public_bytes(key.public_key())
    public_b64 = base64.b64encode(public_raw).decode()
    public_path.write_text(public_b64 + "\n", encoding="ascii")
    print(f"Private key ({args.format}, mode 0600): {private_path}\n"
          f"Public key: {public_b64}\nPublic key file: {public_path}\nFingerprint: {fingerprint(public_raw)}\n"
          "Next: store the private key in GCP Secret Manager, then delete the local file:\n"
          f"  gcloud secrets create {DEFAULT_SECRET_NAME} --project=<project> "
          f"--replication-policy=automatic --data-file={private_path}\n"
          f"  rm {private_path}\n"
          f"and commit {public_path.name} as firmware/esp32-csi-node/release-signing/public-key.ed25519.b64")
    return 0


def add_key_options(parser) -> None:
    parser.add_argument("--kms-key", metavar="KEY_VERSION",
                        help="sign with Cloud KMS: projects/P/locations/L/keyRings/R/cryptoKeys/K/"
                             f"cryptoKeyVersions/N (token from ${TOKEN_ENV} or `gcloud auth print-access-token`)")
    parser.add_argument("--stdin-key", action="store_true",
                        help="read the private key from piped stdin (first source checked)")
    parser.add_argument("--key-file", help="private key file: PEM PKCS8 Ed25519 or base64 raw")
    parser.add_argument("--secret-project",
                        help=f"GCP project for ${SECRET_ENV} (default: ${PROJECT_ENV}, then gcloud's active project)")
    parser.add_argument("--public-key", default=str(PINNED_PUBLIC_KEY),
                        help="pinned public key the signing key must match (default: the committed key)")
    parser.add_argument("--allow-unpinned", action="store_true",
                        help="first-time setup only: sign although no pinned public key exists")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    commands = parser.add_subparsers(dest="command", required=True)

    generate = commands.add_parser("generate", help="build firmware-manifest.json from release assets")
    generate.add_argument("--tag", required=True)
    generate.add_argument("--assets", required=True, help="directory with *.tar.gz and SHA256SUMS.txt")
    generate.add_argument("--channel", required=True, choices=CHANNELS)
    generate.add_argument("--source-commit", required=True)
    generate.add_argument("--workflow-run", required=True)
    generate.add_argument("--revokes", help="comma-separated firmware versions clients must refuse")
    generate.add_argument("--min-client", action="append", metavar="CLIENT=SEMVER")
    generate.add_argument("--qualification", action="append", metavar="VARIANT=LEVEL")
    generate.add_argument("--next-public-key", help="announce the next signing key (rotation)")
    generate.add_argument("--out", required=True)
    generate.set_defaults(handler=command_generate)

    sign = commands.add_parser("sign", help="write a detached Ed25519 signature (CI with --kms-key, or maintainer)")
    sign.add_argument("--manifest", required=True)
    sign.add_argument("--out", required=True)
    add_key_options(sign)
    sign.set_defaults(handler=command_sign)

    verify = commands.add_parser("verify", help="check signature, archives, images and offsets")
    verify.add_argument("--manifest", required=True)
    verify.add_argument("--assets", required=True)
    verify.add_argument("--sig")
    verify.add_argument("--public-key", help="base64 raw public key, or a file containing it")
    verify.add_argument("--allow-unsigned", action="store_true")
    verify.set_defaults(handler=command_verify)

    sign_release = commands.add_parser(
        "sign-release", help="download a published release, verify it, sign its manifest, upload the .sig")
    sign_release.add_argument("--tag", required=True)
    sign_release.add_argument("--repo", default=DEFAULT_REPO)
    sign_release.add_argument("--replace", action="store_true", help="overwrite an existing signature")
    add_key_options(sign_release)
    sign_release.set_defaults(handler=command_sign_release)

    verify_release = commands.add_parser("verify-release", help="download and verify a published release")
    verify_release.add_argument("--tag", required=True)
    verify_release.add_argument("--repo", default=DEFAULT_REPO)
    verify_release.add_argument("--public-key", default=str(PINNED_PUBLIC_KEY))
    verify_release.set_defaults(handler=command_verify_release)

    kms_public = commands.add_parser("kms-public-key",
                                     help="print a Cloud KMS Ed25519 key version's raw public key and fingerprint")
    kms_public.add_argument("--kms-key", required=True, metavar="KEY_VERSION")
    kms_public.add_argument("--out", help="also write the base64 public key to this file")
    kms_public.set_defaults(handler=command_kms_public_key)

    from_pem = commands.add_parser("public-key-from-pem",
                                   help="convert a PEM Ed25519 public key (e.g. gcloud kms ... get-public-key)")
    from_pem.add_argument("--pem", required=True)
    from_pem.add_argument("--out", help="also write the base64 public key to this file")
    from_pem.set_defaults(handler=command_public_key_from_pem)

    keygen = commands.add_parser("keygen", help="maintainer helper; never run in CI")
    keygen.add_argument("--private-out", required=True)
    keygen.add_argument("--public-out", required=True)
    keygen.add_argument("--format", choices=("pem", "raw"), default="pem",
                        help="private key encoding: PEM PKCS8 (default) or base64 raw")
    keygen.set_defaults(handler=command_keygen)
    return parser


def main(argv=None, runner=None, http=None) -> int:
    args = build_parser().parse_args(argv)
    args.runner = runner or run_command
    args.http = http or urllib_transport
    try:
        return args.handler(args)
    except (ManifestError, OSError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
