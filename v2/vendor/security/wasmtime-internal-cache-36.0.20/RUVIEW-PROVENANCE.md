# Dependency compatibility patch

Source: crates.io `wasmtime-internal-cache` 36.0.20.
Published archive SHA-256: `65f09c789d1bb2e7ab1618ac8336d783c1ff0babc38430c7f1940d01c546722b` (verified against crates.io version metadata).

Executable source, tests, fixtures and licenses are byte-for-byte upstream.
Only the normalized Cargo.toml dependency requirement changes:
`zstd` from `0.13.0` to `0.14.3`, retaining the upstream feature selection.
Registry bookkeeping, the nested lockfile, the unconsumed original manifest
and upstream CI files are omitted. No advisory suppression is added.

This preserves the parent API while selecting the patched registry dependency.
Remove this override when a compatible parent release includes the fixed dependency.
