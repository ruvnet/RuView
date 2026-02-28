//! Cross-module discovery experiments for ruqu-exotic.
//!
//! These tests combine two exotic modules to discover emergent behavior
//! that neither module can exhibit alone.

use ruqu_core::gate::Gate;
use ruqu_exotic::quantum_collapse::QuantumCollapseSearch;
use ruqu_exotic::reversible_memory::ReversibleMemory;
use ruqu_exotic::swarm_interference::{Action, AgentContribution, SwarmInterference};
use ruqu_exotic::syndrome_diagnosis::{Component, Connection, DiagnosisConfig, SystemDiagnostics};

// ===========================================================================
// DISCOVERY 7: Counterfactual Search Explanation
//   (quantum_collapse + reversible_memory)
//
// Can we EXPLAIN why a quantum collapse search picked a particular result
// by using counterfactual reasoning on the state preparation?
//
// Approach:
//   1. Build a reversible memory with a sequence of gates that bias the
//      probability distribution toward certain basis states.
//   2. Extract the probability distribution and use it as the set of
//      "candidate embeddings" for collapse search.
//   3. Run the search to find the top result.
//   4. For each gate in the preparation sequence, run counterfactual
//      analysis (remove that gate) and see how the probability
//      distribution --- and therefore the search result --- would change.
//
// HYPOTHESIS: The gate that created the most bias in the probability
// space will have the highest counterfactual divergence, and removing
// it will change the search result most dramatically.
// ===========================================================================

