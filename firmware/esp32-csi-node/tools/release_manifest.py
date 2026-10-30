#!/usr/bin/env python3
"""Generate, sign and verify the ESP32 firmware release manifest (RuView#2394).

The manifest binds every published variant archive to its images, their
SHA-256 digests and their flash offsets. Offsets are read from the
`flasher_args.json` that ESP-IDF wrote for that exact build and that the
bundle carries; they are never hand-written here.

generate/verify use only the standard library. sign/verify-with-signature
need the `cryptography` package for Ed25519 and import it lazily.

Signature format: `firmware-manifest.json.sig` holds the base64 Ed25519
signature (64 bytes) over the exact bytes of `firmware-manifest.json`.
Keys are raw 32-byte Ed25519 keys, base64-encoded.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import sys
import tarfile
from pathlib import Path, PurePosixPath

SCHEMA = "ruview.firmware.release-manifest.v1"
CHANNELS = ("stable", "prerelease")
QUALIFICATIONS = ("experimental", "ci-only", "hardware-qualified")
ARCHIVE_PREFIX = "esp32-csi-node-firmware-"
TAG_PATTERN = re.compile(r"^v(\d+\.\d+\.\d+(?:-[0-9A-Za-z][0-9A-Za-z.]*)?)-esp32$")
VERSION_PATTERN = re.compile(r"^\d+\.\d+\.\d+(?:-[0-9A-Za-z][0-9A-Za-z.]*)?$")
MAX_MEMBER_BYTES = 16 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 64

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


def private_key_from_env(name: str):
    module, _ = ed25519()
    value = os.environ.get(name, "")
    if not value.strip():
        raise ManifestError(f"signing key environment variable {name} is empty or unset")
    return module.Ed25519PrivateKey.from_private_bytes(decode_key(value, f"${name}"))


def command_sign(args) -> int:
    key = private_key_from_env(args.key_env)
    signature = key.sign(Path(args.manifest).read_bytes())
    Path(args.out).write_text(base64.b64encode(signature).decode() + "\n", encoding="ascii")
    public_raw = raw_public_bytes(key.public_key())
    if args.public_key_out:
        Path(args.public_key_out).write_text(base64.b64encode(public_raw).decode() + "\n", encoding="ascii")
    print(f"Signed {args.manifest} -> {args.out} with key {fingerprint(public_raw)}")
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
    try:
        manifest = json.loads(manifest_bytes)
    except ValueError as error:
        raise ManifestError(f"manifest is not JSON ({error})") from error
    if manifest.get("schema") != SCHEMA or manifest.get("channel") not in CHANNELS:
        raise ManifestError("manifest schema or channel is not recognised")
    version = tag_version(str(manifest.get("tag")))
    if manifest.get("version") != version or version in manifest.get("revokes", []):
        raise ManifestError("manifest version does not match its tag, or revokes itself")
    actual = {variant["id"]: variant for variant in collect_variants(Path(args.assets), version)}
    claimed = {variant.get("id"): variant for variant in manifest.get("variants", [])}
    if set(actual) != set(claimed):
        raise ManifestError(f"manifest variants {sorted(claimed)} do not match archives {sorted(actual)}")
    for variant_id, entry in actual.items():
        for field in ("chip", "flashSizeMb", "display", "archive", "images"):
            if claimed[variant_id].get(field) != entry[field]:
                raise ManifestError(f"{variant_id}: manifest {field} does not match the bundle")
        if claimed[variant_id].get("qualification") not in QUALIFICATIONS:
            raise ManifestError(f"{variant_id}: unknown qualification")
    print(f"PASS {manifest['tag']} channel={manifest['channel']} variants={sorted(actual)}: {status}")
    return 0


def command_keygen(args) -> int:
    from cryptography.hazmat.primitives import serialization
    module, _ = ed25519()
    key = module.Ed25519PrivateKey.generate()
    private_raw = key.private_bytes(serialization.Encoding.Raw, serialization.PrivateFormat.Raw,
                                    serialization.NoEncryption())
    private_path, public_path = Path(args.private_out), Path(args.public_out)
    descriptor = os.open(private_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "w", encoding="ascii") as stream:
        stream.write(base64.b64encode(private_raw).decode() + "\n")
    public_raw = raw_public_bytes(key.public_key())
    public_path.write_text(base64.b64encode(public_raw).decode() + "\n", encoding="ascii")
    print(f"Private key (0600, keep offline): {private_path}\nPublic key: {public_path}\n"
          f"Fingerprint: {fingerprint(public_raw)}")
    return 0


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

    sign = commands.add_parser("sign", help="write a detached Ed25519 signature")
    sign.add_argument("--manifest", required=True)
    sign.add_argument("--key-env", required=True, help="env var holding the base64 raw private key")
    sign.add_argument("--out", required=True)
    sign.add_argument("--public-key-out", help="also write the signer's public key (not secret)")
    sign.set_defaults(handler=command_sign)

    verify = commands.add_parser("verify", help="check signature, archives, images and offsets")
    verify.add_argument("--manifest", required=True)
    verify.add_argument("--assets", required=True)
    verify.add_argument("--sig")
    verify.add_argument("--public-key", help="base64 raw public key, or a file containing it")
    verify.add_argument("--allow-unsigned", action="store_true")
    verify.set_defaults(handler=command_verify)

    keygen = commands.add_parser("keygen", help="maintainer helper; never run in CI")
    keygen.add_argument("--private-out", required=True)
    keygen.add_argument("--public-out", required=True)
    keygen.set_defaults(handler=command_keygen)
    return parser


def main(argv=None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return args.handler(args)
    except (ManifestError, OSError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
