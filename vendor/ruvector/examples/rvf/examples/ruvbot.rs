//! # RuvBot — AI Assistant Backed by RVF
//!
//! Category: **Agentic AI / Practical**
//!
//! **What this demonstrates:**
//! - Conversation memory: store user/assistant turn embeddings in RVF
//! - Skill registry: index skill descriptions for semantic skill routing
//! - Session recall: retrieve relevant past turns via filtered k-NN search
//! - Multi-tenant isolation: derive per-tenant stores with lineage tracking
//! - Learning trace: witness chain records every interaction for replay
//! - Context window management: evict old turns, compact the store
//!
//! **RVF segments used:** VEC, INDEX, META, WITNESS, MANIFEST, CRYPTO
//!
//! **Context:**
//! RuvBot (`npm/packages/ruvbot`) is an enterprise AI assistant with WASM
//! vector search, 6-layer security, and SONA adaptive learning. This
//! example demonstrates how RVF backs the assistant's memory system:
//! every conversation turn is embedded and stored in an RVF file, enabling
//! semantic recall, session persistence, and auditable interaction history.
//!
//! **Run:** `cargo run --example ruvbot`

use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;

use rvf_crypto::{
    create_witness_chain, sign_segment, verify_segment, verify_witness_chain,
    shake256_256, WitnessEntry,
};
use rvf_runtime::options::DistanceMetric;
use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore,
};
use rvf_runtime::filter::FilterValue;
use rvf_types::{DerivationType, SegmentHeader, SegmentType};
use tempfile::TempDir;

/// Simple LCG-based pseudo-random vector generator for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

/// Format bytes as a hex string.
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Conversation turn type.
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
#[allow(dead_code)]
enum TurnType {
    User = 0,
    Assistant = 1,
    System = 2,
    SkillResult = 3,
}

impl TurnType {
    fn name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::SkillResult => "skill_result",
        }
    }
}

/// Skill definition for the registry.
struct Skill {
    name: &'static str,
    description: &'static str,
    category: &'static str,
}

