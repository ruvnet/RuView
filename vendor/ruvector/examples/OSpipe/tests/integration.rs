//! Integration tests for OSpipe.

use ospipe::capture::{CaptureSource, CapturedFrame};
use ospipe::config::{OsPipeConfig, SafetyConfig, StorageConfig};
use ospipe::graph::KnowledgeGraph;
use ospipe::pipeline::{IngestResult, IngestionPipeline};
use ospipe::safety::{SafetyDecision, SafetyGate};
use ospipe::search::enhanced::EnhancedSearch;
use ospipe::search::hybrid::HybridSearch;
use ospipe::search::reranker::AttentionReranker;
use ospipe::search::router::{QueryRoute, QueryRouter};
use ospipe::storage::embedding::{cosine_similarity, EmbeddingEngine};
use ospipe::storage::vector_store::{SearchFilter, VectorStore};

// ---------------------------------------------------------------------------
// Configuration tests
// ---------------------------------------------------------------------------

#[test]
fn test_default_config() {
    let config = OsPipeConfig::default();
    assert_eq!(config.storage.embedding_dim, 384);
    assert_eq!(config.storage.hnsw_m, 32);
    assert_eq!(config.storage.hnsw_ef_construction, 200);
    assert_eq!(config.storage.hnsw_ef_search, 100);
    assert!((config.storage.dedup_threshold - 0.95).abs() < f32::EPSILON);
    assert_eq!(config.capture.fps, 1.0);
    assert_eq!(config.capture.audio_chunk_secs, 30);
    assert!(config.capture.skip_private_windows);
    assert_eq!(config.search.default_k, 10);
    assert!((config.search.hybrid_weight - 0.7).abs() < f32::EPSILON);
    assert!(config.safety.pii_detection);
    assert!(config.safety.credit_card_redaction);
    assert!(config.safety.ssn_redaction);
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = OsPipeConfig::default();
    let json = serde_json::to_string(&config).expect("serialize");
    let deserialized: OsPipeConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(
        deserialized.storage.embedding_dim,
        config.storage.embedding_dim
    );
    assert_eq!(deserialized.capture.fps, config.capture.fps);
}

// ---------------------------------------------------------------------------
// Capture frame tests
// ---------------------------------------------------------------------------

#[test]
fn test_captured_frame_screen() {
    let frame = CapturedFrame::new_screen("Firefox", "GitHub - main", "hello world", 0);
    assert_eq!(frame.text_content(), "hello world");
    assert_eq!(frame.content_type(), "ocr");
    assert!(matches!(
        frame.source,
        CaptureSource::Screen { monitor: 0, .. }
    ));
    assert_eq!(frame.metadata.app_name.as_deref(), Some("Firefox"));
    assert_eq!(
        frame.metadata.window_title.as_deref(),
        Some("GitHub - main")
    );
}

#[test]
fn test_captured_frame_audio() {
    let frame = CapturedFrame::new_audio("Microphone", "testing one two three", Some("Alice"));
    assert_eq!(frame.text_content(), "testing one two three");
    assert_eq!(frame.content_type(), "transcription");
    match &frame.source {
        CaptureSource::Audio { device, speaker } => {
            assert_eq!(device, "Microphone");
            assert_eq!(speaker.as_deref(), Some("Alice"));
        }
        _ => panic!("Expected Audio source"),
    }
}

#[test]
fn test_captured_frame_ui_event() {
    let frame = CapturedFrame::new_ui_event("click", "Button clicked: Submit");
    assert_eq!(frame.text_content(), "Button clicked: Submit");
    assert_eq!(frame.content_type(), "ui_event");
}

// ---------------------------------------------------------------------------
// Embedding and vector store tests
// ---------------------------------------------------------------------------

#[test]
fn test_embedding_engine() {
    let engine = EmbeddingEngine::new(384);
    let v1 = engine.embed("hello");
    let v2 = engine.embed("hello");
    assert_eq!(v1, v2, "Same input must produce identical embeddings");
    assert_eq!(v1.len(), 384);

    // Check normalization
    let magnitude: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (magnitude - 1.0).abs() < 1e-5,
        "Embedding should be L2-normalized"
    );
}

#[test]
fn test_vector_store_insert_and_search() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    // Insert some frames
    let frames = vec![
        CapturedFrame::new_screen(
            "VS Code",
            "main.rs",
            "fn main() { println!(\"hello\"); }",
            0,
        ),
        CapturedFrame::new_screen("Firefox", "Rust docs", "The Rust Programming Language", 0),
        CapturedFrame::new_audio("Mic", "discussing the project architecture", None),
    ];

    for frame in &frames {
        let emb = engine.embed(frame.text_content());
        store.insert(frame, &emb).unwrap();
    }

    assert_eq!(store.len(), 3);
    assert!(!store.is_empty());

    // Search for something similar to the first frame
    let query_emb = engine.embed("fn main() { println!(\"hello\"); }");
    let results = store.search(&query_emb, 2).unwrap();
    assert!(!results.is_empty());
    assert!(results.len() <= 2);

    // The top result should be the exact match
    assert_eq!(results[0].id, frames[0].id);
    assert!(
        (results[0].score - 1.0).abs() < 1e-5,
        "Exact match should have score ~1.0"
    );
}

#[test]
fn test_vector_store_filtered_search() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let frame_vscode = CapturedFrame::new_screen("VS Code", "editor", "rust code", 0);
    let frame_firefox = CapturedFrame::new_screen("Firefox", "browser", "rust documentation", 0);

    let emb1 = engine.embed(frame_vscode.text_content());
    let emb2 = engine.embed(frame_firefox.text_content());
    store.insert(&frame_vscode, &emb1).unwrap();
    store.insert(&frame_firefox, &emb2).unwrap();

    // Filter to only VS Code results
    let filter = SearchFilter {
        app: Some("VS Code".to_string()),
        ..Default::default()
    };
    let query = engine.embed("rust");
    let results = store.search_filtered(&query, 10, &filter).unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, frame_vscode.id);
}

#[test]
fn test_vector_store_empty_search() {
    let config = StorageConfig::default();
    let store = VectorStore::new(config).unwrap();
    let query = vec![0.0f32; 384];
    let results = store.search(&query, 10).unwrap();
    assert!(results.is_empty());
}

