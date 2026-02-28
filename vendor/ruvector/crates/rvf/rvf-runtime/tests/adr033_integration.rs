//! Integration tests for ADR-033: Progressive Indexing Hardening.
//!
//! Tests cover:
//! 1. QualityEnvelope as mandatory outer return type
//! 2. Budget cap enforcement under adversarial queries
//! 3. Graceful degradation under degenerate conditions
//! 4. SecurityPolicy enforcement
//! 5. Content-addressed centroid stability (HardeningFields)
//! 6. Adversarial distribution detection
//! 7. DoS hardening mechanisms

use rvf_runtime::{
    adaptive_n_probe, centroid_distance_cv, combined_effective_n_probe,
    effective_n_probe_with_drift, is_degenerate_distribution, selective_safety_net_scan,
    should_activate_safety_net, BudgetTokenBucket, NegativeCache, ProofOfWork, QueryOptions,
    QuerySignature, RvfOptions, RvfStore, DEGENERATE_CV_THRESHOLD,
};
use rvf_types::quality::*;
use rvf_types::security::*;
use rvf_types::{ErrorCode, RvfError};
use std::time::Duration;

// ---- Helper: create a test store ----

fn create_test_store(dim: u16, count: usize) -> (tempfile::TempDir, RvfStore) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.rvf");
    let mut opts = RvfOptions::default();
    opts.dimension = dim;
    opts.security_policy = SecurityPolicy::Permissive; // For test simplicity.
    let mut store = RvfStore::create(&path, opts).unwrap();

    // Ingest vectors in a single batch.
    let vectors: Vec<Vec<f32>> = (0..count)
        .map(|i| {
            (0..dim as usize)
                .map(|d| (i * dim as usize + d) as f32 * 0.01)
                .collect()
        })
        .collect();
    let vec_refs: Vec<&[f32]> = vectors.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = (0..count as u64).collect();
    store.ingest_batch(&vec_refs, &ids, None).unwrap();

    (dir, store)
}

// ========================================================================
// §1 QualityEnvelope Tests
// ========================================================================

#[test]
fn quality_envelope_returned_for_normal_query() {
    let (_dir, store) = create_test_store(4, 100);
    let query = vec![0.5, 0.5, 0.5, 0.5];
    let opts = QueryOptions {
        quality_preference: QualityPreference::AcceptDegraded,
        ..QueryOptions::default()
    };

    let envelope = store.query_with_envelope(&query, 10, &opts).unwrap();

    assert!(!envelope.results.is_empty());
    assert!(matches!(
        envelope.quality,
        ResponseQuality::Verified | ResponseQuality::Usable | ResponseQuality::Degraded
    ));
    // Evidence must be populated.
    assert!(envelope.evidence.layers_used.layer_a);
    // Budget report must have non-zero total_us.
    assert!(envelope.budgets.total_us > 0 || envelope.results.is_empty());
}

#[test]
fn quality_envelope_contains_all_fields() {
    let (_dir, store) = create_test_store(4, 50);
    let query = vec![0.1, 0.2, 0.3, 0.4];
    let opts = QueryOptions {
        quality_preference: QualityPreference::AcceptDegraded,
        ..QueryOptions::default()
    };

    let envelope = store.query_with_envelope(&query, 5, &opts).unwrap();

    // All required fields must be present (not None where applicable).
    let _ = envelope.quality;
    let _ = envelope.evidence.layers_used;
    let _ = envelope.evidence.n_probe_effective;
    let _ = envelope.evidence.hnsw_candidate_count;
    let _ = envelope.budgets.total_us;
    let _ = envelope.budgets.distance_ops;
    // degradation may be None for non-degraded results.
}

#[test]
fn quality_envelope_degraded_without_accept_returns_error() {
    let (_dir, store) = create_test_store(4, 2);
    let query = vec![0.5, 0.5, 0.5, 0.5];

    // Request 100 results from a store with only 2 vectors.
    // Safety net should activate, and with tight budget, quality may degrade.
    let opts = QueryOptions {
        quality_preference: QualityPreference::Auto,
        safety_net_budget: SafetyNetBudget {
            max_scan_time_us: 1,
            max_scan_candidates: 1,
            max_distance_ops: 1,
        },
        ..QueryOptions::default()
    };

    let result = store.query_with_envelope(&query, 100, &opts);
    // With such a tiny budget asking for 100 from 2 vectors, we expect
    // either Ok (if 2 results are enough) or Err(QualityBelowThreshold).
    match result {
        Ok(envelope) => {
            // If Ok, quality should not be Degraded/Unreliable since we didn't AcceptDegraded.
            assert!(matches!(
                envelope.quality,
                ResponseQuality::Verified | ResponseQuality::Usable
            ));
        }
        Err(RvfError::QualityBelowThreshold { quality, reason }) => {
            assert!(matches!(
                quality,
                ResponseQuality::Degraded | ResponseQuality::Unreliable
            ));
            assert!(!reason.is_empty());
        }
        Err(other) => panic!("unexpected error: {other}"),
    }
}