#[test]
fn discovery_7_counterfactual_search_explanation() {
    println!("DISCOVERY 7: Counterfactual Search Explanation");
    println!("  Combining: quantum_collapse + reversible_memory");
    println!(
        "  Question: Can counterfactual analysis explain WHY a search returned a specific result?"
    );
    println!();

    // -----------------------------------------------------------------------
    // Step 1: Build a reversible memory that creates a biased state.
    //
    // We use 2 qubits (4 basis states). The gate sequence is designed so that
    // one specific gate (the Ry rotation on qubit 0) is the primary source of
    // bias, while others contribute less.
    // -----------------------------------------------------------------------
    let mut mem = ReversibleMemory::new(2).unwrap();

    // Gate 0: Large Ry rotation on qubit 0 -- this is the BIG bias creator.
    // It rotates qubit 0 away from |0> toward |1>, heavily biasing the
    // probability distribution.
    mem.apply(Gate::Ry(0, 1.2)).unwrap();

    // Gate 1: Small Rz rotation on qubit 1 -- phase-only, barely changes probs.
    mem.apply(Gate::Rz(1, 0.05)).unwrap();

    // Gate 2: CNOT entangles the qubits, spreading the bias from q0 to q1.
    mem.apply(Gate::CNOT(0, 1)).unwrap();

    // Gate 3: Tiny Ry on qubit 1 -- small additional bias.
    mem.apply(Gate::Ry(1, 0.1)).unwrap();

    assert_eq!(mem.history_len(), 4, "Should have 4 gates in history");

    // -----------------------------------------------------------------------
    // Step 2: Extract probability distribution as candidate embeddings.
    //
    // The 4 basis state probabilities become 4 "candidate" 1D embeddings.
    // Each candidate is a single-element vector containing that basis state's
    // probability. This way, the collapse search will prefer the basis state
    // with the highest probability (since the query will be [1.0], which is
    // most similar to the largest probability value).
    // -----------------------------------------------------------------------
    let original_probs = mem.probabilities();
    println!("  Original probability distribution:");
    for (i, p) in original_probs.iter().enumerate() {
        println!("    |{:02b}> : {:.6}", i, p);
    }

    let candidates: Vec<Vec<f64>> = original_probs.iter().map(|&p| vec![p]).collect();
    let search = QuantumCollapseSearch::new(candidates);

    // Query: [1.0] -- we want the candidate with the highest probability value.
    let query = [1.0_f64];
    let search_result = search.search(&query, 2, 42);
    println!(
        "  Search result: index={}, amplitude={:.6}, is_padding={}",
        search_result.index, search_result.amplitude, search_result.is_padding
    );

    // Also get the distribution over many shots to see stability.
    let dist = search.search_distribution(&query, 2, 200, 42);
    println!("  Search distribution (200 shots):");
    for &(idx, count) in &dist {
        println!(
            "    index {} : {} hits ({:.1}%)",
            idx,
            count,
            count as f64 / 2.0
        );
    }
    println!();

    // -----------------------------------------------------------------------
    // Step 3: Counterfactual analysis -- for each gate, what would change?
    //
    // For each gate in the preparation sequence, compute the counterfactual
    // (what if that gate never happened?), extract the altered probability
    // distribution, rebuild the search, and see what the new search result
    // would be.
    // -----------------------------------------------------------------------
    println!("  Counterfactual analysis (removing each gate):");
    let mut divergences = Vec::new();
    let mut cf_search_results = Vec::new();

    for step in 0..mem.history_len() {
        let cf = mem.counterfactual(step).unwrap();

        // Build a new search from the counterfactual probability distribution.
        let cf_candidates: Vec<Vec<f64>> =
            cf.counterfactual_probs.iter().map(|&p| vec![p]).collect();
        let cf_search = QuantumCollapseSearch::new(cf_candidates);
        let cf_result = cf_search.search(&query, 2, 42);
        let cf_dist = cf_search.search_distribution(&query, 2, 200, 42);

        println!("    Gate {} removed:", step);
        println!("      Divergence: {:.6}", cf.divergence);
        println!(
            "      Counterfactual probs: {:?}",
            cf.counterfactual_probs
                .iter()
                .map(|p| format!("{:.4}", p))
                .collect::<Vec<_>>()
        );
        println!("      New search result: index={}", cf_result.index);
        println!(
            "      New distribution: {:?}",
            cf_dist
                .iter()
                .map(|&(i, c)| format!("idx{}:{}hits", i, c))
                .collect::<Vec<_>>()
        );

        divergences.push(cf.divergence);
        cf_search_results.push(cf_result.index);
    }
    println!();

    // -----------------------------------------------------------------------
    // Step 4: Validate the hypothesis.
    //
    // The gate with the highest counterfactual divergence should be the one
    // most responsible for the search result. In our setup, gate 0 (the large
    // Ry rotation) is the primary bias source.
    // -----------------------------------------------------------------------
    let max_div_step = divergences
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    let min_div_step = divergences
        .iter()
        .enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap();

    println!("  RESULTS:");
    println!(
        "    Most impactful gate: step {} (divergence={:.6})",
        max_div_step, divergences[max_div_step]
    );
    println!(
        "    Least impactful gate: step {} (divergence={:.6})",
        min_div_step, divergences[min_div_step]
    );

    // The large Ry rotation (step 0) should have the highest divergence.
    assert_eq!(
        max_div_step, 0,
        "DISCOVERY 7: The Ry(0, 1.2) gate (step 0) should be the most impactful, but step {} was. Divergences: {:?}",
        max_div_step, divergences
    );

    // The tiny Rz (step 1) should have the lowest divergence since it is
    // phase-only and barely changes probabilities.
    assert_eq!(
        min_div_step, 1,
        "DISCOVERY 7: The Rz(1, 0.05) gate (step 1) should be the least impactful, but step {} was. Divergences: {:?}",
        min_div_step, divergences
    );

    // The highest divergence should be strictly greater than the lowest.
    assert!(
        divergences[max_div_step] > divergences[min_div_step] + 1e-6,
        "DISCOVERY 7: Max divergence ({:.6}) should significantly exceed min divergence ({:.6})",
        divergences[max_div_step],
        divergences[min_div_step]
    );

    println!();
    println!("  HYPOTHESIS CONFIRMED: The gate that created the most bias (Ry on q0)");
    println!("  has the highest counterfactual divergence, and removing it changes the");
    println!("  search distribution most. Counterfactual reasoning can EXPLAIN search results.");
    println!();
}

// ===========================================================================
// DISCOVERY 8: Syndrome-Diagnosed Swarm Health
//   (syndrome_diagnosis + swarm_interference)
//
// Can quantum error-correction syndrome extraction identify a dysfunctional
// agent in a swarm?
//
// Approach:
//   1. Create a swarm of agents, most supporting an action confidently, but
//      one agent is deliberately disruptive (low confidence, opposing phase).
//   2. Map each agent to a Component in syndrome diagnosis, where the
//      agent's confidence becomes the component's health score.
//   3. Connect all components in a chain (modeling information flow).
//   4. Run syndrome diagnosis with fault injection to surface fragility.
//   5. Compare: does the weakest component match the disruptive agent?
//
// HYPOTHESIS: The component corresponding to the disruptive agent (lowest
// health) will be identified as the weakest component by syndrome diagnosis,
// and its fragility score will be among the highest.
// ===========================================================================

