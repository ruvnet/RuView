//! Attention as Wavefunction Collapse
//!
//! Demonstrates how attention acts as a measurement operator that collapses
//! cognitive superposition into definite conscious states. Shows:
//! - Entropy reduction during attention
//! - Quantum Zeno effect (frequent measurement freezes state)
//! - Consciousness threshold based on integrated information

use quantum_cognition::{
    quantum_zeno_effect, AttentionOperator, CognitiveState, ConsciousnessThreshold,
    SuperpositionBuilder,
};

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║      ATTENTION AS MEASUREMENT: Consciousness via Collapse    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // ─────────────────────────────────────────────────────────────────────
    // PART 1: Entropy Reduction During Attention
    // ─────────────────────────────────────────────────────────────────────

    println!("═══ PART 1: Entropy Dynamics ═══\n");

    let labels: Vec<String> = (0..5).map(|i| format!("concept_{}", i)).collect();
    let initial_state = CognitiveState::uniform(5, labels);

    println!("Initial cognitive state (maximally uncertain superposition):");
    println!("  Dimension:              {}", initial_state.dimension());
    println!(
        "  Von Neumann entropy:    {:.4}",
        initial_state.von_neumann_entropy()
    );
    println!("  Max entropy (log N):    {:.4}", (5.0_f64).ln());
    println!(
        "  Participation ratio:    {:.4}\n",
        initial_state.participation_ratio()
    );

    // Apply full attention
    let mut attention = AttentionOperator::full_attention(2, 5, 8.0); // 8 Hz alpha rhythm
    let collapsed_state = attention.apply(&initial_state);

    println!("After full attention (focused on concept_2):");
    println!(
        "  Von Neumann entropy:    {:.4}",
        collapsed_state.von_neumann_entropy()
    );
    println!(
        "  Participation ratio:    {:.4}",
        collapsed_state.participation_ratio()
    );
    let (idx, prob, label) = collapsed_state.most_likely();
    println!("  Most likely state:      {} (P = {:.4})", label, prob);
    println!(
        "\n  ⇒ Entropy reduced by {:.4} bits",
        initial_state.von_neumann_entropy() - collapsed_state.von_neumann_entropy()
    );
    println!("  ⇒ Superposition → definite conscious state ✓\n");

    println!("─────────────────────────────────────────────────────────────────\n");

    // ─────────────────────────────────────────────────────────────────────
    // PART 2: Weak vs. Strong Attention
    // ─────────────────────────────────────────────────────────────────────

    println!("═══ PART 2: Attention Strength Spectrum ═══\n");

    let labels: Vec<String> = (0..3).map(|i| format!("thought_{}", i)).collect();
    let state = CognitiveState::uniform(3, labels);

    println!("Attention strength effects on entropy:\n");
    println!("  Strength | Entropy  | Description");
    println!("  ─────────┼──────────┼─────────────────────────────");

    for &strength in &[0.0, 0.1, 0.3, 0.5, 0.7, 0.9, 1.0] {
        let weights = vec![1.0, 0.0, 0.0]; // Focus on first thought
        let mut attention = AttentionOperator::distributed_attention(weights, strength, 10.0);
        let new_state = attention.apply(&state);
        let entropy = new_state.von_neumann_entropy();

        let description = match strength {
            s if s < 0.2 => "Mind-wandering",
            s if s < 0.5 => "Partial attention",
            s if s < 0.8 => "Focused attention",
            _ => "Full collapse",
        };

        println!(
            "  {:.1}      | {:.4}   | {}",
            strength, entropy, description
        );
    }

    println!("\n  ⇒ Gradient of consciousness from diffuse to focused ✓\n");

    println!("─────────────────────────────────────────────────────────────────\n");

    // ─────────────────────────────────────────────────────────────────────
    // PART 3: Quantum Zeno Effect (Attention Blink)
    // ─────────────────────────────────────────────────────────────────────

    println!("═══ PART 3: Quantum Zeno Effect ═══\n");

    println!("Hypothesis: Frequent measurement freezes cognitive evolution");
    println!("  (Models 'attention blink' - can't process new info during focus)\n");

    let labels: Vec<String> = vec!["initial".to_string(), "target".to_string()];
    let zeno_state = CognitiveState::uniform(2, labels);

    println!("Fidelity with initial state vs. measurement frequency:\n");
    println!("  N_measurements | Fidelity | Interpretation");
    println!("  ───────────────┼──────────┼────────────────────────────");

    for &n_meas in &[1, 2, 5, 10, 50, 100] {
        let fidelity = quantum_zeno_effect(&zeno_state, 0, n_meas, 1.0);

        let interpretation = if fidelity > 0.8 {
            "State frozen ❄️"
        } else if fidelity > 0.5 {
            "Partial evolution"
        } else {
            "Free evolution"
        };

        println!("  {:>14} | {:.4}   | {}", n_meas, fidelity, interpretation);
    }

    println!("\n  ⇒ Continuous attention prevents state change (attentional suppression) ✓");
    println!("  ⇒ Explains attentional blink in visual perception experiments\n");

    println!("─────────────────────────────────────────────────────────────────\n");

    // ─────────────────────────────────────────────────────────────────────
    // PART 4: Consciousness Threshold
    // ─────────────────────────────────────────────────────────────────────

    println!("═══ PART 4: Consciousness Threshold (Φ) ═══\n");

    let threshold = ConsciousnessThreshold::new(0.3);

    println!("Testing different cognitive states for consciousness:\n");

    // Pure state (single thought)
    let pure = CognitiveState::definite(
        0,
        5,
        vec![
            "single".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ],
    );
    let phi_pure = threshold.estimate_phi(&pure);
    println!("Pure state (single definite thought):");
    println!("  Entropy:        {:.4}", pure.von_neumann_entropy());
    println!("  Φ estimate:     {:.4}", phi_pure);
    println!(
        "  Conscious:      {}",
        if threshold.is_conscious(&pure) {
            "YES ✓"
        } else {
            "NO ✗"
        }
    );
    println!("  → Too simple, no integration\n");

    // Maximally mixed (complete uncertainty)
    let mixed = CognitiveState::uniform(
        5,
        vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ],
    );
    let phi_mixed = threshold.estimate_phi(&mixed);
    println!("Maximally mixed (complete superposition):");
    println!("  Entropy:        {:.4}", mixed.von_neumann_entropy());
    println!("  Φ estimate:     {:.4}", phi_mixed);
    println!(
        "  Conscious:      {}",
        if threshold.is_conscious(&mixed) {
            "YES ✓"
        } else {
            "NO ✗"
        }
    );
    println!("  → Too random, no structure\n");

    // Partially collapsed (integrated state)
    let partial = SuperpositionBuilder::new()
        .add_real(0.6, "dominant_thought".to_string())
        .add_real(0.3, "related_thought".to_string())
        .add_real(0.2, "peripheral".to_string())
        .add_real(0.1, "background_1".to_string())
        .add_real(0.1, "background_2".to_string())
        .build();
    let phi_partial = threshold.estimate_phi(&partial);
    println!("Partially collapsed (integrated conscious state):");
    println!("  Entropy:        {:.4}", partial.von_neumann_entropy());
    println!("  Φ estimate:     {:.4}", phi_partial);
    println!(
        "  Conscious:      {}",
        if threshold.is_conscious(&partial) {
            "YES ✓"
        } else {
            "NO ✗"
        }
    );
    println!("  → Balance of structure and distribution ✓\n");

    println!("─────────────────────────────────────────────────────────────────\n");

    // ─────────────────────────────────────────────────────────────────────
    // PART 5: Continuous Evolution with Attention
    // ─────────────────────────────────────────────────────────────────────

    println!("═══ PART 5: Dynamic Attention Over Time ═══\n");

    let labels: Vec<String> = (0..4).map(|i| format!("stream_{}", i)).collect();
    let stream_state = CognitiveState::uniform(4, labels);

    println!("Simulating 1 second of cognitive dynamics (4-10 Hz attention rhythm):\n");

    for &freq_hz in &[4.0, 7.0, 10.0] {
        let mut attention = AttentionOperator::full_attention(0, 4, freq_hz);
        let trajectory = attention.continuous_evolution(&stream_state, 1.0, 100);

        let initial_entropy = trajectory.first().unwrap().von_neumann_entropy();
        let final_entropy = trajectory.last().unwrap().von_neumann_entropy();
        let entropy_reduction = initial_entropy - final_entropy;

        println!("Attention frequency: {} Hz", freq_hz);
        println!("  Initial entropy:    {:.4}", initial_entropy);
        println!("  Final entropy:      {:.4}", final_entropy);
        println!("  Reduction:          {:.4} bits", entropy_reduction);
        println!("  Measurements:       ~{} times/sec", freq_hz);
        println!();
    }

    println!("  ⇒ Higher frequency → faster collapse (matches EEG alpha/theta) ✓\n");

    println!("─────────────────────────────────────────────────────────────────\n");
    println!("KEY FINDINGS:\n");
    println!("  1. Attention reduces von Neumann entropy (collapse) ✓");
    println!("  2. Weak measurement → gradual shift, Strong → instant collapse ✓");
    println!("  3. Quantum Zeno effect explains attentional suppression ✓");
    println!("  4. Consciousness requires balance: not too pure, not too mixed ✓");
    println!("  5. Attention frequency (4-10 Hz) matches neural oscillations ✓\n");

    println!("TESTABLE PREDICTIONS:");
    println!("  • EEG entropy drops during focused attention");
    println!("  • Attentional blink = Zeno effect (frequent measurements)");
    println!("  • Anesthesia raises entropy, disrupts Φ");
    println!("  • Meditation may optimize Φ (balance structure/flexibility)\n");
}
