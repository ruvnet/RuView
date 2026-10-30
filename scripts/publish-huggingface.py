#!/usr/bin/env python3
"""
Publish WiFi-DensePose pre-trained models to HuggingFace Hub.

Retrieves the HuggingFace API token from Google Cloud Secrets,
then uploads model files from dist/models/ to a HuggingFace repo.

Prerequisites:
    - gcloud CLI authenticated with access to cognitum-20260110
    - pip install huggingface_hub google-cloud-secret-manager

Usage:
    python scripts/publish-huggingface.py
    python scripts/publish-huggingface.py --repo ruvnet/wifi-densepose-pretrained --version v0.5.4
    python scripts/publish-huggingface.py --dry-run
    python scripts/publish-huggingface.py --token hf_xxxxx  # skip GCloud lookup
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

EXPECTED_FILES = [
    "pretrained-encoder.onnx",
    "pretrained-heads.onnx",
    "pretrained.rvf",
    "room-profiles.json",
    "collection-witness.json",
    "config.json",
    "README.md",
]

# ADR-298 model release gate (issue #1521). The gate judges the classifier
# artifact the publish actually uploads, in this order of preference: the
# `presence-head.json` sidecar, then the `model.safetensors` container that
# carries the same head.
#
# Fix marker: RuView#1521 — the gate runs before the upload and fails closed.
GATE_ARTIFACTS = ("presence-head.json", "model.safetensors")

# The gate binary and how to run it by hand. The workspace lives in `v2/`.
GATE_PACKAGE = "wifi-densepose-train"
GATE_BIN = "model_release_gate"


@dataclass(frozen=True)
class GateResult:
    """The outcome of the ADR-298 release gate for one artifact set."""

    passed: bool
    status: str
    report: str


def repo_root() -> Path:
    """The repository root (this script lives in `scripts/`)."""
    return Path(__file__).resolve().parents[1]


def gate_artifact(model_dir: Path) -> Path | None:
    """The classifier artifact the release gate judges, or None if there is none."""
    for name in GATE_ARTIFACTS:
        candidate = model_dir / name
        if candidate.is_file():
            return candidate
    return None


def run_model_release_gate(
    model_dir: Path,
    runner=subprocess.run,
) -> GateResult:
    """Run the ADR-298 model release gate against the model's classifier artifact.

    The gate is the Rust entry point `model_release_gate`
    (`v2/crates/wifi-densepose-train/src/bin/model_release_gate.rs`), which reads
    a real `.safetensors` container or `presence-head.json` sidecar and applies
    the ADR-298 head gates to it.

    The gate is applied to the published encoder's output, which is L2-normalized
    (`docs/huggingface/MODEL_CARD.md`: "2-layer FC (BatchNorm + GELU) ->
    L2-normalized 128-dim embedding"), which is the assumption issue #1521's
    arithmetic rests on: with a unit-norm embedding the logit is confined to
    `[bias - ||w||, bias + ||w||]`, so a head whose bias dominates its weight
    norm can never output "absent".

    Fails closed: anything that is not an explicit pass blocks the publish.
    """
    artifact = gate_artifact(model_dir)
    if artifact is None:
        return GateResult(
            passed=True,
            status="not_applicable",
            report=(
                "ADR-298 gate: NOT APPLICABLE — the model set carries no "
                f"classifier artifact ({' or '.join(GATE_ARTIFACTS)}); "
                "nothing for the head gates to judge."
            ),
        )

    command = [
        "cargo",
        "run",
        "--quiet",
        "-p",
        GATE_PACKAGE,
        "--no-default-features",
        "--bin",
        GATE_BIN,
        "--",
        "--artifact",
        str(artifact),
        "--l2-normalized-input",
    ]
    workspace = repo_root() / "v2"

    try:
        completed = runner(
            command,
            cwd=str(workspace),
            capture_output=True,
            text=True,
            timeout=1800,
        )
    except FileNotFoundError:
        return GateResult(
            passed=False,
            status="not_evaluated",
            report=(
                "ADR-298 gate: NOT EVALUATED — `cargo` was not found on PATH, so the "
                f"release gate could not run. Run it by hand:\n  {command}"
            ),
        )
    except subprocess.TimeoutExpired:
        return GateResult(
            passed=False,
            status="not_evaluated",
            report=(
                "ADR-298 gate: NOT EVALUATED — the gate did not finish within the "
                f"timeout. Run it by hand:\n  {command}"
            ),
        )

    output = (completed.stdout or "") + (completed.stderr or "")
    output = output.strip()

    if completed.returncode == 0:
        return GateResult(passed=True, status="pass", report=output)
    if completed.returncode == 1:
        return GateResult(
            passed=False,
            status="fail",
            report=(
                "ADR-298 gate: FAIL — the release gate rejected this artifact.\n"
                f"{output}\n"
                "The artifact must not be published in this state (ADR-298)."
            ),
        )
    return GateResult(
        passed=False,
        status="not_evaluated",
        report=(
            "ADR-298 gate: NOT EVALUATED — the gate could not reach a verdict "
            f"(exit {completed.returncode}).\n{output}\n"
            f"Run it by hand:\n  {command}"
        ),
    )


def get_token_from_gcloud(
    project: str = "cognitum-20260110",
    secret: str = "HUGGINGFACE_API_KEY",
) -> str:
    """Retrieve HuggingFace token from Google Cloud Secret Manager."""
    # Try the gcloud CLI first (simpler, no extra deps)
    try:
        result = subprocess.run(
            [
                "gcloud", "secrets", "versions", "access", "latest",
                f"--secret={secret}",
                f"--project={project}",
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    except FileNotFoundError:
        pass  # gcloud not installed, try Python SDK

    # Fall back to the Python SDK
    try:
        from google.cloud import secretmanager

        client = secretmanager.SecretManagerServiceClient()
        name = f"projects/{project}/secrets/{secret}/versions/latest"
        response = client.access_secret_version(request={"name": name})
        return response.payload.data.decode("utf-8").strip()
    except ImportError:
        print(
            "ERROR: Neither gcloud CLI nor google-cloud-secret-manager is available.",
            file=sys.stderr,
        )
        print("Install: pip install google-cloud-secret-manager", file=sys.stderr)
        sys.exit(1)
    except Exception as exc:
        print(f"ERROR: Failed to retrieve secret: {exc}", file=sys.stderr)
        sys.exit(1)


def auto_version() -> str:
    """Detect version from git describe."""
    try:
        result = subprocess.run(
            ["git", "describe", "--tags", "--always"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode == 0:
            return result.stdout.strip()
    except FileNotFoundError:
        pass
    return "dev"


def validate_model_dir(model_dir: Path) -> list[Path]:
    """List available files and warn about missing expected files."""
    found: list[Path] = []
    missing: list[str] = []

    for fname in EXPECTED_FILES:
        path = model_dir / fname
        if path.is_file():
            size = path.stat().st_size
            print(f"  [OK]      {fname} ({size:,} bytes)")
            found.append(path)
        else:
            print(f"  [MISSING] {fname}")
            missing.append(fname)

    # Also pick up any extra files not in the expected list
    for path in sorted(model_dir.iterdir()):
        if path.is_file() and path.name not in EXPECTED_FILES:
            size = path.stat().st_size
            print(f"  [EXTRA]   {path.name} ({size:,} bytes)")
            found.append(path)

    if missing:
        print(f"\nWARNING: {len(missing)} expected file(s) missing.")
        print("Upload will proceed with available files.\n")

    return found


def publish(
    repo_id: str,
    model_dir: Path,
    version: str,
    token: str,
    dry_run: bool = False,
    gate: GateResult | None = None,
) -> None:
    """Upload model files to HuggingFace Hub."""
    # ADR-298 (issue #1521): the artifact must pass the release gate before
    # anything is uploaded. The gate runs first — before the Hub client is
    # even imported — so a rejected artifact never reaches the Hub.
    if gate is None:
        gate = run_model_release_gate(model_dir)
    print(gate.report)
    if not gate.passed:
        print(
            "ERROR: the ADR-298 model release gate did not pass "
            f"(status: {gate.status}). Nothing was published."
        )
        sys.exit(1)

    try:
        from huggingface_hub import HfApi, login
    except ImportError:
        print("Installing huggingface_hub...")
        subprocess.check_call(
            [sys.executable, "-m", "pip", "install", "--quiet", "huggingface_hub"]
        )
        from huggingface_hub import HfApi, login

    print(f"\n{'=' * 60}")
    print(f"Repo:      https://huggingface.co/{repo_id}")
    print(f"Version:   {version}")
    print(f"Model dir: {model_dir}")
    print(f"{'=' * 60}\n")

    print("Validating model files...")
    files = validate_model_dir(model_dir)

    if not files:
        print("ERROR: No files to upload.")
        sys.exit(1)

    if dry_run:
        print(f"\n[DRY RUN] Would upload {len(files)} file(s) to {repo_id}")
        for f in files:
            print(f"  - {f.name}")
        print(f"[DRY RUN] Version tag: {version}")
        return

    print("Authenticating with HuggingFace...")
    login(token=token, add_to_git_credential=False)
    api = HfApi()

    print("Creating repo (if needed)...")
    api.create_repo(
        repo_id=repo_id,
        repo_type="model",
        exist_ok=True,
        private=False,
    )

    print("Uploading files...")
    commit_info = api.upload_folder(
        folder_path=str(model_dir),
        repo_id=repo_id,
        repo_type="model",
        commit_message=f"Upload WiFi-DensePose pretrained models ({version})",
    )

    # Tag
    try:
        api.create_tag(
            repo_id=repo_id,
            repo_type="model",
            tag=version,
            tag_message=f"WiFi-DensePose pretrained models {version}",
        )
        print(f"Tagged as: {version}")
    except Exception as exc:
        print(f"Tag '{version}' may already exist: {exc}")

    print(f"\n{'=' * 60}")
    print("Published successfully!")
    print(f"URL:     https://huggingface.co/{repo_id}")
    print(f"Version: {version}")
    print(f"Commit:  {commit_info.commit_url}")
    print(f"{'=' * 60}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Publish WiFi-DensePose models to HuggingFace Hub",
    )
    parser.add_argument(
        "--repo",
        default="ruvnet/wifi-densepose-pretrained",
        help="HuggingFace repo ID (default: ruvnet/wifi-densepose-pretrained)",
    )
    parser.add_argument(
        "--version",
        default="",
        help="Version tag (default: auto from git describe)",
    )
    parser.add_argument(
        "--model-dir",
        default="dist/models",
        help="Directory containing model files (default: dist/models)",
    )
    parser.add_argument(
        "--project",
        default="cognitum-20260110",
        help="GCloud project ID (default: cognitum-20260110)",
    )
    parser.add_argument(
        "--secret",
        default="HUGGINGFACE_API_KEY",
        help="GCloud secret name (default: HUGGINGFACE_API_KEY)",
    )
    parser.add_argument(
        "--token",
        default="",
        help="HuggingFace token (skip GCloud lookup if provided)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Preview upload without actually uploading",
    )

    args = parser.parse_args()
    model_dir = Path(args.model_dir)
    version = args.version or auto_version()

    if not model_dir.is_dir():
        print(f"ERROR: Model directory does not exist: {model_dir}")
        print("Create it and populate with model files first.")
        sys.exit(1)

    # ADR-298 release gate (issue #1521) — runs before the token lookup, so a
    # rejected artifact never reaches the Hub and a rejected publish never
    # needs a credential.
    print("Running the ADR-298 model release gate (issue #1521)...")
    gate = run_model_release_gate(model_dir)
    print(gate.report)
    if not gate.passed:
        print(
            "ERROR: the ADR-298 model release gate did not pass "
            f"(status: {gate.status}). Nothing was published."
        )
        sys.exit(1)

    # Get token
    if args.dry_run:
        token = "dry-run-no-token-needed"
    elif args.token:
        token = args.token
    else:
        print(f"Retrieving HuggingFace token from GCloud ({args.project})...")
        token = get_token_from_gcloud(project=args.project, secret=args.secret)
        print("Token retrieved.")

    publish(
        repo_id=args.repo,
        model_dir=model_dir,
        version=version,
        token=token,
        dry_run=args.dry_run,
        gate=gate,
    )


if __name__ == "__main__":
    main()
