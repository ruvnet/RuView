//! Performance Benchmarks for edge-net
//!
//! Comprehensive benchmarking suite for all critical operations.
//! Run with: `cargo bench --features=bench`

#![cfg(all(test, feature = "bench"))]

use test::Bencher;
use super::*;

// ============================================================================
// Credit Operations Benchmarks
// ============================================================================

#[bench]
fn bench_credit_operation(b: &mut Bencher) {
    let mut ledger = credits::WasmCreditLedger::new("bench-node".to_string()).unwrap();

    b.iter(|| {
        ledger.credit(100, "task").unwrap();
    });
}

#[bench]
fn bench_deduct_operation(b: &mut Bencher) {
    let mut ledger = credits::WasmCreditLedger::new("bench-node".to_string()).unwrap();
    ledger.credit(1_000_000, "initial").unwrap();

    b.iter(|| {
        ledger.deduct(10).unwrap();
    });
}

#[bench]
fn bench_balance_calculation(b: &mut Bencher) {
    let mut ledger = credits::WasmCreditLedger::new("bench-node".to_string()).unwrap();

    // Simulate large history
    for i in 0..1000 {
        ledger.credit(100, &format!("task-{}", i)).unwrap();
    }

    b.iter(|| {
        ledger.balance()
    });
}

#[bench]
fn bench_ledger_merge(b: &mut Bencher) {
    let mut ledger1 = credits::WasmCreditLedger::new("node-1".to_string()).unwrap();
    let mut ledger2 = credits::WasmCreditLedger::new("node-2".to_string()).unwrap();

    for i in 0..100 {
        ledger2.credit(100, &format!("task-{}", i)).unwrap();
    }

    let earned = ledger2.export_earned().unwrap();
    let spent = ledger2.export_spent().unwrap();

    b.iter(|| {
        ledger1.merge(&earned, &spent).unwrap();
    });
}

// ============================================================================
// QDAG Transaction Benchmarks
// ============================================================================

#[bench]
fn bench_qdag_transaction_creation(b: &mut Bencher) {
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use rand::rngs::OsRng;

    let mut ledger = credits::qdag::QDAGLedger::new();
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key: VerifyingKey = (&signing_key).into();
    let pubkey = verifying_key.to_bytes();

    // Create genesis
    ledger.create_genesis(1_000_000_000, &pubkey).unwrap();

    let sender_id = hex::encode(&pubkey);
    let privkey = signing_key.to_bytes();

    b.iter(|| {
        // Note: This will fail after first transaction due to PoW, but measures creation speed
        let _ = ledger.create_transaction(
            &sender_id,
            "recipient",
            1000,
            1, // Transfer
            &privkey,
            &pubkey,
        );
    });
}

#[bench]
fn bench_qdag_balance_query(b: &mut Bencher) {
    let ledger = credits::qdag::QDAGLedger::new();

    b.iter(|| {
        ledger.balance("test-node")
    });
}

#[bench]
fn bench_qdag_tip_selection(b: &mut Bencher) {
    use ed25519_dalek::{SigningKey, VerifyingKey};
    use rand::rngs::OsRng;

    let mut ledger = credits::qdag::QDAGLedger::new();
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key: VerifyingKey = (&signing_key).into();
    let pubkey = verifying_key.to_bytes();

    ledger.create_genesis(1_000_000_000, &pubkey).unwrap();

    b.iter(|| {
        ledger.tip_count()
    });
}

// ============================================================================
// Task Queue Performance Benchmarks
// ============================================================================

#[bench]
fn bench_task_creation(b: &mut Bencher) {
    let queue = tasks::WasmTaskQueue::new().unwrap();
    let identity = identity::WasmNodeIdentity::generate("bench").unwrap();
    let payload = vec![0u8; 1024]; // 1KB payload

    b.iter(|| {
        queue.create_task("vectors", &payload, 100, &identity).unwrap()
    });
}

