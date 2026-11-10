#![cfg_attr(docsrs, feature(doc_cfg))]

//! This crate provides methods to read/write [`ndarray`]'s
//! [`ArrayBase`](ndarray::ArrayBase) type from/to [`.npy`] and [`.npz`] files.
//!
//! See the [repository] for information about the default features and how to
//! use this crate with Cargo.
//!
//! [repository]: https://github.com/jturner314/ndarray-npy
//! [`ndarray`]: https://github.com/bluss/ndarray
//! [`.npy`]: https://docs.scipy.org/doc/numpy/reference/generated/numpy.lib.format.html
//! [`.npz`]: https://docs.scipy.org/doc/numpy/reference/generated/numpy.savez.html
//!
//! # .npy Files
//!
//! - Reading
//!   - [`ReadNpyExt`] extension trait
//!   - [`read_npy`] convenience function
//! - Writing
//!   - [`WriteNpyExt`] extension trait
//!   - [`write_npy`] and [`create_new_npy`] convenience functions
//!   - [`write_zeroed_npy`] to write an `.npy` file (sparse if possible) of zeroed data
//! - Readonly viewing (primarily for use with memory-mapped files)
//!   - [`ViewNpyExt`] extension trait
//! - Mutable viewing (primarily for use with memory-mapped files)
//!   - [`ViewMutNpyExt`] extension trait
//!
//! It's possible to create `.npy` files larger than the available memory with
//! [`write_zeroed_npy`] and then modify them by memory-mapping and using
//! [`ViewMutNpyExt`].
//!
//! # .npz Files
//!
//! - Reading: [`NpzReader`]
//! - Writing: [`NpzWriter`]
//!
//! # Limitations
//!
//! * Parsing of `.npy` files is currently limited to files where the `descr`
//!   field of the [header dictionary] is a Python string literal of the form
//!   `'string'`, `"string"`, `'''string'''`, or `"""string"""`.
//!
//! * The element traits ([`WritableElement`], [`ReadableElement`],
//!   [`ViewElement`], and [`ViewMutElement`]) are currently implemented only
//!   for fixed-size integers up to 64 bits, floating point numbers, complex
//!   floating point numbers (if enabled with the crate feature), and [`bool`].
//!
//! The plan is to add support for more element types (including custom
//! user-defined structs) in the future.
//!
//! [header dictionary]: https://docs.scipy.org/doc/numpy/reference/generated/numpy.lib.format.html#format-version-1-0

pub mod npy;
#[cfg(feature = "npz")]
mod npz;

pub use crate::npy::{
    create_new_npy, read_npy, write_npy, write_zeroed_npy, ReadDataError, ReadNpyError, ReadNpyExt,
    ReadableElement, ViewDataError, ViewElement, ViewMutElement, ViewMutNpyExt, ViewNpyError,
    ViewNpyExt, WritableElement, WriteDataError, WriteNpyError, WriteNpyExt,
};
#[cfg(feature = "npz")]
pub use crate::npz::{NpzReader, NpzWriter, ReadNpzError, WriteNpzError};
