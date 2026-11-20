zip-rs
======

[![Build Status](https://img.shields.io/github/workflow/status/zip-rs/zip/CI)](https://github.com/zip-rs/zip/actions?query=branch%3Amaster+workflow%3ACI)
[![Crates.io version](https://img.shields.io/crates/v/zip.svg)](https://crates.io/crates/zip)
[![Discord](https://badgen.net/badge/icon/discord?icon=discord&label)](https://discord.gg/rQ7H9cSsF4)

[Documentation](https://docs.rs/zip/0.6.3/zip/)

Info
----


A zip library for rust which supports reading and writing of simple ZIP files.

Supported compression formats:

* stored (i.e. none)
* deflate
* bzip2
* zstd

Currently unsupported zip extensions:

* Encryption
* Multi-disk

Usage
-----

With all default features:

```toml
[dependencies]
zip = "0.6"
```

Without the default features:

```toml
[dependencies]
zip = { version = "0.6.6", default-features = false }
```

The features available are:

* `aes-crypto`: Enables decryption of files which were encrypted with AES. Supports AE-1 and AE-2 methods.
* `deflate`: Enables the deflate compression algorithm, which is the default for zip files.
* `bzip2`: Enables the BZip2 compression algorithm.
* `time`: Enables features using the [time](https://github.com/rust-lang-deprecated/time) crate.
* `zstd`: Enables the Zstandard compression algorithm.

All of these are enabled by default.

MSRV
----

Our current Minimum Supported Rust Version is **1.59.0**. When adding features,
we will follow these guidelines:

- We will always support the latest four minor Rust versions. This gives you a 6
  month window to upgrade your compiler.
- Any change to the MSRV will be accompanied with a **minor** version bump
   - While the crate is pre-1.0, this will be a change to the PATCH version.

Examples
--------

See the [examples directory](examples) for:
   * How to write a file to a zip.
   * How to write a directory of files to a zip (using [walkdir](https://github.com/BurntSushi/walkdir)).
   * How to extract a zip file.
   * How to extract a single file from a zip.
   * How to read a zip from the standard input.

Fuzzing
-------

Fuzzing support is through [cargo fuzz](https://github.com/rust-fuzz/cargo-fuzz). To install cargo fuzz:

```bash
cargo install cargo-fuzz
```

To list fuzz targets:

```bash
cargo +nightly fuzz list
```

To start fuzzing zip extraction:

```bash
cargo +nightly fuzz run fuzz_read
```