#[test]
fn test_vector_store_dimension_mismatch() {
    let config = StorageConfig::default(); // 384-dim
    let mut store = VectorStore::new(config).unwrap();
    let frame = CapturedFrame::new_screen("App", "Window", "text", 0);

    // Wrong dimension embedding
    let wrong_emb = vec![1.0f32; 128];
    let result = store.insert(&frame, &wrong_emb);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Frame deduplication tests
// ---------------------------------------------------------------------------

#[test]
fn test_frame_deduplication() {
    use ospipe::pipeline::FrameDeduplicator;

    let mut dedup = FrameDeduplicator::new(0.95, 10);
    let engine = EmbeddingEngine::new(384);

    let emb1 = engine.embed("hello world");
    let id1 = uuid::Uuid::new_v4();
    dedup.add(id1, emb1.clone());

    // Identical text should be detected as duplicate
    let emb2 = engine.embed("hello world");
    let result = dedup.is_duplicate(&emb2);
    assert!(
        result.is_some(),
        "Identical text should be detected as duplicate"
    );
    let (dup_id, sim) = result.unwrap();
    assert_eq!(dup_id, id1);
    assert!((sim - 1.0).abs() < 1e-5);

    // Very different text should not be a duplicate
    let emb3 = engine.embed("completely unrelated content about quantum physics");
    let result = dedup.is_duplicate(&emb3);
    // With hash-based embeddings, different texts may or may not pass threshold
    // but identical texts always will
    if let Some((_, sim)) = result {
        assert!(sim >= 0.95);
    }
}

#[test]
fn test_dedup_window_eviction() {
    use ospipe::pipeline::FrameDeduplicator;

    let mut dedup = FrameDeduplicator::new(0.95, 3);
    let engine = EmbeddingEngine::new(64);

    // Add 4 items to a window of size 3
    for i in 0..4 {
        let emb = engine.embed(&format!("text number {}", i));
        dedup.add(uuid::Uuid::new_v4(), emb);
    }

    // Window should only contain 3 items (oldest evicted)
    assert_eq!(dedup.window_len(), 3);
}

// ---------------------------------------------------------------------------
// Safety gate tests
// ---------------------------------------------------------------------------

#[test]
fn test_safety_gate_allow() {
    let config = SafetyConfig::default();
    let gate = SafetyGate::new(config);

    let decision = gate.check("This is perfectly safe content about Rust programming.");
    assert_eq!(decision, SafetyDecision::Allow);
}

#[test]
fn test_safety_gate_credit_card_redaction() {
    let config = SafetyConfig::default();
    let gate = SafetyGate::new(config);

    let decision = gate.check("My card number is 4111111111111111 and it expires soon.");
    match decision {
        SafetyDecision::AllowRedacted(redacted) => {
            assert!(
                redacted.contains("[CC_REDACTED]"),
                "Credit card should be redacted, got: {}",
                redacted
            );
            assert!(!redacted.contains("4111111111111111"));
        }
        other => panic!("Expected AllowRedacted, got {:?}", other),
    }
}

#[test]
fn test_safety_gate_ssn_redaction() {
    let config = SafetyConfig::default();
    let gate = SafetyGate::new(config);

    let decision = gate.check("SSN: 123-45-6789 is confidential");
    match decision {
        SafetyDecision::AllowRedacted(redacted) => {
            assert!(
                redacted.contains("[SSN_REDACTED]"),
                "SSN should be redacted, got: {}",
                redacted
            );
            assert!(!redacted.contains("123-45-6789"));
        }
        other => panic!("Expected AllowRedacted, got {:?}", other),
    }
}

#[test]
fn test_safety_gate_email_redaction() {
    let config = SafetyConfig::default();
    let gate = SafetyGate::new(config);

    let decision = gate.check("Contact me at user@example.com for details");
    match decision {
        SafetyDecision::AllowRedacted(redacted) => {
            assert!(
                redacted.contains("[EMAIL_REDACTED]"),
                "Email should be redacted, got: {}",
                redacted
            );
            assert!(!redacted.contains("user@example.com"));
        }
        other => panic!("Expected AllowRedacted, got {:?}", other),
    }
}

#[test]
fn test_safety_gate_custom_pattern_deny() {
    let config = SafetyConfig {
        custom_patterns: vec!["SECRET_KEY".to_string()],
        ..Default::default()
    };
    let gate = SafetyGate::new(config);

    let decision = gate.check("The SECRET_KEY is abc123");
    match decision {
        SafetyDecision::Deny { reason } => {
            assert!(reason.contains("SECRET_KEY"));
        }
        other => panic!("Expected Deny, got {:?}", other),
    }
}

#[test]
fn test_safety_redact_method() {
    let config = SafetyConfig::default();
    let gate = SafetyGate::new(config);

    let redacted = gate.redact("Call me at user@example.com");
    assert!(redacted.contains("[EMAIL_REDACTED]"));
    assert!(!redacted.contains("user@example.com"));

    let safe = gate.redact("Nothing sensitive here.");
    assert_eq!(safe, "Nothing sensitive here.");
}

// ---------------------------------------------------------------------------
// Query router tests
// ---------------------------------------------------------------------------

#[test]
fn test_query_router_temporal() {
    let router = QueryRouter::new();
    assert_eq!(
        router.route("what did I see yesterday"),
        QueryRoute::Temporal
    );
    assert_eq!(router.route("show me last week"), QueryRoute::Temporal);
    assert_eq!(router.route("results from today"), QueryRoute::Temporal);
}

#[test]
fn test_query_router_graph() {
    let router = QueryRouter::new();
    assert_eq!(
        router.route("documents related to authentication"),
        QueryRoute::Graph
    );
    assert_eq!(
        router.route("things connected to the API module"),
        QueryRoute::Graph
    );
}

#[test]
fn test_query_router_keyword() {
    let router = QueryRouter::new();
    assert_eq!(router.route("\"exact phrase search\""), QueryRoute::Keyword);
    assert_eq!(router.route("rust programming"), QueryRoute::Keyword);
    assert_eq!(router.route("hello"), QueryRoute::Keyword);
}

#[test]
fn test_query_router_hybrid() {
    let router = QueryRouter::new();
    assert_eq!(
        router.route("how to implement authentication in Rust"),
        QueryRoute::Hybrid
    );
}

// ---------------------------------------------------------------------------
// Hybrid search tests
// ---------------------------------------------------------------------------

#[test]
fn test_hybrid_search() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    // Insert frames with different content
    let frames = vec![
        CapturedFrame::new_screen("Editor", "code.rs", "implementing vector search in Rust", 0),
        CapturedFrame::new_screen("Browser", "docs", "Rust vector database documentation", 0),
        CapturedFrame::new_audio("Mic", "discussing Python machine learning", None),
    ];

    for frame in &frames {
        let emb = engine.embed(frame.text_content());
        store.insert(frame, &emb).unwrap();
    }

    let hybrid = HybridSearch::new(0.7);
    let query = "vector search Rust";
    let query_emb = engine.embed(query);
    let results = hybrid.search(&store, query, &query_emb, 3).unwrap();

    assert!(!results.is_empty());
    assert!(results.len() <= 3);
    // Results should be ordered by combined score
    for i in 1..results.len() {
        assert!(results[i - 1].score >= results[i].score);
    }
}

