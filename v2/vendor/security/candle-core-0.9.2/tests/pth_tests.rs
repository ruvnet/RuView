/// Regression test for pth files not loading on Windows.
#[test]
fn test_pth() {
    let tensors = candle_core::pickle::PthTensors::new("tests/test.pt", None).unwrap();
    tensors.get("test").unwrap().unwrap();
}

#[test]
fn test_pth_with_key() {
    let tensors =
        candle_core::pickle::PthTensors::new("tests/test_with_key.pt", Some("model_state_dict"))
            .unwrap();
    tensors.get("test").unwrap().unwrap();
}

#[test]
fn test_pth_fortran_contiguous() {
    let tensors =
        candle_core::pickle::PthTensors::new("tests/fortran_tensor_3d.pth", None).unwrap();
    let tensor = tensors.get("tensor_fortran").unwrap().unwrap();

    assert_eq!(tensor.dims3().unwrap(), (2, 3, 4));

    assert_eq!(
        tensor.to_vec3::<i64>().unwrap(),
        [
            [[1, 2, 3, 4], [5, 6, 7, 8], [9, 10, 11, 12]],
            [[13, 14, 15, 16], [17, 18, 19, 20], [21, 22, 23, 24]]
        ]
    );
}
