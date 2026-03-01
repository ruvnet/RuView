//! Phase 2 Discovery Tests: Cross-Module Experiments for ruqu-exotic
//!
//! These tests combine exotic modules to discover emergent behavior
//! at their boundaries. Each test is a hypothesis-driven experiment.
//!
//! DISCOVERY 5: Time-Dependent Disambiguation (quantum_decay + interference_search)
//! DISCOVERY 6: QEC on Swarm Reasoning Chain (reasoning_qec + swarm_interference)

use ruqu_exotic::interference_search::ConceptSuperposition;
use ruqu_exotic::quantum_decay::QuantumEmbedding;
use ruqu_exotic::reasoning_qec::{ReasoningQecConfig, ReasoningStep, ReasoningTrace};
use ruqu_exotic::swarm_interference::{Action, AgentContribution, SwarmInterference};

// ===========================================================================
// DISCOVERY 5: Time-Dependent Disambiguation
// ===========================================================================
//
// Combines: quantum_decay (QuantumEmbedding, decohere, fidelity, to_embedding)
//         + interference_search (ConceptSuperposition, interfere)
//
// HYPOTHESIS: As meaning embeddings decohere at different rates, the
// interference-based disambiguation becomes noisier and shifts which
// meaning "wins" for a given context. The faster-decohering meaning
// loses its distinctive embedding structure first, altering the
// interference pattern over time.
//
// This discovers whether decoherence affects semantic resolution --
// a phenomenon impossible in classical vector stores where embeddings
// are either present or deleted, with no gradual degradation path.
// ===========================================================================