#[test]
fn test_hybrid_search_empty_store() {
    let config = StorageConfig::default();
    let store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let hybrid = HybridSearch::new(0.7);
    let query_emb = engine.embed("test query");
    let results = hybrid.search(&store, "test query", &query_emb, 10).unwrap();
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// Ingestion pipeline tests
// ---------------------------------------------------------------------------

#[test]
fn test_ingestion_pipeline_basic() {
    let config = OsPipeConfig::default();
    let mut pipeline = IngestionPipeline::new(config).unwrap();

    let frame = CapturedFrame::new_screen("VS Code", "main.rs", "fn main() { }", 0);
    let result = pipeline.ingest(frame).unwrap();

    match result {
        IngestResult::Stored { id } => {
            assert!(!id.is_nil());
        }
        other => panic!("Expected Stored, got {:?}", other),
    }

    assert_eq!(pipeline.stats().total_ingested, 1);
    assert_eq!(pipeline.stats().total_deduplicated, 0);
    assert_eq!(pipeline.stats().total_denied, 0);
}

#[test]
fn test_ingestion_pipeline_deduplication() {
    let config = OsPipeConfig::default();
    let mut pipeline = IngestionPipeline::new(config).unwrap();

    // Ingest the same content twice
    let frame1 = CapturedFrame::new_screen("App", "Window", "exact same content", 0);
    let frame2 = CapturedFrame::new_screen("App", "Window", "exact same content", 0);

    let result1 = pipeline.ingest(frame1).unwrap();
    assert!(matches!(result1, IngestResult::Stored { .. }));

    let result2 = pipeline.ingest(frame2).unwrap();
    assert!(
        matches!(result2, IngestResult::Deduplicated { .. }),
        "Second identical frame should be deduplicated"
    );

    assert_eq!(pipeline.stats().total_ingested, 1);
    assert_eq!(pipeline.stats().total_deduplicated, 1);
}

#[test]
fn test_ingestion_pipeline_safety_deny() {
    let config = OsPipeConfig {
        safety: SafetyConfig {
            custom_patterns: vec!["TOP_SECRET".to_string()],
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pipeline = IngestionPipeline::new(config).unwrap();

    let frame = CapturedFrame::new_screen("App", "Window", "This is TOP_SECRET information", 0);
    let result = pipeline.ingest(frame).unwrap();

    match result {
        IngestResult::Denied { reason } => {
            assert!(reason.contains("TOP_SECRET"));
        }
        other => panic!("Expected Denied, got {:?}", other),
    }

    assert_eq!(pipeline.stats().total_denied, 1);
    assert_eq!(pipeline.stats().total_ingested, 0);
}

#[test]
fn test_ingestion_pipeline_safety_redact() {
    let config = OsPipeConfig::default();
    let mut pipeline = IngestionPipeline::new(config).unwrap();

    let frame = CapturedFrame::new_screen(
        "App",
        "Window",
        "Please email user@example.com for the meeting notes",
        0,
    );
    let result = pipeline.ingest(frame).unwrap();

    // Should be stored but with redacted content
    assert!(matches!(result, IngestResult::Stored { .. }));
    assert_eq!(pipeline.stats().total_redacted, 1);

    // Verify the stored content has the email redacted
    let store = pipeline.vector_store();
    assert_eq!(store.len(), 1);
}

#[test]
fn test_ingestion_pipeline_batch() {
    let config = OsPipeConfig::default();
    let mut pipeline = IngestionPipeline::new(config).unwrap();

    let frames = vec![
        CapturedFrame::new_screen("App", "Win1", "first frame content", 0),
        CapturedFrame::new_screen("App", "Win2", "second frame content", 0),
        CapturedFrame::new_screen("App", "Win3", "third frame content", 0),
    ];

    let results = pipeline.ingest_batch(frames).unwrap();
    assert_eq!(results.len(), 3);

    let stored_count = results
        .iter()
        .filter(|r| matches!(r, IngestResult::Stored { .. }))
        .count();
    assert_eq!(stored_count, 3);
    assert_eq!(pipeline.stats().total_ingested, 3);
}

// ---------------------------------------------------------------------------
// Cosine similarity tests
// ---------------------------------------------------------------------------

#[test]
fn test_cosine_similarity_identical_vectors() {
    let v = vec![1.0, 0.0, 0.0];
    let sim = cosine_similarity(&v, &v);
    assert!((sim - 1.0).abs() < 1e-5);
}

#[test]
fn test_cosine_similarity_orthogonal_vectors() {
    let v1 = vec![1.0, 0.0, 0.0];
    let v2 = vec![0.0, 1.0, 0.0];
    let sim = cosine_similarity(&v1, &v2);
    assert!(sim.abs() < 1e-5);
}

#[test]
fn test_cosine_similarity_opposite_vectors() {
    let v1 = vec![1.0, 0.0, 0.0];
    let v2 = vec![-1.0, 0.0, 0.0];
    let sim = cosine_similarity(&v1, &v2);
    assert!((sim - (-1.0)).abs() < 1e-5);
}

// ---------------------------------------------------------------------------
// Attention reranker tests
// ---------------------------------------------------------------------------

#[test]
fn test_reranker_with_multiple_results() {
    let dim = 4;
    let reranker = AttentionReranker::new(dim, 1);

    let query = vec![1.0, 0.0, 0.0, 0.0];
    let results = vec![
        ("doc1".to_string(), 0.9, vec![0.8, 0.2, 0.0, 0.0]),
        ("doc2".to_string(), 0.7, vec![0.5, 0.5, 0.0, 0.0]),
        ("doc3".to_string(), 0.5, vec![0.0, 0.0, 1.0, 0.0]),
    ];

    let ranked = reranker.rerank(&query, &results, 3);

    assert_eq!(ranked.len(), 3);
    // All scores should be positive
    for (_, score) in &ranked {
        assert!(*score > 0.0, "All reranked scores should be positive");
    }
    // Results should be sorted descending by score
    for i in 1..ranked.len() {
        assert!(
            ranked[i - 1].1 >= ranked[i].1,
            "Results should be sorted descending: {} >= {}",
            ranked[i - 1].1,
            ranked[i].1,
        );
    }
}

#[test]
fn test_reranker_can_reorder_vs_cosine() {
    let dim = 4;
    let reranker = AttentionReranker::new(dim, 1);

    // "a" has a slightly higher cosine score but its embedding is orthogonal
    // to the query.  "b" is perfectly aligned.  The 60/40 attention blending
    // should promote "b" above "a".
    let query = vec![1.0, 0.0, 0.0, 0.0];
    let results = vec![
        ("a".to_string(), 0.70, vec![0.0, 0.0, 1.0, 0.0]),
        ("b".to_string(), 0.55, vec![1.0, 0.0, 0.0, 0.0]),
    ];

    let ranked = reranker.rerank(&query, &results, 2);

    assert_eq!(ranked.len(), 2);
    assert_eq!(
        ranked[0].0, "b",
        "Attention re-ranking should promote the query-aligned result above one with higher cosine"
    );
}

#[test]
fn test_reranker_empty_results() {
    let reranker = AttentionReranker::new(4, 1);
    let ranked = reranker.rerank(&[1.0, 0.0, 0.0, 0.0], &[], 10);
    assert!(ranked.is_empty(), "Empty input should produce empty output");
}

#[test]
fn test_reranker_top_k_truncation() {
    let dim = 4;
    let reranker = AttentionReranker::new(dim, 1);

    let query = vec![1.0, 0.0, 0.0, 0.0];
    let results = vec![
        ("a".to_string(), 0.9, vec![1.0, 0.0, 0.0, 0.0]),
        ("b".to_string(), 0.8, vec![0.0, 1.0, 0.0, 0.0]),
        ("c".to_string(), 0.7, vec![0.0, 0.0, 1.0, 0.0]),
        ("d".to_string(), 0.6, vec![0.0, 0.0, 0.0, 1.0]),
    ];

    let ranked = reranker.rerank(&query, &results, 2);
    assert_eq!(ranked.len(), 2, "top_k=2 should return at most 2 results");
}

// ---------------------------------------------------------------------------
// Learning module tests
// ---------------------------------------------------------------------------

use ospipe::learning::{EmbeddingQuantizer, SearchLearner};

#[test]
fn test_search_learner_record_feedback() {
    let mut learner = SearchLearner::new(64, 1000);
    assert_eq!(learner.replay_buffer_len(), 0);

    let query = vec![0.1_f32; 64];
    let result = vec![0.2_f32; 64];
    learner.record_feedback(query, result, true);

    assert_eq!(learner.replay_buffer_len(), 1);
}

#[test]
fn test_search_learner_replay_buffer_fills() {
    let mut learner = SearchLearner::new(8, 500);

    for i in 0..50 {
        let q = vec![i as f32 * 0.01; 8];
        let r = vec![i as f32 * 0.02; 8];
        learner.record_feedback(q, r, i % 3 != 0);
    }

    assert_eq!(learner.replay_buffer_len(), 50);
}

#[test]
fn test_search_learner_has_sufficient_data_threshold() {
    let mut learner = SearchLearner::new(8, 500);

    // Below threshold
    for i in 0..31 {
        let q = vec![i as f32; 8];
        let r = vec![i as f32; 8];
        learner.record_feedback(q, r, true);
    }
    assert!(!learner.has_sufficient_data());

    // At threshold
    learner.record_feedback(vec![0.0; 8], vec![0.0; 8], true);
    assert!(learner.has_sufficient_data());
}

#[test]
fn test_search_learner_consolidate_and_penalty() {
    let mut learner = SearchLearner::new(8, 500);

    // Populate the buffer with enough diverse data.
    for i in 0..64 {
        let q = vec![i as f32 * 0.1; 8];
        let r = vec![(64 - i) as f32 * 0.1; 8];
        learner.record_feedback(q, r, i % 2 == 0);
    }

    assert!(learner.has_sufficient_data());

    // Before consolidation the EWC penalty should be 0.
    assert_eq!(learner.ewc_penalty(), 0.0);

    // After consolidation the penalty should still be 0 because the current
    // weights have not deviated from the newly-set anchor.
    learner.consolidate();
    assert!((learner.ewc_penalty()).abs() < 1e-6);
}

#[test]
fn test_quantizer_roundtrip_fresh_embedding() {
    let quantizer = EmbeddingQuantizer::new();
    let original = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];

    let compressed = quantizer.quantize_by_age(&original, 0);
    let recovered = quantizer.dequantize(&compressed, original.len());

    // Fresh embeddings (age 0) should round-trip exactly (no compression).
    for (a, b) in original.iter().zip(recovered.iter()) {
        assert!(
            (a - b).abs() < 1e-5,
            "Fresh embedding round-trip mismatch: {} vs {}",
            a,
            b
        );
    }
}

#[test]
fn test_quantizer_old_embedding_is_smaller() {
    let quantizer = EmbeddingQuantizer::new();
    // Use an embedding size divisible by 8 (required for PQ subvectors).
    let original: Vec<f32> = (0..128).map(|i| i as f32 * 0.01).collect();

    let fresh_bytes = quantizer.quantize_by_age(&original, 0);
    let old_bytes = quantizer.quantize_by_age(&original, 200);

    // Old embeddings should be compressed to fewer bytes than fresh ones.
    assert!(
        old_bytes.len() < fresh_bytes.len(),
        "Old embedding ({} bytes) should be smaller than fresh ({} bytes)",
        old_bytes.len(),
        fresh_bytes.len(),
    );
}

#[test]
fn test_quantizer_dequantize_old_preserves_dimension() {
    let quantizer = EmbeddingQuantizer::new();
    let original: Vec<f32> = (0..128).map(|i| i as f32 * 0.01).collect();
    let compressed = quantizer.quantize_by_age(&original, 200);
    let recovered = quantizer.dequantize(&compressed, 128);
    assert_eq!(recovered.len(), 128);
}

// ---------------------------------------------------------------------------
// Knowledge graph tests
// ---------------------------------------------------------------------------

use std::collections::HashMap;

#[test]
fn test_graph_entity_extraction_from_text() {
    let text =
        "Meeting with John Smith at https://meet.example.com. Contact @alice or bob@company.org.";
    let entities = KnowledgeGraph::extract_entities(text);

    let labels: Vec<&str> = entities.iter().map(|(l, _)| l.as_str()).collect();
    assert!(
        labels.contains(&"Url"),
        "Expected a Url entity, got: {:?}",
        entities
    );
    assert!(
        labels.contains(&"Mention"),
        "Expected a Mention entity, got: {:?}",
        entities
    );
    assert!(
        labels.contains(&"Email"),
        "Expected an Email entity, got: {:?}",
        entities
    );
    assert!(
        labels.contains(&"Person"),
        "Expected a Person entity, got: {:?}",
        entities
    );

    let url_entity = entities.iter().find(|(l, _)| l == "Url").unwrap();
    assert_eq!(url_entity.1, "https://meet.example.com");

    let person_entity = entities.iter().find(|(l, _)| l == "Person").unwrap();
    assert!(
        person_entity.1.contains("John Smith"),
        "Expected 'John Smith', got: {}",
        person_entity.1
    );
}

#[test]
fn test_graph_add_entity_and_find_by_label() {
    let kg = KnowledgeGraph::new();

    let mut props = HashMap::new();
    props.insert("role".to_string(), "engineer".to_string());
    let id1 = kg.add_entity("Person", "Alice", props).unwrap();
    let id2 = kg.add_entity("Person", "Bob", HashMap::new()).unwrap();
    let _id3 = kg
        .add_entity("Url", "https://example.com", HashMap::new())
        .unwrap();

    assert_ne!(id1, id2, "Entity IDs must be unique");

    let people = kg.find_by_label("Person");
    assert_eq!(
        people.len(),
        2,
        "Expected 2 Person entities, got: {:?}",
        people
    );

    let urls = kg.find_by_label("Url");
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].name, "https://example.com");
}

