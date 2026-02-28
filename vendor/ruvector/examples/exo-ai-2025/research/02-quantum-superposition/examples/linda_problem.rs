//! Linda Problem: Conjunction Fallacy Demonstration
//!
//! Classic cognitive bias where people judge P(A∧B) > P(A) when A∧B is more
//! "representative" of a description, violating probability axioms.
//!
//! CAFT explains this via amplitude overlap: the conjunction state can have
//! higher amplitude (and thus probability) if it strongly overlaps with the
//! semantically dominant feature.

use quantum_cognition::{CognitiveState, InterferenceDecisionMaker};

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║        LINDA PROBLEM: Conjunction Fallacy via CAFT           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!("Description:");
    println!("  Linda is 31 years old, single, outspoken, and very bright.");
    println!("  She majored in philosophy. As a student, she was deeply");
    println!("  concerned with issues of discrimination and social justice,");
    println!("  and participated in anti-nuclear demonstrations.\n");

    println!("Which is more probable?\n");
    println!("  (A) Linda is a bank teller");
    println!("  (B) Linda is active in the feminist movement");
    println!("  (C) Linda is a bank teller AND active in the feminist movement\n");

    println!("─────────────────────────────────────────────────────────────────\n");

    // Create initial cognitive state with three options
    let labels = vec![
        "bank_teller".to_string(),
        "feminist".to_string(),
        "feminist_bank_teller".to_string(),
    ];
    let initial_state = CognitiveState::uniform(3, labels);

    let mut decision_maker = InterferenceDecisionMaker::new(initial_state);

    println!("CAFT Simulation:\n");

    // Run conjunction decision with varying overlap strengths
    for overlap in [0.3, 0.5, 0.7, 0.9].iter() {
        let (probs, _choice) = decision_maker.conjunction_decision(
            "bank_teller",
            "feminist",
            "feminist_bank_teller",
            *overlap,
        );

        println!("Semantic Overlap = {:.1}", overlap);
        println!("  P(bank teller)                = {:.4}", probs[0]);
        println!("  P(feminist)                   = {:.4}", probs[1]);
        println!("  P(feminist ∧ bank teller)     = {:.4}", probs[2]);

        if probs[2] > probs[0] {
            println!("  ⚠️  CONJUNCTION FALLACY: P(A∧B) > P(A) ✓");
        } else {
            println!("  ✓ Classical probability satisfied");
        }

        println!();
    }

    println!("─────────────────────────────────────────────────────────────────\n");
    println!("Interpretation:");
    println!("  • Low overlap (0.3): Classical probability holds");
    println!("  • High overlap (0.9): Conjunction fallacy emerges");
    println!("  • The 'feminist' feature has high amplitude due to description");
    println!("  • Conjunction inherits this amplitude → appears more probable");
    println!("  • Humans use representativeness (amplitude) not logic (probability)\n");

    println!("Experimental Evidence:");
    println!("  • 85% of subjects judge P(feminist ∧ bank teller) > P(bank teller)");
    println!("  • CAFT reproduces this with high semantic overlap parameter");
    println!("  • Shows human cognition uses quantum-like amplitude superposition\n");

    println!("Key Insight:");
    println!("  Classical: P(A∧B) ≤ min(P(A), P(B))  [always]");
    println!("  CAFT:      P(A∧B) can exceed P(A)   [with amplitude interference]");
    println!("  Human:     Matches CAFT prediction  [representativeness heuristic]\n");
}
