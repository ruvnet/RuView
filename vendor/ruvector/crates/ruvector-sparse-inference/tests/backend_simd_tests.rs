//! Standalone tests for SIMD backend kernels

use ndarray::Array2;
use ruvector_sparse_inference::backend::{cpu::CpuBackend, get_backend, Backend};
use ruvector_sparse_inference::config::ActivationType;

#[test]
fn test_cpu_backend_dot_product() {
    let backend = CpuBackend;

    // Test small vector
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![2.0, 3.0, 4.0, 5.0];
    let result = backend.dot_product(&a, &b);
    assert!(
        (result - 40.0).abs() < 1e-5,
        "Expected 40.0, got {}",
        result
    );

    // Test larger vector (exercises SIMD paths)
    let a: Vec<f32> = (0..256).map(|i| i as f32).collect();
    let b: Vec<f32> = (0..256).map(|i| (i * 2) as f32).collect();
    let result = backend.dot_product(&a, &b);
    let expected: f32 = (0..256).map(|i| (i * i * 2) as f32).sum();
    assert!(
        (result - expected).abs() < 1.0,
        "Expected {}, got {}",
        expected,
        result
    );
}

#[test]
fn test_cpu_backend_relu() {
    let backend = CpuBackend;

    let mut data = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, -4.0, 5.0];
    backend.activation(&mut data, ActivationType::Relu);
    assert_eq!(data, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 5.0]);

    // Test larger array (exercises SIMD paths)
    let mut data: Vec<f32> = (0..256).map(|i| i as f32 - 128.0).collect();
    backend.activation(&mut data, ActivationType::Relu);
    for (i, &val) in data.iter().enumerate() {
        let expected = (i as f32 - 128.0).max(0.0);
        assert!(
            (val - expected).abs() < 1e-5,
            "Index {}: expected {}, got {}",
            i,
            expected,
            val
        );
    }
}

#[test]
fn test_cpu_backend_gelu() {
    let backend = CpuBackend;

    let mut data = vec![0.0, 1.0, -1.0, 2.0];
    backend.activation(&mut data, ActivationType::Gelu);

    // GELU(0) ≈ 0
    assert!(
        data[0].abs() < 0.01,
        "GELU(0) should be ≈0, got {}",
        data[0]
    );

    // GELU(1) ≈ 0.841
    assert!(
        (data[1] - 0.841).abs() < 0.01,
        "GELU(1) should be ≈0.841, got {}",
        data[1]
    );

    // GELU(-1) ≈ -0.159 (GELU is NOT an odd function)
    assert!(
        (data[2] + 0.159).abs() < 0.1,
        "GELU(-1) should be ≈-0.159, got {}",
        data[2]
    );
}

#[test]
fn test_cpu_backend_silu() {
    let backend = CpuBackend;

    let mut data = vec![0.0, 1.0, -1.0, 2.0];
    backend.activation(&mut data, ActivationType::Silu);

    // SiLU(0) ≈ 0
    assert!(
        data[0].abs() < 0.01,
        "SiLU(0) should be ≈0, got {}",
        data[0]
    );

    // SiLU(1) ≈ 0.731
    assert!(
        (data[1] - 0.731).abs() < 0.01,
        "SiLU(1) should be ≈0.731, got {}",
        data[1]
    );
}

#[test]
fn test_cpu_backend_add() {
    let backend = CpuBackend;

    let mut a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0];
    backend.add(&mut a, &b);
    assert_eq!(a, vec![11.0, 22.0, 33.0, 44.0, 55.0, 66.0, 77.0, 88.0]);
}

#[test]
fn test_cpu_backend_axpy() {
    let backend = CpuBackend;

    let mut a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
    backend.axpy(&mut a, &b, 2.5);
    assert_eq!(a, vec![3.5, 4.5, 5.5, 6.5, 7.5, 8.5, 9.5, 10.5]);
}

#[test]
fn test_cpu_backend_sparse_matmul() {
    let backend = CpuBackend;

    // Create a 4x4 matrix
    let matrix = Array2::from_shape_vec(
        (4, 4),
        vec![
            1.0, 0.0, 2.0, 0.0, 0.0, 3.0, 0.0, 4.0, 5.0, 0.0, 6.0, 0.0, 0.0, 7.0, 0.0, 8.0,
        ],
    )
    .unwrap();

    let input = vec![1.0, 2.0, 3.0, 4.0];

    // Only compute rows 0 and 2
    let active_rows = vec![0, 2];
    let output = backend.sparse_matmul(&matrix, &input, &active_rows);

    // Row 0: 1*1 + 0*2 + 2*3 + 0*4 = 7
    // Row 2: 5*1 + 0*2 + 6*3 + 0*4 = 23
    assert_eq!(output.len(), 2);
    assert!((output[0] - 7.0).abs() < 1e-5);
    assert!((output[1] - 23.0).abs() < 1e-5);
}

#[test]
fn test_cpu_backend_sparse_matmul_accumulate() {
    let backend = CpuBackend;

    let matrix = Array2::from_shape_vec(
        (4, 4),
        vec![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        ],
    )
    .unwrap();

    let input = vec![1.0, 2.0];
    let active_cols = vec![0, 2];
    let mut output = vec![0.0; 4];

    backend.sparse_matmul_accumulate(&matrix, &input, &active_cols, &mut output);

    // Column 0 * 1.0 + Column 2 * 2.0
    // [1, 5, 9, 13] * 1.0 + [3, 7, 11, 15] * 2.0
    assert!((output[0] - 7.0).abs() < 1e-5); // 1 + 6
    assert!((output[1] - 19.0).abs() < 1e-5); // 5 + 14
    assert!((output[2] - 31.0).abs() < 1e-5); // 9 + 22
    assert!((output[3] - 43.0).abs() < 1e-5); // 13 + 30
}

#[test]
fn test_get_backend() {
    let backend = get_backend();
    println!("Using backend: {}", backend.name());
    println!("SIMD width: {}", backend.simd_width());

    // Verify backend works
    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![2.0, 3.0, 4.0, 5.0];
    let result = backend.dot_product(&a, &b);
    assert!((result - 40.0).abs() < 1e-5);
}

#[test]
fn test_backend_simd_width() {
    let backend = CpuBackend;
    let width = backend.simd_width();

    // Width should be 1, 4, or 8 depending on CPU features
    assert!(
        width == 1 || width == 4 || width == 8,
        "Unexpected SIMD width: {}",
        width
    );

    println!("Backend: {}", backend.name());
    println!("SIMD width: {}", width);
}