#[bench]
fn bench_task_queue_operations(b: &mut Bencher) {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();
    let mut queue = tasks::WasmTaskQueue::new().unwrap();
    let identity = identity::WasmNodeIdentity::generate("bench").unwrap();

    b.iter(|| {
        rt.block_on(async {
            let payload = vec![0u8; 100];
            let task = queue.create_task("vectors", &payload, 100, &identity).unwrap();
            queue.submit(task).await.unwrap();
        });
    });
}

#[bench]
fn bench_parallel_task_processing(b: &mut Bencher) {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();

    b.iter(|| {
        rt.block_on(async {
            let mut queue = tasks::WasmTaskQueue::new().unwrap();
            let identity = identity::WasmNodeIdentity::generate("bench").unwrap();

            // Simulate 10 parallel tasks
            let mut handles = vec![];
            for _ in 0..10 {
                let payload = vec![0u8; 100];
                let task = queue.create_task("vectors", &payload, 100, &identity).unwrap();
                handles.push(queue.submit(task));
            }

            futures::future::join_all(handles).await;
        });
    });
}

// ============================================================================
// Security Operations Benchmarks
// ============================================================================

#[bench]
fn bench_qlearning_decision(b: &mut Bencher) {
    let security = security::AdaptiveSecurity::new();

    b.iter(|| {
        security.choose_action("normal_load", "allow,block,throttle")
    });
}

#[bench]
fn bench_qlearning_update(b: &mut Bencher) {
    let mut security = security::AdaptiveSecurity::new();

    b.iter(|| {
        security.learn("normal_load", "allow", 0.8, "low_attack");
    });
}

#[bench]
fn bench_attack_pattern_matching(b: &mut Bencher) {
    let mut security = security::AdaptiveSecurity::new();

    // Record some attack patterns
    for i in 0..10 {
        let features = vec![i as f32 * 0.1, 0.5, 0.3];
        security.record_attack_pattern("ddos", &features, 0.8);
    }

    let test_features = vec![0.5, 0.5, 0.3];

    b.iter(|| {
        security.detect_attack(&test_features)
    });
}

#[bench]
fn bench_threshold_updates(b: &mut Bencher) {
    let mut security = security::AdaptiveSecurity::new();

    // Generate learning history
    for i in 0..100 {
        security.learn(
            "state",
            if i % 2 == 0 { "allow" } else { "block" },
            if i % 3 == 0 { 0.8 } else { 0.2 },
            "next_state"
        );
    }

    b.iter(|| {
        security.get_rate_limit_window();
        security.get_rate_limit_max();
        security.get_spot_check_probability();
    });
}

#[bench]
fn bench_rate_limiter(b: &mut Bencher) {
    let mut limiter = security::RateLimiter::new(60_000, 100);

    b.iter(|| {
        limiter.check_allowed("test-node")
    });
}

#[bench]
fn bench_reputation_update(b: &mut Bencher) {
    let mut reputation = security::ReputationSystem::new();

    b.iter(|| {
        reputation.record_success("test-node");
    });
}

// ============================================================================
// Network Topology Benchmarks
// ============================================================================

#[bench]
fn bench_node_registration_1k(b: &mut Bencher) {
    b.iter(|| {
        let mut topology = evolution::NetworkTopology::new();
        for i in 0..1_000 {
            topology.register_node(&format!("node-{}", i), &[0.5, 0.3, 0.2]);
        }
    });
}

#[bench]
fn bench_node_registration_10k(b: &mut Bencher) {
    b.iter(|| {
        let mut topology = evolution::NetworkTopology::new();
        for i in 0..10_000 {
            topology.register_node(&format!("node-{}", i), &[0.5, 0.3, 0.2]);
        }
    });
}

#[bench]
fn bench_optimal_peer_selection(b: &mut Bencher) {
    let mut topology = evolution::NetworkTopology::new();

    // Register nodes and create connections
    for i in 0..100 {
        topology.register_node(&format!("node-{}", i), &[0.5, 0.3, 0.2]);
    }

    for i in 0..100 {
        for j in 0..10 {
            topology.update_connection(
                &format!("node-{}", i),
                &format!("node-{}", (i + j + 1) % 100),
                0.8 + (j as f32 * 0.01)
            );
        }
    }

    b.iter(|| {
        topology.get_optimal_peers("node-0", 5)
    });
}

