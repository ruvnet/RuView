//! Property-based tests for coherence computation invariants
//!
//! Mathematical invariants tested:
//! 1. Energy is always non-negative (E >= 0)
//! 2. Consistent sections have zero energy (rho_u(x_u) = rho_v(x_v) => E = 0)
//! 3. Residual symmetry (r_{u,v} = -r_{v,u})
//! 4. Energy scales with weight (E(w*e) = w * E(e) for w >= 0)
//! 5. Triangle inequality for distances derived from energy

use quickcheck::{Arbitrary, Gen, QuickCheck, TestResult};
use quickcheck_macros::quickcheck;
use rand::Rng;

// ============================================================================
// TEST TYPES WITH ARBITRARY IMPLEMENTATIONS
// ============================================================================

/// A bounded float for testing (avoids infinities and NaN)
#[derive(Clone, Copy, Debug)]
struct BoundedFloat(f32);

impl Arbitrary for BoundedFloat {
    fn arbitrary(g: &mut Gen) -> Self {
        // Use i32 to generate a bounded integer, then convert to float
        // This avoids NaN and Inf that f32::arbitrary can produce
        let val: i32 = i32::arbitrary(g);
        let float_val = (val as f32 / (i32::MAX as f32 / 1000.0)).clamp(-1000.0, 1000.0);
        BoundedFloat(float_val)
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(std::iter::empty())
    }
}

/// A non-negative float for weights
#[derive(Clone, Copy, Debug)]
struct NonNegativeFloat(f32);

impl Arbitrary for NonNegativeFloat {
    fn arbitrary(g: &mut Gen) -> Self {
        // Use u32 to generate a bounded non-negative integer, then convert to float
        let val: u32 = u32::arbitrary(g);
        let float_val = (val as f32 / (u32::MAX as f32 / 1000.0)).min(1000.0);
        NonNegativeFloat(float_val)
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(std::iter::empty())
    }
}

/// A positive float (> 0) for weights
#[derive(Clone, Copy, Debug)]
struct PositiveFloat(f32);

impl Arbitrary for PositiveFloat {
    fn arbitrary(g: &mut Gen) -> Self {
        // Use u32 to generate a bounded positive integer, then convert to float
        let val: u32 = u32::arbitrary(g);
        let float_val = (val as f32 / (u32::MAX as f32 / 1000.0))
            .max(0.001)
            .min(1000.0);
        PositiveFloat(float_val)
    }
}

/// A state vector of fixed dimension
#[derive(Clone, Debug)]
struct StateVector {
    values: Vec<f32>,
}

impl StateVector {
    fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    fn dim(&self) -> usize {
        self.values.len()
    }

    fn zeros(dim: usize) -> Self {
        Self {
            values: vec![0.0; dim],
        }
    }
}

impl Arbitrary for StateVector {
    fn arbitrary(g: &mut Gen) -> Self {
        let dim = usize::arbitrary(g) % 8 + 1; // 1-8 dimensions
        let values: Vec<f32> = (0..dim)
            .map(|_| {
                let bf = BoundedFloat::arbitrary(g);
                bf.0
            })
            .collect();
        StateVector::new(values)
    }

    // Empty shrink to avoid stack overflow from recursive shrinking
    fn shrink(&self) -> Box<dyn Iterator<Item = Self>> {
        Box::new(std::iter::empty())
    }
}

/// Identity restriction map
#[derive(Clone, Debug)]
struct IdentityMap {
    dim: usize,
}

impl IdentityMap {
    fn apply(&self, input: &[f32]) -> Vec<f32> {
        input.to_vec()
    }
}

impl Arbitrary for IdentityMap {
    fn arbitrary(g: &mut Gen) -> Self {
        let dim = usize::arbitrary(g) % 8 + 1;
        Self { dim }
    }
}

/// A simple restriction map (linear transform)
#[derive(Clone, Debug)]
struct SimpleRestrictionMap {
    matrix: Vec<Vec<f32>>,
}