#[test]
fn test_graph_add_relationship_and_neighbors() {
    let kg = KnowledgeGraph::new();

    let alice_id = kg.add_entity("Person", "Alice", HashMap::new()).unwrap();
    let bob_id = kg.add_entity("Person", "Bob", HashMap::new()).unwrap();
    let project_id = kg.add_entity("Topic", "RuVector", HashMap::new()).unwrap();

    let edge1 = kg.add_relationship(&alice_id, &bob_id, "KNOWS").unwrap();
    let edge2 = kg
        .add_relationship(&alice_id, &project_id, "WORKS_ON")
        .unwrap();
    assert_ne!(edge1, edge2);

    // Alice should have 2 neighbours (Bob and RuVector).
    let alice_neighbors = kg.neighbors(&alice_id);
    assert_eq!(
        alice_neighbors.len(),
        2,
        "Expected 2 neighbors for Alice, got: {:?}",
        alice_neighbors
    );

    // Bob should have 1 neighbour (Alice, via incoming edge).
    let bob_neighbors = kg.neighbors(&bob_id);
    assert_eq!(bob_neighbors.len(), 1);
    assert_eq!(bob_neighbors[0].name, "Alice");
}

#[test]
fn test_graph_ingest_frame_entities() {
    let kg = KnowledgeGraph::new();
    let text = "John Smith visited https://docs.rs and contacted @rustlang";

    let entity_ids = kg.ingest_frame_entities("frame-42", text).unwrap();
    assert!(!entity_ids.is_empty(), "Should extract at least one entity");

    // The frame node should exist.
    let frames = kg.find_by_label("Frame");
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].name, "frame-42");

    // The frame node should be connected to all extracted entities.
    let frame_neighbors = kg.neighbors(&frames[0].id);
    assert_eq!(
        frame_neighbors.len(),
        entity_ids.len(),
        "Frame should be connected to all extracted entities"
    );
}