#[bench]
fn bench_cluster_assignment(b: &mut Bencher) {
    let mut topology = evolution::NetworkTopology::new();

    b.iter(|| {
        topology.register_node("test-node", &[0.7, 0.2, 0.1]);
    });
}

// ============================================================================
// Economic Engine Benchmarks
// ============================================================================

#[bench]
fn bench_reward_distribution(b: &mut Bencher) {
    let mut engine = evolution::EconomicEngine::new();

    b.iter(|| {
        engine.process_reward(100, 2.5)
    });
}

#[bench]
fn bench_epoch_processing(b: &mut Bencher) {
    let mut engine = evolution::EconomicEngine::new();

    // Build up some state
    for _ in 0..1000 {
        engine.process_reward(100, 1.0);
    }

    b.iter(|| {
        engine.advance_epoch()
    });
}

#[bench]
fn bench_sustainability_check(b: &mut Bencher) {
    let mut engine = evolution::EconomicEngine::new();

    // Build treasury
    for _ in 0..10000 {
        engine.process_reward(100, 1.0);
    }

    b.iter(|| {
        engine.is_self_sustaining(1000, 5000)
    });
}

// ============================================================================
// Evolution Engine Benchmarks
// ============================================================================

#[bench]
fn bench_performance_recording(b: &mut Bencher) {
    let mut engine = evolution::EvolutionEngine::new();

    b.iter(|| {
        engine.record_performance("node-1", 0.95, 75.0);
    });
}

#[bench]
fn bench_replication_check(b: &mut Bencher) {
    let mut engine = evolution::EvolutionEngine::new();

    // Record high performance
    for _ in 0..10 {
        engine.record_performance("node-1", 0.98, 90.0);
    }

    b.iter(|| {
        engine.should_replicate("node-1")
    });
}

#[bench]
fn bench_evolution_step(b: &mut Bencher) {
    let mut engine = evolution::EvolutionEngine::new();

    b.iter(|| {
        engine.evolve()
    });
}

// ============================================================================
// Optimization Engine Benchmarks
// ============================================================================

#[bench]
fn bench_routing_record(b: &mut Bencher) {
    let mut engine = evolution::OptimizationEngine::new();

    b.iter(|| {
        engine.record_routing("vectors", "node-1", 150, true);
    });
}

#[bench]
fn bench_optimal_node_selection(b: &mut Bencher) {
    let mut engine = evolution::OptimizationEngine::new();

    // Build routing history
    for i in 0..100 {
        engine.record_routing("vectors", &format!("node-{}", i % 10), 100 + i, i % 3 == 0);
    }

    let candidates: Vec<String> = (0..10).map(|i| format!("node-{}", i)).collect();

    b.iter(|| {
        engine.select_optimal_node("vectors", candidates.clone())
    });
}

// ============================================================================
// Network Manager Benchmarks
// ============================================================================

#[bench]
fn bench_peer_registration(b: &mut Bencher) {
    let mut manager = network::WasmNetworkManager::new("bench-node");

    b.iter(|| {
        manager.register_peer(
            "peer-1",
            &[1, 2, 3, 4],
            vec!["vectors".to_string()],
            1000
        );
    });
}

#[bench]
fn bench_worker_selection(b: &mut Bencher) {
    let mut manager = network::WasmNetworkManager::new("bench-node");

    // Register 100 peers
    for i in 0..100 {
        manager.register_peer(
            &format!("peer-{}", i),
            &[1, 2, 3, 4],
            vec!["vectors".to_string()],
            1000
        );
        manager.update_reputation(&format!("peer-{}", i), (i as f32) * 0.005);
    }

    b.iter(|| {
        manager.select_workers("vectors", 5)
    });
}

