//! Tests for `*Element` trait implementations for primitives.

use crate::MaybeAlignedBytes;
use ndarray_npy::{
    ReadDataError, ReadableElement, ViewDataError, ViewElement, ViewMutElement, WritableElement,
};
use py_literal::Value as PyValue;
use std::convert::TryInto;
use std::io::Cursor;
use std::mem;

#[test]
fn view_i32() {
    let elems: &[i32] = &[34234324, -980780878, 2849874];
    let mut buf: Vec<u8> = Vec::new();
    <i32>::write_slice(elems, &mut buf).unwrap();
    let aligned = MaybeAlignedBytes::aligned_from_bytes(buf, mem::align_of::<i32>());
    #[cfg(target_endian = "little")]
    let type_desc = PyValue::String(String::from("<i4"));
    #[cfg(target_endian = "big")]
    let type_desc = PyValue::String(String::from(">i4"));
    let out: &[i32] = <i32>::bytes_as_slice(&aligned, &type_desc, elems.len()).unwrap();
    assert_eq!(out, elems);
}

#[test]
fn view_i32_mut() {
    let elems: &[i32] = &[34234324, -980780878, 2849874];
    let mut buf: Vec<u8> = Vec::new();
    <i32>::write_slice(elems, &mut buf).unwrap();
    let mut aligned = MaybeAlignedBytes::aligned_from_bytes(buf, mem::align_of::<i32>());
    #[cfg(target_endian = "little")]
    let type_desc = PyValue::String(String::from("<i4"));
    #[cfg(target_endian = "big")]
    let type_desc = PyValue::String(String::from(">i4"));
    let out: &mut [i32] = <i32>::bytes_as_mut_slice(&mut aligned, &type_desc, elems.len()).unwrap();
    assert_eq!(out, elems);
    out[2] += 1;
    let buf_last = i32::from_ne_bytes(aligned[2 * mem::size_of::<i32>()..].try_into().unwrap());
    assert_eq!(buf_last, elems[2] + 1);
}

#[test]
fn view_i32_non_native_endian() {
    const LEN: usize = 3;
    let aligned =
        MaybeAlignedBytes::aligned_zeros(LEN * mem::size_of::<i32>(), mem::align_of::<i32>());
    #[cfg(target_endian = "little")]
    let type_desc = PyValue::String(String::from(">i4"));
    #[cfg(target_endian = "big")]
    let type_desc = PyValue::String(String::from("<i4"));
    let out = <i32>::bytes_as_slice(&aligned, &type_desc, LEN);
    assert!(matches!(out, Err(ViewDataError::NonNativeEndian)));
}

#[test]
fn view_bool() {
    let data = &[0x00, 0x01, 0x00, 0x00, 0x01];
    let type_desc = PyValue::String(String::from("|b1"));
    let out = <bool>::bytes_as_slice(data, &type_desc, data.len()).unwrap();
    assert_eq!(out, &[false, true, false, false, true]);
}

#[test]
fn view_bool_bad_value() {
    let data = &[0x00, 0x01, 0x05, 0x00, 0x01];
    let type_desc = PyValue::String(String::from("|b1"));
    let out = <bool>::bytes_as_slice(data, &type_desc, data.len());
    assert!(matches!(out, Err(ViewDataError::InvalidData(_))));
}

#[test]
fn view_bool_mut() {
    let data = &mut [0x00, 0x01, 0x00, 0x00, 0x01];
    let len = data.len();
    let type_desc = PyValue::String(String::from("|b1"));
    let out = <bool>::bytes_as_mut_slice(data, &type_desc, len).unwrap();
    out[0] = true;
    out[1] = false;
    assert_eq!(data, &[0x01, 0x00, 0x00, 0x00, 0x01]);
}

#[test]
fn view_bool_mut_bad_value() {
    let data = &mut [0x00, 0x01, 0x05, 0x00, 0x01];
    let len = data.len();
    let type_desc = PyValue::String(String::from("|b1"));
    let out = <bool>::bytes_as_mut_slice(data, &type_desc, len);
    assert!(matches!(out, Err(ViewDataError::InvalidData(_))));
}

#[test]
fn read_bool() {
    let data = &[0x00, 0x01, 0x00, 0x00, 0x01];
    let type_desc = PyValue::String(String::from("|b1"));
    let out = <bool>::read_to_end_exact_vec(Cursor::new(data), &type_desc, data.len()).unwrap();
    assert_eq!(out, vec![false, true, false, false, true]);
}

#[test]
fn read_bool_bad_value() {
    let data = &[0x00, 0x01, 0x05, 0x00, 0x01];
    let type_desc = PyValue::String(String::from("|b1"));
    match <bool>::read_to_end_exact_vec(Cursor::new(data), &type_desc, data.len()) {
        Err(ReadDataError::ParseData(err)) => {
            assert_eq!(format!("{}", err), "error parsing value 0x05 as a bool");
        }
        _ => panic!(),
    }
}
