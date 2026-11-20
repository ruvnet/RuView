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
//! |                                 | Reading                               | Writing                                                  |
//! | ------------------------------- | ------------------------------------- | -------------------------------------------------------- |
//! | Stored                          | ✅                                    | ✅                                                       |
//! | Deflate                         | ✅ [->](`crate::ZipArchive::by_name`) | ✅ [->](`crate::write::FileOptions::compression_method`) |
//! | Deflate64                       | ✅                                    |                                                          |
//! | Bzip2                           | ✅                                    | ✅                                                       |
//! | ZStandard                       | ✅                                    | ✅                                                       |
//! | LZMA                            | ✅                                    |                                                          |
//! | XZ                              | ✅                                    | ✅                                                       |
//! | PPMd                            | ✅                                    | ✅                                                       |
//! | AES encryption                  | ✅                                    | ✅                                                       |
//! | ZipCrypto deprecated encryption | ✅                                    | ✅                                                       |
//!

#![cfg_attr(docsrs, feature(doc_cfg))]
#![warn(missing_docs)]
#![allow(unexpected_cfgs)] // Needed for cfg(fuzzing)
pub use crate::compression::{CompressionMethod, SUPPORTED_COMPRESSION_METHODS};
pub use crate::read::HasZipMetadata;
pub use crate::read::{ZipArchive, ZipReadOptions};
pub use crate::spec::{ZIP64_BYTES_THR, ZIP64_ENTRY_THR};
pub use crate::types::{AesMode, DateTime};
pub use crate::write::ZipWriter;

#[cfg(feature = "aes-crypto")]
mod aes;
#[cfg(feature = "aes-crypto")]
mod aes_ctr;
mod compression;
mod cp437;
mod crc32;
pub mod extra_fields;
mod path;
pub mod read;
pub mod result;
mod spec;
mod types;
pub mod write;
mod zipcrypto;
pub use extra_fields::ExtraField;
#[cfg(feature = "legacy-zip")]
mod legacy;

#[doc = "Unstable APIs\n\
\
All APIs accessible by importing this module are unstable; They may be changed in patch \
releases. You MUST use an exact version specifier in `Cargo.toml`, to indicate the version of this \
API you're using:\n\
\
```toml\n
[dependencies]\n
zip = \"="]
#[doc=env!("CARGO_PKG_VERSION")]
#[doc = "\"\n\
```"]
pub mod unstable;

#[doc(hidden)]
pub mod macros;