// ============================================================================
// End-to-End Benchmarks
// ============================================================================

#[bench]
fn bench_full_task_lifecycle(b: &mut Bencher) {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();

    b.iter(|| {
        rt.block_on(async {
            let identity = identity::WasmNodeIdentity::generate("bench").unwrap();
            let mut ledger = credits::WasmCreditLedger::new(identity.node_id()).unwrap();
            let mut queue = tasks::WasmTaskQueue::new().unwrap();
            let executor = tasks::WasmTaskExecutor::new(1024 * 1024).unwrap();

            // Initial credits
            ledger.credit(1000, "initial").unwrap();

            // Create and submit task
            let payload = vec![0u8; 256];
            let task = queue.create_task("vectors", &payload, 100, &identity).unwrap();
            queue.submit(task).await.unwrap();

            // Claim and complete (simulated)
            if let Some(claimed_task) = queue.claim_next(&identity).await.unwrap() {
                // Simulated execution
                ledger.credit(10, &format!("task:{}", claimed_task.id)).unwrap();
            }
        });
    });
}

#[bench]
fn bench_network_coordination(b: &mut Bencher) {
    let mut manager = network::WasmNetworkManager::new("coordinator");
    let mut topology = evolution::NetworkTopology::new();
    let mut optimizer = evolution::OptimizationEngine::new();

    // Setup network
    for i in 0..50 {
        let node_id = format!("node-{}", i);
        manager.register_peer(&node_id, &[1, 2, 3, 4], vec!["vectors".to_string()], 1000);
        topology.register_node(&node_id, &[0.5, 0.3, 0.2]);
    }

    b.iter(|| {
        // Select workers
        let workers = manager.select_workers("vectors", 3);

        // Get optimal peers
        for worker in &workers {
            topology.get_optimal_peers(worker, 5);
        }

        // Record routing
        if let Some(worker) = workers.first() {
            optimizer.record_routing("vectors", worker, 120, true);
        }
    });
}

// ============================================================================
// Spike-Driven Attention Benchmarks
// ============================================================================

#[bench]
fn bench_spike_encoding_small(b: &mut Bencher) {
    let attn = learning::SpikeDrivenAttention::new();
    let values: Vec<i8> = (0..64).map(|i| (i % 128) as i8).collect();

    b.iter(|| {
        attn.encode_spikes(&values)
    });
}

#[bench]
fn bench_spike_encoding_medium(b: &mut Bencher) {
    let attn = learning::SpikeDrivenAttention::new();
    let values: Vec<i8> = (0..256).map(|i| (i % 128) as i8).collect();

    b.iter(|| {
        attn.encode_spikes(&values)
    });
}

#[bench]
fn bench_spike_encoding_large(b: &mut Bencher) {
    let attn = learning::SpikeDrivenAttention::new();
    let values: Vec<i8> = (0..1024).map(|i| (i % 128) as i8).collect();

    b.iter(|| {
        attn.encode_spikes(&values)
    });
}

#[bench]
fn bench_spike_attention_seq16_dim64(b: &mut Bencher) {
    let attn = learning::SpikeDrivenAttention::new();
    let values: Vec<i8> = (0..64).map(|i| (i % 128 - 64) as i8).collect();
    let spikes = attn.encode_spikes(&values);

    b.iter(|| {
        attn.attention(&spikes[0..16], &spikes[0..16], &spikes[0..64])
    });
}

#[bench]
fn bench_spike_attention_seq64_dim128(b: &mut Bencher) {
    let attn = learning::SpikeDrivenAttention::new();
    let values: Vec<i8> = (0..128).map(|i| (i % 128 - 64) as i8).collect();
    let spikes = attn.encode_spikes(&values);

    b.iter(|| {
        attn.attention(&spikes[0..64], &spikes[0..64], &spikes[0..128])
    });
}