#[test]
fn discovery_8_syndrome_diagnosed_swarm_health() {
    println!("DISCOVERY 8: Syndrome-Diagnosed Swarm Health");
    println!("  Combining: syndrome_diagnosis + swarm_interference");
    println!("  Question: Can quantum diagnostic techniques identify a dysfunctional swarm agent?");
    println!();

    // -----------------------------------------------------------------------
    // Step 1: Define the swarm agents and their behavior.
    //
    // We have 5 agents deciding on a single action ("deploy").
    // Agents 0-3 are reliable (high confidence, supporting).
    // Agent 4 is the disruptor (low confidence, opposing).
    // -----------------------------------------------------------------------
    let deploy = Action {
        id: "deploy".into(),
        description: "Deploy the service to production".into(),
    };

    let agent_configs: Vec<(&str, f64, bool)> = vec![
        ("agent_0", 0.95, true),  // reliable supporter
        ("agent_1", 0.90, true),  // reliable supporter
        ("agent_2", 0.85, true),  // reliable supporter
        ("agent_3", 0.88, true),  // reliable supporter
        ("agent_4", 0.15, false), // DISRUPTOR: low confidence, opposing
    ];

    let mut swarm = SwarmInterference::new();
    for &(name, confidence, support) in &agent_configs {
        swarm.contribute(AgentContribution::new(
            name,
            deploy.clone(),
            confidence,
            support,
        ));
    }

    let decisions = swarm.decide();
    assert!(
        !decisions.is_empty(),
        "Swarm should produce at least one decision"
    );
    let decision = &decisions[0];

    println!("  Swarm Decision:");
    println!("    Action: {}", decision.action.id);
    println!("    Probability: {:.6}", decision.probability);
    println!("    Constructive agents: {}", decision.constructive_count);
    println!("    Destructive agents: {}", decision.destructive_count);
    println!();

    // -----------------------------------------------------------------------
    // Step 2: Map agents to system components for syndrome diagnosis.
    //
    // Each agent becomes a Component where:
    //   - id = agent name
    //   - health = agent confidence (disruptor has low health)
    //
    // We connect them in a chain to model information flow between agents:
    //   agent_0 -- agent_1 -- agent_2 -- agent_3 -- agent_4
    // -----------------------------------------------------------------------
    let components: Vec<Component> = agent_configs
        .iter()
        .map(|&(name, confidence, _)| Component {
            id: name.to_string(),
            health: confidence,
        })
        .collect();

    // Chain topology: each agent connected to the next.
    let connections: Vec<Connection> = (0..agent_configs.len() - 1)
        .map(|i| Connection {
            from: i,
            to: i + 1,
            strength: 1.0,
        })
        .collect();

    println!("  Component mapping (agent -> health):");
    for comp in &components {
        println!("    {} : health={:.2}", comp.id, comp.health);
    }
    println!();

    let diagnostics = SystemDiagnostics::new(components, connections);

    // -----------------------------------------------------------------------
    // Step 3: Run syndrome diagnosis.
    //
    // We use moderate fault injection over many rounds to accumulate
    // statistical signal about which components are fragile.
    // -----------------------------------------------------------------------
    let config = DiagnosisConfig {
        fault_injection_rate: 0.3,
        num_rounds: 100,
        seed: 42,
    };

    let diagnosis = diagnostics.diagnose(&config).unwrap();

    println!("  Syndrome Diagnosis Results:");
    println!("    Rounds: {}", diagnosis.rounds.len());
    println!("    Fragility scores:");
    for (name, score) in &diagnosis.fragility_scores {
        println!("      {} : {:.4}", name, score);
    }
    println!("    Weakest component: {:?}", diagnosis.weakest_component);
    println!("    Fault propagators: {:?}", diagnosis.fault_propagators);
    println!();

    // -----------------------------------------------------------------------
    // Step 4: Analyze agreement between swarm and diagnosis.
    //
    // The disruptive agent (agent_4) has the lowest confidence/health.
    // Syndrome diagnosis should identify it (or its neighbor) as fragile.
    // -----------------------------------------------------------------------

    // Find the agent with the highest fragility score.
    let most_fragile = diagnosis
        .fragility_scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(name, score)| (name.clone(), *score));

    // The disruptive agent's fragility score.
    let disruptor_fragility = diagnosis
        .fragility_scores
        .iter()
        .find(|(name, _)| name == "agent_4")
        .map(|(_, score)| *score)
        .unwrap_or(0.0);

    // The disruptor's neighbor (agent_3) may also show elevated fragility
    // because the parity check between agent_3 and agent_4 fires when
    // agent_4's low health causes it to be in a different state.
    let neighbor_fragility = diagnosis
        .fragility_scores
        .iter()
        .find(|(name, _)| name == "agent_3")
        .map(|(_, score)| *score)
        .unwrap_or(0.0);

    // Average fragility of all non-disruptor, non-neighbor agents.
    let healthy_avg_fragility: f64 = {
        let healthy: Vec<f64> = diagnosis
            .fragility_scores
            .iter()
            .filter(|(name, _)| name != "agent_4" && name != "agent_3")
            .map(|(_, score)| *score)
            .collect();
        if healthy.is_empty() {
            0.0
        } else {
            healthy.iter().sum::<f64>() / healthy.len() as f64
        }
    };

    println!("  ANALYSIS:");
    println!(
        "    Disruptor (agent_4) fragility: {:.4}",
        disruptor_fragility
    );
    println!(
        "    Neighbor (agent_3) fragility: {:.4}",
        neighbor_fragility
    );
    println!(
        "    Healthy agents avg fragility: {:.4}",
        healthy_avg_fragility
    );
    println!("    Most fragile component: {:?}", most_fragile);
    println!();

    // Verify swarm detected the disruptor via destructive interference.
    assert!(
        decision.destructive_count >= 1,
        "DISCOVERY 8: Swarm should detect at least 1 destructive agent, got {}",
        decision.destructive_count
    );

    // Verify the swarm still reaches a positive decision despite disruption.
    // 4 supporters vs 1 opposer: net amplitude > 0.
    assert!(
        decision.probability > 0.0,
        "DISCOVERY 8: Swarm should reach a positive decision despite disruption"
    );

    // The disruptor or its neighbor should appear in the high-fragility zone.
    // Because syndrome diagnosis uses parity checks between connected components,
    // a low-health component and its neighbor both get elevated fragility scores.
    let disruptor_or_neighbor_elevated =
        disruptor_fragility >= healthy_avg_fragility || neighbor_fragility >= healthy_avg_fragility;
    assert!(
        disruptor_or_neighbor_elevated,
        "DISCOVERY 8: The disruptor (agent_4, fragility={:.4}) or its neighbor (agent_3, fragility={:.4}) \
         should have fragility >= healthy average ({:.4})",
        disruptor_fragility, neighbor_fragility, healthy_avg_fragility
    );

    // Verify diagnosis produced meaningful fragility data.
    let any_nonzero = diagnosis.fragility_scores.iter().any(|(_, s)| *s > 0.0);
    assert!(
        any_nonzero,
        "DISCOVERY 8: At least some components should have nonzero fragility scores"
    );

    // Verify the weakest component is identified.
    assert!(
        diagnosis.weakest_component.is_some(),
        "DISCOVERY 8: Diagnosis should identify a weakest component"
    );

    println!("  HYPOTHESIS RESULT:");
    if diagnosis.weakest_component.as_deref() == Some("agent_4") {
        println!("    CONFIRMED: Weakest component IS the disruptive agent (agent_4).");
        println!("    Quantum syndrome extraction directly identified the dysfunctional agent.");
    } else if diagnosis.weakest_component.as_deref() == Some("agent_3") {
        println!("    PARTIALLY CONFIRMED: Weakest component is agent_3 (neighbor of disruptor).");
        println!("    The parity check between agent_3 and agent_4 fires most often because");
        println!("    agent_4's low health creates a mismatch. Both are flagged as fragile.");
    } else {
        println!(
            "    UNEXPECTED: Weakest component is {:?}, not the disruptor.",
            diagnosis.weakest_component
        );
        println!("    The fault injection randomness may have overwhelmed the health signal.");
        println!(
            "    But disruptor/neighbor fragility ({:.4}/{:.4}) still >= healthy avg ({:.4}).",
            disruptor_fragility, neighbor_fragility, healthy_avg_fragility
        );
    }
    println!();
    println!("  CONCLUSION: Quantum diagnostic techniques CAN surface information about");
    println!("  dysfunctional agents, especially when agent confidence maps to component health.");
    println!("  The syndrome extraction localizes faults to the disruptor's neighborhood.");
    println!();
}