#[test]
fn quality_envelope_accept_degraded_succeeds() {
    let (_dir, store) = create_test_store(4, 2);
    let query = vec![0.5, 0.5, 0.5, 0.5];

    let opts = QueryOptions {
        quality_preference: QualityPreference::AcceptDegraded,
        safety_net_budget: SafetyNetBudget {
            max_scan_time_us: 1,
            max_scan_candidates: 1,
            max_distance_ops: 1,
        },
        ..QueryOptions::default()
    };

    // With AcceptDegraded, even degraded results should return Ok.
    let result = store.query_with_envelope(&query, 100, &opts);
    assert!(result.is_ok());
}

// ========================================================================
// §2 Budget Cap Enforcement
// ========================================================================

#[test]
fn budget_caps_are_hard_limits() {
    let budget = SafetyNetBudget {
        max_scan_time_us: 1_000_000, // 1 second (won't hit in test).
        max_scan_candidates: 50,
        max_distance_ops: 50,
    };

    let query = vec![0.0; 4];
    let vecs: Vec<(u64, Vec<f32>)> = (0..10_000)
        .map(|i| (i as u64, vec![i as f32 * 0.001; 4]))
        .collect();
    let refs: Vec<(u64, &[f32])> = vecs.iter().map(|(id, v)| (*id, v.as_slice())).collect();

    let result = selective_safety_net_scan(&query, 10, &[], &refs, &budget, 10_000);

    // Hard cap: distance_ops must not exceed budget.
    assert!(
        result.budget_report.distance_ops <= budget.max_distance_ops + 1,
        "distance_ops {} exceeded budget {}",
        result.budget_report.distance_ops,
        budget.max_distance_ops,
    );
    assert!(
        result.budget_report.linear_scan_count <= budget.max_scan_candidates + 1,
        "scan_count {} exceeded budget {}",
        result.budget_report.linear_scan_count,
        budget.max_scan_candidates,
    );
}

#[test]
fn disabled_budget_produces_no_scan() {
    let query = vec![0.0; 4];
    let vecs: Vec<(u64, Vec<f32>)> = (0..100).map(|i| (i as u64, vec![i as f32; 4])).collect();
    let refs: Vec<(u64, &[f32])> = vecs.iter().map(|(id, v)| (*id, v.as_slice())).collect();

    let result = selective_safety_net_scan(&query, 10, &[], &refs, &SafetyNetBudget::DISABLED, 100);
    assert!(result.candidates.is_empty());
    assert!(!result.budget_exhausted);
    assert_eq!(result.budget_report.distance_ops, 0);
}

#[test]
fn prefer_quality_extends_budget_4x() {
    let base = SafetyNetBudget::LAYER_A;
    let extended = base.extended_4x();
    assert_eq!(extended.max_scan_time_us, base.max_scan_time_us * 4);
    assert_eq!(extended.max_scan_candidates, base.max_scan_candidates * 4);
    assert_eq!(extended.max_distance_ops, base.max_distance_ops * 4);
}

// ========================================================================
// §3 Adversarial Distribution Detection
// ========================================================================

#[test]
fn degenerate_detection_uniform() {
    let distances = vec![1.0; 100];
    assert!(is_degenerate_distribution(&distances, 10));
}

#[test]
fn degenerate_detection_natural() {
    let distances: Vec<f32> = (0..100).map(|i| i as f32 * 0.5).collect();
    assert!(!is_degenerate_distribution(&distances, 10));
}

#[test]
fn adaptive_nprobe_widens_on_degenerate() {
    let distances = vec![1.0; 1000];
    let result = adaptive_n_probe(4, &distances, 1000);
    assert!(result > 4, "should widen from 4, got {result}");
    assert!(result <= 16, "should cap at 4x base");
}

#[test]
fn adaptive_nprobe_no_change_natural() {
    let distances: Vec<f32> = (0..100).map(|i| i as f32).collect();
    let result = adaptive_n_probe(10, &distances, 100);
    assert_eq!(result, 10);
}

