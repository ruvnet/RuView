//! Miscellaneous example cases.

use crate::{file_to_aligned_bytes, file_to_aligned_mut_bytes, MaybeAlignedBytes};
use ndarray::prelude::*;
use ndarray::Slice;
use ndarray_npy::{
    write_zeroed_npy, ReadNpyError, ReadNpyExt, ViewMutNpyExt, ViewNpyError, ViewNpyExt,
    WriteNpyExt,
};
#[cfg(feature = "num-complex-0_4")]
use num_complex_0_4::Complex;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::mem;

#[track_caller]
fn assert_written_is_correct<T: WriteNpyExt + ?Sized>(arr: &T, correct: &[u8]) {
    let mut writer = Vec::<u8>::new();
    arr.write_npy(&mut writer).unwrap();
    assert_eq!(&correct, &writer);
}

#[test]
fn write_f64_standard() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_f64_little_endian_standard.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_f64_big_endian_standard.npy";

    let correct = fs::read(path).unwrap();
    let mut arr = Array3::<f64>::zeros((2, 3, 4));
    for (i, elem) in arr.iter_mut().enumerate() {
        *elem = i as f64;
    }
    assert_written_is_correct(&arr, &correct);
    let arr_ref: &ArrayRef3<f64> = &*arr;
    assert_written_is_correct(arr_ref, &correct);
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn write_c64_standard() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_c64_little_endian_standard.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_c64_big_endian_standard.npy";

    let correct = fs::read(path).unwrap();
    let mut arr = Array3::<Complex<f64>>::zeros((2, 3, 4));
    for (i, elem) in arr.iter_mut().enumerate() {
        // The `+ 0.` is necessary to get the same behavior as Python with
        // respect to signed zeros.
        *elem = Complex::new(i as f64, -(i as f64) + 0.);
    }
    assert_written_is_correct(&arr, &correct);
    let arr_ref: &ArrayRef3<Complex<f64>> = &*arr;
    assert_written_is_correct(arr_ref, &correct);
}

#[test]
fn write_f64_fortran() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_f64_little_endian_fortran.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_f64_big_endian_fortran.npy";

    let correct = fs::read(path).unwrap();
    let mut arr = Array3::<f64>::zeros((2, 3, 4).f());
    for (i, elem) in arr.iter_mut().enumerate() {
        *elem = i as f64;
    }
    assert_written_is_correct(&arr, &correct);
    let arr_ref: &ArrayRef3<f64> = &*arr;
    assert_written_is_correct(arr_ref, &correct);
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn write_c64_fortran() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_c64_little_endian_fortran.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_c64_big_endian_fortran.npy";

    let correct = fs::read(path).unwrap();
    let mut arr = Array3::<Complex<f64>>::zeros((2, 3, 4).f());
    for (i, elem) in arr.iter_mut().enumerate() {
        // The `+ 0.` is necessary to get the same behavior as Python with
        // respect to signed zeros.
        *elem = Complex::new(i as f64, -(i as f64) + 0.);
    }
    assert_written_is_correct(&arr, &correct);
    let arr_ref: &ArrayRef3<Complex<f64>> = &*arr;
    assert_written_is_correct(arr_ref, &correct);
}

#[test]
fn write_f64_discontiguous() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_f64_little_endian_standard.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_f64_big_endian_standard.npy";

    let correct = fs::read(path).unwrap();
    let mut arr = Array3::<f64>::zeros((3, 4, 4));
    arr.slice_axis_inplace(Axis(1), Slice::new(0, None, 2));
    arr.swap_axes(0, 1);
    for (i, elem) in arr.iter_mut().enumerate() {
        *elem = i as f64;
    }
    assert_written_is_correct(&arr, &correct);
    let arr_ref: &ArrayRef3<f64> = &*arr;
    assert_written_is_correct(arr_ref, &correct);
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn write_c64_discontiguous() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_c64_little_endian_standard.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_c64_big_endian_standard.npy";

    let correct = fs::read(path).unwrap();
    let mut arr = Array3::<Complex<f64>>::zeros((3, 4, 4));
    arr.slice_axis_inplace(Axis(1), Slice::new(0, None, 2));
    arr.swap_axes(0, 1);
    for (i, elem) in arr.iter_mut().enumerate() {
        // The `+ 0.` is necessary to get the same behavior as Python with
        // respect to signed zeros.
        *elem = Complex::new(i as f64, -(i as f64) + 0.);
    }
    assert_written_is_correct(&arr, &correct);
    let arr_ref: &ArrayRef3<Complex<f64>> = &*arr;
    assert_written_is_correct(arr_ref, &correct);
}

