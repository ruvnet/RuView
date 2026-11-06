# Dependency compatibility patch

Source: crates.io `wasmtime-internal-cache` 36.0.20.
Published archive SHA-256: `65f09c789d1bb2e7ab1618ac8336d783c1ff0babc38430c7f1940d01c546722b` (verified against the crates.io checksum in the workspace lockfile).

Upstream executable source code and tests are unchanged. Trailing whitespace
in upstream documentation/comments is normalized for clean repository diffs.
The normalized Cargo.toml changes only the
zstd dependency requirement from 0.13.0 to 0.14.3, retaining existing features.
Registry bookkeeping, CI files, and the nested lockfile are omitted.
The upstream license and tests are retained. This preserves the parent API
while using the fixed Zstandard implementation without vendoring ML frameworks.

Remove this patch when the parent releases a compatible dependency update.
