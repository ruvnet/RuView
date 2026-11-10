use super::check_for_extra_bytes;
use crate::{ReadDataError, ReadableElement};
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use num_complex_0_4::Complex;
use py_literal::Value as PyValue;
use std::io;
use std::mem;
use std::slice;

/// Casts the slice of complex numbers to a slice of the inner values.
///
/// The resulting slice alternates between the real and imaginary parts of
/// consecutive complex elements.
///
/// # Panics
///
/// Panics if `T` is zero-sized.
fn complex_slice_as_inner_slice_mut<T>(slice: &mut [Complex<T>]) -> &mut [T] {
    assert!(mem::size_of::<T>() > 0);

    // These assertions should always pass, since `Complex` is `repr(C)` and
    // has only two fields, both of type `T`.
    assert_eq!(
        mem::size_of::<Complex<T>>(),
        mem::size_of::<T>().checked_mul(2).unwrap()
    );
    assert_eq!(mem::align_of::<Complex<T>>(), mem::align_of::<T>());

    // This should never panic, because we've checked that `T` is not
    // zero-sized, and slices are guaranteed to contain no more than
    // `isize::MAX` bytes. (See the docs of `std::slice::from_raw_parts`.)
    let inner_len = slice.len().checked_mul(2).unwrap();

    // This is sound because:
    //
    // - Since `slice` is an existing slice, its pointer is non-null, it's
    //   properly aligned, its length is correct, the elements are initialized,
    //   and the lifetime is correct.
    //
    // - We've checked above that `inner_len` will be the correct length and
    //   the alignment will be the same.
    //
    // - The fields of `Complex` are public, so we aren't violating any
    //   visibility constraints.
    unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr().cast(), inner_len) }
}

macro_rules! impl_readable_complex_multi_byte {
    ($elem:ty, $little_desc:expr, $big_desc:expr, $zero:expr, $inner_read_into:ident) => {
        impl ReadableElement for $elem {
            fn read_to_end_exact_vec<R: io::Read>(
                mut reader: R,
                type_desc: &PyValue,
                len: usize,
            ) -> Result<Vec<Self>, ReadDataError> {
                let mut out = vec![$zero; len];
                let inner_slice = complex_slice_as_inner_slice_mut(&mut out);
                match *type_desc {
                    PyValue::String(ref s) if s == $little_desc => {
                        reader.$inner_read_into::<LittleEndian>(inner_slice)?;
                    }
                    PyValue::String(ref s) if s == $big_desc => {
                        reader.$inner_read_into::<BigEndian>(inner_slice)?;
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

macro_rules! impl_complex_multi_byte {
    ($elem:ty, $little_desc:expr, $big_desc:expr, $zero:expr, $inner_read_into:ident) => {
        impl_writable_element_always_valid_cast!($elem, $little_desc, $big_desc);
        impl_readable_complex_multi_byte!($elem, $little_desc, $big_desc, $zero, $inner_read_into);
        #[cfg(target_endian = "little")]
        impl_view_and_view_mut_always_valid_cast_multi_byte!($elem, $little_desc, $big_desc);
        #[cfg(target_endian = "big")]
        impl_view_and_view_mut_always_valid_cast_multi_byte!($elem, $big_desc, $little_desc);
    };
}

impl_complex_multi_byte!(
    Complex<f32>,
    "<c8",
    ">c8",
    Complex::new(0., 0.),
    read_f32_into
);
impl_complex_multi_byte!(
    Complex<f64>,
    "<c16",
    ">c16",
    Complex::new(0., 0.),
    read_f64_into
);
