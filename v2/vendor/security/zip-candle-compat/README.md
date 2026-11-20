# Private Candle ZIP compatibility adapter

This crate satisfies Candle 0.9's `zip ^7.2.0` dependency using the unmodified
registry `zip = 9.0.5`. It contains no ZIP parsing, extraction,
compression, or cryptography implementation. The upstream source and checksum
remain visible as a separate registry package in `Cargo.lock`; the adapter's
version metadata explicitly identifies the ZIP 9 migration.

ZIP 9 makes filename decoding fallible. The archive and entry wrappers validate
that decoding and then preserve Candle's `file_names()` and `name()` access
patterns. Decoding errors propagate when opening the archive or entry; no error
is replaced with an empty name or silently ignored. Other supported operations delegate
to upstream ZIP 9.

The upstream release includes the fixes for RUSTSEC-2026-0545, -0546, -0547
and -0548. Header consistency remains an explicit check in upstream ZIP 9;
upgrading does not automatically make streaming header comparisons strict.
Current Candle consumers use `ZipArchive` entry reads and stored entry writes,
not the streaming extractor. The regression tests execute the strict header
check and reject writes through a symlink escaping the extraction directory.

Only the feature-free API used by the current Candle dependency is supported.
Unsupported feature requests fail dependency resolution rather than silently
changing compression or cryptographic behavior. The repository's Rust toolchain
meets upstream ZIP 9.0.5's Rust 1.88 minimum.

Validate this adapter and its actual consumers with the checked-in lockfile.
Run `cargo test --manifest-path vendor/security/zip-candle-compat/Cargo.toml --locked`
from `v2/`, and the root workspace's Candle consumer tests. The OccWorld consumer
has synthetic NPZ roundtrip, named-entry, and truncated-archive regressions.
The existing ZIP 0.6 compatibility patch remains a separate dependency; no
advisory ignores or security workflow exceptions are added by this adapter.

Remove the adapter after the parent dependencies adopt the patched ZIP major.
