//! Basic usage example for the sparse inference engine

use ndarray::Array2;
use ruvector_sparse_inference::backend::get_backend;
use ruvector_sparse_inference::sparse::ActivationType;

fn main() {
    // Get the best available backend for this platform
    let backend = get_backend();
    println!("Using backend: {}", backend.name());
    println!("SIMD width: {} f32s per register", backend.simd_width());

    // Example 1: Dot product
    println!("\n=== Dot Product ===");
    let a = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
    let b = vec![2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0];
    let dot = backend.dot_product(&a, &b);
    println!("a · b = {}", dot);
    assert_eq!(dot, 72.0);

    // Example 2: ReLU activation
    println!("\n=== ReLU Activation ===");
    let mut data = vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, -4.0, 5.0];
    println!("Before: {:?}", data);
    backend.activation(&mut data, ActivationType::Relu);
    println!("After:  {:?}", data);
    assert_eq!(data, vec![0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 0.0, 5.0]);

    // Example 3: AXPY (a = a + b * scalar)
    println!("\n=== AXPY (a = a + b * 2.5) ===");
    let mut a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![1.0, 1.0, 1.0, 1.0];
    println!("Before: a = {:?}", a);
    backend.axpy(&mut a, &b, 2.5);
    println!("After:  a = {:?}", a);
    assert_eq!(a, vec![3.5, 4.5, 5.5, 6.5]);

    // Example 4: Sparse matrix-vector multiplication
    println!("\n=== Sparse MatMul ===");
    let matrix = Array2::from_shape_vec(
        (4, 4),
        vec![
            1.0, 0.0, 2.0, 0.0, 0.0, 3.0, 0.0, 4.0, 5.0, 0.0, 6.0, 0.0, 0.0, 7.0, 0.0, 8.0,
        ],
    )
    .unwrap();
    let input = vec![1.0, 2.0, 3.0, 4.0];

    // Only compute rows 0 and 2 (sparse computation)
    let active_rows = vec![0, 2];
    let output = backend.sparse_matmul(&matrix, &input, &active_rows);
    println!("Matrix (4x4):");
    println!("{:?}", matrix);
    println!("Input: {:?}", input);
    println!("Active rows: {:?}", active_rows);
    println!("Output: {:?}", output);
    assert_eq!(output, vec![7.0, 23.0]); // row 0: 1*1 + 2*3 = 7, row 2: 5*1 + 6*3 = 23

    // Example 5: Different activation functions
    println!("\n=== Activation Functions ===");
    for activation in [
        ActivationType::Relu,
        ActivationType::Gelu,
        ActivationType::Silu,
    ] {
        let mut data = vec![-1.0, 0.0, 1.0, 2.0];
        backend.activation(&mut data, activation);
        println!("{:?}: {:?}", activation, data);
    }

    println!("\n✓ All examples completed successfully!");
}
