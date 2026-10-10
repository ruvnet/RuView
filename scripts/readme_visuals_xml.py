"""Bounded, hardened XML parsing for the README visual maintenance tools."""

from pathlib import Path, PurePosixPath, PureWindowsPath

from defusedxml import ElementTree


# Keep the visual verifier's existing exclusive per-asset size limit.
MAX_SVG_BYTES = 20_000


def resolve_repo_file(root: Path, relative: str, *, asset: bool = False) -> Path:
    """Resolve a portable manifest path inside its trusted repository boundary."""
    if (
        not isinstance(relative, str)
        or not relative
        or "\\" in relative
        or ":" in relative
        or "\x00" in relative
        or PurePosixPath(relative).is_absolute()
        or PureWindowsPath(relative).drive
        or ".." in relative.split("/")
    ):
        raise ValueError("Expected a repository-relative path without traversal")
    repository = root.resolve()
    boundary = repository / "assets" / "readme" if asset else repository
    resolved = (repository / relative).resolve()
    if not resolved.is_relative_to(boundary):
        raise ValueError("Manifest path escapes its allowed repository directory")
    if not resolved.is_file():
        raise ValueError("Manifest path must name an existing file")
    return resolved


def parse_svg(raw: bytes):
    """Reject oversized input and DTD/entity declarations before building a tree."""
    if not isinstance(raw, bytes):
        raise TypeError("SVG input must be bytes")
    if len(raw) >= MAX_SVG_BYTES:
        raise ValueError(f"SVG must be smaller than {MAX_SVG_BYTES} bytes")
    return ElementTree.fromstring(
        raw, forbid_dtd=True, forbid_entities=True, forbid_external=True
    )


def read_svg(path: Path):
    """Read at most the rejection boundary, including when a file grows mid-read."""
    with path.open("rb") as stream:
        raw = stream.read(MAX_SVG_BYTES)
    return raw, parse_svg(raw)