#[test]
fn read_f64_standard() {
    let mut correct = Array3::<f64>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = i as f64;
    }
    for path in &[
        "resources/example_f64_little_endian_standard.npy",
        "resources/example_f64_big_endian_standard.npy",
    ] {
        let file = File::open(path).unwrap();
        let arr = Array3::<f64>::read_npy(file).unwrap();
        assert_eq!(correct, arr);
        assert!(arr.is_standard_layout());
    }
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn read_c64_standard() {
    let mut correct = Array3::<Complex<f64>>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = Complex::new(i as f64, -(i as f64));
    }
    for path in &[
        "resources/example_c64_little_endian_standard.npy",
        "resources/example_c64_big_endian_standard.npy",
    ] {
        let file = File::open(path).unwrap();
        let arr = Array3::<Complex<f64>>::read_npy(file).unwrap();
        assert_eq!(correct, arr);
        assert!(arr.is_standard_layout());
    }
}

#[test]
fn read_f64_fortran() {
    let mut correct = Array3::<f64>::zeros((2, 3, 4).f());
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = i as f64;
    }
    for path in &[
        "resources/example_f64_little_endian_fortran.npy",
        "resources/example_f64_big_endian_fortran.npy",
    ] {
        let file = File::open(path).unwrap();
        let arr = Array3::<f64>::read_npy(file).unwrap();
        assert_eq!(correct, arr);
        assert!(arr.t().is_standard_layout());
    }
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn read_c64_fortran() {
    let mut correct = Array3::<Complex<f64>>::zeros((2, 3, 4).f());
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = Complex::new(i as f64, -(i as f64));
    }
    for path in &[
        "resources/example_c64_little_endian_fortran.npy",
        "resources/example_c64_big_endian_fortran.npy",
    ] {
        let file = File::open(path).unwrap();
        let arr = Array3::<Complex<f64>>::read_npy(file).unwrap();
        assert_eq!(correct, arr);
        assert!(arr.t().is_standard_layout());
    }
}

#[test]
fn read_bool() {
    let mut correct = Array3::from_elem((2, 3, 4), false);
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = (i % 5) % 2 == 0;
    }
    let file = File::open("resources/example_bool_standard.npy").unwrap();
    let arr = Array3::<bool>::read_npy(file).unwrap();
    assert_eq!(correct, arr);
}

#[test]
fn read_bool_bad_value() {
    let file = File::open("resources/example_bool_bad_value.npy").unwrap();
    assert!(matches!(
        Array3::<bool>::read_npy(file),
        Err(ReadNpyError::ParseData(_))
    ));
}

#[test]
fn view_f64_standard() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_f64_little_endian_standard.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_f64_big_endian_standard.npy";

    let mut correct = Array3::<f64>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = i as f64;
    }
    let file = File::open(path).unwrap();
    let bytes = unsafe { file_to_aligned_bytes(&file).unwrap() };
    let view = ArrayView3::<f64>::view_npy(&bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.is_standard_layout());
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn view_c64_standard() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_c64_little_endian_standard.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_c64_big_endian_standard.npy";

    let mut correct = Array3::<Complex<f64>>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = Complex::new(i as f64, -(i as f64));
    }
    let file = File::open(path).unwrap();
    let bytes = unsafe { file_to_aligned_bytes(&file).unwrap() };
    let view = ArrayView3::<Complex<f64>>::view_npy(&bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.is_standard_layout());
}

#[test]
fn view_f64_fortran() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_f64_little_endian_fortran.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_f64_big_endian_fortran.npy";

    let mut correct = Array3::<f64>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = i as f64;
    }
    let file = File::open(path).unwrap();
    let bytes = unsafe { file_to_aligned_bytes(&file).unwrap() };
    let view = ArrayView3::<f64>::view_npy(&bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.t().is_standard_layout());
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn view_c64_fortran() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_c64_little_endian_fortran.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_c64_big_endian_fortran.npy";

    let mut correct = Array3::<Complex<f64>>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = Complex::new(i as f64, -(i as f64));
    }
    let file = File::open(path).unwrap();
    let bytes = unsafe { file_to_aligned_bytes(&file).unwrap() };
    let view = ArrayView3::<Complex<f64>>::view_npy(&bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.t().is_standard_layout());
}