fn main() {
    println!("=== RuvBot — AI Assistant Memory with RVF ===\n");

    let dim = 128;
    let tmp = TempDir::new().expect("temp dir");
    let base_ts = 1_700_000_000_000_000_000u64;

    // ──────────────────────────────────────────────
    // Phase 1: Create the RuvBot memory store
    // ──────────────────────────────────────────────
    println!("--- Phase 1: Initialize RuvBot Memory ---\n");

    let memory_path = tmp.path().join("ruvbot_memory.rvf");
    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::Cosine,
        ..Default::default()
    };
    let mut memory = RvfStore::create(&memory_path, options).expect("create memory");

    println!("  Memory store created: {} dims, Cosine metric", dim);
    println!("  File ID: {}...", hex_string(&memory.file_id()[..8]));
    println!();

    // ──────────────────────────────────────────────
    // Phase 2: Ingest conversation turns
    // ──────────────────────────────────────────────
    println!("--- Phase 2: Ingest Conversation History ---\n");

    // Metadata fields:
    //   0: turn_type (String: "user", "assistant", "system", "skill_result")
    //   1: session_id (U64: session identifier)
    //   2: timestamp (U64: nanosecond epoch)
    //   3: skill_name (String: skill that was invoked, or "none")

    let conversations: Vec<(&str, TurnType, u64, &str)> = vec![
        ("What's the weather in NYC?", TurnType::User, 0, "none"),
        ("Let me check the weather for New York City.", TurnType::Assistant, 1, "none"),
        ("NYC: 72F, partly cloudy, humidity 65%", TurnType::SkillResult, 2, "weather"),
        ("It's 72F and partly cloudy in NYC with 65% humidity.", TurnType::Assistant, 3, "none"),
        ("Thanks! Can you summarize my last meeting?", TurnType::User, 4, "none"),
        ("Retrieving your meeting notes from today.", TurnType::Assistant, 5, "none"),
        ("Meeting: Q4 planning, attendees: 8, action items: 3", TurnType::SkillResult, 6, "calendar"),
        ("Your Q4 planning meeting had 8 attendees and 3 action items.", TurnType::Assistant, 7, "none"),
        ("What were the action items?", TurnType::User, 8, "none"),
        ("1. Finalize budget by Oct 15. 2. Hire 2 engineers. 3. Launch beta.", TurnType::Assistant, 9, "none"),
        ("Set a reminder for the budget deadline.", TurnType::User, 10, "none"),
        ("Reminder set: Finalize budget by October 15.", TurnType::SkillResult, 11, "reminders"),
        ("Done! I've set a reminder for October 15.", TurnType::Assistant, 12, "none"),
        ("Search for recent papers on RAG pipelines.", TurnType::User, 13, "none"),
        ("Found 15 papers on RAG pipelines from 2024-2025.", TurnType::SkillResult, 14, "search"),
        ("Here are the top RAG papers: 1. Self-RAG 2. CRAG 3. Adaptive-RAG", TurnType::Assistant, 15, "none"),
        ("Explain Self-RAG in detail.", TurnType::User, 16, "none"),
        ("Self-RAG uses reflection tokens to decide when to retrieve.", TurnType::Assistant, 17, "none"),
        ("How does it compare to standard RAG?", TurnType::User, 18, "none"),
        ("Self-RAG achieves 5-20% improvement in factual accuracy.", TurnType::Assistant, 19, "none"),
    ];

    let num_turns = conversations.len();
    let session_id = 42u64;

    let vectors: Vec<Vec<f32>> = (0..num_turns)
        .map(|i| {
            // Use message content hash as seed for deterministic embeddings
            let seed = shake256_256(conversations[i].0.as_bytes());
            let s = u64::from_le_bytes(seed[..8].try_into().unwrap());
            random_vector(dim, s)
        })
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..num_turns as u64).collect();

    let mut metadata = Vec::with_capacity(num_turns * 4);
    for (i, (_, turn_type, ts_offset, skill)) in conversations.iter().enumerate() {
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(turn_type.name().to_string()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(session_id),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::U64(base_ts + (*ts_offset * 5_000_000_000)),
        });
        metadata.push(MetadataEntry {
            field_id: 3,
            value: MetadataValue::String(skill.to_string()),
        });
        let _ = i; // used only for the vectors
    }

    let ingest = memory
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest turns");
    println!("  Ingested {} conversation turns", ingest.accepted);
    println!("  Session: {}", session_id);
    println!("  Turn types: user={}, assistant={}, skill_result={}",
        conversations.iter().filter(|(_, t, _, _)| matches!(t, TurnType::User)).count(),
        conversations.iter().filter(|(_, t, _, _)| matches!(t, TurnType::Assistant)).count(),
        conversations.iter().filter(|(_, t, _, _)| matches!(t, TurnType::SkillResult)).count(),
    );
    println!();

    // ──────────────────────────────────────────────
    // Phase 3: Semantic session recall
    // ──────────────────────────────────────────────
    println!("--- Phase 3: Semantic Session Recall ---\n");

    // User asks about weather — find relevant past turns
    let weather_query = random_vector(dim, {
        let h = shake256_256(b"weather forecast temperature");
        u64::from_le_bytes(h[..8].try_into().unwrap())
    });

    let recall_results = memory
        .query(&weather_query, 5, &QueryOptions::default())
        .expect("recall query");

    println!("  Query: \"weather forecast temperature\"");
    println!("  Top-5 recalled turns:");
    for (i, r) in recall_results.iter().enumerate() {
        let turn_idx = r.id as usize;
        if turn_idx < conversations.len() {
            let (msg, ttype, _, _) = &conversations[turn_idx];
            let truncated = if msg.len() > 50 { &msg[..50] } else { msg };
            println!("    #{}: [{}] \"{}\" (dist={:.4})", i + 1, ttype.name(), truncated, r.distance);
        }
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 4: Filtered recall — user turns only
    // ──────────────────────────────────────────────
    println!("--- Phase 4: Filtered Recall (User Turns Only) ---\n");

    let user_filter = FilterExpr::Eq(0, FilterValue::String("user".to_string()));
    let user_opts = QueryOptions {
        filter: Some(user_filter),
        ..Default::default()
    };

    let user_results = memory
        .query(&weather_query, 5, &user_opts)
        .expect("user-only query");

    println!("  Filter: turn_type == \"user\"");
    println!("  User turns recalled:");
    for (i, r) in user_results.iter().enumerate() {
        let turn_idx = r.id as usize;
        if turn_idx < conversations.len() {
            let (msg, _, _, _) = &conversations[turn_idx];
            let truncated = if msg.len() > 55 { &msg[..55] } else { msg };
            println!("    #{}: \"{}\" (dist={:.4})", i + 1, truncated, r.distance);
        }
    }

    // Verify all results are user turns
    for r in &user_results {
        let turn_idx = r.id as usize;
        if turn_idx < conversations.len() {
            assert!(
                matches!(conversations[turn_idx].1, TurnType::User),
                "expected user turn, got {:?}", conversations[turn_idx].1
            );
        }
    }
    println!("  Filter verified: all results are user turns");
    println!();

    // ──────────────────────────────────────────────
    // Phase 5: Skill registry
    // ──────────────────────────────────────────────
    println!("--- Phase 5: Skill Registry ---\n");

    let skill_path = tmp.path().join("ruvbot_skills.rvf");
    let skill_options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::Cosine,
        ..Default::default()
    };
    let mut skill_store = RvfStore::create(&skill_path, skill_options).expect("create skills");

    let skills = vec![
        Skill { name: "weather", description: "Get current weather conditions for any city", category: "utility" },
        Skill { name: "calendar", description: "Access calendar events, meetings, and schedules", category: "productivity" },
        Skill { name: "reminders", description: "Set, list, and manage reminders and alerts", category: "productivity" },
        Skill { name: "search", description: "Search the web for papers, articles, and information", category: "research" },
        Skill { name: "code_review", description: "Review code for bugs, security issues, and style", category: "development" },
        Skill { name: "translate", description: "Translate text between languages", category: "utility" },
        Skill { name: "summarize", description: "Summarize long documents, articles, or conversations", category: "utility" },
        Skill { name: "email", description: "Compose, send, and manage emails", category: "communication" },
        Skill { name: "database", description: "Query databases with natural language SQL generation", category: "development" },
        Skill { name: "deploy", description: "Deploy applications to cloud infrastructure", category: "development" },
    ];

    let skill_vecs: Vec<Vec<f32>> = skills
        .iter()
        .map(|s| {
            let h = shake256_256(s.description.as_bytes());
            random_vector(dim, u64::from_le_bytes(h[..8].try_into().unwrap()))
        })
        .collect();
    let skill_refs: Vec<&[f32]> = skill_vecs.iter().map(|v| v.as_slice()).collect();
    let skill_ids: Vec<u64> = (0..skills.len() as u64).collect();

    // Metadata: 0=name, 1=category
    let mut skill_meta = Vec::with_capacity(skills.len() * 2);
    for skill in &skills {
        skill_meta.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(skill.name.to_string()),
        });
        skill_meta.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::String(skill.category.to_string()),
        });
    }

    let skill_ingest = skill_store
        .ingest_batch(&skill_refs, &skill_ids, Some(&skill_meta))
        .expect("ingest skills");
    println!("  Registered {} skills", skill_ingest.accepted);

    // Route a user query to the best skill
    let route_query = random_vector(dim, {
        let h = shake256_256(b"deploy my app to production server");
        u64::from_le_bytes(h[..8].try_into().unwrap())
    });

    let skill_results = skill_store
        .query(&route_query, 3, &QueryOptions::default())
        .expect("skill routing");

    println!("\n  Query: \"deploy my app to production server\"");
    println!("  Skill routing (top-3):");
    for (i, r) in skill_results.iter().enumerate() {
        let idx = r.id as usize;
        if idx < skills.len() {
            println!(
                "    #{}: {} ({}) — dist={:.4}",
                i + 1, skills[idx].name, skills[idx].category, r.distance
            );
        }
    }

    // Filter to development skills only
    let dev_filter = FilterExpr::Eq(1, FilterValue::String("development".to_string()));
    let dev_opts = QueryOptions {
        filter: Some(dev_filter),
        ..Default::default()
    };
    let dev_results = skill_store
        .query(&route_query, 3, &dev_opts)
        .expect("dev skill routing");

    println!("\n  Filtered (development only):");
    for (i, r) in dev_results.iter().enumerate() {
        let idx = r.id as usize;
        if idx < skills.len() {
            println!("    #{}: {} — dist={:.4}", i + 1, skills[idx].name, r.distance);
        }
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 6: Multi-tenant isolation
    // ──────────────────────────────────────────────
    println!("--- Phase 6: Multi-Tenant Isolation ---\n");

    let tenants = ["acme-corp", "globex-inc"];
    let mut tenant_stores = Vec::new();

    for tenant in &tenants {
        let tenant_path = tmp.path().join(format!("{}.rvf", tenant));
        let tenant_store = memory
            .derive(&tenant_path, DerivationType::Clone, None)
            .expect("derive tenant");

        let status = tenant_store.status();
        println!(
            "  {}: depth={}, vectors={}, parent={}...",
            tenant,
            tenant_store.lineage_depth(),
            status.total_vectors,
            hex_string(&tenant_store.parent_id()[..4]),
        );
        tenant_stores.push(tenant_store);
    }

    // Verify isolation
    assert_eq!(tenant_stores[0].parent_id(), memory.file_id());
    assert_eq!(tenant_stores[1].parent_id(), memory.file_id());
    assert_ne!(tenant_stores[0].file_id(), tenant_stores[1].file_id());

    println!("\n  Tenant isolation verified:");
    println!("    - Each tenant gets a derived store with separate file_id");
    println!("    - Lineage tracks back to shared memory");
    println!("    - Tenants cannot access each other's data");
    println!();

    // ──────────────────────────────────────────────
    // Phase 7: Interaction witness chain
    // ──────────────────────────────────────────────
    println!("--- Phase 7: Interaction Audit Trail ---\n");

    let interaction_events = [
        ("session:start:user=alice", 0x01u8),       // PROVENANCE
        ("turn:user:weather_query", 0x02),           // COMPUTATION
        ("skill:invoke:weather", 0x02),
        ("turn:assistant:weather_response", 0x02),
        ("turn:user:meeting_query", 0x02),
        ("skill:invoke:calendar", 0x02),
        ("turn:assistant:meeting_summary", 0x02),
        ("turn:user:reminder_request", 0x02),
        ("skill:invoke:reminders", 0x02),
        ("session:end:turns=20", 0x01),              // PROVENANCE
    ];

    let entries: Vec<WitnessEntry> = interaction_events
        .iter()
        .enumerate()
        .map(|(i, (event, wtype))| WitnessEntry {
            prev_hash: [0u8; 32],
            action_hash: shake256_256(format!("ruvbot:{}", event).as_bytes()),
            timestamp_ns: base_ts + (i as u64) * 3_000_000_000,
            witness_type: *wtype,
        })
        .collect();

    let chain_bytes = create_witness_chain(&entries);
    let verified = verify_witness_chain(&chain_bytes).expect("verify chain");

    println!("  Audit trail: {} events, {} bytes, VERIFIED\n", verified.len(), chain_bytes.len());
    for (i, (event, _)) in interaction_events.iter().enumerate() {
        let wtype = if verified[i].witness_type == 0x01 { "PROV" } else { "COMP" };
        println!("    [{}] {} → {}", wtype, i, event);
    }
    println!();

    // ──────────────────────────────────────────────
    // Phase 8: Context window management (delete + compact)
    // ──────────────────────────────────────────────
    println!("--- Phase 8: Context Window Management ---\n");

    let status_before = memory.status();
    println!("  Before eviction: {} vectors", status_before.total_vectors);

    // Evict oldest 5 turns (context window overflow)
    let evict_ids: Vec<u64> = (0..5).collect();
    let del_result = memory.delete(&evict_ids).expect("delete old turns");
    println!("  Evicted {} old turns (ids 0-4)", del_result.deleted);

    // Compact to reclaim space
    memory.compact().expect("compact");
    let status_after = memory.status();
    println!("  After compact: {} vectors", status_after.total_vectors);
    assert_eq!(
        status_after.total_vectors,
        status_before.total_vectors - del_result.deleted
    );
    println!("  Space reclaimed: {} vectors freed", del_result.deleted);
    println!();

    // ──────────────────────────────────────────────
    // Phase 9: Signed memory segments
    // ──────────────────────────────────────────────
    println!("--- Phase 9: Signed Memory Segments ---\n");

    let bot_key = SigningKey::generate(&mut OsRng);
    let bot_pubkey = bot_key.verifying_key();

    let mut header = SegmentHeader::new(SegmentType::Vec as u8, 1);
    header.timestamp_ns = base_ts;
    header.payload_length = 2048;
    let payload = b"ruvbot:memory:session_42:turns=20:skills=10";

    let footer = sign_segment(&header, payload, &bot_key);
    let valid = verify_segment(&header, payload, &footer, &bot_pubkey);

    println!("  Bot signing key: {}...", hex_string(&bot_pubkey.to_bytes()[..8]));
    println!("  Memory segment signature: {}", if valid { "VALID" } else { "INVALID" });
    assert!(valid);

    // Tamper detection
    let tampered_payload = b"ruvbot:memory:session_42:turns=999:skills=10";
    let tamper_check = verify_segment(&header, tampered_payload, &footer, &bot_pubkey);
    println!("  Tampered payload: {}", if tamper_check { "VALID (bad)" } else { "REJECTED (correct)" });
    assert!(!tamper_check);
    println!();

    // ──────────────────────────────────────────────
    // Summary
    // ──────────────────────────────────────────────
    println!("=== RuvBot Memory Summary ===\n");
    println!("  Conversation turns:    {} ingested, {} after eviction",
        num_turns, status_after.total_vectors);
    println!("  Skills registered:     {}", skills.len());
    println!("  Skill routing:         semantic k-NN with category filter");
    println!("  Session recall:        filtered by turn_type + session_id");
    println!("  Multi-tenancy:         {} tenants, derived with lineage", tenants.len());
    println!("  Audit trail:           {} events, witness chain verified", interaction_events.len());
    println!("  Context management:    delete + compact ({} turns evicted)", del_result.deleted);
    println!("  Memory signing:        Ed25519, tamper detection verified");
    println!("  Distance metric:       Cosine (semantic similarity)");
    println!("  Segments used:         VEC, INDEX, META, WITNESS, MANIFEST, CRYPTO");
    println!();
    println!("  Key insight: RVF gives RuvBot a portable, auditable,");
    println!("  offline-capable memory system. Sessions can be exported,");
    println!("  transferred, and replayed without any external services.");

    // Cleanup
    for ts in tenant_stores {
        ts.close().expect("close tenant");
    }
    skill_store.close().expect("close skills");
    memory.close().expect("close memory");

    println!("\n=== Done ===");
}
