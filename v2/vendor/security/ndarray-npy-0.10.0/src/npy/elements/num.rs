//! Trait implementations for primitive numeric types.

use super::{bytes_as_mut_slice, bytes_as_slice, check_for_extra_bytes};
use crate::{ReadDataError, ReadableElement, ViewDataError, ViewElement, ViewMutElement};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use py_literal::Value as PyValue;
use std::io;

macro_rules! impl_readable_primitive_one_byte {
    ($elem:ty, [$($desc:expr),*], $zero:expr, $read_into:ident) => {
        impl ReadableElement for $elem {
            fn read_to_end_exact_vec<R: io::Read>(
                mut reader: R,
                type_desc: &PyValue,
                len: usize,
            ) -> Result<Vec<Self>, ReadDataError> {
                match *type_desc {
                    PyValue::String(ref s) if $(s == $desc)||* => {
                        let mut out = vec![$zero; len];
                        reader.$read_into(&mut out)?;
                        check_for_extra_bytes(&mut reader)?;
                        Ok(out)
                    }
                    ref other => Err(ReadDataError::WrongDescriptor(other.clone())),
                }
            }
        }
    };
}

macro_rules! impl_view_and_view_mut_primitive_one_byte {
    ($elem:ty, [$($desc:expr),*]) => {
        impl ViewElement for $elem {
            fn bytes_as_slice<'a>(
                bytes: &'a [u8],
                type_desc: &PyValue,
                len: usize,
            ) -> Result<&'a [Self], ViewDataError> {
                match *type_desc {
                    PyValue::String(ref s) if $(s == $desc)||* => unsafe {
                        bytes_as_slice(bytes, len)
                    }
                    ref other => Err(ViewDataError::WrongDescriptor(other.clone())),
                }
            }
        }

        impl ViewMutElement for $elem {
            fn bytes_as_mut_slice<'a>(
                bytes: &'a mut [u8],
                type_desc: &PyValue,
                len: usize,
            ) -> Result<&'a mut [Self], ViewDataError> {
                match *type_desc {
                    PyValue::String(ref s) if $(s == $desc)||* => unsafe {
                        bytes_as_mut_slice(bytes, len)
                    }
                    ref other => Err(ViewDataError::WrongDescriptor(other.clone())),
                }
            }
        }
    };
}

macro_rules! impl_primitive_one_byte {
    ($elem:ty, $write_desc:expr, [$($read_desc:expr),*], $zero:expr, $read_into:ident) => {
        impl_writable_element_always_valid_cast!($elem, $write_desc, $write_desc);
        impl_readable_primitive_one_byte!($elem, [$($read_desc),*], $zero, $read_into);
        impl_view_and_view_mut_primitive_one_byte!($elem, [$($read_desc),*]);
    };
}

impl_primitive_one_byte!(i8, "|i1", ["|i1", "i1", "b"], 0, read_i8_into);
impl_primitive_one_byte!(u8, "|u1", ["|u1", "u1", "B"], 0, read_exact);

macro_rules! impl_readable_primitive_multi_byte {
    ($elem:ty, $little_desc:expr, $big_desc:expr, $zero:expr, $read_into:ident) => {
        impl ReadableElement for $elem {
            fn read_to_end_exact_vec<R: io::Read>(
                mut reader: R,
                type_desc: &PyValue,
                len: usize,
            ) -> Result<Vec<Self>, ReadDataError> {
                let mut out = vec![$zero; len];
                match *type_desc {
                    PyValue::String(ref s) if s == $little_desc => {
                        reader.$read_into::<LittleEndian>(&mut out)?;
                    }
                    PyValue::String(ref s) if s == $big_desc => {
                        reader.$read_into::<BigEndian>(&mut out)?;
                    }
                    ref other => {
                        return Err(ReadDataError::WrongDescriptor(other.clone()));
                    }
                }
                check_for_extra_bytes(&mut reader)?;
                Ok(out)
            }
        }
    };
}

macro_rules! impl_primitive_multi_byte {
    ($elem:ty, $little_desc:expr, $big_desc:expr, $zero:expr, $read_into:ident) => {
        impl_writable_element_always_valid_cast!($elem, $little_desc, $big_desc);
        impl_readable_primitive_multi_byte!($elem, $little_desc, $big_desc, $zero, $read_into);
        #[cfg(target_endian = "little")]
        impl_view_and_view_mut_always_valid_cast_multi_byte!($elem, $little_desc, $big_desc);
        #[cfg(target_endian = "big")]
        impl_view_and_view_mut_always_valid_cast_multi_byte!($elem, $big_desc, $little_desc);
    };
}

impl_primitive_multi_byte!(i16, "<i2", ">i2", 0, read_i16_into);
impl_primitive_multi_byte!(i32, "<i4", ">i4", 0, read_i32_into);
impl_primitive_multi_byte!(i64, "<i8", ">i8", 0, read_i64_into);

impl_primitive_multi_byte!(u16, "<u2", ">u2", 0, read_u16_into);
impl_primitive_multi_byte!(u32, "<u4", ">u4", 0, read_u32_into);
impl_primitive_multi_byte!(u64, "<u8", ">u8", 0, read_u64_into);

impl_primitive_multi_byte!(f32, "<f4", ">f4", 0., read_f32_into);
impl_primitive_multi_byte!(f64, "<f8", ">f8", 0., read_f64_into);
