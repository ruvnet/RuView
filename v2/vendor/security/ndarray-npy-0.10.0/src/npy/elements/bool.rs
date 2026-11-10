//! Trait implementations for `bool`.

use super::{bytes_as_mut_slice, bytes_as_slice, check_for_extra_bytes};
use crate::{ReadDataError, ReadableElement, ViewDataError, ViewElement, ViewMutElement};
use py_literal::Value as PyValue;
use std::error::Error;
use std::fmt;
use std::io;
use std::mem;

/// An error parsing a `bool` from a byte.
#[derive(Debug)]
struct ParseBoolError {
    bad_value: u8,
}

impl Error for ParseBoolError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl fmt::Display for ParseBoolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "error parsing value {:#04x} as a bool", self.bad_value)
    }
}

impl From<ParseBoolError> for ReadDataError {
    fn from(err: ParseBoolError) -> ReadDataError {
        ReadDataError::ParseData(Box::new(err))
    }
}

impl From<ParseBoolError> for ViewDataError {
    fn from(err: ParseBoolError) -> ViewDataError {
        ViewDataError::InvalidData(Box::new(err))
    }
}

/// Returns `Ok(_)` iff each of the bytes is a valid bitwise representation for
/// `bool`.
///
/// In other words, this checks that each byte is `0x00` or `0x01`, which is
/// important for the bytes to be reinterpreted as `bool`, since creating a
/// `bool` with an invalid value is undefined behavior. Rust guarantees that
/// `false` is represented as `0x00` and `true` is represented as `0x01`.
fn check_valid_for_bool(bytes: &[u8]) -> Result<(), ParseBoolError> {
    for &byte in bytes {
        if byte > 1 {
            return Err(ParseBoolError { bad_value: byte });
        }
    }
    Ok(())
}

impl ReadableElement for bool {
    fn read_to_end_exact_vec<R: io::Read>(
        mut reader: R,
        type_desc: &PyValue,
        len: usize,
    ) -> Result<Vec<Self>, ReadDataError> {
        match *type_desc {
            PyValue::String(ref s) if s == "|b1" => {
                // Read the data.
                let mut bytes: Vec<u8> = vec![0; len];
                reader.read_exact(&mut bytes)?;
                check_for_extra_bytes(&mut reader)?;

                // Check that the data is valid for interpretation as `bool`.
                check_valid_for_bool(&bytes)?;

                // Cast the `Vec<u8>` to `Vec<bool>`.
                {
                    let ptr: *mut u8 = bytes.as_mut_ptr();
                    let len: usize = bytes.len();
                    let cap: usize = bytes.capacity();
                    mem::forget(bytes);
                    // This is safe because:
                    //
                    // * All elements are valid `bool`s. (See the call to
                    //   `check_valid_for_bool` above.)
                    //
                    // * `ptr` was originally allocated by `Vec`.
                    //
                    // * `bool` has the same size and alignment as `u8`.
                    //
                    // * `len` and `cap` are copied directly from the
                    //   `Vec<u8>`, so `len <= cap` and `cap` is the capacity
                    //   `ptr` was allocated with.
                    Ok(unsafe { Vec::from_raw_parts(ptr.cast::<bool>(), len, cap) })
                }
            }
            ref other => Err(ReadDataError::WrongDescriptor(other.clone())),
        }
    }
}

// Rust guarantees that `bool` is one byte, the bitwise representation of
// `false` is `0x00`, and the bitwise representation of `true` is `0x01`, so we
// can just cast the data in-place.
impl_writable_element_always_valid_cast!(bool, "|b1", "|b1");

impl ViewElement for bool {
    fn bytes_as_slice<'a>(
        bytes: &'a [u8],
        type_desc: &PyValue,
        len: usize,
    ) -> Result<&'a [Self], ViewDataError> {
        match *type_desc {
            PyValue::String(ref s) if s == "|b1" => {
                check_valid_for_bool(bytes)?;
                unsafe { bytes_as_slice(bytes, len) }
            }
            ref other => Err(ViewDataError::WrongDescriptor(other.clone())),
        }
    }
}

impl ViewMutElement for bool {
    fn bytes_as_mut_slice<'a>(
        bytes: &'a mut [u8],
        type_desc: &PyValue,
        len: usize,
    ) -> Result<&'a mut [Self], ViewDataError> {
        match *type_desc {
            PyValue::String(ref s) if s == "|b1" => {
                check_valid_for_bool(bytes)?;
                unsafe { bytes_as_mut_slice(bytes, len) }
            }
            ref other => Err(ViewDataError::WrongDescriptor(other.clone())),
        }
    }
}
