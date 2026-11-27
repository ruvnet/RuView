"""ADR-117 §7.2 — Unit test for the v1.99.0 tombstone wheel.

Verifies the *file content* of the tombstone module without actually
importing it (importing it would raise ImportError, which is the
behaviour under test). The CI workflow `pip-release.yml` runs the
real end-to-end install + import test inside an ephemeral venv.
"""

from __future__ import annotations

import pathlib

TOMBSTONE = pathlib.Path(__file__).parent.parent / "src" / "wifi_densepose" / "__init__.py"


def test_tombstone_file_exists() -> None:
    assert TOMBSTONE.is_file(), f"tombstone module missing: {TOMBSTONE}"


def test_tombstone_raises_import_error() -> None:
    """The source must call `raise ImportError(...)`. We grep rather
    than exec because actually running it would terminate the test."""
    src = TOMBSTONE.read_text(encoding="utf-8")
    assert "raise ImportError(" in src, "tombstone does not raise ImportError"


def test_tombstone_contains_v2_install_hint() -> None:
    src = TOMBSTONE.read_text(encoding="utf-8")
    assert "pip install wifi-densepose==2.0.0" in src, (
        "tombstone ImportError message must include the v2 pip install hint"
    )


def test_tombstone_contains_migration_url() -> None:
    src = TOMBSTONE.read_text(encoding="utf-8")
    assert "docs/pip-migration.md" in src, (
        "tombstone must point users at the migration guide"
    )


def test_tombstone_is_minimal() -> None:
    """The whole point of the tombstone is that it's MINIMAL — no
    imports, no helper functions, no class definitions. Lock that
    down so a well-intentioned refactor doesn't accidentally bloat it
    into a real module that loads partway before failing."""
    src = TOMBSTONE.read_text(encoding="utf-8")
    forbidden = ("def ", "class ", "import wifi_densepose", "import os", "import sys")
    for f in forbidden:
        assert f not in src, f"tombstone must not contain {f!r} — it should ONLY raise"