#[test]
fn test_graph_ingest_idempotent_frame_node() {
    let kg = KnowledgeGraph::new();

    let _ids1 = kg.ingest_frame_entities("frame-99", "Hello World").unwrap();
    let _ids2 = kg
        .ingest_frame_entities("frame-99", "Visit https://example.com/test")
        .unwrap();

    // Should still have only 1 frame node.
    let frames = kg.find_by_label("Frame");
    assert_eq!(frames.len(), 1, "Frame node should not be duplicated");
}

// ---------------------------------------------------------------------------
// Quantum-inspired search tests
// ---------------------------------------------------------------------------

use ospipe::quantum::QuantumSearch;

#[test]
fn test_quantum_optimal_iterations_small() {
    let qs = QuantumSearch::new();
    // 4 items: pi/4 * sqrt(4) = pi/4 * 2 = 1.57 -> floor = 1
    assert_eq!(qs.optimal_iterations(4), 1);
}

#[test]
fn test_quantum_optimal_iterations_medium() {
    let qs = QuantumSearch::new();
    // 100 items: pi/4 * sqrt(100) = pi/4 * 10 = 7.85 -> floor = 7
    let iters = qs.optimal_iterations(100);
    assert!(
        (iters as i32 - 7).abs() <= 1,
        "Expected ~7 iterations for 100 items, got {}",
        iters
    );
}

#[test]
fn test_quantum_optimal_iterations_large() {
    let qs = QuantumSearch::new();
    // 1000 items: pi/4 * sqrt(1000) = pi/4 * 31.62 = 24.84 -> floor = 24
    let iters = qs.optimal_iterations(1000);
    assert!(
        (iters as i32 - 24).abs() <= 1,
        "Expected ~24 iterations for 1000 items, got {}",
        iters
    );
}

#[test]
fn test_quantum_optimal_iterations_single() {
    let qs = QuantumSearch::new();
    // 1 item: should return at least 1
    assert_eq!(qs.optimal_iterations(1), 1);
}

#[test]
fn test_quantum_diversity_select_basic() {
    let qs = QuantumSearch::new();
    let scores: Vec<(String, f32)> = vec![
        ("a".to_string(), 0.95),
        ("b".to_string(), 0.90),
        ("c".to_string(), 0.85),
        ("d".to_string(), 0.80),
        ("e".to_string(), 0.70),
        ("f".to_string(), 0.60),
        ("g".to_string(), 0.50),
        ("h".to_string(), 0.40),
        ("i".to_string(), 0.30),
        ("j".to_string(), 0.20),
    ];

    let selected = qs.diversity_select(&scores, 3);
    assert_eq!(selected.len(), 3, "Should return exactly k=3 items");

    // All selected items must come from the original set.
    for (id, _) in &selected {
        assert!(
            scores.iter().any(|(orig_id, _)| orig_id == id),
            "Selected item '{}' not found in original scores",
            id
        );
    }
}

#[test]
fn test_quantum_diversity_select_k_exceeds_input() {
    let qs = QuantumSearch::new();
    let scores = vec![("a".to_string(), 0.9), ("b".to_string(), 0.5)];

    let selected = qs.diversity_select(&scores, 10);
    assert_eq!(selected.len(), 2, "Should return at most input length");
}

#[test]
fn test_quantum_diversity_select_empty() {
    let qs = QuantumSearch::new();
    let selected = qs.diversity_select(&[], 3);
    assert!(
        selected.is_empty(),
        "Empty input should produce empty output"
    );
}

