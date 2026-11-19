# Dependency compatibility patch

Source: crates.io `wasmtime-internal-cache` 36.0.21.
Published archive SHA-256: `1ff2377df26271c42d9649ac4ee13656ef89a51f02f0daa7917e67c635d9bc1d`.

Executable source, tests, fixtures and licenses are byte-for-byte upstream.
Only the normalized Cargo.toml zstd requirement changes from0.13.0 to0.14.3,
retaining upstream features and the previously reviewed compatibility repair.
Registry bookkeeping, nested lockfile, unconsumed original manifest and upstream
CI files are omitted. No source backport or advisory ignore is added here.

This version matches the compatible Wasmtime36.0.21 core security update.
Remove the override when a compatible parent selects the fixed zstd dependency.
