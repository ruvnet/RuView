//! Integration tests for hyperbolic attention mechanisms

use ruvector_attention::traits::Attention;
use ruvector_attention::hyperbolic::{
    HyperbolicAttention, HyperbolicAttentionConfig,
    MixedCurvatureAttention, MixedCurvatureConfig,
    poincare_distance, mobius_add, exp_map, log_map, project_to_ball,
};

#[test]
fn test_hyperbolic_attention_numerical_stability() {
    let config = HyperbolicAttentionConfig {
        dim: 16,
        curvature: -1.0,
        adaptive_curvature: false,
        temperature: 1.0,
        frechet_max_iter: 100,
        frechet_tol: 1e-6,
    };

    let attention = HyperbolicAttention::new(config);

    // Test with points near boundary of Poincaré ball
    let query = vec![0.9; 16];
    let keys: Vec<Vec<f32>> = vec![
        vec![0.85; 16],
        vec![0.8; 16],
        vec![0.1; 16],
    ];
    let values: Vec<Vec<f32>> = vec![
        vec![1.0; 16],
        vec![0.5; 16],
        vec![0.0; 16],
    ];

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();

    // Verify numerical stability
    assert_eq!(result.len(), 16);
    assert!(result.iter().all(|&x| x.is_finite()), "All values should be finite");
    assert!(result.iter().all(|&x| !x.is_nan()), "No NaN values");
}

#[test]
fn test_poincare_distance_properties() {
    let u = vec![0.2, 0.3, 0.1];
    let v = vec![0.4, 0.1, 0.2];
    let w = vec![0.1, 0.4, 0.3];
    let c = 1.0;

    // Symmetry: d(u,v) = d(v,u)
    let d_uv = poincare_distance(&u, &v, c);
    let d_vu = poincare_distance(&v, &u, c);
    assert!((d_uv - d_vu).abs() < 1e-6, "Distance should be symmetric");

    // Identity: d(u,u) = 0
    let d_uu = poincare_distance(&u, &u, c);
    assert!(d_uu.abs() < 1e-6, "Distance to self should be zero");

    // Triangle inequality: d(u,w) ≤ d(u,v) + d(v,w)
    let d_uw = poincare_distance(&u, &w, c);
    let d_vw = poincare_distance(&v, &w, c);
    assert!(
        d_uw <= d_uv + d_vw + 1e-5,
        "Triangle inequality should hold: {} <= {} + {}",
        d_uw, d_uv, d_vw
    );
}

#[test]
fn test_mobius_addition_properties() {
    let u = vec![0.2, 0.3];
    let v = vec![0.1, -0.2];
    let c = 1.0;

    // Identity: u ⊕ 0 = u
    let zero = vec![0.0, 0.0];
    let result = mobius_add(&u, &zero, c);
    for (ui, &ri) in u.iter().zip(&result) {
        assert!((ui - ri).abs() < 1e-6, "Möbius addition with zero should be identity");
    }

    // Result should be in ball
    let result_uv = mobius_add(&u, &v, c);
    let norm_sq: f32 = result_uv.iter().map(|x| x * x).sum();
    assert!(norm_sq < 1.0, "Möbius addition result should be in Poincaré ball");
}

#[test]
fn test_exp_log_map_inverse() {
    let p = vec![0.3, 0.2, 0.1];
    let v = vec![0.1, -0.1, 0.05];
    let c = 1.0;

    // exp_p(log_p(q)) = q
    let q = exp_map(&v, &p, c);
    let v_recovered = log_map(&q, &p, c);

    for (vi, &vr) in v.iter().zip(&v_recovered) {
        assert!(
            (vi - vr).abs() < 1e-4,
            "exp and log should be inverses: {} vs {}",
            vi, vr
        );
    }
}

#[test]
fn test_hyperbolic_attention_hierarchical_structure() {
    let config = HyperbolicAttentionConfig {
        dim: 4,
        curvature: -1.0,
        ..Default::default()
    };
    let attention = HyperbolicAttention::new(config);

    // Simulate a tree: root -> branch1, branch2 -> leaf1, leaf2
    let root = vec![0.0, 0.0, 0.0, 0.0];
    let branch1 = vec![0.3, 0.0, 0.0, 0.0];
    let branch2 = vec![-0.3, 0.0, 0.0, 0.0];
    let leaf1 = vec![0.4, 0.1, 0.0, 0.0];
    let leaf2 = vec![0.4, -0.1, 0.0, 0.0];

    // Query near branch1
    let query = vec![0.35, 0.0, 0.0, 0.0];

    let keys = vec![&root[..], &branch1[..], &branch2[..], &leaf1[..], &leaf2[..]];
    let weights = attention.compute_weights(&query, &keys);

    // Should attend more to nearby branch and leaves
    assert!(weights[1] > weights[0], "Should attend more to close branch than root");
    assert!(weights[1] > weights[2], "Should attend more to close branch than far branch");
    assert!(weights[3] + weights[4] > weights[0] + weights[2],
            "Should attend more to close leaves than to root and far branch combined");
}

