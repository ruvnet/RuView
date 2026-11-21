# Dependency compatibility patch

Source: crates.io `wasmtime-internal-cache` 36.0.21.
Published archive SHA-256: `1ff2377df26271c42d9649ac4ee13656ef89a51f02f0daa7917e67c635d9bc1d` (verified against crates.io version metadata).

Executable source, tests, fixtures and licenses are byte-for-byte upstream.
Only the normalized Cargo.toml dependency requirement changes:
`zstd` from `0.13.0` to `0.14.3`, retaining the upstream feature selection.
Registry bookkeeping, the nested lockfile, the unconsumed original manifest
and upstream CI files are omitted. No advisory suppression is added.

This preserves the parent API while selecting the patched registry dependency.
Remove this override when a compatible parent release includes the fixed dependency.