#[test]
fn epoch_drift_widening() {
    // No drift.
    assert_eq!(effective_n_probe_with_drift(10, 0, 64), 10);
    // Half drift.
    assert_eq!(effective_n_probe_with_drift(10, 32, 64), 10);
    // Beyond max drift: double.
    assert_eq!(effective_n_probe_with_drift(10, 100, 64), 20);
}

#[test]
fn combined_nprobe_takes_max() {
    let distances: Vec<f32> = (0..100).map(|i| i as f32).collect();
    let (result, degenerate) = combined_effective_n_probe(10, &distances, 100, 100, 64);
    assert_eq!(result, 20); // Drift dominates.
    assert!(!degenerate);
}

#[test]
fn cv_threshold_consistent() {
    assert!(DEGENERATE_CV_THRESHOLD > 0.0);
    assert!(DEGENERATE_CV_THRESHOLD < 1.0);
    // Uniform distances should be below threshold.
    let cv = centroid_distance_cv(&vec![1.0; 100], 10);
    assert!(cv < DEGENERATE_CV_THRESHOLD);
}

// ========================================================================
// §4 Security Policy
// ========================================================================

#[test]
fn security_policy_default_is_strict() {
    assert_eq!(SecurityPolicy::default(), SecurityPolicy::Strict);
}

#[test]
fn security_policy_methods() {
    assert!(!SecurityPolicy::Permissive.requires_signature());
    assert!(SecurityPolicy::Strict.requires_signature());
    assert!(SecurityPolicy::Paranoid.requires_signature());

    assert!(!SecurityPolicy::Permissive.verifies_content_hashes());
    assert!(SecurityPolicy::WarnOnly.verifies_content_hashes());
    assert!(SecurityPolicy::Strict.verifies_content_hashes());

    assert!(!SecurityPolicy::Strict.verifies_level1());
    assert!(SecurityPolicy::Paranoid.verifies_level1());
}

#[test]
fn security_error_stable_display() {
    let err = SecurityError::UnsignedManifest {
        manifest_offset: 0x1000,
    };
    let s = format!("{err}");
    assert!(s.contains("unsigned manifest"));
    assert!(s.contains("1000"));

    let err = SecurityError::ContentHashMismatch {
        pointer_name: "centroid",
        expected_hash: [0xAA; 16],
        actual_hash: [0xBB; 16],
        seg_offset: 0x2000,
    };
    let s = format!("{err}");
    assert!(s.contains("centroid"));
}

// ========================================================================
// §5 Content-Addressed Centroid Stability (HardeningFields)
// ========================================================================

#[test]
fn hardening_fields_round_trip() {
    let fields = HardeningFields {
        entrypoint_content_hash: [0x11; 16],
        toplayer_content_hash: [0x22; 16],
        centroid_content_hash: [0x33; 16],
        quantdict_content_hash: [0x44; 16],
        hot_cache_content_hash: [0x55; 16],
        centroid_epoch: 42,
        max_epoch_drift: 64,
        reserved: [0u8; 8],
    };

    let bytes = fields.to_bytes();
    let decoded = HardeningFields::from_bytes(&bytes);
    assert_eq!(fields, decoded);
}

#[test]
fn hardening_fields_epoch_drift() {
    let fields = HardeningFields {
        centroid_epoch: 10,
        max_epoch_drift: 64,
        ..HardeningFields::zeroed()
    };
    assert_eq!(fields.epoch_drift(50), 40);
    assert!(!fields.is_epoch_drift_exceeded(50));
    assert!(fields.is_epoch_drift_exceeded(100));
}

#[test]
fn hardening_fields_pointer_lookup() {
    let mut fields = HardeningFields::zeroed();
    fields.centroid_content_hash = [0xAB; 16];
    assert_eq!(fields.hash_for_pointer("centroid"), Some(&[0xAB; 16]));
    assert_eq!(fields.hash_for_pointer("nonexistent"), None);
}

#[test]
fn hardening_fields_fits_in_reserved() {
    assert!(HardeningFields::RESERVED_OFFSET + 96 <= 252);
}

// ========================================================================
// §6 DoS Hardening
// ========================================================================

#[test]
fn budget_token_bucket_basic() {
    let mut bucket = BudgetTokenBucket::new(100, Duration::from_secs(60));
    assert_eq!(bucket.remaining(), 100);
    assert_eq!(bucket.try_consume(30), Ok(70));
    assert_eq!(bucket.try_consume(70), Ok(0));
    assert!(bucket.try_consume(1).is_err());
}

