use super::{NodeState, NOVELTY_VECTOR_DIM};

#[test]
fn first_frame_yields_max_novelty_then_zero_on_repeat() {
    let mut ns = NodeState::new();
    let amplitudes: Vec<f64> = (0..NOVELTY_VECTOR_DIM).map(|i| (i as f64).sin()).collect();

    ns.update_novelty(&amplitudes);
    let first = ns.last_novelty_score.expect("sketch bank initialised");
    assert!(
        (first - 1.0).abs() < 1e-6,
        "empty bank → max novelty 1.0, got {first}"
    );

    for _ in 0..10 {
        ns.update_novelty(&amplitudes);
    }
    let repeated = ns.last_novelty_score.expect("sketch bank updated");
    assert!(
        repeated < 0.05,
        "identical repeated frame must have novelty ~0, got {repeated}"
    );
}

#[test]
fn perturbed_frame_yields_intermediate_novelty() {
    let mut ns = NodeState::new();
    let base: Vec<f64> = (0..NOVELTY_VECTOR_DIM).map(|i| (i as f64).cos()).collect();
    for _ in 0..10 {
        ns.update_novelty(&base);
    }

    let mut perturbed = base.clone();
    for x in perturbed.iter_mut().take(8) {
        *x += 3.0;
    }
    ns.update_novelty(&perturbed);
    let score = ns.last_novelty_score.expect("novelty score present");
    assert!(
        score > 0.05 && score < 0.95,
        "perturbed frame must yield intermediate novelty, got {score}"
    );
}

