zip
===

[![Build Status](https://github.com/zip-rs/zip2/actions/workflows/ci.yaml/badge.svg)](https://github.com/zip-rs/zip2/actions?query=branch%3Amaster+workflow%3ACI)
[![Crates.io version](https://img.shields.io/crates/v/zip.svg)](https://crates.io/crates/zip)

[Documentation](https://docs.rs/zip/latest/zip/)

# Info

A zip library for rust which supports reading and writing of simple ZIP files. Formerly hosted at
https://github.com/zip-rs/zip2.

Supported compression formats:

* stored (i.e. none)
* deflate
* deflate64 (decompression only)
* bzip2
* zstd
* lzma (decompression only)
* xz
* ppmd

Currently unsupported zip extensions:

* Multi-disk

# Features

The features available are:

* `aes-crypto`: Enables decryption of files which were encrypted with AES. Supports AE-1 and AE-2 methods.
* `deflate`: Enables compressing and decompressing an unspecified implementation (that may change in future versions) of
  the deflate compression algorithm, which is the default for zip files. Supports compression quality 1..=264.
* `deflate-flate2`: Combine this with any `flate2` feature flag that enables a back-end, to support deflate compression
  at quality 1..=9.
* `deflate-zopfli`: Enables deflating files with the `zopfli` library (used when compression quality is 10..=264). This
  is the most effective `deflate` implementation available, but also among the slowest. If `flate2` isn't also enabled,
  only compression will be supported and not decompression.
* `deflate64`: Enables the deflate64 compression algorithm. Only decompression is supported.
* `lzma`: Enables the LZMA compression algorithm. Only decompression is supported.
* `bzip2`: Enables the BZip2 compression algorithm.
* `ppmd`: Enables the PPMd compression algorithm.
* `time`: Enables features using the [time](https://github.com/rust-lang-deprecated/time) crate.
* `chrono`: Enables converting last-modified `zip::DateTime` to and from `chrono::NaiveDateTime`.
* `jiff-02`: Enables converting last-modified `zip::DateTime` to and from `jiff::civil::DateTime`.
* `nt-time`: Enables returning timestamps stored in the NTFS extra field as `nt_time::FileTime`.
* `xz`: Enables the XZ compression algorithm.
* `zstd`: Enables the Zstandard compression algorithm.

By default `aes-crypto`, `bzip2`, `deflate`, `deflate64`, `lzma`, `ppmd`, `time`, `xz` and `zstd` are enabled.

# MSRV

Our current Minimum Supported Rust Version is **1.83**. When adding features,
we will follow these guidelines:

- We will always support a minor Rust version that has been stable for at least 6 months.
- Any change to the MSRV will be accompanied with a **minor** version bump.

# Examples

See the [examples directory](examples) for:

* How to write a file to a zip.
* How to write a directory of files to a zip (using [walkdir](https://github.com/BurntSushi/walkdir)).
* How to extract a zip file.
* How to extract a single file from a zip.
* How to read a zip from the standard input.
* How to append a directory to an existing archive

# Fuzzing

Fuzzing support is through [`cargo afl`](https://rust-fuzz.github.io/book/afl/tutorial.html). To install `cargo afl`:

```bash
cargo install cargo-afl
```

To start fuzzing zip extraction:

```bash
mkdir -vp fuzz-read-out
cargo afl build --manifest-path=fuzz/Cargo.toml --all-features -p fuzz_read
# Curated input corpus:
cargo afl fuzz -i fuzz/read/in -o fuzz-read-out fuzz/target/debug/fuzz_read
# Test data files:
cargo afl fuzz -i tests/data -e zip -o fuzz-read-out fuzz/target/debug/fuzz_read
```

To start fuzzing zip creation:

```bash
mkdir -vp fuzz-write-out
cargo afl build --manifest-path=fuzz/Cargo.toml --all-features -p fuzz_write
# Curated input corpus and dictionary schema:
cargo afl fuzz -x fuzz/write/fuzz.dict -i fuzz/write/in -o fuzz-write-out fuzz/target/debug/fuzz_write
```

## Fuzzing stdio

The read and write fuzzers can also receive input over stdin for one-off validation. Note here that the fuzzers can be configured to build in support for DEFLATE, or not:
```bash
# Success, no output:
cargo run --manifest-path=fuzz/Cargo.toml --quiet --all-features -p fuzz_read <tests/data/deflate64.zip
# Error, without deflate64 support:
cargo run --manifest-path=fuzz/Cargo.toml --quiet -p fuzz_read <tests/data/deflate64.zip

thread 'main' (537304) panicked at fuzz_read/src/main.rs:40:36:
called `Result::unwrap()` on an `Err` value: UnsupportedArchive("Compression method not supported")
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

The zip creation fuzzer will try to print out a description of the kind of input it translated the input bytes into:
```bash
# This is an empty input case:
<fuzz/write/in/id-000000,time-0,execs-0,orig-0011743621118ab6c5278ffbb8fd14bddd8369ee.min \
  cargo run --manifest-path=fuzz/Cargo.toml --quiet --all-features -p fuzz_write
# This input was translated into one or more test cases:
<fuzz/write/in/id-000000,time-0,execs-0,orig-0011743621118ab6c5278ffbb8fd14bddd8369ee.min \
  cargo run --manifest-path=fuzz/Cargo.toml --quiet -p fuzz_write
writer.start_file_from_path("", FileOptions { compression_method: Stored, compression_level: None, last_modified_time: DateTime::from_date_and_time(2048, 1, 1, 0, 0, 0)?, permissions: None, large_file: false, encrypt_with: None, extended_options: ExtendedFileOptions {extra_data: vec![].into(), central_extra_data: vec![].into()}, alignment: 0 })?;
writer.write_all(&[])?;
writer
let _ = writer.finish_into_readable()?;
```

The zip creation fuzzer uses [`arbitrary::Unstructured`](https://docs.rs/arbitrary/latest/arbitrary/struct.Unstructured.html) to convert bytes over stdin to random inputs, so it can be triggered with other sources of random input:
```bash
# Usually, the random input is translated into zero test cases:
head -c50 /dev/random | cargo run --manifest-path=fuzz/Cargo.toml --quiet --all-features -p fuzz_write
# Sometimes, one or more test cases are generated and successfully evaluated:
head -c50 /dev/random | cargo run --manifest-path=fuzz/Cargo.toml --quiet --all-features -p fuzz_write
writer.set_raw_comment([20, 202])?;
let mut writer = ZipWriter::new_append(writer.finish()?)?;
let sub_writer = {
let mut initial_junk = Cursor::new(vec![106]);
initial_junk.seek(SeekFrom::End(0))?;
                          let mut writer = ZipWriter::new(initial_junk);
writer
};
writer.merge_archive(sub_writer.finish_into_readable()?)?;
let mut writer = ZipWriter::new_append(writer.finish()?)?;
```
