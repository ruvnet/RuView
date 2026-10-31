//! A library for reading and writing ZIP archives.
//! ZIP is a format designed for cross-platform file "archiving".
//! That is, storing a collection of files in a single datastream
//! to make them easier to share between computers.
//! Additionally, ZIP is able to compress and encrypt files in its
//! archives.
//!
//! The current implementation is based on [PKWARE's APPNOTE.TXT v6.3.9](https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT)
//!
//! ---
//!
//! [`zip`](`crate`) has support for the most common ZIP archives found in common use.
//! However, in special cases,
//! there are some zip archives that are difficult to read or write.
//!
//! This is a list of supported features:
//!
//! |         | Reading | Writing |
//! | ------- | ------  | ------- |
//! | Deflate | ✅ [->](`crate::ZipArchive::by_name`)      | ✅ [->](`crate::write::FileOptions::compression_method`) |
//!
//!
//!

#![warn(missing_docs)]

pub use crate::compression::{CompressionMethod, SUPPORTED_COMPRESSION_METHODS};
pub use crate::read::ZipArchive;
pub use crate::types::DateTime;
pub use crate::write::ZipWriter;

#[cfg(feature = "aes-crypto")]
mod aes;
#[cfg(feature = "aes-crypto")]
mod aes_ctr;
mod compression;
mod cp437;
mod crc32;
pub mod read;
pub mod result;
mod spec;
mod types;
pub mod write;
mod zipcrypto;

/// Unstable APIs
///
/// All APIs accessible by importing this module are unstable; They may be changed in patch releases.
/// You MUST you an exact version specifier in `Cargo.toml`, to indicate the version of this API you're using:
///
/// ```toml
/// [dependencies]
/// zip = "=0.6.6"
/// ```
pub mod unstable;
