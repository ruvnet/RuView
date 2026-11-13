# Changelog

## [0.6.6]
### Changed

- Updated `aes` dependency to `0.8.2` (https://github.com/zip-rs/zip/pull/354)

## [0.6.5]
### Changed

- Added experimental [`zip::unstable::write::FileOptions::with_deprecated_encryption`] API to enable encrypting files with PKWARE encryption.

## [0.6.4]

### Changed

 - [#333](https://github.com/zip-rs/zip/pull/333): disabled the default features of the `time` dependency, and also `formatting` and `macros`, as they were enabled by mistake.
 - Deprecated [`DateTime::from_time`](https://docs.rs/zip/0.6/zip/struct.DateTime.html#method.from_time) in favor of [`DateTime::try_from`](https://docs.rs/zip/0.6/zip/struct.DateTime.html#impl-TryFrom-for-DateTime)