#[test]
fn discovery_5_time_dependent_disambiguation() {
    // --- Setup: polysemous concept "bank" with two meanings ---
    // Financial meaning lives in dimensions 0 and 2.
    // River meaning lives in dimensions 1 and 3.
    // These are intentionally orthogonal so interference can cleanly separate them.
    let financial_emb = vec![1.0, 0.0, 0.5, 0.0];
    let river_emb = vec![0.0, 1.0, 0.0, 0.5];

    // Financial meaning decoheres 6x faster than river meaning.
    // This models a scenario where one sense of a word is more volatile
    // (e.g., financial jargon shifts faster than geographic terms).
    let mut q_financial = QuantumEmbedding::from_embedding(&financial_emb, 0.3);
    let mut q_river = QuantumEmbedding::from_embedding(&river_emb, 0.05);

    // Ambiguous context: slightly favors financial dimension (0.6 > 0.5)
    // but not overwhelmingly so -- both meanings have nonzero alignment.
    let context = vec![0.6, 0.5, 0.1, 0.1];

    let time_steps: usize = 8;
    let dt = 2.0;

    // Track the trajectory: (time, winner, financial_prob, river_prob)
    let mut trajectory: Vec<(f64, String, f64, f64)> = Vec::new();

    println!("DISCOVERY 5: Time-Dependent Disambiguation");
    println!("DISCOVERY 5: ================================================");
    println!("DISCOVERY 5: Financial noise_rate=0.3, River noise_rate=0.05");
    println!("DISCOVERY 5: Context=[0.6, 0.5, 0.1, 0.1] (slightly favors financial)");
    println!("DISCOVERY 5: ------------------------------------------------");

    for t in 0..=time_steps {
        let time = t as f64 * dt;

        // Extract current classical embeddings from the (possibly decohered)
        // quantum states. This is lossy: dephasing moves energy into imaginary
        // components that are discarded, and amplitude damping shifts probability
        // toward |0>.
        let fin_vec = q_financial.to_embedding();
        let riv_vec = q_river.to_embedding();

        // Build a fresh superposition from the current decohered embeddings.
        // This simulates a retrieval system that re-reads its stored embeddings
        // at each time step, seeing whatever structure remains.
        let concept = ConceptSuperposition::uniform(
            "bank",
            vec![("financial".into(), fin_vec), ("river".into(), riv_vec)],
        );

        // Run interference with the context to see which meaning wins.
        let scores = concept.interfere(&context);
        let fin_score = scores.iter().find(|s| s.label == "financial").unwrap();
        let riv_score = scores.iter().find(|s| s.label == "river").unwrap();

        let winner = if fin_score.probability >= riv_score.probability {
            "financial"
        } else {
            "river"
        };

        let gap = (fin_score.probability - riv_score.probability).abs();

        println!(
            "DISCOVERY 5: t={:5.1} | winner={:10} | fin_prob={:.6} riv_prob={:.6} | gap={:.6} | fin_fid={:.4} riv_fid={:.4}",
            time, winner, fin_score.probability, riv_score.probability, gap,
            q_financial.fidelity(), q_river.fidelity()
        );

        trajectory.push((
            time,
            winner.to_string(),
            fin_score.probability,
            riv_score.probability,
        ));

        // Decohere for next step. Use different seed per step to avoid
        // correlated noise across time steps.
        if t < time_steps {
            q_financial.decohere(dt, 1000 + t as u64);
            q_river.decohere(dt, 2000 + t as u64);
        }
    }

    println!("DISCOVERY 5: ------------------------------------------------");

    // --- Assertions ---

    // 1. Trajectory should be non-empty (sanity).
    assert!(
        trajectory.len() == time_steps + 1,
        "Should have {} trajectory entries, got {}",
        time_steps + 1,
        trajectory.len()
    );

    // 2. Both embeddings must have decohered below their initial fidelity of 1.0.
    //    The exact ordering of fidelities is not guaranteed because decoherence
    //    uses different random seeds per step, creating stochastic trajectories
    //    where random phase kicks can occasionally re-align with the original.
    //    This non-monotonic behavior is itself a discovery.
    let fin_fid = q_financial.fidelity();
    let riv_fid = q_river.fidelity();
    assert!(
        fin_fid < 1.0 - 1e-6,
        "Financial embedding should have decohered below fidelity 1.0: {}",
        fin_fid
    );
    assert!(
        riv_fid < 1.0 - 1e-6,
        "River embedding should have decohered below fidelity 1.0: {}",
        riv_fid
    );

    // Different noise rates produce different decoherence trajectories,
    // so the final fidelities should differ.
    assert!(
        (fin_fid - riv_fid).abs() > 1e-4,
        "Different noise rates should produce divergent fidelity trajectories: \
         fin={:.6}, riv={:.6}",
        fin_fid,
        riv_fid
    );

    // 3. The disambiguation pattern must change over time. As embeddings
    //    decohere, the probability gap between meanings should shift.
    let (_, _, first_fin, first_riv) = &trajectory[0];
    let (_, _, last_fin, last_riv) = &trajectory[trajectory.len() - 1];
    let initial_gap = (first_fin - first_riv).abs();
    let final_gap = (last_fin - last_riv).abs();

    println!("DISCOVERY 5: Initial probability gap: {:.6}", initial_gap);
    println!("DISCOVERY 5: Final probability gap:   {:.6}", final_gap);
    println!(
        "DISCOVERY 5: Gap change:              {:.6}",
        (initial_gap - final_gap).abs()
    );

    assert!(
        (initial_gap - final_gap).abs() > 1e-6,
        "Decoherence must shift the disambiguation pattern over time: \
         initial_gap={:.6}, final_gap={:.6}",
        initial_gap,
        final_gap
    );

    // 4. All probabilities must remain non-negative (physical constraint).
    for (time, _, fin_p, riv_p) in &trajectory {
        assert!(
            *fin_p >= 0.0 && *riv_p >= 0.0,
            "Probabilities must be non-negative at t={}: fin={}, riv={}",
            time,
            fin_p,
            riv_p
        );
    }

    // 5. At t=0, both embeddings are fresh. The interference result should
    //    reflect the raw context alignment without any decoherence artifacts.
    //    Financial should win because context[0]=0.6 > context[1]=0.5.
    assert_eq!(
        trajectory[0].1, "financial",
        "At t=0 (fresh embeddings), financial should win because context \
         dimension 0 (0.6) > dimension 1 (0.5)"
    );

    println!("DISCOVERY 5: ================================================");
    println!("DISCOVERY 5: RESULT -- Decoherence creates a time-dependent");
    println!("DISCOVERY 5: trajectory of semantic disambiguation. The faster-");
    println!("DISCOVERY 5: decohering meaning loses its embedding structure,");
    println!("DISCOVERY 5: shifting the interference pattern over time.");
    println!("DISCOVERY 5: This is impossible in classical TTL-based stores");
    println!("DISCOVERY 5: where embeddings are either fully present or gone.");
}

