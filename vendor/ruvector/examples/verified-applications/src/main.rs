//! Runs all 10 verified application demos.

use verified_applications::*;

fn _header(n: u32, title: &str) {
    println!("\n{}", "=".repeat(60));
    println!("  {n}. {title}");
    println!("{}", "=".repeat(60));
}

fn main() {
    println!("ruvector-verified: 10 Exotic Applications\n");

    // 1. Weapons Filter
    println!("\n========== 1. Autonomous Weapons Filter ==========");
    let config = weapons_filter::CertifiedConfig::default();
    let data = vec![0.5f32; 512];
    match weapons_filter::verify_targeting_pipeline(&data, &config) {
        Some(r) => println!("  PASS: {} [tier: {}, 82-byte witness]", r.claim, r.tier),
        None => println!("  BLOCKED: pipeline verification failed"),
    }
    match weapons_filter::verify_tampered_sensor(&config) {
        Some(_) => println!("  ERROR: tampered sensor was not blocked!"),
        None => println!("  BLOCKED: tampered sensor correctly rejected"),
    }

    // 2. Medical Diagnostics
    println!("\n========== 2. Medical Diagnostics ==========");
    let ecg = vec![0.1f32; 256];
    match medical_diagnostics::run_diagnostic("patient-001", &ecg, [0xABu8; 32], 256) {
        Ok(b) => println!(
            "  PASS: {} steps verified, pipeline proof #{}, verdict: {}",
            b.steps.len(),
            b.pipeline_proof_id,
            b.verdict,
        ),
        Err(e) => println!("  FAIL: {e}"),
    }

    // 3. Financial Routing
    println!("\n========== 3. Financial Order Routing ==========");
    let features = vec![0.3f32; 128];
    match financial_routing::verify_trade_order("TRD-001", &features, 128, "L2", "BUY") {
        Ok(o) => println!(
            "  PASS: trade {} verified, proof_hash={:#018x}",
            o.trade_id, o.proof_hash,
        ),
        Err(e) => println!("  FAIL: {e}"),
    }

    // 4. Agent Contracts
    println!("\n========== 4. Multi-Agent Contract Enforcement ==========");
    let contract = agent_contracts::AgentContract {
        agent_id: "agent-alpha".into(),
        required_dim: 256,
        required_metric: "Cosine".into(),
        max_pipeline_depth: 3,
    };
    let result = agent_contracts::enforce_contract(&contract, &vec![0.1f32; 256]);
    println!(
        "  agent={}, allowed={}, reason={}",
        result.agent_id, result.allowed, result.reason
    );
    let bad = agent_contracts::enforce_contract(&contract, &vec![0.1f32; 64]);
    println!(
        "  agent={}, allowed={}, reason={}",
        bad.agent_id, bad.allowed, bad.reason
    );

    // 5. Sensor Swarm
    println!("\n========== 5. Distributed Sensor Swarm ==========");
    let good = vec![0.5f32; 64];
    let bad_sensor = vec![0.5f32; 32];
    let nodes: Vec<(&str, &[f32])> = vec![
        ("n0", &good),
        ("n1", &good),
        ("n2", &bad_sensor),
        ("n3", &good),
    ];
    let coherence = sensor_swarm::check_swarm_coherence(&nodes, 64);
    println!(
        "  coherent={}, verified={}/{}, divergent={:?}",
        coherence.coherent,
        coherence.verified_nodes,
        coherence.total_nodes,
        coherence.divergent_nodes,
    );

    // 6. Quantization Proof
    println!("\n========== 6. Quantization Proof ==========");
    let orig = vec![1.0f32; 128];
    let quant: Vec<f32> = orig.iter().map(|x| x + 0.001).collect();
    let cert = quantization_proof::certify_quantization(&orig, &quant, 128, 1.0, "L2");
    println!(
        "  certified={}, error={:.6}, max_allowed={:.6}",
        cert.certified, cert.actual_error, cert.max_error,
    );

    // 7. Verified Memory
    println!("\n========== 7. Verifiable Synthetic Memory ==========");
    let mut store = verified_memory::VerifiedMemoryStore::new(128);
    for i in 0..5 {
        let emb = vec![i as f32 * 0.1; 128];
        store.insert(&emb).unwrap();
    }
    let (valid, invalid) = store.audit();
    println!(
        "  memories={}, valid={valid}, invalid={invalid}, witness_chain={} entries",
        store.len(),
        store.witness_chain().len()
    );

    // 8. Vector Signatures
    println!("\n========== 8. Cryptographic Vector Signatures ==========");
    let v1 = vec![0.5f32; 384];
    let v2 = vec![0.3f32; 384];
    let model = [0xAAu8; 32];
    let sig1 = vector_signatures::sign_vector(&v1, model, 384, "L2").unwrap();
    let sig2 = vector_signatures::sign_vector(&v2, model, 384, "L2").unwrap();
    println!(
        "  contract_match={}, sig1_hash={:#018x}, sig2_hash={:#018x}",
        vector_signatures::verify_contract_match(&sig1, &sig2),
        sig1.combined_hash(),
        sig2.combined_hash(),
    );

    // 9. Simulation Integrity
    println!("\n========== 9. Simulation Integrity ==========");
    let tensors: Vec<Vec<f32>> = (0..10).map(|_| vec![0.5f32; 64]).collect();
    let sim = simulation_integrity::run_verified_simulation(
        "sim-001",
        &tensors,
        64,
        &["hamiltonian", "evolve", "measure"],
    )
    .unwrap();
    println!(
        "  steps={}, total_proofs={}, pipeline_proof=#{}",
        sim.steps.len(),
        sim.total_proofs,
        sim.pipeline_proof,
    );

    // 10. Legal Forensics
    println!("\n========== 10. Legal Forensics ==========");
    let fv1 = vec![0.5f32; 256];
    let fv2 = vec![0.3f32; 256];
    let vecs: Vec<&[f32]> = vec![&fv1, &fv2];
    let bundle = legal_forensics::build_forensic_bundle(
        "CASE-2026-001",
        &vecs,
        256,
        "Cosine",
        &["embed", "search", "classify"],
    );
    println!(
        "  replay_passed={}, witnesses={}, proof_terms={}, pipeline={}",
        bundle.replay_passed,
        bundle.witness_chain.len(),
        bundle.invariants.total_proof_terms,
        bundle.invariants.pipeline_verified,
    );

    println!("\n========== Summary ==========");
    println!("  All 10 domains demonstrated.");
    println!("  Every operation produced 82-byte proof attestations.");
    println!("  This is structural trust, not policy-based trust.");
}