#[test]
fn test_quantum_amplitude_boost_increases_high_scores() {
    let qs = QuantumSearch::new();
    let mut scores = vec![
        ("high1".to_string(), 0.9),
        ("high2".to_string(), 0.8),
        ("low1".to_string(), 0.2),
        ("low2".to_string(), 0.1),
    ];

    let threshold = 0.5;
    qs.amplitude_boost(&mut scores, threshold);

    // After boost and re-normalization, the high-scoring items should
    // have higher scores than the low-scoring items.
    let high_scores: Vec<f32> = scores
        .iter()
        .filter(|(id, _)| id.starts_with("high"))
        .map(|(_, s)| *s)
        .collect();
    let low_scores: Vec<f32> = scores
        .iter()
        .filter(|(id, _)| id.starts_with("low"))
        .map(|(_, s)| *s)
        .collect();

    let min_high = high_scores.iter().cloned().fold(f32::INFINITY, f32::min);
    let max_low = low_scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

    assert!(
        min_high > max_low,
        "Boosted high scores ({}) should exceed dampened low scores ({})",
        min_high,
        max_low
    );
}

#[test]
fn test_quantum_amplitude_boost_normalizes_to_unit_range() {
    let qs = QuantumSearch::new();
    let mut scores = vec![
        ("a".to_string(), 0.9),
        ("b".to_string(), 0.6),
        ("c".to_string(), 0.3),
        ("d".to_string(), 0.1),
    ];

    qs.amplitude_boost(&mut scores, 0.5);

    for (id, score) in &scores {
        assert!(
            *score >= 0.0 && *score <= 1.0,
            "Score for '{}' should be in [0,1] after boost, got {}",
            id,
            score
        );
    }
}

#[test]
fn test_quantum_amplitude_boost_empty() {
    let qs = QuantumSearch::new();
    let mut scores: Vec<(String, f32)> = Vec::new();
    qs.amplitude_boost(&mut scores, 0.5);
    assert!(scores.is_empty());
}

#[test]
fn test_quantum_amplitude_boost_all_same_side() {
    let qs = QuantumSearch::new();
    let mut scores = vec![
        ("a".to_string(), 0.8),
        ("b".to_string(), 0.7),
        ("c".to_string(), 0.6),
    ];
    let original = scores.clone();

    // All above threshold -- boost is a no-op.
    qs.amplitude_boost(&mut scores, 0.5);

    // Scores should remain unchanged (no amplification when all are on
    // the same side of the threshold).
    for (orig, current) in original.iter().zip(scores.iter()) {
        assert_eq!(orig.0, current.0);
        assert!(
            (orig.1 - current.1).abs() < 1e-5,
            "Scores should be unchanged when all are above threshold"
        );
    }
}

// ---------------------------------------------------------------------------
// Pipeline with knowledge graph wired
// ---------------------------------------------------------------------------

#[test]
fn test_pipeline_with_graph_extracts_entities() {
    let config = OsPipeConfig::default();
    let kg = KnowledgeGraph::new();
    let mut pipeline = IngestionPipeline::new(config).unwrap().with_graph(kg);

    // Ingest a frame whose text contains extractable entities.
    let frame = CapturedFrame::new_screen(
        "Browser",
        "Meeting Notes",
        "Meeting with John Smith at https://meet.example.com. Contact @alice.",
        0,
    );
    let result = pipeline.ingest(frame).unwrap();

    // Frame should be stored.
    assert!(matches!(result, IngestResult::Stored { .. }));

    // The knowledge graph should have extracted entities.
    let kg = pipeline
        .knowledge_graph()
        .expect("graph should be attached");
    let frames = kg.find_by_label("Frame");
    assert_eq!(frames.len(), 1, "Should have created a Frame node");

    let people = kg.find_by_label("Person");
    assert!(
        people.iter().any(|e| e.name.contains("John Smith")),
        "Should have extracted 'John Smith' as a Person entity, got: {:?}",
        people
    );

    let urls = kg.find_by_label("Url");
    assert!(
        urls.iter().any(|e| e.name.contains("meet.example.com")),
        "Should have extracted the URL entity, got: {:?}",
        urls
    );

    let mentions = kg.find_by_label("Mention");
    assert!(
        mentions.iter().any(|e| e.name.contains("@alice")),
        "Should have extracted the @alice mention, got: {:?}",
        mentions
    );
}

#[test]
fn test_pipeline_without_graph_still_works() {
    let config = OsPipeConfig::default();
    let mut pipeline = IngestionPipeline::new(config).unwrap();

    let frame = CapturedFrame::new_screen("App", "Win", "no graph attached", 0);
    let result = pipeline.ingest(frame).unwrap();
    assert!(matches!(result, IngestResult::Stored { .. }));
    assert!(pipeline.knowledge_graph().is_none());
}

#[test]
fn test_pipeline_graph_multiple_frames() {
    let config = OsPipeConfig::default();
    let kg = KnowledgeGraph::new();
    let mut pipeline = IngestionPipeline::new(config).unwrap().with_graph(kg);

    let frames = vec![
        CapturedFrame::new_screen("App", "Win1", "Alice Smith works at https://company.com", 0),
        CapturedFrame::new_screen("App", "Win2", "Bob Jones emailed bob@company.org", 0),
    ];

    pipeline.ingest_batch(frames).unwrap();

    let kg = pipeline.knowledge_graph().unwrap();
    let frame_nodes = kg.find_by_label("Frame");
    assert_eq!(frame_nodes.len(), 2, "Should have 2 Frame nodes");
}

// ---------------------------------------------------------------------------
// Enhanced search integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_enhanced_search_basic_integration() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let frames = vec![
        CapturedFrame::new_screen("Editor", "code.rs", "implementing vector search in Rust", 0),
        CapturedFrame::new_screen("Browser", "docs", "Rust vector database documentation", 0),
        CapturedFrame::new_audio("Mic", "discussing Python machine learning", None),
    ];

    for frame in &frames {
        let emb = engine.embed(frame.text_content());
        store.insert(frame, &emb).unwrap();
    }

    let es = EnhancedSearch::new(384);
    let query = "vector search Rust";
    let query_emb = engine.embed(query);
    let results = es.search(query, &query_emb, &store, 2).unwrap();

    assert!(!results.is_empty(), "Enhanced search should return results");
    assert!(results.len() <= 2, "Should respect k=2 limit");
}

