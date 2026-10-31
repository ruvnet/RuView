# Dependency compatibility patch

Source: crates.io `wasmtime-internal-cache`36.0.20.
Published archive SHA-256: `65f09c789d1bb2e7ab1618ac8336d783c1ff0babc38430c7f1940d01c546722b` (verified against the local crates.io index checksum).

Executable source and tests are byte-identical to the36.0.20 archive; its source
also matches the previous36.0.19 patch. The normalized manifest changes only
zstd0.13.0 to0.14.3, preserving the existing fixed-codec patch. Registry metadata,
CI files and nested lockfile remain omitted. License/tests are retained.

Remove this patch when upstream accepts the fixed Zstandard dependency.