#[bench]
fn bench_spike_attention_seq128_dim256(b: &mut Bencher) {
    let attn = learning::SpikeDrivenAttention::new();
    let values: Vec<i8> = (0..256).map(|i| (i % 128 - 64) as i8).collect();
    let spikes = attn.encode_spikes(&values);

    b.iter(|| {
        attn.attention(&spikes[0..128], &spikes[0..128], &spikes[0..256])
    });
}

#[bench]
fn bench_spike_energy_ratio_calculation(b: &mut Bencher) {
    let attn = learning::SpikeDrivenAttention::new();

    b.iter(|| {
        attn.energy_ratio(64, 256)
    });
}

// ============================================================================
// RAC Coherence Benchmarks
// ============================================================================

#[bench]
fn bench_rac_event_ingestion(b: &mut Bencher) {
    use sha2::{Sha256, Digest};
    use rac::{Event, EventKind, AssertEvent, Ruvector, EvidenceRef};

    let mut engine = rac::CoherenceEngine::new();

    b.iter(|| {
        let proposition = b"test-proposition";
        let mut hasher = Sha256::new();
        hasher.update(proposition);
        let id_bytes = hasher.finalize();
        let mut event_id = [0u8; 32];
        event_id.copy_from_slice(&id_bytes);

        let event = Event {
            id: event_id,
            prev: None,
            ts_unix_ms: js_sys::Date::now() as u64,
            author: [0u8; 32],
            context: [0u8; 32],
            ruvector: Ruvector::new(vec![0.1, 0.2, 0.3]),
            kind: EventKind::Assert(AssertEvent {
                proposition: proposition.to_vec(),
                evidence: vec![EvidenceRef::hash(&[1, 2, 3])],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            sig: vec![0u8; 64],
        };

        engine.ingest(event);
    });
}

#[bench]
fn bench_rac_event_ingestion_1k(b: &mut Bencher) {
    use sha2::{Sha256, Digest};
    use rac::{Event, EventKind, AssertEvent, Ruvector, EvidenceRef};

    b.iter(|| {
        let mut engine = rac::CoherenceEngine::new();

        for i in 0..1000 {
            let proposition = format!("test-proposition-{}", i);
            let mut hasher = Sha256::new();
            hasher.update(proposition.as_bytes());
            let id_bytes = hasher.finalize();
            let mut event_id = [0u8; 32];
            event_id.copy_from_slice(&id_bytes);

            let event = Event {
                id: event_id,
                prev: None,
                ts_unix_ms: js_sys::Date::now() as u64,
                author: [0u8; 32],
                context: [0u8; 32],
                ruvector: Ruvector::new(vec![0.1, 0.2, 0.3]),
                kind: EventKind::Assert(AssertEvent {
                    proposition: proposition.as_bytes().to_vec(),
                    evidence: vec![EvidenceRef::hash(&[1, 2, 3])],
                    confidence: 0.9,
                    expires_at_unix_ms: None,
                }),
                sig: vec![0u8; 64],
            };

            engine.ingest(event);
        }
    });
}

#[bench]
fn bench_rac_quarantine_check(b: &mut Bencher) {
    let quarantine = rac::QuarantineManager::new();

    // Setup some quarantined claims
    for i in 0..100 {
        quarantine.set_level(&format!("claim-{}", i), i % 4);
    }

    b.iter(|| {
        quarantine.can_use("claim-50")
    });
}

#[bench]
fn bench_rac_quarantine_set_level(b: &mut Bencher) {
    let quarantine = rac::QuarantineManager::new();

    let mut counter = 0;
    b.iter(|| {
        quarantine.set_level(&format!("claim-{}", counter), counter % 4);
        counter += 1;
    });
}

#[bench]
fn bench_rac_merkle_root_update(b: &mut Bencher) {
    use sha2::{Sha256, Digest};
    use rac::{Event, EventKind, AssertEvent, Ruvector, EvidenceRef};

    let mut engine = rac::CoherenceEngine::new();

    // Pre-populate with some events
    for i in 0..100 {
        let proposition = format!("test-{}", i);
        let mut hasher = Sha256::new();
        hasher.update(proposition.as_bytes());
        let id_bytes = hasher.finalize();
        let mut event_id = [0u8; 32];
        event_id.copy_from_slice(&id_bytes);

        let event = Event {
            id: event_id,
            prev: None,
            ts_unix_ms: js_sys::Date::now() as u64,
            author: [0u8; 32],
            context: [0u8; 32],
            ruvector: Ruvector::new(vec![0.1, 0.2, 0.3]),
            kind: EventKind::Assert(AssertEvent {
                proposition: proposition.as_bytes().to_vec(),
                evidence: vec![],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            sig: vec![0u8; 64],
        };

        engine.ingest(event);
    }

    b.iter(|| {
        engine.get_merkle_root()
    });
}

#[bench]
fn bench_rac_ruvector_similarity(b: &mut Bencher) {
    let v1 = rac::Ruvector::new(vec![1.0, 0.5, 0.3, 0.2, 0.1, 0.05, 0.02, 0.01]);
    let v2 = rac::Ruvector::new(vec![0.9, 0.6, 0.25, 0.15, 0.12, 0.04, 0.03, 0.015]);

    b.iter(|| {
        v1.similarity(&v2)
    });
}

// ============================================================================
// Learning Module Benchmarks
// ============================================================================

#[bench]
fn bench_reasoning_bank_lookup_1k(b: &mut Bencher) {
    let bank = learning::ReasoningBank::new();

    // Store 1000 patterns
    for i in 0..1000 {
        let pattern = learning::LearnedPattern::new(
            vec![i as f32 * 0.01, 0.5, 0.3],
            0.8,
            100,
            0.9,
            10,
            50.0,
            Some(0.95),
        );
        let json = serde_json::to_string(&pattern).unwrap();
        bank.store(&json);
    }

    let query = vec![0.5f32, 0.5, 0.3];
    let query_json = serde_json::to_string(&query).unwrap();

    b.iter(|| {
        bank.lookup(&query_json, 10)
    });
}

#[bench]
fn bench_reasoning_bank_lookup_10k(b: &mut Bencher) {
    let bank = learning::ReasoningBank::new();

    // Store 10000 patterns
    for i in 0..10000 {
        let pattern = learning::LearnedPattern::new(
            vec![i as f32 * 0.001, 0.5, 0.3],
            0.8,
            100,
            0.9,
            10,
            50.0,
            Some(0.95),
        );
        let json = serde_json::to_string(&pattern).unwrap();
        bank.store(&json);
    }

    let query = vec![0.5f32, 0.5, 0.3];
    let query_json = serde_json::to_string(&query).unwrap();

    b.iter(|| {
        bank.lookup(&query_json, 10)
    });
}

#[bench]
fn bench_reasoning_bank_store(b: &mut Bencher) {
    let bank = learning::ReasoningBank::new();

    let mut counter = 0;
    b.iter(|| {
        let pattern = learning::LearnedPattern::new(
            vec![counter as f32 * 0.01, 0.5, 0.3],
            0.8,
            100,
            0.9,
            10,
            50.0,
            Some(0.95),
        );
        let json = serde_json::to_string(&pattern).unwrap();
        bank.store(&json);
        counter += 1;
    });
}

#[bench]
fn bench_trajectory_recording(b: &mut Bencher) {
    let tracker = learning::TrajectoryTracker::new(1000);

    let mut counter = 0;
    b.iter(|| {
        let trajectory = learning::TaskTrajectory::new(
            vec![1.0, 0.5, 0.3],
            100,
            50,
            100,
            true,
            format!("node-{}", counter),
        );
        let json = serde_json::to_string(&trajectory).unwrap();
        tracker.record(&json);
        counter += 1;
    });
}

#[bench]
fn bench_pattern_similarity_computation(b: &mut Bencher) {
    let pattern = learning::LearnedPattern::new(
        vec![1.0, 0.5, 0.3, 0.2, 0.1],
        0.8,
        100,
        0.9,
        10,
        50.0,
        Some(0.95),
    );

    let query = vec![0.9, 0.6, 0.25, 0.15, 0.12];

    b.iter(|| {
        pattern.similarity(&query)
    });
}

// ============================================================================
// Multi-Head Attention Scaling Benchmarks
// ============================================================================

#[bench]
fn bench_multi_head_attention_2heads_dim8(b: &mut Bencher) {
    let attn = learning::MultiHeadAttention::new(8, 2);
    let query = vec![1.0f32; 8];
    let key = vec![0.5f32; 8];
    let val = vec![1.0f32; 8];
    let keys: Vec<&[f32]> = vec![key.as_slice()];
    let values: Vec<&[f32]> = vec![val.as_slice()];

    b.iter(|| {
        attn.compute(&query, &keys, &values)
    });
}

#[bench]
fn bench_multi_head_attention_4heads_dim64(b: &mut Bencher) {
    let attn = learning::MultiHeadAttention::new(64, 4);
    let query = vec![1.0f32; 64];
    let key = vec![0.5f32; 64];
    let val = vec![1.0f32; 64];
    let keys: Vec<&[f32]> = vec![key.as_slice()];
    let values: Vec<&[f32]> = vec![val.as_slice()];

    b.iter(|| {
        attn.compute(&query, &keys, &values)
    });
}

#[bench]
fn bench_multi_head_attention_8heads_dim128(b: &mut Bencher) {
    let attn = learning::MultiHeadAttention::new(128, 8);
    let query = vec![1.0f32; 128];
    let key = vec![0.5f32; 128];
    let val = vec![1.0f32; 128];
    let keys: Vec<&[f32]> = vec![key.as_slice()];
    let values: Vec<&[f32]> = vec![val.as_slice()];

    b.iter(|| {
        attn.compute(&query, &keys, &values)
    });
}

#[bench]
fn bench_multi_head_attention_8heads_dim256_10keys(b: &mut Bencher) {
    let attn = learning::MultiHeadAttention::new(256, 8);
    let query = vec![1.0f32; 256];
    let keys_data: Vec<Vec<f32>> = (0..10).map(|_| vec![0.5f32; 256]).collect();
    let values_data: Vec<Vec<f32>> = (0..10).map(|_| vec![1.0f32; 256]).collect();
    let keys: Vec<&[f32]> = keys_data.iter().map(|k| k.as_slice()).collect();
    let values: Vec<&[f32]> = values_data.iter().map(|v| v.as_slice()).collect();

    b.iter(|| {
        attn.compute(&query, &keys, &values)
    });
}

// ============================================================================
// Integration Benchmarks
// ============================================================================

#[bench]
fn bench_end_to_end_task_routing_with_learning(b: &mut Bencher) {
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();

    b.iter(|| {
        rt.block_on(async {
            let identity = identity::WasmNodeIdentity::generate("bench").unwrap();
            let learning = learning::NetworkLearning::new();
            let mut queue = tasks::WasmTaskQueue::new().unwrap();

            // Create task
            let payload = vec![0u8; 256];
            let task = queue.create_task("vectors", &payload, 100, &identity).unwrap();

            // Record trajectory
            let trajectory = learning::TaskTrajectory::new(
                vec![1.0, 0.5, 0.3],
                100,
                50,
                100,
                true,
                identity.node_id(),
            );
            let traj_json = serde_json::to_string(&trajectory).unwrap();
            learning.record_trajectory(&traj_json);

            // Lookup patterns
            let query = vec![1.0f32, 0.5, 0.3];
            let query_json = serde_json::to_string(&query).unwrap();
            learning.lookup_patterns(&query_json, 5);

            // Submit task
            queue.submit(task).await.unwrap();
        });
    });
}

#[bench]
fn bench_combined_learning_coherence_overhead(b: &mut Bencher) {
    use sha2::{Sha256, Digest};
    use rac::{Event, EventKind, AssertEvent, Ruvector, EvidenceRef};

    b.iter(|| {
        let learning = learning::NetworkLearning::new();
        let mut coherence = rac::CoherenceEngine::new();

        // Learning operations
        for i in 0..10 {
            let trajectory = learning::TaskTrajectory::new(
                vec![i as f32 * 0.1, 0.5, 0.3],
                100,
                50,
                100,
                true,
                format!("node-{}", i),
            );
            let json = serde_json::to_string(&trajectory).unwrap();
            learning.record_trajectory(&json);
        }

        // Coherence operations
        for i in 0..10 {
            let proposition = format!("test-{}", i);
            let mut hasher = Sha256::new();
            hasher.update(proposition.as_bytes());
            let id_bytes = hasher.finalize();
            let mut event_id = [0u8; 32];
            event_id.copy_from_slice(&id_bytes);

            let event = Event {
                id: event_id,
                prev: None,
                ts_unix_ms: js_sys::Date::now() as u64,
                author: [0u8; 32],
                context: [0u8; 32],
                ruvector: Ruvector::new(vec![0.1, 0.2, 0.3]),
                kind: EventKind::Assert(AssertEvent {
                    proposition: proposition.as_bytes().to_vec(),
                    evidence: vec![],
                    confidence: 0.9,
                    expires_at_unix_ms: None,
                }),
                sig: vec![0u8; 64],
            };

            coherence.ingest(event);
        }

        // Query operations
        let query = vec![0.5f32, 0.5, 0.3];
        let query_json = serde_json::to_string(&query).unwrap();
        learning.lookup_patterns(&query_json, 5);
        coherence.get_stats();
    });
}

#[bench]
fn bench_memory_usage_trajectory_1k(b: &mut Bencher) {
    b.iter(|| {
        let tracker = learning::TrajectoryTracker::new(1000);

        for i in 0..1000 {
            let trajectory = learning::TaskTrajectory::new(
                vec![i as f32 * 0.001, 0.5, 0.3],
                100,
                50,
                100,
                true,
                format!("node-{}", i),
            );
            let json = serde_json::to_string(&trajectory).unwrap();
            tracker.record(&json);
        }

        tracker.get_stats()
    });
}

#[bench]
fn bench_concurrent_learning_and_rac_ops(b: &mut Bencher) {
    use sha2::{Sha256, Digest};
    use rac::{Event, EventKind, AssertEvent, Ruvector, EvidenceRef};

    let learning = learning::NetworkLearning::new();
    let mut coherence = rac::CoherenceEngine::new();

    b.iter(|| {
        // Concurrent pattern lookup
        let query = vec![0.5f32, 0.5, 0.3];
        let query_json = serde_json::to_string(&query).unwrap();
        let _patterns = learning.lookup_patterns(&query_json, 5);

        // Concurrent quarantine check
        let _can_use = coherence.can_use_claim("claim-test");

        // Concurrent trajectory recording
        let trajectory = learning::TaskTrajectory::new(
            vec![0.5, 0.5, 0.3],
            100,
            50,
            100,
            true,
            "node-test".to_string(),
        );
        let traj_json = serde_json::to_string(&trajectory).unwrap();
        learning.record_trajectory(&traj_json);

        // Concurrent event ingestion
        let mut hasher = Sha256::new();
        hasher.update(b"concurrent-test");
        let id_bytes = hasher.finalize();
        let mut event_id = [0u8; 32];
        event_id.copy_from_slice(&id_bytes);

        let event = Event {
            id: event_id,
            prev: None,
            ts_unix_ms: js_sys::Date::now() as u64,
            author: [0u8; 32],
            context: [0u8; 32],
            ruvector: Ruvector::new(vec![0.1, 0.2, 0.3]),
            kind: EventKind::Assert(AssertEvent {
                proposition: b"concurrent-test".to_vec(),
                evidence: vec![],
                confidence: 0.9,
                expires_at_unix_ms: None,
            }),
            sig: vec![0u8; 64],
        };

        coherence.ingest(event);
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn bench_compilation_test() {
        // Ensures benchmarks compile
        assert!(true);
    }
}
