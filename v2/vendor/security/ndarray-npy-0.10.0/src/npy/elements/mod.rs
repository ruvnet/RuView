//! Implementations of the `*Element` traits.

use crate::{ReadDataError, ViewDataError};
use std::io;
use std::mem;
use std::slice;

/// Returns `Ok(_)` iff the `reader` had no more bytes on entry to this
/// function.
///
/// **Warning** This will consume the remainder of the reader.
fn check_for_extra_bytes<R: io::Read>(reader: &mut R) -> Result<(), ReadDataError> {
    let num_extra_bytes = reader.read_to_end(&mut Vec::new())?;
    if num_extra_bytes == 0 {
        Ok(())
    } else {
        Err(ReadDataError::ExtraBytes(num_extra_bytes))
    }
}

/// Returns `Ok(_)` iff a slice containing `bytes_len` bytes is the correct length to cast to
/// a slice with element type `T` and length `len`.
///
/// **Panics** if `len * size_of::<T>()` overflows.
fn check_bytes_len<T>(bytes_len: usize, len: usize) -> Result<(), ViewDataError> {
    use std::cmp::Ordering;
    let needed_bytes = len
        .checked_mul(mem::size_of::<T>())
        .expect("Required number of bytes should not overflow.");
    match bytes_len.cmp(&needed_bytes) {
        Ordering::Less => Err(ViewDataError::MissingBytes(needed_bytes - bytes_len)),
        Ordering::Equal => Ok(()),
        Ordering::Greater => Err(ViewDataError::ExtraBytes(bytes_len - needed_bytes)),
    }
}

/// Returns `Ok(_)` iff the slice of bytes is properly aligned to be cast to a
/// slice with element type `T`.
fn check_bytes_align<T>(bytes: &[u8]) -> Result<(), ViewDataError> {
    if bytes.as_ptr() as usize % mem::align_of::<T>() == 0 {
        Ok(())
    } else {
        Err(ViewDataError::Misaligned)
    }
}

/// Cast `&[u8]` to `&[T]`, where the resulting slice should have length `len`.
///
/// Returns `Err` if the length or alignment of `bytes` is incorrect.
///
/// # Safety
///
/// The caller must ensure that the cast is valid for the type `T`. For
/// example, this is not true for `bool` unless the caller has previously
/// checked that the byte slice contains only `0x00` and `0x01` values. (This
/// function checks only that the length and alignment are correct.)
unsafe fn bytes_as_slice<T>(bytes: &[u8], len: usize) -> Result<&[T], ViewDataError> {
    check_bytes_len::<T>(bytes.len(), len)?;
    check_bytes_align::<T>(bytes)?;
    Ok(slice::from_raw_parts(bytes.as_ptr().cast(), len))
}

/// Cast `&mut [u8]` to `&mut [T]`, where the resulting slice should have
/// length `len`.
///
/// Returns `Err` if the length or alignment of `bytes` is incorrect.
///
/// # Safety
///
/// The caller must ensure that the cast is valid for the type `T`. For
/// example, this is not true for `bool` unless the caller has previously
/// checked that the byte slice contains only `0x00` and `0x01` values. (This
/// function checks only that the length and alignment are correct.)
unsafe fn bytes_as_mut_slice<T>(bytes: &mut [u8], len: usize) -> Result<&mut [T], ViewDataError> {
    check_bytes_len::<T>(bytes.len(), len)?;
    check_bytes_align::<T>(bytes)?;
    Ok(slice::from_raw_parts_mut(bytes.as_mut_ptr().cast(), len))
}

/// Cast `&T` to `&[u8]`.
///
/// # Safety
///
/// The caller must ensure that it is safe to read all bytes in the element.
/// For example, this is violated if `T` has padding bytes.
unsafe fn value_as_bytes<T>(elem: &T) -> &[u8] {
    let ptr: *const T = elem;
    slice::from_raw_parts(ptr.cast::<u8>(), mem::size_of::<T>())
}

