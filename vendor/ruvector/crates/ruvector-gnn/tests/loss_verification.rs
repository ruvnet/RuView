/// Verification test for GNN loss function implementations
use ndarray::Array2;
use ruvector_gnn::training::{Loss, LossType};

#[test]
fn test_mse_loss_implementation() {
    // Test MSE: mean((pred - target)^2)
    let predictions = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let targets = Array2::from_shape_vec((2, 2), vec![0.5, 1.5, 2.5, 3.5]).unwrap();

    let loss = Loss::compute(LossType::Mse, &predictions, &targets).unwrap();

    // Expected: mean([0.25, 0.25, 0.25, 0.25]) = 0.25
    assert!(
        (loss - 0.25).abs() < 1e-6,
        "MSE loss should be 0.25, got {}",
        loss
    );
}

#[test]
fn test_mse_gradient_implementation() {
    let predictions = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let targets = Array2::from_shape_vec((2, 2), vec![0.5, 1.5, 2.5, 3.5]).unwrap();

    let gradient = Loss::gradient(LossType::Mse, &predictions, &targets).unwrap();

    // Expected gradient: 2 * (pred - target) / n = 2 * [0.5, 0.5, 0.5, 0.5] / 4
    assert!((gradient[[0, 0]] - 0.25).abs() < 1e-6);
    assert!((gradient[[0, 1]] - 0.25).abs() < 1e-6);
    assert!((gradient[[1, 0]] - 0.25).abs() < 1e-6);
    assert!((gradient[[1, 1]] - 0.25).abs() < 1e-6);
}

#[test]
fn test_cross_entropy_loss_implementation() {
    let predictions = Array2::from_shape_vec((2, 2), vec![0.7, 0.3, 0.8, 0.2]).unwrap();
    let targets = Array2::from_shape_vec((2, 2), vec![1.0, 0.0, 1.0, 0.0]).unwrap();

    let loss = Loss::compute(LossType::CrossEntropy, &predictions, &targets).unwrap();

    // Loss should be positive and finite
    assert!(loss > 0.0, "Cross entropy loss should be positive");
    assert!(loss.is_finite(), "Cross entropy loss should be finite");
}

#[test]
fn test_binary_cross_entropy_loss_implementation() {
    let predictions = Array2::from_shape_vec((2, 2), vec![0.9, 0.1, 0.8, 0.2]).unwrap();
    let targets = Array2::from_shape_vec((2, 2), vec![1.0, 0.0, 1.0, 0.0]).unwrap();

    let loss = Loss::compute(LossType::BinaryCrossEntropy, &predictions, &targets).unwrap();

    // For good predictions (close to targets), loss should be small
    assert!(loss < 1.0, "BCE loss should be small for good predictions");
    assert!(loss > 0.0, "BCE loss should be positive");
}

#[test]
fn test_loss_gradient_shapes_match() {
    let predictions = Array2::from_shape_vec(
        (3, 4),
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.8, 0.7, 0.6],
    )
    .unwrap();
    let targets = Array2::zeros((3, 4));

    for loss_type in [
        LossType::Mse,
        LossType::CrossEntropy,
        LossType::BinaryCrossEntropy,
    ] {
        let gradient = Loss::gradient(loss_type, &predictions, &targets).unwrap();
        assert_eq!(
            gradient.shape(),
            predictions.shape(),
            "Gradient shape should match predictions for {:?}",
            loss_type
        );
    }
}

#[test]
fn test_loss_dimension_mismatch_error() {
    let predictions = Array2::from_shape_vec((2, 2), vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let targets = Array2::from_shape_vec((3, 2), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();

    let result = Loss::compute(LossType::Mse, &predictions, &targets);
    assert!(
        result.is_err(),
        "Should return error for mismatched dimensions"
    );

    let result = Loss::gradient(LossType::Mse, &predictions, &targets);
    assert!(
        result.is_err(),
        "Should return error for mismatched dimensions"
    );
}
