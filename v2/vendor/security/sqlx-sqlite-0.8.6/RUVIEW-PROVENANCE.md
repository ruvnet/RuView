# Dependency compatibility patch

Source: crates.io `sqlx-sqlite` 0.8.6.
Published archive SHA-256: `c2d12fe70b2c1b4401038055f90f151b78208de1f9f89a7dbfd41587a10c3eea` (verified against crates.io version metadata).

Executable source, tests, fixtures and licenses are byte-for-byte upstream.
Only the normalized Cargo.toml dependency requirement changes:
`libsqlite3-sys` from `0.30.1` to `0.39.1`, retaining the upstream feature selection.
Registry bookkeeping, the nested lockfile, the unconsumed original manifest
and upstream CI files are omitted. No advisory suppression is added.

This preserves the parent API while selecting the patched registry dependency.
Remove this override when a compatible parent release includes the fixed dependency.
