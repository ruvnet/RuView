//! Manual test/demo for Loss functions
//!
//! Run with: cargo run -p ruvector-gnn --example loss_demo

use ndarray::Array2;
use ruvector_gnn::training::{Loss, LossType, Optimizer, OptimizerType};

fn main() {
    println!("=== RuVector GNN Loss Functions Demo ===\n");

    // 1. Basic MSE Loss
    println!("1. MSE Loss Demo");
    println!("   -----------------");
    let predictions = Array2::from_shape_vec((2, 3), vec![0.1, 0.2, 0.7, 0.8, 0.1, 0.1]).unwrap();
    let targets = Array2::from_shape_vec((2, 3), vec![0.0, 0.0, 1.0, 1.0, 0.0, 0.0]).unwrap();

    let mse_loss = Loss::compute(LossType::Mse, &predictions, &targets).unwrap();
    let mse_grad = Loss::gradient(LossType::Mse, &predictions, &targets).unwrap();

    println!("   Predictions: {:?}", predictions.as_slice().unwrap());
    println!("   Targets:     {:?}", targets.as_slice().unwrap());
    println!("   MSE Loss:    {:.6}", mse_loss);
    println!("   Gradient:    {:?}\n", mse_grad.as_slice().unwrap());

    // 2. Binary Cross Entropy Loss
    println!("2. Binary Cross Entropy Demo");
    println!("   --------------------------");
    let pred_bce = Array2::from_shape_vec((1, 4), vec![0.9, 0.1, 0.8, 0.3]).unwrap();
    let target_bce = Array2::from_shape_vec((1, 4), vec![1.0, 0.0, 1.0, 0.0]).unwrap();

    let bce_loss = Loss::compute(LossType::BinaryCrossEntropy, &pred_bce, &target_bce).unwrap();
    let bce_grad = Loss::gradient(LossType::BinaryCrossEntropy, &pred_bce, &target_bce).unwrap();

    println!("   Predictions: {:?}", pred_bce.as_slice().unwrap());
    println!("   Targets:     {:?}", target_bce.as_slice().unwrap());
    println!("   BCE Loss:    {:.6}", bce_loss);
    println!("   Gradient:    {:?}\n", bce_grad.as_slice().unwrap());

    // 3. Cross Entropy Loss (multi-class)
    println!("3. Cross Entropy Demo (multi-class)");
    println!("   ----------------------------------");
    // Softmax-like predictions (each row sums to ~1)
    let pred_ce = Array2::from_shape_vec((2, 3), vec![0.7, 0.2, 0.1, 0.1, 0.1, 0.8]).unwrap();
    let target_ce = Array2::from_shape_vec((2, 3), vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0]).unwrap();

    let ce_loss = Loss::compute(LossType::CrossEntropy, &pred_ce, &target_ce).unwrap();
    let ce_grad = Loss::gradient(LossType::CrossEntropy, &pred_ce, &target_ce).unwrap();

    println!(
        "   Predictions (row 1): {:?}",
        &pred_ce.as_slice().unwrap()[0..3]
    );
    println!(
        "   Predictions (row 2): {:?}",
        &pred_ce.as_slice().unwrap()[3..6]
    );
    println!("   Targets (one-hot):   [1,0,0] and [0,0,1]");
    println!("   CE Loss:    {:.6}", ce_loss);
    println!("   Gradient:   {:?}\n", ce_grad.as_slice().unwrap());

    // 4. Training loop demo - minimize MSE
    println!("4. Training Loop Demo (minimizing MSE)");
    println!("   ------------------------------------");

    let target = Array2::from_shape_vec((1, 4), vec![1.0, 0.0, 1.0, 0.0]).unwrap();
    let mut pred = Array2::from_shape_vec((1, 4), vec![0.5, 0.5, 0.5, 0.5]).unwrap();

    let mut optimizer = Optimizer::new(OptimizerType::Adam {
        learning_rate: 0.1,
        beta1: 0.9,
        beta2: 0.999,
        epsilon: 1e-8,
    });

    println!("   Target:     {:?}", target.as_slice().unwrap());
    println!("   Initial:    {:?}", pred.as_slice().unwrap());

    let initial_loss = Loss::compute(LossType::Mse, &pred, &target).unwrap();
    println!("   Initial loss: {:.6}\n", initial_loss);

    for epoch in 0..20 {
        let loss = Loss::compute(LossType::Mse, &pred, &target).unwrap();
        let grad = Loss::gradient(LossType::Mse, &pred, &target).unwrap();
        optimizer.step(&mut pred, &grad).unwrap();

        if epoch % 5 == 0 || epoch == 19 {
            println!(
                "   Epoch {:2}: loss={:.6}, pred={:?}",
                epoch,
                loss,
                pred.as_slice()
                    .unwrap()
                    .iter()
                    .map(|x| format!("{:.3}", x))
                    .collect::<Vec<_>>()
            );
        }
    }

    let final_loss = Loss::compute(LossType::Mse, &pred, &target).unwrap();
    println!("\n   Final loss: {:.6}", final_loss);
    println!(
        "   Improvement: {:.1}%",
        (1.0 - final_loss / initial_loss) * 100.0
    );

    // 5. Numerical stability test
    println!("\n5. Numerical Stability Test");
    println!("   -------------------------");

    // Test with extreme values
    let extreme_pred = Array2::from_shape_vec((1, 2), vec![1e-10, 1.0 - 1e-10]).unwrap();
    let extreme_target = Array2::from_shape_vec((1, 2), vec![1.0, 0.0]).unwrap();

    let bce_extreme = Loss::compute(LossType::BinaryCrossEntropy, &extreme_pred, &extreme_target);
    let ce_extreme = Loss::compute(LossType::CrossEntropy, &extreme_pred, &extreme_target);

    println!(
        "   Extreme predictions: [{:.2e}, {:.2e}]",
        1e-10,
        1.0 - 1e-10
    );
    println!("   BCE result: {:?}", bce_extreme);
    println!("   CE result:  {:?}", ce_extreme);

    // Test gradient stability
    let grad_extreme = Loss::gradient(LossType::BinaryCrossEntropy, &extreme_pred, &extreme_target);
    println!("   BCE gradient: {:?}", grad_extreme);

    println!("\n=== Demo Complete ===");
}
