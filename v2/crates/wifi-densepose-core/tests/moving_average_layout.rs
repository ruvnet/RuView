//! Moving-average behavior for owned arrays with nonstandard strides.
use ndarray::{array, Axis, Slice};
use wifi_densepose_core::utils::moving_average;

#[test]
fn moving_average_filters_strided_owned_input() {
    let mut data = array![1., 90., 2., 90., 9., 90., 4., 90., 5.];
    data.slice_axis_inplace(Axis(0), Slice::new(0, None, 2));
    assert!(data.as_slice().is_none());
    let result = moving_average(&data, 3);
    assert_eq!(result, array![1.5, 4., 5., 6., 4.5]);
}

#[test]
fn moving_average_preserves_logical_order_for_reversed_input() {
    let mut data = array![1., 2., 9., 4., 5.];
    data.invert_axis(Axis(0));
    assert!(data.as_slice().is_none());
    let result = moving_average(&data, 3);
    assert_eq!(result, array![4.5, 6., 5., 4., 1.5]);
}

#[test]
fn moving_average_keeps_existing_window_edge_behavior() {
    let data = array![1., 2., 9., 4., 5.];
    assert_eq!(moving_average(&data, 0), data);
    assert_eq!(moving_average(&data, 6), data);
    assert_eq!(moving_average(&data, 1), data);
    assert_eq!(moving_average(&data, 3), array![1.5, 4., 5., 6., 4.5]);
}

#[test]
fn moving_average_layout_does_not_change_window_behavior() {
    let contiguous = array![1., 2., 9., 4., 5.];
    let mut strided = array![1., 90., 2., 90., 9., 90., 4., 90., 5.];
    strided.slice_axis_inplace(Axis(0), Slice::new(0, None, 2));
    for width in 0..=6 {
        assert_eq!(
            moving_average(&strided, width),
            moving_average(&contiguous, width)
        );
    }
}