#[test]
fn view_bool() {
    let mut correct = Array3::from_elem((2, 3, 4), false);
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = (i % 5) % 2 == 0;
    }
    let file = File::open("resources/example_bool_standard.npy").unwrap();
    let bytes = unsafe { file_to_aligned_bytes(&file).unwrap() };
    let view = ArrayView3::<bool>::view_npy(&bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.is_standard_layout());
}

#[test]
fn view_bool_bad_value() {
    let file = File::open("resources/example_bool_bad_value.npy").unwrap();
    let bytes = unsafe { file_to_aligned_bytes(&file).unwrap() };
    assert!(matches!(
        ArrayView3::<bool>::view_npy(&bytes),
        Err(ViewNpyError::InvalidData(_))
    ));
}

#[test]
fn view_mut_f64_standard() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_f64_little_endian_standard.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_f64_big_endian_standard.npy";

    let mut correct = Array3::<f64>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = i as f64;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    let mut bytes = unsafe { file_to_aligned_mut_bytes(&file).unwrap() };
    let view = ArrayViewMut3::<f64>::view_mut_npy(&mut bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.is_standard_layout());
}

#[test]
fn view_mut_f64_fortran() {
    #[cfg(target_endian = "little")]
    let path = "resources/example_f64_little_endian_fortran.npy";
    #[cfg(target_endian = "big")]
    let path = "resources/example_f64_big_endian_fortran.npy";

    let mut correct = Array3::<f64>::zeros((2, 3, 4));
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = i as f64;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    let mut bytes = unsafe { file_to_aligned_mut_bytes(&file).unwrap() };
    let view = ArrayViewMut3::<f64>::view_mut_npy(&mut bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.t().is_standard_layout());
}

#[test]
fn view_mut_bool() {
    let mut correct = Array3::from_elem((2, 3, 4), false);
    for (i, elem) in correct.iter_mut().enumerate() {
        *elem = (i % 5) % 2 == 0;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("resources/example_bool_standard.npy")
        .unwrap();
    let mut bytes = unsafe { file_to_aligned_mut_bytes(&file).unwrap() };
    let view = ArrayViewMut3::<bool>::view_mut_npy(&mut bytes).unwrap();
    assert_eq!(correct, view);
    assert!(view.is_standard_layout());
}

#[test]
fn view_mut_bool_bad_value() {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("resources/example_bool_bad_value.npy")
        .unwrap();
    let mut bytes = unsafe { file_to_aligned_mut_bytes(&file).unwrap() };
    assert!(matches!(
        ArrayViewMut3::<bool>::view_mut_npy(&mut bytes),
        Err(ViewNpyError::InvalidData(_))
    ));
}

#[test]
fn misaligned() {
    let mut buf = Vec::<u8>::new();
    let mut arr = Array3::<f64>::zeros((2, 3, 4));
    for (i, elem) in arr.iter_mut().enumerate() {
        *elem = i as f64;
    }
    arr.write_npy(&mut buf).unwrap();
    let mut misaligned = MaybeAlignedBytes::misaligned_from_bytes(buf, mem::align_of::<f64>());
    assert!(matches!(
        ArrayView3::<f64>::view_npy(&misaligned[..]),
        Err(ViewNpyError::MisalignedData)
    ));
    assert!(matches!(
        ArrayViewMut3::<f64>::view_mut_npy(&mut misaligned[..]),
        Err(ViewNpyError::MisalignedData)
    ));
}

#[test]
#[cfg_attr(miri, ignore)] // issues with tempfile
fn zeroed() {
    const EXISTING_DATA: &[u8] = b"hello";
    const SHAPE: [usize; 3] = [3, 4, 5];

    // Create a file with some existing data.
    let mut file: File = tempfile::tempfile().unwrap();
    file.write_all(EXISTING_DATA).unwrap();

    // Write `.npy` file with zeroed data.
    write_zeroed_npy::<i32, _>(&file, SHAPE).unwrap();

    // Reset cursor and verify EXISTING_DATA is still there.
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut buf = [0; EXISTING_DATA.len()];
    file.read_exact(&mut buf).unwrap();
    assert_eq!(EXISTING_DATA, buf);

    // Read and verify the `.npy` file.
    let arr = Array3::<i32>::read_npy(file).unwrap();
    assert_eq!(arr, Array3::<i32>::zeros(SHAPE));
    assert!(arr.is_standard_layout());
}