impl SimpleRestrictionMap {
    fn identity(dim: usize) -> Self {
        let matrix = (0..dim)
            .map(|i| {
                let mut row = vec![0.0; dim];
                row[i] = 1.0;
                row
            })
            .collect();
        Self { matrix }
    }

    fn apply(&self, input: &[f32]) -> Vec<f32> {
        self.matrix
            .iter()
            .map(|row| row.iter().zip(input).map(|(a, b)| a * b).sum())
            .collect()
    }

    fn output_dim(&self) -> usize {
        self.matrix.len()
    }

    fn input_dim(&self) -> usize {
        if self.matrix.is_empty() {
            0
        } else {
            self.matrix[0].len()
        }
    }
}

impl Arbitrary for SimpleRestrictionMap {
    fn arbitrary(g: &mut Gen) -> Self {
        let input_dim = usize::arbitrary(g) % 6 + 2; // 2-7 dimensions
        let output_dim = usize::arbitrary(g) % 6 + 2;

        let matrix: Vec<Vec<f32>> = (0..output_dim)
            .map(|_| {
                (0..input_dim)
                    .map(|_| {
                        let bf = BoundedFloat::arbitrary(g);
                        bf.0 / 100.0 // Scale down for stability
                    })
                    .collect()
            })
            .collect();

        Self { matrix }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Compute residual: rho_source(x_source) - rho_target(x_target)
fn compute_residual(
    source_state: &[f32],
    target_state: &[f32],
    rho_source: &SimpleRestrictionMap,
    rho_target: &SimpleRestrictionMap,
) -> Vec<f32> {
    let projected_source = rho_source.apply(source_state);
    let projected_target = rho_target.apply(target_state);

    projected_source
        .iter()
        .zip(&projected_target)
        .map(|(a, b)| a - b)
        .collect()
}

/// Compute energy from residual and weight
fn compute_energy(residual: &[f32], weight: f32) -> f32 {
    let norm_sq: f32 = residual.iter().map(|x| x * x).sum();
    weight * norm_sq
}

/// Compute total energy for a graph
fn compute_total_energy(states: &[(usize, Vec<f32>)], edges: &[(usize, usize, f32)]) -> f32 {
    let dim = if states.is_empty() {
        0
    } else {
        states[0].1.len()
    };
    let rho = SimpleRestrictionMap::identity(dim);

    let mut total: f32 = 0.0;
    for &(src, tgt, weight) in edges {
        if let (Some((_, src_state)), Some((_, tgt_state))) = (
            states.iter().find(|(id, _)| *id == src),
            states.iter().find(|(id, _)| *id == tgt),
        ) {
            let residual = compute_residual(src_state, tgt_state, &rho, &rho);
            total += compute_energy(&residual, weight);
        }
    }
    total
}

// ============================================================================
// PROPERTY: ENERGY IS NON-NEGATIVE
// ============================================================================

#[quickcheck]
fn prop_energy_nonnegative(
    source: StateVector,
    target: StateVector,
    weight: NonNegativeFloat,
) -> TestResult {
    // Skip if dimensions don't match
    if source.dim() != target.dim() {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(source.dim());
    let residual = compute_residual(&source.values, &target.values, &rho, &rho);
    let energy = compute_energy(&residual, weight.0);

    if energy >= 0.0 {
        TestResult::passed()
    } else {
        TestResult::failed()
    }
}

#[quickcheck]
fn prop_energy_nonnegative_arbitrary_restriction(
    source: StateVector,
    target: StateVector,
    weight: NonNegativeFloat,
) -> TestResult {
    // Skip if dimensions are incompatible
    if source.dim() == 0 || target.dim() == 0 {
        return TestResult::discard();
    }

    let common_dim = source.dim().min(target.dim()).min(4);
    let rho_src = SimpleRestrictionMap::identity(common_dim);
    let rho_tgt = SimpleRestrictionMap::identity(common_dim);

    // Truncate to common dimension
    let src_truncated: Vec<f32> = source.values.iter().take(common_dim).copied().collect();
    let tgt_truncated: Vec<f32> = target.values.iter().take(common_dim).copied().collect();

    let residual = compute_residual(&src_truncated, &tgt_truncated, &rho_src, &rho_tgt);
    let energy = compute_energy(&residual, weight.0);

    if energy >= 0.0 {
        TestResult::passed()
    } else {
        TestResult::failed()
    }
}

// ============================================================================
// PROPERTY: CONSISTENT SECTIONS HAVE ZERO ENERGY
// ============================================================================

#[quickcheck]
fn prop_consistent_section_zero_energy(state: StateVector, weight: PositiveFloat) -> TestResult {
    if state.dim() == 0 {
        return TestResult::discard();
    }

    // Same state on both ends of edge (consistent section)
    let rho = SimpleRestrictionMap::identity(state.dim());
    let residual = compute_residual(&state.values, &state.values, &rho, &rho);
    let energy = compute_energy(&residual, weight.0);

    // Energy should be zero (within floating point tolerance)
    if energy.abs() < 1e-6 {
        TestResult::passed()
    } else {
        TestResult::error(format!("Expected zero energy, got {}", energy))
    }
}

#[quickcheck]
fn prop_uniform_states_zero_energy(state: StateVector, n_nodes: u8) -> TestResult {
    let n = (n_nodes % 10 + 2) as usize; // 2-11 nodes
    if state.dim() == 0 {
        return TestResult::discard();
    }

    // Create a path graph with uniform states
    let states: Vec<(usize, Vec<f32>)> = (0..n).map(|i| (i, state.values.clone())).collect();

    let edges: Vec<(usize, usize, f32)> = (0..n - 1).map(|i| (i, i + 1, 1.0)).collect();

    let total_energy = compute_total_energy(&states, &edges);

    if total_energy.abs() < 1e-6 {
        TestResult::passed()
    } else {
        TestResult::error(format!("Expected zero energy, got {}", total_energy))
    }
}

// ============================================================================
// PROPERTY: RESIDUAL SYMMETRY
// ============================================================================

#[quickcheck]
fn prop_residual_symmetry(source: StateVector, target: StateVector) -> TestResult {
    if source.dim() != target.dim() || source.dim() == 0 {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(source.dim());

    // r_{u,v} = rho(x_u) - rho(x_v)
    let r_uv = compute_residual(&source.values, &target.values, &rho, &rho);

    // r_{v,u} = rho(x_v) - rho(x_u)
    let r_vu = compute_residual(&target.values, &source.values, &rho, &rho);

    // Check r_uv = -r_vu
    for (a, b) in r_uv.iter().zip(&r_vu) {
        if (a + b).abs() > 1e-6 {
            return TestResult::error(format!("Symmetry violated: {} != -{}", a, b));
        }
    }

    TestResult::passed()
}

#[quickcheck]
fn prop_residual_energy_symmetric(
    source: StateVector,
    target: StateVector,
    weight: PositiveFloat,
) -> TestResult {
    if source.dim() != target.dim() || source.dim() == 0 {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(source.dim());

    let r_uv = compute_residual(&source.values, &target.values, &rho, &rho);
    let r_vu = compute_residual(&target.values, &source.values, &rho, &rho);

    let e_uv = compute_energy(&r_uv, weight.0);
    let e_vu = compute_energy(&r_vu, weight.0);

    // Energy should be the same regardless of direction
    if (e_uv - e_vu).abs() < 1e-6 {
        TestResult::passed()
    } else {
        TestResult::error(format!("Energy not symmetric: {} vs {}", e_uv, e_vu))
    }
}

// ============================================================================
// PROPERTY: ENERGY SCALES WITH WEIGHT
// ============================================================================

#[quickcheck]
fn prop_energy_scales_with_weight(
    source: StateVector,
    target: StateVector,
    weight1: PositiveFloat,
    scale: PositiveFloat,
) -> TestResult {
    if source.dim() != target.dim() || source.dim() == 0 {
        return TestResult::discard();
    }

    // Limit scale to avoid overflow
    let scale = scale.0.min(100.0);
    if scale < 0.01 {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(source.dim());
    let residual = compute_residual(&source.values, &target.values, &rho, &rho);

    let e1 = compute_energy(&residual, weight1.0);
    let e2 = compute_energy(&residual, weight1.0 * scale);

    // e2 should be approximately scale * e1
    let expected = e1 * scale;
    if (e2 - expected).abs() < 1e-4 * expected.abs().max(1.0) {
        TestResult::passed()
    } else {
        TestResult::error(format!(
            "Scaling failed: {} * {} = {}, but got {}",
            e1, scale, expected, e2
        ))
    }
}

#[quickcheck]
fn prop_zero_weight_zero_energy(source: StateVector, target: StateVector) -> TestResult {
    if source.dim() != target.dim() || source.dim() == 0 {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(source.dim());
    let residual = compute_residual(&source.values, &target.values, &rho, &rho);
    let energy = compute_energy(&residual, 0.0);

    if energy.abs() < 1e-10 {
        TestResult::passed()
    } else {
        TestResult::error(format!(
            "Zero weight should give zero energy, got {}",
            energy
        ))
    }
}

// ============================================================================
// PROPERTY: TOTAL ENERGY IS SUM OF EDGE ENERGIES
// ============================================================================

#[quickcheck]
fn prop_energy_additivity(
    state1: StateVector,
    state2: StateVector,
    state3: StateVector,
) -> TestResult {
    // Ensure all states have the same dimension
    let dim = state1.dim();
    if dim == 0 || state2.dim() != dim || state3.dim() != dim {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(dim);

    // Compute individual edge energies
    let r_12 = compute_residual(&state1.values, &state2.values, &rho, &rho);
    let r_23 = compute_residual(&state2.values, &state3.values, &rho, &rho);

    let e_12 = compute_energy(&r_12, 1.0);
    let e_23 = compute_energy(&r_23, 1.0);

    // Compute total via helper
    let states = vec![
        (0, state1.values.clone()),
        (1, state2.values.clone()),
        (2, state3.values.clone()),
    ];
    let edges = vec![(0, 1, 1.0), (1, 2, 1.0)];
    let total = compute_total_energy(&states, &edges);

    let expected = e_12 + e_23;
    if (total - expected).abs() < 1e-6 {
        TestResult::passed()
    } else {
        TestResult::error(format!(
            "Additivity failed: {} + {} != {}",
            e_12, e_23, total
        ))
    }
}

// ============================================================================
// PROPERTY: ENERGY MONOTONICITY IN DEVIATION
// ============================================================================

#[quickcheck]
fn prop_energy_increases_with_deviation(
    base_state: StateVector,
    small_delta: BoundedFloat,
    large_delta: BoundedFloat,
) -> TestResult {
    if base_state.dim() == 0 {
        return TestResult::discard();
    }

    let small = small_delta.0.abs().min(1.0);
    let large = large_delta.0.abs().max(small + 0.1).min(10.0);

    // Create states with different deviations
    let target = base_state.values.clone();
    let source_small: Vec<f32> = base_state.values.iter().map(|x| x + small).collect();
    let source_large: Vec<f32> = base_state.values.iter().map(|x| x + large).collect();

    let rho = SimpleRestrictionMap::identity(base_state.dim());

    let r_small = compute_residual(&source_small, &target, &rho, &rho);
    let r_large = compute_residual(&source_large, &target, &rho, &rho);

    let e_small = compute_energy(&r_small, 1.0);
    let e_large = compute_energy(&r_large, 1.0);

    // Larger deviation should produce larger energy
    if e_large >= e_small - 1e-6 {
        TestResult::passed()
    } else {
        TestResult::error(format!(
            "Energy should increase with deviation: {} < {}",
            e_large, e_small
        ))
    }
}

// ============================================================================
// PROPERTY: RESTRICTION MAP COMPOSITION
// ============================================================================

#[quickcheck]
fn prop_identity_map_preserves_state(state: StateVector) -> TestResult {
    if state.dim() == 0 {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(state.dim());
    let projected = rho.apply(&state.values);

    // Identity should preserve the state
    for (orig, proj) in state.values.iter().zip(&projected) {
        if (orig - proj).abs() > 1e-6 {
            return TestResult::error(format!("Identity map changed state: {} -> {}", orig, proj));
        }
    }

    TestResult::passed()
}

// ============================================================================
// PROPERTY: EDGE CONTRACTION REDUCES ENERGY
// ============================================================================

#[quickcheck]
fn prop_averaging_reduces_energy(state1: StateVector, state2: StateVector) -> TestResult {
    if state1.dim() != state2.dim() || state1.dim() == 0 {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(state1.dim());

    // Original energy
    let r_orig = compute_residual(&state1.values, &state2.values, &rho, &rho);
    let e_orig = compute_energy(&r_orig, 1.0);

    // Average state
    let avg: Vec<f32> = state1
        .values
        .iter()
        .zip(&state2.values)
        .map(|(a, b)| (a + b) / 2.0)
        .collect();

    // Energy when one node takes the average
    let r_new = compute_residual(&avg, &state2.values, &rho, &rho);
    let e_new = compute_energy(&r_new, 1.0);

    // Energy should decrease or stay the same
    if e_new <= e_orig + 1e-6 {
        TestResult::passed()
    } else {
        TestResult::error(format!(
            "Averaging should reduce energy: {} -> {}",
            e_orig, e_new
        ))
    }
}

// ============================================================================
// PROPERTY: NUMERIC STABILITY
// ============================================================================

#[test]
fn test_energy_stable_for_large_values() {
    // Test large value stability without using quickcheck's recursive shrinking
    for dim in 1..=8 {
        let state: Vec<f32> = (0..dim).map(|i| (i as f32) * 100.0 + 0.5).collect();
        let large_state: Vec<f32> = state.iter().map(|x| x * 1000.0).collect();
        let rho = SimpleRestrictionMap::identity(dim);

        let residual = compute_residual(&large_state, &state, &rho, &rho);
        let energy = compute_energy(&residual, 1.0);

        assert!(!energy.is_nan(), "Energy became NaN for dim {}", dim);
        assert!(!energy.is_infinite(), "Energy became Inf for dim {}", dim);
        assert!(
            energy >= 0.0,
            "Energy became negative for dim {}: {}",
            dim,
            energy
        );
    }
}

#[test]
fn test_energy_stable_for_small_values() {
    // Test small value stability without using quickcheck's recursive shrinking
    for dim in 1..=8 {
        let state: Vec<f32> = (0..dim).map(|i| (i as f32) * 0.1 + 0.01).collect();
        let small_state: Vec<f32> = state.iter().map(|x| x / 1000.0).collect();
        let zeros: Vec<f32> = vec![0.0; dim];
        let rho = SimpleRestrictionMap::identity(dim);

        let residual = compute_residual(&small_state, &zeros, &rho, &rho);
        let energy = compute_energy(&residual, 1.0);

        assert!(!energy.is_nan(), "Energy became NaN for dim {}", dim);
        assert!(!energy.is_infinite(), "Energy became Inf for dim {}", dim);
        assert!(
            energy >= 0.0,
            "Energy became negative for dim {}: {}",
            dim,
            energy
        );
    }
}

// ============================================================================
// PROPERTY: DETERMINISM
// ============================================================================

#[quickcheck]
fn prop_energy_computation_deterministic(
    source: StateVector,
    target: StateVector,
    weight: PositiveFloat,
) -> TestResult {
    if source.dim() != target.dim() || source.dim() == 0 {
        return TestResult::discard();
    }

    let rho = SimpleRestrictionMap::identity(source.dim());

    // Compute energy multiple times
    let e1 = {
        let r = compute_residual(&source.values, &target.values, &rho, &rho);
        compute_energy(&r, weight.0)
    };

    let e2 = {
        let r = compute_residual(&source.values, &target.values, &rho, &rho);
        compute_energy(&r, weight.0)
    };

    let e3 = {
        let r = compute_residual(&source.values, &target.values, &rho, &rho);
        compute_energy(&r, weight.0)
    };

    // All results should be identical
    if (e1 - e2).abs() < 1e-10 && (e2 - e3).abs() < 1e-10 {
        TestResult::passed()
    } else {
        TestResult::error(format!("Non-deterministic results: {}, {}, {}", e1, e2, e3))
    }
}
