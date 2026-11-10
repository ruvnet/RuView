# Dependency compatibility patch

Source: crates.io `candle-core` 0.9.2.
Published archive SHA-256: `c15b675b80d994b2eadb20a4bbe434eabeb454eac3ee5e2b4cf6f147ee9be091` (verified against crates.io version metadata).

Changes from the published archive:

- The normalized Cargo.toml requirement for `zip` changes from `7.2.0` to
  `9.5.0`, retaining the upstream feature selection.
- `src/npy.rs` handles ZIP 9's fallible, borrowed-or-owned filename API in
  eager and lazy NPZ readers. `src/pickle.rs` collects filename results before
  iterating PTH members. Decode errors propagate through the existing error API.

All other source, tests, fixtures and licenses are byte-for-byte upstream.
Registry bookkeeping, the nested lockfile, the unconsumed original manifest
and upstream CI files are omitted. No advisory suppression is added.

This keeps the Candle 0.9 public API and model operations while selecting the
patched registry dependency. Candle remains a patched dependency, not a workspace
member, to preserve consumer-selected features without adding unused accelerators.
The first-party `cog-pose-estimation/tests/zip_compatibility.rs` tests exercise
eager/lazy NPZ values, Unicode and legacy CP437 names, PTH member enumeration,
and errors from truncated archives on CPU.
Remove this override when a compatible parent release includes the fixed dependency.