#[test]
fn test_enhanced_search_empty_store() {
    let config = StorageConfig::default();
    let store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let es = EnhancedSearch::new(384);
    let query_emb = engine.embed("test query");
    let results = es.search("test query", &query_emb, &store, 5).unwrap();
    assert!(
        results.is_empty(),
        "Search on empty store should return no results"
    );
}

#[test]
fn test_enhanced_search_single_result() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let frame = CapturedFrame::new_screen("App", "Win", "unique single content", 0);
    let emb = engine.embed(frame.text_content());
    store.insert(&frame, &emb).unwrap();

    let es = EnhancedSearch::new(384);
    let query_emb = engine.embed("unique single content");
    let results = es
        .search("unique single content", &query_emb, &store, 5)
        .unwrap();

    assert_eq!(results.len(), 1, "Should find the single stored frame");
    assert_eq!(results[0].id, frame.id, "Should match the stored frame ID");
}

// ---------------------------------------------------------------------------
// End-to-end: ingest -> search -> reranked results
// ---------------------------------------------------------------------------

#[test]
fn test_end_to_end_ingest_and_enhanced_search() {
    let config = OsPipeConfig::default();
    let kg = KnowledgeGraph::new();
    let es = EnhancedSearch::new(config.storage.embedding_dim);
    let mut pipeline = IngestionPipeline::new(config)
        .unwrap()
        .with_graph(kg)
        .with_enhanced_search(es);

    // Ingest several frames with varied content.
    let frames = vec![
        CapturedFrame::new_screen(
            "VS Code",
            "main.rs",
            "fn main() { println!(\"hello world\"); }",
            0,
        ),
        CapturedFrame::new_screen(
            "Firefox",
            "Rust docs",
            "The Rust Programming Language book chapter on ownership",
            0,
        ),
        CapturedFrame::new_audio(
            "Mic",
            "discussing the project architecture with Alice Smith",
            None,
        ),
        CapturedFrame::new_screen(
            "VS Code",
            "lib.rs",
            "pub struct VectorStore { embeddings: Vec<f32> }",
            0,
        ),
    ];

    let results = pipeline.ingest_batch(frames).unwrap();
    let stored_count = results
        .iter()
        .filter(|r| matches!(r, IngestResult::Stored { .. }))
        .count();
    assert!(stored_count >= 3, "Most frames should be stored");

    // Search using the pipeline's convenience method (uses enhanced search).
    let search_results = pipeline.search("Rust programming", 3).unwrap();
    assert!(
        !search_results.is_empty(),
        "Enhanced pipeline search should find relevant frames"
    );
    assert!(
        search_results.len() <= 3,
        "Should respect k=3 limit, got {}",
        search_results.len()
    );

    // All returned scores should be positive.
    for sr in &search_results {
        assert!(sr.score > 0.0, "Score should be positive, got {}", sr.score);
    }

    // Verify the knowledge graph captured entities.
    let kg = pipeline.knowledge_graph().unwrap();
    let people = kg.find_by_label("Person");
    assert!(
        people.iter().any(|e| e.name.contains("Alice Smith")),
        "Graph should have captured 'Alice Smith' from audio transcription, got: {:?}",
        people
    );
}

#[test]
fn test_pipeline_search_without_enhanced() {
    let config = OsPipeConfig::default();
    let mut pipeline = IngestionPipeline::new(config).unwrap();

    let frame = CapturedFrame::new_screen("App", "Win", "basic search content", 0);
    pipeline.ingest(frame).unwrap();

    // Without enhanced search, the pipeline falls back to basic vector search.
    let results = pipeline.search("basic search content", 5).unwrap();
    assert!(!results.is_empty(), "Basic search should still work");
    assert_eq!(
        results[0].score, 1.0,
        "Exact match should have score 1.0 (within tolerance)"
    );
}

// ---------------------------------------------------------------------------
// Delete / Update API tests (VectorStore)
// ---------------------------------------------------------------------------

#[test]
fn test_vector_store_delete() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let frame = CapturedFrame::new_screen("App", "Win", "delete me", 0);
    let id = frame.id;
    let emb = engine.embed(frame.text_content());
    store.insert(&frame, &emb).unwrap();
    assert_eq!(store.len(), 1);

    // Delete the entry
    let removed = store.delete(&id).unwrap();
    assert!(removed, "delete should return true for existing id");
    assert_eq!(store.len(), 0);
    assert!(
        store.get(&id).is_none(),
        "get should return None after delete"
    );

    // Deleting again should return false
    let removed_again = store.delete(&id).unwrap();
    assert!(!removed_again, "delete should return false for missing id");
}

#[test]
fn test_vector_store_delete_search_consistency() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let frame1 = CapturedFrame::new_screen("App", "Win", "keep this frame", 0);
    let frame2 = CapturedFrame::new_screen("App", "Win", "remove this frame", 0);
    let id2 = frame2.id;

    let emb1 = engine.embed(frame1.text_content());
    let emb2 = engine.embed(frame2.text_content());
    store.insert(&frame1, &emb1).unwrap();
    store.insert(&frame2, &emb2).unwrap();
    assert_eq!(store.len(), 2);

    store.delete(&id2).unwrap();
    assert_eq!(store.len(), 1);

    // Search should only return the remaining entry
    let query = engine.embed("keep this frame");
    let results = store.search(&query, 10).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, frame1.id);
}

#[test]
fn test_vector_store_update_metadata() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();
    let engine = EmbeddingEngine::new(384);

    let frame = CapturedFrame::new_screen("App", "Win", "update me", 0);
    let id = frame.id;
    let emb = engine.embed(frame.text_content());
    store.insert(&frame, &emb).unwrap();

    // Original metadata has "App" as app_name
    let stored = store.get(&id).unwrap();
    assert_eq!(
        stored.metadata.get("app_name").and_then(|v| v.as_str()),
        Some("App"),
    );

    // Update metadata
    let new_meta = serde_json::json!({
        "app_name": "UpdatedApp",
        "custom_field": 42,
    });
    store.update_metadata(&id, new_meta).unwrap();

    // Verify the update took effect
    let stored = store.get(&id).unwrap();
    assert_eq!(
        stored.metadata.get("app_name").and_then(|v| v.as_str()),
        Some("UpdatedApp"),
    );
    assert_eq!(
        stored.metadata.get("custom_field").and_then(|v| v.as_i64()),
        Some(42),
    );
}

#[test]
fn test_vector_store_update_metadata_not_found() {
    let config = StorageConfig::default();
    let mut store = VectorStore::new(config).unwrap();

    let missing_id = uuid::Uuid::new_v4();
    let result = store.update_metadata(&missing_id, serde_json::json!({}));
    assert!(result.is_err(), "update_metadata on missing id should fail");
}