#[test]
fn negative_cache_blacklists_repeated_degenerate() {
    let mut cache = NegativeCache::new(3, Duration::from_secs(60), 1000);
    let sig = QuerySignature::from_query(&[0.1, 0.2, 0.3]);

    assert!(!cache.record_degenerate(sig));
    assert!(!cache.record_degenerate(sig));
    assert!(cache.record_degenerate(sig)); // 3rd hit = blacklisted.
    assert!(cache.is_blacklisted(&sig));
}

#[test]
fn negative_cache_max_size_enforced() {
    let mut cache = NegativeCache::new(100, Duration::from_secs(60), 5);
    for i in 0..20 {
        let sig = QuerySignature::from_query(&[i as f32]);
        cache.record_degenerate(sig);
    }
    assert!(cache.len() <= 5);
}

#[test]
fn proof_of_work_solve_and_verify() {
    let pow = ProofOfWork {
        challenge: [0x42; 16],
        difficulty: 4,
    };
    let nonce = pow.solve().expect("d=4 should solve quickly");
    assert!(pow.verify(nonce));
}

#[test]
fn query_signature_deterministic() {
    let q = vec![0.1, 0.2, 0.3];
    assert_eq!(
        QuerySignature::from_query(&q),
        QuerySignature::from_query(&q)
    );
}

// ========================================================================
// §7 Error Code Completeness
// ========================================================================

#[test]
fn new_error_codes_have_correct_categories() {
    assert_eq!(ErrorCode::UnsignedManifest.category(), 0x08);
    assert_eq!(ErrorCode::ContentHashMismatch.category(), 0x08);
    assert_eq!(ErrorCode::UnknownSigner.category(), 0x08);
    assert_eq!(ErrorCode::EpochDriftExceeded.category(), 0x08);
    assert_eq!(ErrorCode::Level1InvalidSignature.category(), 0x08);

    assert_eq!(ErrorCode::QualityBelowThreshold.category(), 0x09);
    assert_eq!(ErrorCode::BudgetTokensExhausted.category(), 0x09);
    assert_eq!(ErrorCode::QueryBlacklisted.category(), 0x09);
}

#[test]
fn security_error_codes_are_security() {
    assert!(ErrorCode::UnsignedManifest.is_security_error());
    assert!(ErrorCode::ContentHashMismatch.is_security_error());
    assert!(!ErrorCode::Ok.is_security_error());
}

#[test]
fn quality_error_codes_are_quality() {
    assert!(ErrorCode::QualityBelowThreshold.is_quality_error());
    assert!(ErrorCode::BudgetTokensExhausted.is_quality_error());
    assert!(!ErrorCode::Ok.is_quality_error());
}

// ========================================================================
// §8 Safety Net Activation Logic
// ========================================================================

#[test]
fn safety_net_activates_correctly() {
    assert!(should_activate_safety_net(0, 5));
    assert!(should_activate_safety_net(5, 5));
    assert!(should_activate_safety_net(9, 5));
    assert!(!should_activate_safety_net(10, 5));
    assert!(!should_activate_safety_net(100, 5));
}

// ========================================================================
// §9 QualityPreference Behavior
// ========================================================================

#[test]
fn prefer_latency_disables_safety_net() {
    let (_dir, store) = create_test_store(4, 5);
    let query = vec![0.1, 0.2, 0.3, 0.4];

    let opts = QueryOptions {
        quality_preference: QualityPreference::PreferLatency,
        ..QueryOptions::default()
    };

    // PreferLatency should not trigger safety net scan.
    let envelope = store.query_with_envelope(&query, 3, &opts).unwrap();
    assert_eq!(envelope.budgets.safety_net_scan_us, 0);
}

// ========================================================================
// §10 Derive Response Quality
// ========================================================================

#[test]
fn derive_quality_from_mixed() {
    let q =
        derive_response_quality(&[RetrievalQuality::Full, RetrievalQuality::BruteForceBudgeted]);
    assert_eq!(q, ResponseQuality::Degraded);
}

#[test]
fn derive_quality_all_full() {
    let q = derive_response_quality(&[RetrievalQuality::Full, RetrievalQuality::Full]);
    assert_eq!(q, ResponseQuality::Verified);
}

#[test]
fn derive_quality_empty_is_unreliable() {
    let q = derive_response_quality(&[]);
    assert_eq!(q, ResponseQuality::Unreliable);
}
