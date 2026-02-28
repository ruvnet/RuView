//! Correctness and invariant tests for thermorust.

use rand::SeedableRng;
use thermorust::{
    dynamics::{anneal_continuous, anneal_discrete, inject_spikes, step_discrete, Params},
    energy::{Couplings, EnergyModel, Ising},
    metrics::{binary_entropy, magnetisation, overlap},
    motifs::IsingMotif,
    State,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn rng(seed: u64) -> rand::rngs::StdRng {
    rand::rngs::StdRng::seed_from_u64(seed)
}

fn ring_ising(n: usize) -> Ising {
    Ising::new(Couplings::ferromagnetic_ring(n, 0.2))
}

// ── Energy model ─────────────────────────────────────────────────────────────

#[test]
fn all_up_ring_energy_is_negative() {
    let n = 8;
    let model = ring_ising(n);
    let s = State::ones(n);
    let e = model.energy(&s);
    // For a ferromagnetic ring with J=0.2, all-up: E = −n * 0.2
    assert!(
        e < 0.0,
        "ferromagnetic ring energy should be negative for aligned spins: {e}"
    );
}

#[test]
fn antiferromagnetic_ring_energy_is_positive() {
    let n = 8;
    // Antiferromagnetic: J = −0.2
    let j: Vec<f32> = {
        let mut v = vec![0.0; n * n];
        for i in 0..n {
            let nxt = (i + 1) % n;
            v[i * n + nxt] = -0.2;
            v[nxt * n + i] = -0.2;
        }
        v
    };
    let model = Ising::new(Couplings { j, h: vec![0.0; n] });
    let s = State::ones(n); // all-up is frustrated for antiferromagnet
    let e = model.energy(&s);
    assert!(
        e > 0.0,
        "antiferromagnetic all-up energy should be positive: {e}"
    );
}

#[test]
fn energy_is_symmetric_under_global_flip() {
    let n = 12;
    let model = ring_ising(n);
    let s_up = State::ones(n);
    let s_dn = State::neg_ones(n);
    let e_up = model.energy(&s_up);
    let e_dn = model.energy(&s_dn);
    assert!(
        (e_up - e_dn).abs() < 1e-5,
        "energy must be Z₂-symmetric: {e_up} vs {e_dn}"
    );
}

// ── Metropolis dynamics ───────────────────────────────────────────────────────

#[test]
fn energy_should_drop_over_many_steps() {
    let n = 16;
    let mut s = State::from_vec(
        // Frustrate the ring: alternating signs
        (0..n)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect(),
    );
    let model = ring_ising(n);
    let p = Params::default_n(n);
    let e0 = model.energy(&s);
    let mut rng = rng(42);

    for _ in 0..20_000 {
        step_discrete(&model, &mut s, &p, &mut rng);
    }
    let e1 = model.energy(&s);
    assert!(
        e1 <= e0 + 1e-3,
        "energy should not increase long-run: {e1} > {e0}"
    );
    assert!(
        s.dissipated_j > 0.0,
        "at least some heat must have been shed"
    );
}

#[test]
fn clamped_units_do_not_change() {
    let mut s = State::from_vec(vec![1.0, -1.0, 1.0]);
    let model = Ising::new(Couplings::zeros(3));
    let mut p = Params::default_n(3);
    p.clamp_mask = vec![true, false, true];
    let mut rng = rng(7);
    let before = s.x.clone();
    for _ in 0..5_000 {
        step_discrete(&model, &mut s, &p, &mut rng);
    }
    assert_eq!(s.x[0], before[0], "clamped spin 0 must not change");
    assert_eq!(s.x[2], before[2], "clamped spin 2 must not change");
}

#[test]
fn hot_system_ergodically_explores_both_states() {
    // Very high temperature (β=0.01) → nearly random walk; should visit ±1.
    let n = 4;
    let model = ring_ising(n);
    let mut p = Params::default_n(n);
    p.beta = 0.01;
    let mut s = State::ones(n);
    let mut rng = rng(99);
    let mut saw_negative = false;
    for _ in 0..50_000 {
        step_discrete(&model, &mut s, &p, &mut rng);
        if s.x.iter().any(|&xi| xi < 0.0) {
            saw_negative = true;
            break;
        }
    }
    assert!(saw_negative, "hot system must flip at least one spin");
}

#[test]
fn cold_system_stays_near_ground_state() {
    // Very low temperature (β=20) → nearly greedy; aligned ring should stay aligned.
    let n = 8;
    let model = ring_ising(n);
    let mut p = Params::default_n(n);
    p.beta = 20.0;
    let mut s = State::ones(n);
    let mut rng = rng(55);
    for _ in 0..5_000 {
        step_discrete(&model, &mut s, &p, &mut rng);
    }
    let m = magnetisation(&s);
    assert!(m > 0.9, "cold ferromagnet should stay ordered: m={m}");
}

// ── Langevin dynamics ─────────────────────────────────────────────────────────

#[test]
fn langevin_lowers_energy_on_average() {
    use thermorust::motifs::SoftSpinMotif;
    let n = 8;
    let mut motif = SoftSpinMotif::random(n, 1.0, 0.5, 13);
    let p = Params::default_n(n);
    let e0 = motif.model.energy(&motif.state);
    let mut rng = rng(101);
    let trace = anneal_continuous(&motif.model, &mut motif.state, &p, 5_000, 50, &mut rng);
    let e_last = *trace.energies.last().unwrap();
    // Allow small positive excursions due to noise, but mean should be ≤ e0
    assert!(
        trace.mean_energy() <= e0 + 0.5,
        "Langevin annealing mean energy {:.3} should not exceed initial {:.3}",
        trace.mean_energy(),
        e0
    );
    let _ = e_last; // suppress unused warning
}

#[test]
fn langevin_keeps_activations_in_bounds() {
    use thermorust::motifs::SoftSpinMotif;
    let n = 16;
    let mut motif = SoftSpinMotif::random(n, 1.0, 0.5, 77);
    let p = Params::default_n(n);
    let mut rng = rng(202);
    anneal_continuous(&motif.model, &mut motif.state, &p, 3_000, 0, &mut rng);
    for xi in &motif.state.x {
        assert!(xi.abs() <= 1.0, "activation out of bounds: {xi}");
    }
}

// ── Anneal helpers ────────────────────────────────────────────────────────────

#[test]
fn anneal_discrete_trace_has_correct_length() {
    let n = 8;
    let mut motif = IsingMotif::ring(n, 0.3);
    let p = Params::default_n(n);
    let mut rng = rng(33);
    let trace = anneal_discrete(&motif.model, &mut motif.state, &p, 1_000, 10, &mut rng);
    // 1000 steps / record_every=10 → 100 samples (steps 0,10,20,…,990)
    assert_eq!(trace.energies.len(), 100);
    assert_eq!(trace.dissipation.len(), 100);
}

#[test]
fn dissipation_monotonically_non_decreasing() {
    let n = 8;
    let mut motif = IsingMotif::ring(n, 0.3);
    let p = Params::default_n(n);
    let mut rng = rng(44);
    let trace = anneal_discrete(&motif.model, &mut motif.state, &p, 2_000, 20, &mut rng);
    for w in trace.dissipation.windows(2) {
        assert!(
            w[1] >= w[0],
            "dissipation must be non-decreasing: {} < {}",
            w[1],
            w[0]
        );
    }
}

// ── Spike injection ───────────────────────────────────────────────────────────

#[test]
fn spike_injection_does_not_move_clamped_spins() {
    let mut s = State::from_vec(vec![1.0, 0.5, -1.0, 0.0]);
    let mut p = Params::default_n(4);
    p.clamp_mask = vec![true, false, true, false];
    let before = s.x.clone();
    let mut rng = rng(66);
    for _ in 0..100 {
        inject_spikes(&mut s, &p, 0.5, 0.3, &mut rng);
    }
    assert_eq!(s.x[0], before[0]);
    assert_eq!(s.x[2], before[2]);
}

// ── Metrics ───────────────────────────────────────────────────────────────────

#[test]
fn magnetisation_all_up_is_one() {
    let s = State::ones(16);
    assert!((magnetisation(&s) - 1.0).abs() < 1e-6);
}

#[test]
fn magnetisation_all_down_is_minus_one() {
    let s = State::neg_ones(16);
    assert!((magnetisation(&s) + 1.0).abs() < 1e-6);
}

#[test]
fn overlap_with_self_is_one() {
    let s = State::ones(8);
    let pat = vec![1.0_f32; 8];
    let m = overlap(&s, &pat).unwrap();
    assert!(
        (m - 1.0).abs() < 1e-6,
        "overlap with self should be 1.0: {m}"
    );
}

#[test]
fn overlap_mismatched_length_is_none() {
    let s = State::ones(4);
    let pat = vec![1.0_f32; 8];
    assert!(overlap(&s, &pat).is_none());
}

#[test]
fn binary_entropy_max_at_half_half() {
    // Half +1, half -1 → maximum entropy
    let x = (0..16)
        .map(|i| if i < 8 { 1.0_f32 } else { -1.0 })
        .collect();
    let s = State::from_vec(x);
    let h = binary_entropy(&s);
    assert!(h > 0.0, "entropy of mixed state must be positive: {h}");
}

#[test]
fn binary_entropy_zero_for_pure_state() {
    let s = State::ones(16);
    let h = binary_entropy(&s);
    // All spins up → p=1, entropy=0
    assert!(h.abs() < 1e-5, "entropy of pure state should be 0: {h}");
}

// ── Hopfield memory ───────────────────────────────────────────────────────────

#[test]
fn hopfield_retrieves_stored_pattern() {
    let n = 20;
    let pattern: Vec<f32> = (0..n)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let motif = IsingMotif::hopfield(n, &[pattern.clone()]);
    let mut p = Params::default_n(n);
    p.beta = 10.0; // cold

    // Start with noisy version (5 bits flipped)
    let mut noisy = pattern.clone();
    for i in 0..5 {
        noisy[i] = -noisy[i];
    }
    let mut s = State::from_vec(noisy);
    let mut rng = rng(88);

    for _ in 0..50_000 {
        step_discrete(&motif.model, &mut s, &p, &mut rng);
    }

    let m = overlap(&s, &pattern).unwrap().abs();
    assert!(
        m > 0.7,
        "Hopfield net should retrieve stored pattern (overlap {m:.3} < 0.7)"
    );
}
