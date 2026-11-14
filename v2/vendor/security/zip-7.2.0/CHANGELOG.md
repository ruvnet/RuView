# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [7.2.0](https://github.com/zip-rs/zip2/compare/v7.1.0...v7.2.0) - 2026-01-20

### <!-- 0 -->🚀 Features

- add read_zipfile_from_stream_with_compressed_size ([#70](https://github.com/zip-rs/zip2/pull/70))
- Allow choosing bzip2 rust backend ([#329](https://github.com/zip-rs/zip2/pull/329))

### <!-- 1 -->🐛 Bug Fixes

- Need to include zip64 extra field in central directory (fix #353) ([#360](https://github.com/zip-rs/zip2/pull/360))
- Fails to extract file which might or might not be malformed ([#376](https://github.com/zip-rs/zip2/pull/376)) ([#426](https://github.com/zip-rs/zip2/pull/426))
- *(aes)* Allow AES encryption while streaming ([#463](https://github.com/zip-rs/zip2/pull/463))
- Default "platform" field in zip files should be set to the local platform, rather than always "Unix" ([#470](https://github.com/zip-rs/zip2/pull/470)) ([#471](https://github.com/zip-rs/zip2/pull/471))

### <!-- 2 -->🚜 Refactor

- Define cfg_if! and cfg_if_expr! internal macros ([#438](https://github.com/zip-rs/zip2/pull/438))

### <!-- 4 -->⚡ Performance

- Change an assert to debug_assert when encrypting/decrypting AES, and eliminate a fallible operation ([#521](https://github.com/zip-rs/zip2/pull/521))
- eliminate a String clone per new file added to archive, and other related refactors ([#522](https://github.com/zip-rs/zip2/pull/522))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Fix another merge error, this one affecting only builds with flate2 and not zopfli
- Fix more merge issues
- Fix merge
- Fix write_dir build errors on specific feature configs
- Fix clippy warning
- Fix --all-features build error
- Fix merge

## [7.1.0](https://github.com/zip-rs/zip2/compare/v7.0.0...v7.1.0) - 2026-01-14

### <!-- 0 -->🚀 Features

- display the underlying error in Display impl for ZipError::Display ([#483](https://github.com/zip-rs/zip2/pull/483))
- Enable creation of `ZipArchive` without reparsing ([#485](https://github.com/zip-rs/zip2/pull/485))

### <!-- 1 -->🐛 Bug Fixes

- Return InvalidPassword rather than panic when AES key is the wrong length ([#457](https://github.com/zip-rs/zip2/pull/457))
- bench with auto zip64 comment ([#505](https://github.com/zip-rs/zip2/pull/505))
- add condition for `getrandom` dependency ([#504](https://github.com/zip-rs/zip2/pull/504))
- *(zipcrypto)* Support streaming ZipCrypto encryption, don't store entire file in memory ([#462](https://github.com/zip-rs/zip2/pull/462))

### <!-- 2 -->🚜 Refactor

- Clean up imports and move types ([#461](https://github.com/zip-rs/zip2/pull/461))
- Replace handwritten `Ord` and `PartialOrd` for `DateTime` ([#484](https://github.com/zip-rs/zip2/pull/484))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Lock `lzma-rust2` to at least 0.15.5 ([#491](https://github.com/zip-rs/zip2/pull/491))

## [7.0.0](https://github.com/zip-rs/zip2/compare/v6.0.0...v7.0.0) - 2025-12-05

### <!-- 9 -->⚠️ Breaking Changes

- Removed the following features: `getrandom`, `hmac`, `pbkdf2`, `sha1`, `zeroize`.
- Removed `lzma-static` and `xz-static` feature flags, which were deprecated synonyms of `lzma` and `xz`. ([#405](https://github.com/zip-rs/zip2/pull/405), [#425](https://github.com/zip-rs/zip2/pull/425))

### <!-- 0 -->🚀 Features

- *(`SimpleFileOptions`)* const DEFAULT implementation ([#474](https://github.com/zip-rs/zip2/pull/474))
- ZipWriter `set_auto_large_file()` method to enable large-file data descriptor when necessary ([#468](https://github.com/zip-rs/zip2/pull/468))

### <!-- 1 -->🐛 Bug Fixes

- print previous error when failing to search another cde ([#460](https://github.com/zip-rs/zip2/pull/460))
- cargo doc warnings ([#472](https://github.com/zip-rs/zip2/pull/472))
- Write ZIP64 data descriptors when large_file option is true ([#467](https://github.com/zip-rs/zip2/pull/467))
- Pin generic-array to an old version to work around https://github.com/RustCrypto/traits/issues/2036 until next RustCrypto & aes-crypto releases ([#458](https://github.com/zip-rs/zip2/pull/458))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Revert version bump so that release-plz will trigger
- expose more flate2 feature flags ([#476](https://github.com/zip-rs/zip2/pull/476))
- Next release will be 7.0.0
- release v6.0.0 ([#442](https://github.com/zip-rs/zip2/pull/442))

### Deps

- Bump lzma-rust2 to v0.15 ([#465](https://github.com/zip-rs/zip2/pull/465))

## [6.0.0](https://github.com/zip-rs/zip2/compare/v5.1.1...v6.0.0) - 2025-10-09

### <!-- 0 -->🚀 Features

- Add by_index_with_options(), which can be used to ignore encryption in a file's metadata ([#439](https://github.com/zip-rs/zip2/pull/439)) and may be used for other file-specific overrides in the future.

### <!-- 7 -->⚙️ Miscellaneous Tasks

- [**breaking**] `FileOptions::add_extra_data` is now generic and accepts any `AsRef<[u8]>`. (#435)

## [5.1.1](https://github.com/zip-rs/zip2/compare/v5.1.0...v5.1.1) - 2025-09-11

### <!-- 1 -->🐛 Bug Fixes

- panic when reading empty extended-timestamp field ([#404](https://github.com/zip-rs/zip2/pull/404)) ([#422](https://github.com/zip-rs/zip2/pull/422))
- Restore original file timestamp when unzipping with `chrono` ([#46](https://github.com/zip-rs/zip2/pull/46))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Configure Amazon Q rules ([#421](https://github.com/zip-rs/zip2/pull/421))

## [5.1.0](https://github.com/zip-rs/zip2/compare/v5.0.1...v5.1.0) - 2025-09-10

### <!-- 0 -->🚀 Features

- Add legacy shrink/reduce/implode compression ([#303](https://github.com/zip-rs/zip2/pull/303))

## [5.0.1](https://github.com/zip-rs/zip2/compare/v5.0.0...v5.0.1) - 2025-09-09

### <!-- 1 -->🐛 Bug Fixes

- AES metadata was not copied correctly in raw copy methods, which could corrupt the copied file. ([#417](https://github.com/zip-rs/zip2/pull/417))

## [5.0.0](https://github.com/zip-rs/zip2/compare/v4.6.1...v5.0.0) - 2025-09-05

### <!-- 0 -->🚀 Features

- Implement by_path*() methods on ZipArchive ([#382](https://github.com/zip-rs/zip2/pull/382))

## [4.6.1](https://github.com/zip-rs/zip2/compare/v4.6.0...v4.6.1) - 2025-09-03

### <!-- 1 -->🐛 Bug Fixes

- Fixes an issue introduced by the swap from `lzma-rs` to `liblzma` ([#407](https://github.com/zip-rs/zip2/pull/407))

## [4.6.0](https://github.com/zip-rs/zip2/compare/v4.5.0...v4.6.0) - 2025-08-30

### <!-- 0 -->🚀 Features

- Allow to read zip files with unsupported extended timestamps ([#400](https://github.com/zip-rs/zip2/pull/400))

### <!-- 1 -->🐛 Bug Fixes

- enable clamp_opt for ppmd and xz ([#401](https://github.com/zip-rs/zip2/pull/401))

## [4.5.0](https://github.com/zip-rs/zip2/compare/v4.4.0...v4.5.0) - 2025-08-21

### <!-- 0 -->🚀 Features

- Allow reading ZIP files where the central directory comes *before* the files ([#384](https://github.com/zip-rs/zip2/pull/384)) ([#396](https://github.com/zip-rs/zip2/pull/396))

## [4.4.0](https://github.com/zip-rs/zip2/compare/v4.3.0...v4.4.0) - 2025-08-21

### <!-- 0 -->🚀 Features

- Add `lzma-static` and `xz-static` features that enable `liblzma/static` ([#393](https://github.com/zip-rs/zip2/pull/393))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Move deprecated annotations to fix a Clippy warning ([#391](https://github.com/zip-rs/zip2/pull/391))

## [4.3.0](https://github.com/zip-rs/zip2/compare/v4.2.0...v4.3.0) - 2025-07-09

### <!-- 0 -->🚀 Features

- Add support for PPMd ([#370](https://github.com/zip-rs/zip2/pull/370))

## [4.2.0](https://github.com/zip-rs/zip2/compare/v4.1.0...v4.2.0) - 2025-06-21

### <!-- 0 -->🚀 Features

- Write ZIP file to stream ([#246](https://github.com/zip-rs/zip2/pull/246))

## [4.1.0](https://github.com/zip-rs/zip2/compare/v4.0.0...v4.1.0) - 2025-06-14

### <!-- 0 -->🚀 Features

- Add has_overlapping_files method

## [4.0.0](https://github.com/zip-rs/zip2/compare/v3.0.0...v4.0.0) - 2025-05-21

### <!-- 1 -->🐛 Bug Fixes

- Allow extraction of Zip64 where "Version needed to extract" is higher than "Version made by" ([#356](https://github.com/zip-rs/zip2/pull/356))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Revert nt-time upgrade (would increase MSRV)
- Revert constant_time_eq update (would increase MSRV)
- Update fully-qualified names of liblzma imports

## [3.0.0](https://github.com/zip-rs/zip2/compare/v2.6.1...v3.0.0) - 2025-05-14

### <!-- 1 -->🐛 Bug Fixes

- return correct offset in SeekableTake::seek ([#342](https://github.com/zip-rs/zip2/pull/342))
- When only zopfli is available, decompression of deflate should not be possible ([#348](https://github.com/zip-rs/zip2/pull/348))
- Specify `flate2` dependency of the `deflate-flate2` feature. ([#345](https://github.com/zip-rs/zip2/pull/345))

### <!-- 7 -->⚙️ Miscellaneous Tasks

- drop unused crossbeam-utils dependency ([#339](https://github.com/zip-rs/zip2/pull/339))
- fix typo
- remove `deflate-flate2` dependency on specific backend
- [**breaking**] Drop deprecated `deflate-miniz` feature flag ([#351](https://github.com/zip-rs/zip2/pull/351))

## [2.6.1](https://github.com/zip-rs/zip2/compare/v2.6.0...v2.6.1) - 2025-04-03

### <!-- 1 -->🐛 Bug Fixes

- avoid scanning through all local file headers while opening an archive ([#281](https://github.com/zip-rs/zip2/pull/281))

## [2.5.0](https://github.com/zip-rs/zip2/compare/v2.4.2...v2.5.0) - 2025-03-23

### <!-- 0 -->🚀 Features

- Add support for `time::PrimitiveDateTime` ([#322](https://github.com/zip-rs/zip2/pull/322))
- Add `jiff` integration ([#323](https://github.com/zip-rs/zip2/pull/323))

### <!-- 1 -->🐛 Bug Fixes

- improve error message for duplicated file ([#277](https://github.com/zip-rs/zip2/pull/277))

## [2.4.2](https://github.com/zip-rs/zip2/compare/v2.4.1...v2.4.2) - 2025-03-18

### <!-- 1 -->🐛 Bug Fixes

- `deep_copy_file` produced a mangled file header on big-endian platforms (#309)

## [2.4.1](https://github.com/zip-rs/zip2/compare/v2.4.0...v2.4.1) - 2025-03-17

### <!-- 1 -->🐛 Bug Fixes

- type issue in test
- double as_ref().canonicalize()?
- CI failures
- Create directory for extraction if necessary ([#314](https://github.com/zip-rs/zip2/pull/314))

## [2.4.0](https://github.com/zip-rs/zip2/compare/v2.3.0...v2.4.0) - 2025-03-17

### <!-- 0 -->🚀 Features

- `ZipArchive::root_dir` and `ZipArchive::extract_unwrapped_root_dir` ([#304](https://github.com/zip-rs/zip2/pull/304))

### <!-- 1 -->🐛 Bug Fixes

- wasm build failure due to a missing use statement  ([#313](https://github.com/zip-rs/zip2/pull/313))

## [2.3.0](https://github.com/zip-rs/zip2/compare/v2.2.3...v2.3.0) - 2025-03-16

### <!-- 0 -->🚀 Features

- Add support for NTFS extra field ([#279](https://github.com/zip-rs/zip2/pull/279))

### <!-- 1 -->🐛 Bug Fixes

- *(test)* Conditionalize a zip64 doctest ([#308](https://github.com/zip-rs/zip2/pull/308))
- fix failing tests, remove symlink loop check
- Canonicalize output path to avoid false negatives
- Symlink handling in stream extraction
- Canonicalize output paths and symlink targets, and ensure they descend from the destination

### <!-- 7 -->⚙️ Miscellaneous Tasks

- Fix clippy and cargo fmt warnings ([#310](https://github.com/zip-rs/zip2/pull/310))

## [2.2.3](https://github.com/zip-rs/zip2/compare/v2.2.2...v2.2.3) - 2025-02-26

### <!-- 2 -->🚜 Refactor

- Change the inner structure of `DateTime` (#267)

### <!-- 7 -->⚙️ Miscellaneous Tasks

- cargo fix --edition

## [2.2.2](https://github.com/zip-rs/zip2/compare/v2.2.1...v2.2.2) - 2024-12-16

### <!-- 1 -->🐛 Bug Fixes

- rewrite the EOCD/EOCD64 detection to fix extreme performance regression (#247)

## [2.2.1](https://github.com/zip-rs/zip2/compare/v2.2.0...v2.2.1) - 2024-11-20

### <!-- 1 -->🐛 Bug Fixes

- remove executable bit ([#238](https://github.com/zip-rs/zip2/pull/238))
- *(lzma)* fixed panic in case of invalid lzma stream ([#259](https://github.com/zip-rs/zip2/pull/259))
- resolve new clippy warnings on nightly ([#262](https://github.com/zip-rs/zip2/pull/262))
- resolve clippy warning in nightly ([#252](https://github.com/zip-rs/zip2/pull/252))

### <!-- 4 -->⚡ Performance

- Faster cde rejection ([#255](https://github.com/zip-rs/zip2/pull/255))

## [2.2.0](https://github.com/zip-rs/zip2/compare/v2.1.6...v2.2.0) - 2024-08-11

### <!-- 0 -->🚀 Features
- Expose `ZipArchive::central_directory_start` ([#232](https://github.com/zip-rs/zip2/pull/232))

## [2.1.6](https://github.com/zip-rs/zip2/compare/v2.1.5...v2.1.6) - 2024-07-29

### <!-- 1 -->🐛 Bug Fixes
- ([#33](https://github.com/zip-rs/zip2/pull/33)) Rare combination of settings could lead to writing a corrupt archive with overlength extra data, and data_start locations when reading the archive back were also wrong ([#221](https://github.com/zip-rs/zip2/pull/221))

### <!-- 2 -->🚜 Refactor
- Eliminate some magic numbers and unnecessary path prefixes ([#225](https://github.com/zip-rs/zip2/pull/225))

## [2.1.5](https://github.com/zip-rs/zip2/compare/v2.1.4...v2.1.5) - 2024-07-20

### <!-- 2 -->🚜 Refactor
- change invalid_state() return type to io::Result<T>

## [2.1.4](https://github.com/zip-rs/zip2/compare/v2.1.3...v2.1.4) - 2024-07-18

### <!-- 1 -->🐛 Bug Fixes
- fix([#215](https://github.com/zip-rs/zip2/pull/215)): Upgrade to deflate64 0.1.9
- Panic when reading a file truncated in the middle of an XZ block header
- Some archives with over u16::MAX files were handled incorrectly or slowly ([#189](https://github.com/zip-rs/zip2/pull/189))
- Check number of files when deciding whether a CDE is the real one
- Could still select a fake CDE over a real one in some cases
- May have to consider multiple CDEs before filtering for validity
- We now keep searching for a real CDE header after read an invalid one from the file comment
- Always search for data start when opening an archive for append, and reject the header if data appears to start after central directory
- `deep_copy_file` no longer allows overwriting an existing file, to match the behavior of `shallow_copy_file`
- File start position was wrong when extra data was present
- Abort file if central extra data is too large
- Overflow panic when central directory extra data is too large
- ZIP64 header was being written twice when copying a file
- ZIP64 header was being written to central header twice
- Start position was incorrect when file had no extra data
- Allow all reserved headers we can create
- Fix a bug where alignment padding interacts with other extra-data fields
- Fix bugs involving alignment padding and Unicode extra fields
- Incorrect header when adding AES-encrypted files
- Parse the extra field and reject it if invalid
- Incorrect behavior following a rare combination of `merge_archive`, `abort_file` and `deep_copy_file`. As well, we now return an error when a file is being copied to itself.
- path_to_string now properly handles the case of an empty path
- Implement `Debug` for `ZipWriter` even when it's not implemented for the inner writer's type
- Fix an issue where the central directory could be incorrectly detected
- `finish_into_readable()` would corrupt the archive if the central directory had moved

### <!-- 2 -->🚜 Refactor
- Verify with debug assertions that no FixedSizeBlock expects a multi-byte alignment ([#198](https://github.com/zip-rs/zip2/pull/198))
- Use new do_or_abort_file method

### <!-- 4 -->⚡ Performance
- Speed up CRC when encrypting small files
- Limit the number of extra fields
- Refactor extra-data validation
- Store extra data in plain vectors until after validation
- Only build one IndexMap after choosing among the possible valid headers
- Simplify validation of empty extra-data fields
- Validate automatic extra-data fields only once, even if several are present
- Remove redundant `validate_extra_data()` call
- Skip searching for the ZIP32 header if a valid ZIP64 header is present ([#189](https://github.com/zip-rs/zip2/pull/189))

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Fix a bug introduced by c934c824
- Fix a failing unit test
- Fix build errors on older Rust versions
- Fix build
- Fix another fuzz failure
- Switch to `ok_or_abort_file`, and inline when that fails borrow checker
- Switch to `ok_or_abort_file`, and inline when that fails borrow checker
- Fix a build error
- Fix boxed_local warning (can borrow instead)
- Partial debug
- Fix more errors when parsing multiple extra fields
- Fix an error when decoding AES header
- Fix an error caused by not allowing 0xa11e field
- Bug fix: crypto_header was being counted toward extra_data_end
- Bug fix: revert a change where crypto_header was incorrectly treated as an extra field
- Fix a bug where a modulo of 0 was used
- Fix a bug when ZipCrypto, alignment *and* a custom header are used
- Fix a bug when both ZipCrypto and alignment are used
- Fix another bug: header_end vs extra_data_end
- Fix use of a stale value in a `debug_assert_eq!`
- Fix: may still get an incorrect size if opening an invalid file for append
- Fix: may need the absolute start as tiebreaker to ensure deterministic behavior

## [2.1.3](https://github.com/zip-rs/zip2/compare/v2.1.2...v2.1.3) - 2024-06-04

### <!-- 1 -->🐛 Bug Fixes
- Some date/time filters were previously unreliable (i.e. later-pass filters had no earliest-pass or latest-fail, and vice-versa)
- Decode Zip-Info UTF8 name and comment fields ([#159](https://github.com/zip-rs/zip2/pull/159))

### <!-- 2 -->🚜 Refactor
- Return extended timestamp fields copied rather than borrowed ([#183](https://github.com/zip-rs/zip2/pull/183))

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Fix a new Clippy warning
- Fix a bug and inline `deserialize` for safety
- Add check for wrong-length blocks, and incorporate fixed-size requirement into the trait name
- Fix a fuzz failure by using checked_sub
- Add feature gate for new unit test

## [2.1.1](https://github.com/zip-rs/zip2/compare/v2.1.0...v2.1.1) - 2024-05-28

### <!-- 1 -->🐛 Bug Fixes
- Derive `Debug` for `ZipWriter`
- lower default version to 4.5 and use the version-needed-to-extract where feasible.

### <!-- 2 -->🚜 Refactor
- use a MIN_VERSION constant

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Bug fixes for debug implementation
- Bug fixes for debug implementation
- Update unit tests
- Remove unused import

## [2.1.0](https://github.com/zip-rs/zip2/compare/v2.0.0...v2.1.0) - 2024-05-25

### <!-- 0 -->🚀 Features
- Support mutual conversion between `DateTime` and MS-DOS pair

### <!-- 1 -->🐛 Bug Fixes
- version-needed-to-extract was incorrect in central header, and version-made-by could be lower than that ([#100](https://github.com/zip-rs/zip2/pull/100))
- version-needed-to-extract was incorrect in central header, and version-made-by could be lower than that ([#100](https://github.com/zip-rs/zip2/pull/100))

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Another tweak to ensure `version_needed` is applied
- Tweaks to make `version_needed` and `version_made_by` work with recently-merged changes

## [2.0.0](https://github.com/zip-rs/zip2/compare/v1.3.1...v2.0.0) - 2024-05-24

### <!-- 0 -->🚀 Features
- Add `fmt::Display` for `DateTime`
- Implement more traits for `DateTime`

### <!-- 2 -->🚜 Refactor
- Change type of `last_modified_time` to `Option<DateTime>`
- [**breaking**] Rename `from_msdos` to `from_msdos_unchecked`, make it unsafe, and add `try_from_msdos` ([#145](https://github.com/zip-rs/zip2/pull/145))

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Continue to accept archives with invalid DateTime, and use `now_utc()` as default only when writing, not reading

## [1.3.1](https://github.com/zip-rs/zip2/compare/v1.3.0...v1.3.1) - 2024-05-21

### <!-- 2 -->🚜 Refactor
- Make `deflate` enable both default implementations
- Merge the hidden deflate-flate2 flag into the public one
- Rename _deflate-non-zopfli to _deflate-flate2
- Reject encrypted and using_data_descriptor files slightly faster in read_zipfile_from_stream
- Convert `impl TryInto<NaiveDateTime> for DateTime` to `impl TryFrom<DateTime> for NaiveDateTime` ([#136](https://github.com/zip-rs/zip2/pull/136))

### <!-- 4 -->⚡ Performance
- Change default compression implementation to `flate2/zlib-ng`

### <!-- 7 -->⚙️ Miscellaneous Tasks
- chore([#132](https://github.com/zip-rs/zip2/pull/132)): Attribution for some copied test data
- chore([#133](https://github.com/zip-rs/zip2/pull/133)): chmod -x src/result.rs

## [1.3.0](https://github.com/zip-rs/zip2/compare/v1.2.3...v1.3.0) - 2024-05-17

### <!-- 0 -->🚀 Features
- Add `is_symlink` method

### <!-- 1 -->🐛 Bug Fixes
- Extract symlinks into symlinks on Unix and Windows, and fix a bug that affected making directories writable on MacOS

### <!-- 2 -->🚜 Refactor
- Eliminate deprecation warning when `--all-features` implicitly enables the deprecated feature
- Check if archive contains a symlink's target, without borrowing both at the same time
- Eliminate a clone that's no longer necessary
- is_dir only needs to look at the filename
- Remove unnecessary #[cfg] attributes

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Fix borrow-of-moved-value
- Box<str> doesn't directly convert to PathBuf, so convert back to String first
- partial revert - only &str has chars(), but Box<str> should auto-deref
- contains_key needs a `Box<str>`, so generify `is_dir` to accept one
- Add missing `ZipFileData::is_dir()` method
- Fix another Windows-specific error
- More bug fixes for Windows-specific symlink code
- More bug fixes for Windows-specific symlink code
- Bug fix: variable name change
- Bug fix: need both internal and output path to determine whether to symlink_dir
- Another bug fix
- Fix another error-type conversion error
- Fix error-type conversion on Windows
- Fix conditionally-unused import
- Fix continued issues, and factor out the Vec<u8>-to-OsString conversion (cc: [#125](https://github.com/zip-rs/zip2/pull/125))
- Fix CI failure involving conversion to OsString for symlinks (see my comments on [#125](https://github.com/zip-rs/zip2/pull/125))
- Move path join into platform-independent code

## [1.2.3](https://github.com/zip-rs/zip2/compare/v1.2.2...v1.2.3) - 2024-05-10

### <!-- 1 -->🐛 Bug Fixes
- Remove a window when an extracted directory might be unexpectedly listable and/or `cd`able by non-owners
- Extract directory contents on Unix even if the directory doesn't have write permission (https://github.com/zip-rs/zip-old/issues/423)

### <!-- 7 -->⚙️ Miscellaneous Tasks
- More conditionally-unused imports

## [1.2.2](https://github.com/zip-rs/zip2/compare/v1.2.1...v1.2.2) - 2024-05-09

### <!-- 1 -->🐛 Bug Fixes
- Failed to clear "writing_raw" before finishing a symlink, leading to dropped extra fields

### <!-- 4 -->⚡ Performance
- Use boxed slice for archive comment, since it can't be concatenated
- Optimize for the fact that false signatures can't overlap with real ones

## [1.2.1](https://github.com/zip-rs/zip2/compare/v1.2.0...v1.2.1) - 2024-05-06

### <!-- 1 -->🐛 Bug Fixes
- Prevent panic when trying to read a file with an unsupported compression method
- Prevent panic after reading an invalid LZMA file
- Make `Stored` the default compression method if `Deflated` isn't available, so that zip files are readable by as much software as possible
- version_needed was wrong when e.g. cfg(bzip2) but current file wasn't bzip2 ([#100](https://github.com/zip-rs/zip2/pull/100))
- file paths shouldn't start with slashes ([#102](https://github.com/zip-rs/zip2/pull/102))

### <!-- 2 -->🚜 Refactor
- Overhaul `impl Arbitrary for FileOptions`
- Remove unused `atomic` module

## [1.2.0](https://github.com/zip-rs/zip2/compare/v1.1.4...v1.2.0) - 2024-05-06

### <!-- 0 -->🚀 Features
- Add method `decompressed_size()` so non-recursive ZIP bombs can be detected

### <!-- 2 -->🚜 Refactor
- Make `ZipWriter::finish()` consume the `ZipWriter`

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Use panic! rather than abort to ensure the fuzz harness can process the failure
- Update fuzz_write to use replace_with
- Remove a drop that can no longer be explicit
- Add `#![allow(unexpected_cfgs)]` in nightly

## [1.1.4](https://github.com/zip-rs/zip2/compare/v1.1.3...v1.1.4) - 2024-05-04

### <!-- 1 -->🐛 Bug Fixes
- Build was failing with bzip2 enabled
- use is_dir in more places where Windows paths might be handled incorrectly

### <!-- 4 -->⚡ Performance
- Quick filter for paths that contain "/../" or "/./" or start with "./" or "../"
- Fast handling for separator-free paths
- Speed up logic if main separator isn't '/'
- Drop `normalized_components` slightly sooner when not using it
- Speed up `path_to_string` in cases where the path is already in the proper format

### <!-- 7 -->⚙️ Miscellaneous Tasks
- Refactor: can short-circuit handling of paths that start with MAIN_SEPARATOR, no matter what MAIN_SEPARATOR is
- Bug fix: non-canonical path detection when MAIN_SEPARATOR is not slash or occurs twice in a row
- Bug fix: must recreate if . or .. is a path element
- Bug fix

### <!-- 9 -->◀️ Revert
- [#58](https://github.com/zip-rs/zip2/pull/58) (partial): `bzip2-rs` can't replace `bzip2` because it's decompress-only

## [1.1.3](https://github.com/zip-rs/zip2/compare/v1.1.2...v1.1.3) - 2024-04-30

### <!-- 1 -->🐛 Bug Fixes
- Rare bug where find_and_parse would give up prematurely on detecting a false end-of-CDR header

## [1.1.2](https://github.com/Pr0methean/zip/compare/v1.1.1...v1.1.2) - 2024-04-28

### <!-- 1 -->🐛 Bug Fixes
- Alignment was previously handled incorrectly ([#33](https://github.com/Pr0methean/zip/pull/33))

### <!-- 2 -->🚜 Refactor
- deprecate `deflate-miniz` feature since it's now equivalent to `deflate` ([#35](https://github.com/Pr0methean/zip/pull/35))

## [1.1.1]

### Added

- `index_for_name`, `index_for_path`, `name_for_index`: get the index of a file given its path or vice-versa, without
  initializing metadata from the local-file header or needing to mutably borrow the `ZipArchive`.
- `add_symlink_from_path`, `shallow_copy_file_from_path`, `deep_copy_file_from_path`, `raw_copy_file_to_path`: copy a
  file or create a symlink using `AsRef<Path>` arguments

### Changed

- `add_directory_from_path` and `start_file_from_path` are no longer deprecated, and they now normalize `..` as well as
  `.`.

## [1.1.0]

### Added

- Support for decoding LZMA.

### Changed

- Eliminated a custom `AtomicU64` type by replacing it with `OnceLock` in the only place it's used.
- `FileOptions` now has the subtype `SimpleFileOptions` which implements `Copy` but has no extra data.

## [1.0.1]

### Changed

- The published package on crates.io no longer includes the tests or examples.

## [1.0.0]

### Changed

- Now uses boxed slices rather than `String` or `Vec` for metadata fields that aren't likely to grow.

## [0.11.0]

### Added

- Support for `DEFLATE64` (decompression only).
- Support for Zopfli compression levels up to `i64::MAX`.

### Changed

- `InvalidPassword` is now a kind of `ZipError` to eliminate the need for nested `Result` structs.
- Updated dependencies.

## [0.10.3]

### Changed

- Updated dependencies.
- MSRV increased to `1.67`.

### Fixed

- Fixed some rare bugs that could cause panics when trying to read an invalid ZIP file or using an incorrect password.

## [0.10.2]

### Changed

- Where possible, methods are now `const`. This improves performance, especially when reading.

## [0.10.1]

### Changed

- Date and time conversion methods now return `DateTimeRangeError` rather than `()` on error.

## [0.10.0]

### Changed

- Replaces the `flush_on_finish_file` parameter of `ZipWriter::new` and `ZipWriter::Append` with
  a `set_flush_on_finish_file` method.

### Fixed

- Fixes build errors that occur when all default features are disabled.
- Fixes more cases of a bug when ZIP64 magic bytes occur in filenames.

## [0.9.2]

### Added

- `zlib-ng` for fast Deflate compression. This is now the default for compression levels 0-9.
- `chrono` to convert zip::DateTime to and from chrono::NaiveDateTime

## [0.9.1]

### Added

- Zopfli for aggressive Deflate compression.

## [0.9.0]

### Added

 - `flush_on_finish_file` parameter for `ZipWriter`.

## [0.8.3]

### Changed

- Uses the `aes::cipher::KeyInit` trait from `aes` 0.8.2 where appropriate.

### Fixed

- Calling `abort_file()` no longer corrupts the archive if called on a
  shallow copy of a remaining file, or on an archive whose CDR entries are out
  of sequence. However, it may leave an unused entry in the archive.
- Calling `abort_file()` while writing a ZipCrypto-encrypted file no longer
  causes a crash.
- Calling `abort_file()` on the last file before `finish()` no longer produces
  an invalid ZIP file or garbage in the comment.

### Added

- `ZipWriter` methods `get_comment()` and `get_raw_comment()`.

## [0.8.2]

### Fixed

- Fixed an issue where code might spuriously fail during write fuzzing.

### Added

- New method `with_alignment` on `FileOptions`.

## [0.8.1]

### Fixed

- `ZipWriter` now once again implements `Send` if the underlying writer does.

## [0.8.0]

### Deleted

- Methods `start_file_aligned`, `start_file_with_extra_data`, `end_local_start_central_extra_data` and
  `end_extra_data` (see below).

### Changed

- Alignment and extra-data fields are now attributes of [`zip::unstable::write::FileOptions`], allowing them to be
  specified for `add_directory` and `add_symlink`.
- Extra-data fields are now formatted by the `FileOptions` method `add_extra_data`.
- Improved performance, especially for `shallow_copy_file` and `deep_copy_file` on files with extra data.

### Fixed

- Fixes a rare bug where the size of the extra-data field could overflow when `large_file` was set.
- Fixes more cases of a bug when ZIP64 magic bytes occur in filenames.

## [0.7.5]

### Fixed

- Fixed a bug that occurs when ZIP64 magic bytes occur twice in a filename or across two filenames.

## [0.7.4]

### Added

- Added experimental [`zip::unstable::write::FileOptions::with_deprecated_encryption`] API to enable encrypting
  files with PKWARE encryption.

## [0.7.3]

### Fixed

- Fixed a bug that occurs when a filename in a ZIP32 file includes the ZIP64 magic bytes.

## [0.7.2]

### Added

- Method `abort_file` - removes the current or most recently-finished file from the archive.

### Fixed

- Fixed a bug where a file could remain open for writing after validations failed.

## [0.7.1]

### Changed

- Bumped the version number in order to upload an updated README to crates.io.

## [0.7.0]

### Fixed

- Calling `start_file` with invalid parameters no longer closes the `ZipWriter`.
- Attempting to write a 4GiB file without calling `FileOptions::large_file(true)` now removes the file from the archive
  but does not close the `ZipWriter`.
- Attempting to write a file with an unrepresentable or invalid last-modified date will instead add it with a date of
  1980-01-01 00:00:00.

### Added

- Method `is_writing_file` - indicates whether a file is open for writing.

## [0.6.13]

### Fixed

- Fixed a possible bug in deep_copy_file.

## [0.6.12]

### Fixed

- Fixed a Clippy warning that was missed during the last release.

## [0.6.11]

### Fixed

- Fixed a bug that could cause later writes to fail after a `deep_copy_file` call.

## [0.6.10]

### Changed

- Updated dependency versions.

## [0.6.9]

### Fixed

- Fixed an issue that prevented `ZipWriter` from implementing `Send`.

## [0.6.8]

### Added

- Detects duplicate filenames.

### Fixed

- `deep_copy_file` could set incorrect Unix permissions.
- `deep_copy_file` could handle files incorrectly if their compressed size was u32::MAX bytes or less but their
  uncompressed size was not.
- Documented that `deep_copy_file` does not copy a directory's contents.

### Changed

- Improved performance of `deep_copy_file` by using a HashMap and eliminating a redundant search.

## [0.6.7]

### Added

- `deep_copy_file` method: more standards-compliant way to copy a file from within the ZipWriter

## [0.6.6]

### Fixed

- Unused flag `#![feature(read_buf)]` was breaking compatibility with stable compiler.

### Changed

- Updated `aes` dependency to `0.8.2` (https://github.com/zip-rs/zip/pull/354)
- Updated other dependency versions.

## [0.6.5]
### Changed

- Added experimental [`zip::unstable::write::FileOptions::with_deprecated_encryption`] API to enable encrypting files with PKWARE encryption.

### Added

- `shallow_copy_file` method: copy a file from within the ZipWriter


## [0.6.4]

### Changed

- [#333](https://github.com/zip-rs/zip/pull/333): disabled the default features of the `time` dependency, and also `formatting` and `macros`, as they were enabled by mistake.
- Deprecated [`DateTime::from_time`](https://docs.rs/zip/0.6/zip/struct.DateTime.html#method.from_time) in favor of [`DateTime::try_from`](https://docs.rs/zip/0.6/zip/struct.DateTime.html#impl-TryFrom-for-DateTime)
