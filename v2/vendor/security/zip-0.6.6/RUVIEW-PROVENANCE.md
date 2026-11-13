# Dependency compatibility patch

Source: crates.io `zip` 0.6.6.
Published archive SHA-256: `760394e246e4c28189f19d488c058bf16f564016aefac5d32bb1f3b51d5e9261` (verified against the pre-patch workspace lockfile at `fefb0f918`).

Upstream executable source code and tests are unchanged. Trailing whitespace
in upstream documentation/comments is normalized for clean repository diffs.
The normalized Cargo.toml changes only the
zstd dependency requirement from 0.11.2 to 0.14.3, retaining existing features.
Registry bookkeeping, CI files, and the nested lockfile are omitted.
The upstream license and tests are retained. This preserves the parent API
while using the fixed Zstandard implementation without vendoring ML frameworks.

Remove this patch when the parent releases a compatible dependency update.
