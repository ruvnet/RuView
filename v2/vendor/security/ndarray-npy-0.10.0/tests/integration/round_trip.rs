//! Tests that read/viewed data match the original written data.

use crate::MaybeAlignedBytes;
use ndarray::array;
use ndarray::prelude::*;
use ndarray_npy::{
    ReadNpyExt, ReadableElement, ViewElement, ViewMutElement, ViewMutNpyExt, ViewNpyExt,
    WritableElement, WriteNpyExt,
};
#[cfg(feature = "num-complex-0_4")]
use num_complex_0_4::Complex;
use std::fmt::Debug;
use std::mem;

/// Tests the following:
///
/// - Writing the array as an `.npy` file and reading it back again or viewing
///   it doesn't change the shape or data.
///
/// - Modifying an `ArrayViewMut` created with `ViewMutNpyExt` correctly
///   changes the underlying data.
fn test_round_trip_single_layout<A, D, F>(
    original: ArrayView<'_, A, D>,
    modified: ArrayView<'_, A, D>,
    modify: F,
) where
    A: Debug + PartialEq + ReadableElement + ViewElement + ViewMutElement + WritableElement,
    D: Dimension,
    F: for<'a> FnOnce(ArrayViewMut<'a, A, D>),
{
    // Write the `.npy` file to a buffer.
    let mut buf = Vec::<u8>::new();
    original.write_npy(&mut buf).unwrap();

    // Get a slice of the bytes in the the written `.npy` file with an
    // alignment usable with `ViewNpyExt`/`ViewMutNpyExt`.
    let mut npy = MaybeAlignedBytes::aligned_from_bytes(buf, mem::align_of::<A>());

    // The read/viewed array should be the same as the original.
    let read = Array::<A, D>::read_npy(&npy[..]).unwrap();
    assert_eq!(&original, &read);
    let view = ArrayView::<A, D>::view_npy(&npy[..]).unwrap();
    assert_eq!(&original, &view);
    let mut view_mut = ArrayViewMut::<A, D>::view_mut_npy(&mut npy[..]).unwrap();
    assert_eq!(&original, &view_mut);

    // Modify the view.
    modify(view_mut.view_mut());
    assert_eq!(&modified, &view_mut);

    // The underlying data should have been modified.
    let read_modified = Array::<A, D>::read_npy(&npy[..]).unwrap();
    assert_eq!(&modified, &read_modified);
}

/// Calls `test_round_trip_single_layout` with standard layout, Fortran layout,
/// and (if ndim > 2) a permuted layout.
fn test_round_trip_multiple_layouts<A, D, F>(
    original: ArrayView<'_, A, D>,
    modified: ArrayView<'_, A, D>,
    mut modify: F,
) where
    A: Clone + Debug + PartialEq + ReadableElement + ViewElement + ViewMutElement + WritableElement,
    D: Dimension,
    F: for<'a> FnMut(ArrayViewMut<'a, A, D>),
{
    // Test with standard layout.
    let standard =
        Array::from_shape_vec(original.raw_dim(), original.iter().cloned().collect()).unwrap();
    test_round_trip_single_layout(standard.view(), modified.view(), &mut modify);

    // Test with Fortran layout.
    let fortran = Array::from_shape_vec(
        original.raw_dim().f(),
        original.t().iter().cloned().collect(),
    )
    .unwrap();
    test_round_trip_single_layout(fortran.view(), modified.view(), &mut modify);

    // Test with permuted axes layout.
    if original.ndim() > 2 {
        // Data with axes 1 and 2 swapped.
        let permuted_data: Vec<_> = {
            let mut perm = original.view();
            perm.swap_axes(1, 2);
            perm.iter().cloned().collect()
        };
        // Shape with axes 1 and 2 swapped.
        let permuted_shape: D = {
            let mut shape = original.raw_dim();
            shape[1] = original.len_of(Axis(2));
            shape[2] = original.len_of(Axis(1));
            shape
        };
        let mut permuted = Array::from_shape_vec(permuted_shape, permuted_data).unwrap();
        permuted.swap_axes(1, 2);
        test_round_trip_single_layout(permuted.view(), modified.view(), &mut modify);
    }
}

#[test]
fn round_trip_i32() {
    test_round_trip_multiple_layouts(
        array![[[1i32, 8], [-3, 4], [2, 9]], [[-5, 0], [7, 38], [-4, 1]]].view(),
        array![[[1i32, 8], [-3, 12], [2, 9]], [[-5, 0], [7, 38], [42, 1]]].view(),
        |mut v| {
            v[[0, 1, 1]] = 12;
            v[[1, 2, 0]] = 42;
        },
    );
}

#[test]
fn round_trip_f32() {
    test_round_trip_multiple_layouts(
        array![
            [[3f32, -1.4], [-159., 26.], [5., -3.5]],
            [[-89.7, 93.], [2., 384.], [-626.4, 3.]],
        ]
        .view(),
        array![
            [[3f32, -1.4], [-159., 12.], [5., -3.5]],
            [[-89.7, 93.], [2., 384.], [42., 3.]],
        ]
        .view(),
        |mut v| {
            v[[0, 1, 1]] = 12.;
            v[[1, 2, 0]] = 42.;
        },
    );
}

#[test]
fn round_trip_f64() {
    test_round_trip_multiple_layouts(
        array![
            [2.7f64, -40.4, -23., 27.8, -49., -43.3],
            [-25.2, 11.8, -8.9, -17.8, 36.4, -25.6],
        ]
        .view(),
        array![
            [2.7f64, 12., -23., 27.8, -49., -43.3],
            [-25.2, 11.8, 42., -17.8, 36.4, -25.6],
        ]
        .view(),
        |mut v| {
            v[[0, 1]] = 12.;
            v[[1, 2]] = 42.;
        },
    );
}

#[cfg(feature = "num-complex-0_4")]
#[test]
fn round_trip_c32() {
    test_round_trip_multiple_layouts(
        array![
            [
                Complex::new(2.7f32, -40.4),
                Complex::new(-23., 27.8),
                Complex::new(-49., -43.3)
            ],
            [
                Complex::new(-25.2, 11.8),
                Complex::new(-8.9, -17.8),
                Complex::new(36.4, -25.6)
            ],
        ]
        .view(),
        array![
            [
                Complex::new(2.7f32, 12.),
                Complex::new(-23., 27.8),
                Complex::new(-49., -43.3)
            ],
            [
                Complex::new(-25.2, 11.8),
                Complex::new(-8.9, -17.8),
                Complex::new(42., -25.6)
            ],
        ]
        .view(),
        |mut v| {
            v[[0, 0]].im = 12.;
            v[[1, 2]].re = 42.;
        },
    );
}

#[test]
fn round_trip_bool() {
    test_round_trip_multiple_layouts(
        array![[[true], [true], [false]], [[false], [true], [false]]].view(),
        array![[[true], [false], [false]], [[false], [true], [true]]].view(),
        |mut v| {
            v[[0, 1, 0]] = false;
            v[[1, 2, 0]] = true;
        },
    );
}