#[test]
fn test_mixed_curvature_interpolation() {
    // Test that mixing weight correctly interpolates between Euclidean and Hyperbolic

    let euclidean_config = MixedCurvatureConfig {
        euclidean_dim: 3,
        hyperbolic_dim: 3,
        mixing_weight: 0.0, // Pure Euclidean
        ..Default::default()
    };

    let hyperbolic_config = MixedCurvatureConfig {
        euclidean_dim: 3,
        hyperbolic_dim: 3,
        mixing_weight: 1.0, // Pure Hyperbolic
        ..Default::default()
    };

    let mixed_config = MixedCurvatureConfig {
        euclidean_dim: 3,
        hyperbolic_dim: 3,
        mixing_weight: 0.5, // Mixed
        ..Default::default()
    };

    let euc_attention = MixedCurvatureAttention::new(euclidean_config);
    let hyp_attention = MixedCurvatureAttention::new(hyperbolic_config);
    let mix_attention = MixedCurvatureAttention::new(mixed_config);

    let x = vec![0.1, 0.2, 0.3, 0.1, 0.2, 0.3];
    let y = vec![0.2, 0.1, 0.4, 0.2, 0.1, 0.4];

    let sim_euc = euc_attention.compute_similarity(&x, &y);
    let sim_hyp = hyp_attention.compute_similarity(&x, &y);
    let sim_mix = mix_attention.compute_similarity(&x, &y);

    // Mixed should be between pure versions
    assert!(
        (sim_mix >= sim_euc.min(sim_hyp) - 1e-5) && (sim_mix <= sim_euc.max(sim_hyp) + 1e-5),
        "Mixed similarity should interpolate between Euclidean and Hyperbolic"
    );
}

#[test]
fn test_projection_to_ball_correctness() {
    let c = 1.0;
    let max_norm = 1.0 / c.sqrt() - 1e-7;

    // Point inside ball - should remain unchanged
    let inside = vec![0.3, 0.4];
    let projected_inside = project_to_ball(&inside, c, 1e-7);
    for (i, &p) in inside.iter().zip(&projected_inside) {
        assert!((i - p).abs() < 1e-6, "Point inside ball should remain unchanged");
    }

    // Point outside ball - should be projected to boundary
    let outside = vec![2.0, 2.0];
    let projected_outside = project_to_ball(&outside, c, 1e-7);
    let norm: f32 = projected_outside.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(norm <= max_norm, "Projected point should be inside ball");
}

#[test]
fn test_batch_processing_consistency() {
    let config = HyperbolicAttentionConfig {
        dim: 8,
        curvature: -1.0,
        ..Default::default()
    };
    let attention = HyperbolicAttention::new(config);

    let queries: Vec<Vec<f32>> = vec![
        vec![0.1; 8],
        vec![0.2; 8],
        vec![0.3; 8],
    ];
    let keys: Vec<Vec<f32>> = vec![
        vec![0.15; 8],
        vec![0.25; 8],
    ];
    let values: Vec<Vec<f32>> = vec![
        vec![1.0; 8],
        vec![0.0; 8],
    ];

    let queries_refs: Vec<&[f32]> = queries.iter().map(|q| q.as_slice()).collect();
    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    // Batch processing
    let batch_results = attention.compute_batch(&queries_refs, &keys_refs, &values_refs).unwrap();

    // Individual processing
    let individual_results: Vec<Vec<f32>> = queries_refs
        .iter()
        .map(|q| attention.compute(q, &keys_refs, &values_refs).unwrap())
        .collect();

    // Results should match
    for (batch, individual) in batch_results.iter().zip(&individual_results) {
        for (&b, &i) in batch.iter().zip(individual) {
            assert!((b - i).abs() < 1e-5, "Batch and individual results should match");
        }
    }
}

#[test]
fn test_adaptive_curvature() {
    let mut config = HyperbolicAttentionConfig {
        dim: 4,
        curvature: -1.0,
        adaptive_curvature: true,
        ..Default::default()
    };

    let mut attention = HyperbolicAttention::new(config.clone());

    let initial_curvature = attention.get_curvature();
    assert_eq!(initial_curvature, 1.0, "Initial curvature should be 1.0");

    // Update curvature
    attention.update_curvature(-2.0);
    let new_curvature = attention.get_curvature();
    assert_eq!(new_curvature, 2.0, "Curvature should update when adaptive");

    // Non-adaptive should not update
    config.adaptive_curvature = false;
    let mut fixed_attention = HyperbolicAttention::new(config);
    fixed_attention.update_curvature(-5.0);
    let unchanged_curvature = fixed_attention.get_curvature();
    assert_eq!(unchanged_curvature, 1.0, "Curvature should not update when non-adaptive");
}

#[test]
fn test_temperature_scaling() {
    let low_temp_config = HyperbolicAttentionConfig {
        dim: 3,
        curvature: -1.0,
        temperature: 0.1, // Sharp attention
        ..Default::default()
    };

    let high_temp_config = HyperbolicAttentionConfig {
        dim: 3,
        curvature: -1.0,
        temperature: 10.0, // Smooth attention
        ..Default::default()
    };

    let low_temp = HyperbolicAttention::new(low_temp_config);
    let high_temp = HyperbolicAttention::new(high_temp_config);

    let query = vec![0.0, 0.0, 0.0];
    let keys = vec![
        &vec![0.1, 0.0, 0.0][..],
        &vec![0.5, 0.0, 0.0][..],
    ];

    let low_weights = low_temp.compute_weights(&query, &keys);
    let high_weights = high_temp.compute_weights(&query, &keys);

    // Low temperature should produce more peaked distribution
    let low_entropy = -low_weights.iter().map(|&w| w * w.ln()).sum::<f32>();
    let high_entropy = -high_weights.iter().map(|&w| w * w.ln()).sum::<f32>();

    assert!(high_entropy > low_entropy, "Higher temperature should produce higher entropy");
}