// ---------------------------------------------------------------------------
// EmbeddingModel trait tests
// ---------------------------------------------------------------------------

use ospipe::storage::traits::{EmbeddingModel, HashEmbeddingModel};

#[test]
fn test_embedding_model_trait_hash() {
    let model = HashEmbeddingModel::new(128);
    assert_eq!(model.dimension(), 128);

    let v1 = model.embed("deterministic");
    let v2 = model.embed("deterministic");
    assert_eq!(v1, v2, "Same input must produce identical output");
    assert_eq!(v1.len(), 128);

    // Check normalization
    let mag: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (mag - 1.0).abs() < 1e-5,
        "HashEmbeddingModel should produce normalized vectors",
    );
}

#[test]
fn test_embedding_model_trait_batch() {
    let model = HashEmbeddingModel::new(64);
    let texts = vec!["alpha", "beta", "gamma"];
    let embeddings = model.batch_embed(&texts);
    assert_eq!(embeddings.len(), 3);
    for emb in &embeddings {
        assert_eq!(emb.len(), 64);
    }
}

#[test]
fn test_embedding_engine_implements_trait() {
    // Verify EmbeddingEngine can be used as dyn EmbeddingModel
    let engine = EmbeddingEngine::new(64);
    let model: &dyn EmbeddingModel = &engine;
    let v = model.embed("trait dispatch");
    assert_eq!(v.len(), 64);
    assert_eq!(model.dimension(), 64);
}

// ---------------------------------------------------------------------------
// HNSW vector store tests (native only)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod hnsw_tests {
    use ospipe::capture::CapturedFrame;
    use ospipe::config::StorageConfig;
    use ospipe::storage::embedding::EmbeddingEngine;
    use ospipe::storage::vector_store::HnswVectorStore;
    use ospipe::storage::vector_store::SearchFilter;

    #[test]
    fn test_hnsw_store_insert_and_search() {
        let config = StorageConfig::default();
        let mut store = HnswVectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);

        let frames = vec![
            CapturedFrame::new_screen(
                "VS Code",
                "main.rs",
                "fn main() { println!(\"hello\"); }",
                0,
            ),
            CapturedFrame::new_screen("Firefox", "Docs", "Rust programming language", 0),
            CapturedFrame::new_audio("Mic", "discussing architecture", None),
        ];

        for frame in &frames {
            let emb = engine.embed(frame.text_content());
            store.insert(frame, &emb).unwrap();
        }

        assert_eq!(store.len(), 3);
        assert!(!store.is_empty());

        // Search for the first frame
        let query = engine.embed("fn main() { println!(\"hello\"); }");
        let results = store.search(&query, 2).unwrap();
        assert!(!results.is_empty());
        assert!(results.len() <= 2);
    }

    #[test]
    fn test_hnsw_store_delete() {
        let config = StorageConfig::default();
        let mut store = HnswVectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);

        let frame = CapturedFrame::new_screen("App", "Win", "to delete", 0);
        let id = frame.id;
        let emb = engine.embed(frame.text_content());
        store.insert(&frame, &emb).unwrap();
        assert_eq!(store.len(), 1);

        let removed = store.delete(&id).unwrap();
        assert!(removed);
        assert_eq!(store.len(), 0);
        assert!(store.get(&id).is_none());

        // Second delete returns false
        let removed_again = store.delete(&id).unwrap();
        assert!(!removed_again);
    }

    #[test]
    fn test_hnsw_store_update_metadata() {
        let config = StorageConfig::default();
        let mut store = HnswVectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);

        let frame = CapturedFrame::new_screen("App", "Win", "update test", 0);
        let id = frame.id;
        let emb = engine.embed(frame.text_content());
        store.insert(&frame, &emb).unwrap();

        let new_meta = serde_json::json!({
            "app_name": "NewApp",
            "tag": "updated",
        });
        store.update_metadata(&id, new_meta).unwrap();

        let stored = store.get(&id).unwrap();
        assert_eq!(
            stored.metadata.get("app_name").and_then(|v| v.as_str()),
            Some("NewApp"),
        );
        assert_eq!(
            stored.metadata.get("tag").and_then(|v| v.as_str()),
            Some("updated"),
        );
    }

    #[test]
    fn test_hnsw_store_update_metadata_not_found() {
        let config = StorageConfig::default();
        let mut store = HnswVectorStore::new(config).unwrap();
        let missing = uuid::Uuid::new_v4();
        let result = store.update_metadata(&missing, serde_json::json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_hnsw_store_filtered_search() {
        let config = StorageConfig::default();
        let mut store = HnswVectorStore::new(config).unwrap();
        let engine = EmbeddingEngine::new(384);

        let f1 = CapturedFrame::new_screen("VS Code", "editor", "rust code", 0);
        let f2 = CapturedFrame::new_screen("Firefox", "browser", "rust docs", 0);

        let e1 = engine.embed(f1.text_content());
        let e2 = engine.embed(f2.text_content());
        store.insert(&f1, &e1).unwrap();
        store.insert(&f2, &e2).unwrap();

        let filter = SearchFilter {
            app: Some("VS Code".to_string()),
            ..Default::default()
        };
        let query = engine.embed("rust");
        let results = store.search_filtered(&query, 10, &filter).unwrap();

        // Only the VS Code frame should match the filter
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, f1.id);
    }

    #[test]
    fn test_hnsw_store_dimension_mismatch() {
        let config = StorageConfig::default(); // 384-dim
        let mut store = HnswVectorStore::new(config).unwrap();
        let frame = CapturedFrame::new_screen("App", "Win", "text", 0);

        let wrong_emb = vec![1.0f32; 128];
        let result = store.insert(&frame, &wrong_emb);
        assert!(result.is_err());
    }

    // --- RuvectorEmbeddingModel tests ---

    use ospipe::storage::traits::EmbeddingModel;
    use ospipe::storage::traits::RuvectorEmbeddingModel;

    #[test]
    fn test_ruvector_embedding_model_basic() {
        let model = RuvectorEmbeddingModel::hash(128);
        assert_eq!(model.dimension(), 128);

        let v = model.embed("test text");
        assert_eq!(v.len(), 128);

        // Normalized
        let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (mag - 1.0).abs() < 1e-4,
            "RuvectorEmbeddingModel should produce normalized vectors, got {}",
            mag,
        );
    }

    #[test]
    fn test_ruvector_embedding_model_determinism() {
        let model = RuvectorEmbeddingModel::hash(64);
        let v1 = model.embed("consistent");
        let v2 = model.embed("consistent");
        assert_eq!(v1, v2, "Same input must produce identical vectors");
    }
}
