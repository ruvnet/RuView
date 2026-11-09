#!/usr/bin/env python3
"""Validate every YAML file under examples/ha-blueprints/.

HA Blueprints use the `!input` YAML tag, which stock PyYAML doesn't
know how to construct. We register a no-op constructor for it so we
can still safe_load the files and assert on their structure.

Exits 0 if all blueprints are well-formed, non-zero otherwise. Intended
to run in CI on every PR that touches examples/ha-blueprints/.

Usage:
    python scripts/validate-ha-blueprints.py
"""

from __future__ import annotations

import glob
import re
import sys
from pathlib import Path

import yaml


class InputTag(str):
    """No-op holder for HA `!input` markers — we don't expand them, just
    verify the file parses."""


def _input_constructor(loader, node):
    return InputTag(loader.construct_scalar(node))


def _secret_constructor(loader, node):
    return f"<!secret {loader.construct_scalar(node)}>"


yaml.SafeLoader.add_constructor("!input", _input_constructor)
yaml.SafeLoader.add_constructor("!secret", _secret_constructor)


REQUIRED_BLUEPRINT_KEYS = {"name", "description", "domain"}

# A blueprint whose primitive the server does not publish must say so in its
# description with this exact phrase, so nobody imports it expecting it to fire.
UNPUBLISHED_MARKER = "Not yet published by the RuView server"
PLANNER = Path("v2/crates/wifi-densepose-sensing-server/src/mqtt/planner/mod.rs")
DISCOVERY = Path("v2/crates/wifi-densepose-sensing-server/src/mqtt/discovery.rs")


def _snake(camel: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", camel).lower()


def unpublished_primitives(root: Path) -> set[str]:
    """Entity kinds the MQTT planner never makes available (no server source).

    Read from the `=> false` arm under the "No source on the sensing
    broadcast" comment in the planner, so this list follows the server.
    """
    text = (root / PLANNER).read_text(encoding="utf-8")
    m = re.search(r"No source on the sensing broadcast.*?\n(.*?)=>\s*false", text, re.S)
    if not m:
        raise ValueError(f"{PLANNER}: could not find the unpublished-entity arm")
    kinds = re.findall(r"EntityKind::(\w+)", m.group(1))
    if not kinds:
        raise ValueError(f"{PLANNER}: unpublished-entity arm lists no EntityKind")
    return {_snake(k) for k in kinds}


def known_primitives(root: Path) -> set[str]:
    text = (root / DISCOVERY).read_text(encoding="utf-8")
    return set(re.findall(r'EntityKind::\w+ => "([a-z_]+)"', text))


def check_publication(description: str, unpublished: set[str], known: set[str]) -> list[str]:
    """The description names the primitives it relies on; check the marker."""
    names = {n for n in known if re.search(rf"\b{n}\b", description)}
    missing_source = sorted(names & unpublished)
    has_marker = UNPUBLISHED_MARKER in description
    if missing_source and not has_marker:
        return [f"relies on {', '.join(missing_source)}, which the server does not publish; "
                f"add '{UNPUBLISHED_MARKER}' to the description"]
    if has_marker and not missing_source:
        return [f"says '{UNPUBLISHED_MARKER}' but every primitive it names is published"]
    return []


ALLOWED_DOMAINS = {"automation", "script"}


def validate(path: Path, unpublished: set[str] | None = None,
             known: set[str] | None = None) -> list[str]:
    """Return a list of issues; empty list means the blueprint is valid."""
    issues: list[str] = []
    try:
        with path.open(encoding="utf-8") as fh:
            doc = yaml.safe_load(fh)
    except yaml.YAMLError as e:
        return [f"YAML parse error: {e}"]
    except OSError as e:
        return [f"could not open: {e}"]

    if not isinstance(doc, dict):
        return ["top-level must be a mapping"]

    bp = doc.get("blueprint")
    if not isinstance(bp, dict):
        issues.append("missing `blueprint` mapping at top level")
        return issues

    missing = REQUIRED_BLUEPRINT_KEYS - bp.keys()
    if missing:
        issues.append(f"missing blueprint keys: {', '.join(sorted(missing))}")

    domain = bp.get("domain")
    if domain not in ALLOWED_DOMAINS:
        issues.append(
            f"unsupported blueprint.domain={domain!r}; allowed: {ALLOWED_DOMAINS}"
        )

    if unpublished is not None and known is not None:
        issues.extend(check_publication(str(bp.get("description", "")), unpublished, known))

    if not isinstance(bp.get("input"), dict) or not bp["input"]:
        issues.append("blueprint.input must declare at least one input")

    # The automation body must contain at least one of: trigger,
    # action, sequence (script body).
    if "trigger" not in doc and "action" not in doc and "sequence" not in doc:
        issues.append(
            "no `trigger`/`action`/`sequence` block — blueprint can't fire"
        )

    return issues


def main() -> int:
    root = Path(__file__).resolve().parent.parent
    files = sorted(glob.glob(str(root / "examples" / "ha-blueprints" / "*.yaml")))
    if not files:
        print("ERROR: no blueprint YAML files found", file=sys.stderr)
        return 2

    try:
        unpublished = unpublished_primitives(root)
        known = known_primitives(root)
    except (OSError, ValueError) as e:
        print(f"ERROR: cannot read the server's published primitives: {e}", file=sys.stderr)
        return 2
    print(f"server does not publish: {', '.join(sorted(unpublished))}")

    fails = 0
    for f in files:
        issues = validate(Path(f), unpublished, known)
        rel = Path(f).relative_to(root)
        if issues:
            fails += 1
            print(f"FAIL  {rel}")
            for i in issues:
                print(f"      {i}")
        else:
            print(f"ok    {rel}")

    if fails:
        print(f"\n{fails} blueprint(s) failed validation", file=sys.stderr)
        return 1
    print(f"\nAll {len(files)} HA Blueprints validate OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