/// Cast `&[T]` to `&[u8]`.
///
/// # Safety
///
/// The caller must ensure that it is safe to read all bytes in the slice. For
/// example, this is violated if `T` has padding bytes.
unsafe fn slice_as_bytes<T>(slice: &[T]) -> &[u8] {
    slice::from_raw_parts(
        slice.as_ptr().cast::<u8>(),
        // This unwrap should never panic, because slices always contain no
        // more than `isize::MAX` bytes.
        slice.len().checked_mul(mem::size_of::<T>()).unwrap(),
    )
}

/// Implements `ViewElement` and `ViewMutElement` for a multi-byte type.
///
/// # Safety
///
/// The caller must ensure that it is always safe to call `bytes_as_slice` and
/// `bytes_as_mut_slice` with `$elem` as the type `T`.
macro_rules! impl_view_and_view_mut_always_valid_cast_multi_byte {
    ($elem:ty, $native_desc:expr, $non_native_desc:expr) => {
        impl $crate::ViewElement for $elem {
            fn bytes_as_slice<'a>(
                bytes: &'a [u8],
                type_desc: &::py_literal::Value,
                len: usize,
            ) -> Result<&'a [Self], $crate::ViewDataError> {
                match *type_desc {
                    ::py_literal::Value::String(ref s) if s == $native_desc => unsafe {
                        $crate::npy::elements::bytes_as_slice(bytes, len)
                    },
                    ::py_literal::Value::String(ref s) if s == $non_native_desc => {
                        Err($crate::ViewDataError::NonNativeEndian)
                    }
                    ref other => Err($crate::ViewDataError::WrongDescriptor(
                        ::std::clone::Clone::clone(other),
                    )),
                }
            }
        }

        impl $crate::ViewMutElement for $elem {
            fn bytes_as_mut_slice<'a>(
                bytes: &'a mut [u8],
                type_desc: &::py_literal::Value,
                len: usize,
            ) -> Result<&'a mut [Self], $crate::ViewDataError> {
                match *type_desc {
                    ::py_literal::Value::String(ref s) if s == $native_desc => unsafe {
                        $crate::npy::elements::bytes_as_mut_slice(bytes, len)
                    },
                    ::py_literal::Value::String(ref s) if s == $non_native_desc => {
                        Err($crate::ViewDataError::NonNativeEndian)
                    }
                    ref other => Err($crate::ViewDataError::WrongDescriptor(
                        ::std::clone::Clone::clone(other),
                    )),
                }
            }
        }
    };
}

/// Implements `WritableElement` for a type.
///
/// # Safety
///
/// The caller must ensure that it is always safe to call `value_as_bytes` and
/// `slice_as_bytes` with `$elem` as the type `T`.
macro_rules! impl_writable_element_always_valid_cast {
    ($elem:ty, $little_desc:expr, $big_desc:expr) => {
        impl $crate::WritableElement for $elem {
            fn type_descriptor() -> ::py_literal::Value {
                use std::convert::Into;
                if cfg!(target_endian = "little") {
                    ::py_literal::Value::String($little_desc.into())
                } else if cfg!(target_endian = "big") {
                    ::py_literal::Value::String($big_desc.into())
                } else {
                    unreachable!()
                }
            }

            fn write<W: ::std::io::Write>(
                &self,
                mut writer: W,
            ) -> Result<(), $crate::WriteDataError> {
                ::std::io::Write::write_all(&mut writer, unsafe {
                    $crate::npy::elements::value_as_bytes::<Self>(self)
                })?;
                Ok(())
            }

            fn write_slice<W: ::std::io::Write>(
                slice: &[Self],
                mut writer: W,
            ) -> Result<(), $crate::WriteDataError> {
                ::std::io::Write::write_all(&mut writer, unsafe {
                    $crate::npy::elements::slice_as_bytes::<Self>(slice)
                })?;
                Ok(())
            }
        }
    };
}

mod bool;
#[cfg(feature = "num-complex-0_4")]
mod complex;
mod num;
