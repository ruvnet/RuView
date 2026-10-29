# Dependency compatibility patch

Source: crates.io `wasmtime-internal-cache` 36.0.19.
Published archive SHA-256: `4108e1964b3a02b4b5604ea1a2918a0cbcab512e5de0ccf4c062fc6c06b2eb8e` (verified against the pre-patch workspace lockfile at `fefb0f918`).

Upstream executable source code and tests are unchanged. Trailing whitespace
in upstream documentation/comments is normalized for clean repository diffs.
The normalized Cargo.toml changes only the
zstd dependency requirement from 0.13.0 to 0.14.3, retaining existing features.
Registry bookkeeping, CI files, and the nested lockfile are omitted.
The upstream license and tests are retained. This preserves the parent API
while using the fixed Zstandard implementation without vendoring ML frameworks.

Remove this patch when the parent releases a compatible dependency update.