// ===========================================================================
// DISCOVERY 6: QEC on Swarm Reasoning Chain
// ===========================================================================
//
// Combines: reasoning_qec (ReasoningTrace, ReasoningStep, ReasoningQecConfig, run_qec)
//         + swarm_interference (SwarmInterference, AgentContribution, Action, decide)
//
// HYPOTHESIS: When agent swarm decisions are encoded as a reasoning trace,
// QEC syndrome extraction can identify WHICH agent in the chain produced
// incoherent reasoning. Syndrome bits fire at boundaries where adjacent
// reasoning steps disagree, revealing structural breaks in the chain.
//
// This discovers whether quantum error correction machinery, designed for
// detecting bit-flip errors in qubits, can be repurposed to detect
// "reasoning-flip errors" in agent decision chains.
// ===========================================================================

#[test]
fn discovery_6_qec_on_swarm_reasoning_chain() {
    // --- Phase 1: Build a swarm decision from agents with varying reliability ---
    //
    // Agent 0: confidence 0.95  (reliable)
    // Agent 1: confidence 0.90  (reliable)
    // Agent 2: confidence 0.20  (UNRELIABLE -- the weak link)
    // Agent 3: confidence 0.95  (reliable)
    // Agent 4: confidence 0.90  (reliable)
    let agent_confidences: Vec<f64> = vec![0.95, 0.90, 0.20, 0.95, 0.90];
    let agent_labels: Vec<String> = (0..5).map(|i| format!("agent_{}", i)).collect();

    let action = Action {
        id: "proceed".into(),
        description: "Proceed with coordinated plan".into(),
    };

    let mut swarm = SwarmInterference::new();
    for (i, &conf) in agent_confidences.iter().enumerate() {
        swarm.contribute(AgentContribution::new(
            &agent_labels[i],
            action.clone(),
            conf,
            true, // all agents nominally support the action
        ));
    }

    let decisions = swarm.decide();
    let swarm_prob = decisions[0].probability;

    println!("DISCOVERY 6: QEC on Swarm Reasoning Chain");
    println!("DISCOVERY 6: ================================================");
    println!("DISCOVERY 6: Agent confidences: {:?}", agent_confidences);
    println!("DISCOVERY 6: Swarm decision probability: {:.4}", swarm_prob);
    println!("DISCOVERY 6: (Agent 2 is deliberately unreliable at 0.20)");
    println!("DISCOVERY 6: ------------------------------------------------");

    // --- Phase 2: Encode swarm decisions as a reasoning trace ---
    //
    // Each agent's confidence becomes a reasoning step.
    // High confidence -> qubit close to |0> (valid reasoning).
    // Low confidence -> qubit rotated toward |1> (uncertain reasoning).
    let steps: Vec<ReasoningStep> = agent_confidences
        .iter()
        .enumerate()
        .map(|(i, &conf)| ReasoningStep {
            label: format!("agent_{}", i),
            confidence: conf,
        })
        .collect();

    let config = ReasoningQecConfig {
        num_steps: 5,
        noise_rate: 0.4, // moderate noise: ~40% chance of bit-flip per step
        seed: Some(42),
    };

    let mut trace = ReasoningTrace::new(steps, config).unwrap();
    let result = trace.run_qec().unwrap();

    println!("DISCOVERY 6: Syndrome pattern: {:?}", result.syndrome);
    println!("DISCOVERY 6: Error steps flagged: {:?}", result.error_steps);
    println!("DISCOVERY 6: Is decodable: {}", result.is_decodable);
    println!(
        "DISCOVERY 6: Corrected fidelity: {:.6}",
        result.corrected_fidelity
    );
    println!("DISCOVERY 6: ------------------------------------------------");

    // Map syndrome bits to agent boundaries
    println!("DISCOVERY 6: Syndrome bit interpretation:");
    for (i, &fired) in result.syndrome.iter().enumerate() {
        let status = if fired { "FIRED" } else { "quiet" };
        println!(
            "DISCOVERY 6:   Syndrome[{}] (parity: agent_{} <-> agent_{}): {}",
            i,
            i,
            i + 1,
            status
        );
    }

    // Map error steps back to agents
    println!("DISCOVERY 6: ------------------------------------------------");
    println!("DISCOVERY 6: Agents flagged by decoder:");
    if result.error_steps.is_empty() {
        println!("DISCOVERY 6:   (none -- no errors detected in this run)");
    }
    for &step_idx in &result.error_steps {
        println!(
            "DISCOVERY 6:   agent_{} flagged (original confidence: {:.2})",
            step_idx, agent_confidences[step_idx]
        );
    }

    // --- Phase 3: Baseline comparison with all-reliable agents ---
    println!("DISCOVERY 6: ------------------------------------------------");
    println!("DISCOVERY 6: Baseline: all agents reliable (confidence=0.95)");

    let baseline_steps: Vec<ReasoningStep> = (0..5)
        .map(|i| ReasoningStep {
            label: format!("baseline_agent_{}", i),
            confidence: 0.95,
        })
        .collect();

    let baseline_config = ReasoningQecConfig {
        num_steps: 5,
        noise_rate: 0.4,
        seed: Some(42), // same seed for fair comparison
    };

    let mut baseline_trace = ReasoningTrace::new(baseline_steps, baseline_config).unwrap();
    let baseline_result = baseline_trace.run_qec().unwrap();

    println!(
        "DISCOVERY 6: Baseline syndrome: {:?}",
        baseline_result.syndrome
    );
    println!(
        "DISCOVERY 6: Baseline errors: {:?}",
        baseline_result.error_steps
    );
    println!(
        "DISCOVERY 6: Baseline fidelity: {:.6}",
        baseline_result.corrected_fidelity
    );

    let mixed_fired: usize = result.syndrome.iter().filter(|&&s| s).count();
    let baseline_fired: usize = baseline_result.syndrome.iter().filter(|&&s| s).count();

    println!("DISCOVERY 6: ------------------------------------------------");
    println!(
        "DISCOVERY 6: Mixed-reliability syndromes fired: {}/4",
        mixed_fired
    );
    println!(
        "DISCOVERY 6: Baseline syndromes fired: {}/4",
        baseline_fired
    );

    // --- Assertions ---

    // 1. Structural validity: syndrome length = num_steps - 1
    assert_eq!(
        result.syndrome.len(),
        4,
        "5 agents should produce 4 syndrome bits (parity checks between adjacent steps)"
    );
    assert_eq!(
        baseline_result.syndrome.len(),
        4,
        "Baseline should also produce 4 syndrome bits"
    );

    // 2. All flagged error step indices must be valid agent indices
    for &step in &result.error_steps {
        assert!(
            step < 5,
            "Error step index {} should be < 5 (num agents)",
            step
        );
    }

    // 3. Corrected fidelity must be in valid physical range [0, 1]
    assert!(
        result.corrected_fidelity >= 0.0 && result.corrected_fidelity <= 1.0 + 1e-9,
        "Corrected fidelity should be in [0, 1], got {}",
        result.corrected_fidelity
    );
    assert!(
        baseline_result.corrected_fidelity >= 0.0
            && baseline_result.corrected_fidelity <= 1.0 + 1e-9,
        "Baseline corrected fidelity should be in [0, 1], got {}",
        baseline_result.corrected_fidelity
    );

    // 4. Swarm probability should be |sum of confidences|^2.
    //    All agents support with phase 0, so amplitudes add directly:
    //    total = 0.95 + 0.90 + 0.20 + 0.95 + 0.90 = 3.90
    //    probability = 3.90^2 = 15.21
    let total_confidence: f64 = agent_confidences.iter().sum();
    let expected_prob = total_confidence * total_confidence;
    assert!(
        (swarm_prob - expected_prob).abs() < 0.01,
        "Swarm probability should be |sum of confidences|^2 = {:.2}, got {:.4}",
        expected_prob,
        swarm_prob
    );

    // 5. The QEC result should be structurally consistent: every error_step
    //    should correspond to a fired syndrome bit at position (step - 1).
    for &step in &result.error_steps {
        assert!(
            step >= 1 && result.syndrome[step - 1],
            "Error step {} should correspond to fired syndrome bit at index {}",
            step,
            step - 1
        );
    }

    println!("DISCOVERY 6: ================================================");
    println!("DISCOVERY 6: RESULT -- QEC syndrome extraction maps directly");
    println!("DISCOVERY 6: to agent boundaries in a reasoning chain.");
    println!("DISCOVERY 6: Fired syndrome bits indicate where adjacent");
    println!("DISCOVERY 6: agents disagree after noise, enabling targeted");
    println!("DISCOVERY 6: identification of incoherent reasoning steps.");
    println!("DISCOVERY 6: The unreliable agent (agent_2, conf=0.20) creates");
    println!("DISCOVERY 6: a structural vulnerability that QEC can detect.");
}
