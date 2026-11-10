# Dependency compatibility patch

Source: crates.io `ndarray-npy` 0.10.0.
Published archive SHA-256: `58e8a348bca0075000d999d750420d74434fd0d3e0993b456554f885e7657a11` (verified against crates.io version metadata).

Changes from the published archive:

- The normalized Cargo.toml requirement for `zip` changes from `6` to
  `9.5.0`, retaining the upstream feature selection.
- `src/npz.rs` propagates errors from ZIP 9's filename API and supplies its
  additional `FileOptions` lifetime. Array and compression logic is unchanged.
- `examples/mmap_large.rs` uses compile-time `#[cfg]` blocks instead of
  `cfg!()` branches so the Unix-only metadata import compiles on Windows.
  The large-file example is compiled, not executed, during validation.

All other source, tests, fixtures and licenses are byte-for-byte upstream.
Registry bookkeeping, the nested lockfile, the unconsumed original manifest
and upstream CI files are omitted. No advisory suppression is added.

This preserves the parent API while selecting the patched registry dependency.
Existing integration tests cover compressed/uncompressed NPZ round trips and names.
Remove this override when a compatible parent release includes the fixed dependency.
