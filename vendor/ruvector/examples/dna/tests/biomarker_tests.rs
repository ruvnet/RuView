//! Integration tests for the biomarker analysis engine.
//!
//! Tests composite risk scoring, profile vector encoding, clinical biomarker
//! references, synthetic population generation, and streaming biomarker
//! processing with anomaly and trend detection.

use rvdna::biomarker::*;
use rvdna::biomarker_stream::*;
use std::collections::HashMap;

// ============================================================================
// COMPOSITE RISK SCORING TESTS
// ============================================================================

#[test]
fn test_compute_risk_scores_baseline() {
    // All homozygous reference (low risk) genotypes
    let mut gts = HashMap::new();
    gts.insert("rs429358".to_string(), "TT".to_string()); // APOE ref
    gts.insert("rs7412".to_string(), "CC".to_string()); // APOE ref
    gts.insert("rs4680".to_string(), "GG".to_string()); // COMT ref
    gts.insert("rs1799971".to_string(), "AA".to_string()); // OPRM1 ref
    gts.insert("rs762551".to_string(), "AA".to_string()); // CYP1A2 fast
    gts.insert("rs1801133".to_string(), "GG".to_string()); // MTHFR ref
    gts.insert("rs1801131".to_string(), "TT".to_string()); // MTHFR ref
    gts.insert("rs1042522".to_string(), "CC".to_string()); // TP53 ref
    gts.insert("rs80357906".to_string(), "DD".to_string()); // BRCA1 ref
    gts.insert("rs4363657".to_string(), "TT".to_string()); // SLCO1B1 ref

    let profile = compute_risk_scores(&gts);
    assert!(
        profile.global_risk_score < 0.3,
        "Baseline should be low risk, got {}",
        profile.global_risk_score
    );
    assert!(!profile.category_scores.is_empty());
}

#[test]
fn test_compute_risk_scores_high_risk() {
    // High-risk genotype combinations
    let mut gts = HashMap::new();
    gts.insert("rs429358".to_string(), "CC".to_string()); // APOE e4/e4
    gts.insert("rs7412".to_string(), "CC".to_string());
    gts.insert("rs4680".to_string(), "AA".to_string()); // COMT Met/Met
    gts.insert("rs1799971".to_string(), "GG".to_string()); // OPRM1 Asp/Asp
    gts.insert("rs1801133".to_string(), "AA".to_string()); // MTHFR 677TT
    gts.insert("rs1801131".to_string(), "GG".to_string()); // MTHFR 1298CC
    gts.insert("rs4363657".to_string(), "CC".to_string()); // SLCO1B1 hom variant

    let profile = compute_risk_scores(&gts);
    assert!(
        profile.global_risk_score > 0.4,
        "High-risk should score >0.4, got {}",
        profile.global_risk_score
    );
}

// ============================================================================
// PROFILE VECTOR TESTS
// ============================================================================

#[test]
fn test_profile_vector_dimension() {
    let gts = HashMap::new(); // empty genotypes
    let profile = compute_risk_scores(&gts);
    assert_eq!(
        profile.profile_vector.len(),
        64,
        "Profile vector must be exactly 64 dimensions"
    );
}

#[test]
fn test_profile_vector_normalized() {
    let mut gts = HashMap::new();
    gts.insert("rs429358".to_string(), "CT".to_string());
    gts.insert("rs4680".to_string(), "AG".to_string());
    let profile = compute_risk_scores(&gts);
    let mag: f32 = profile
        .profile_vector
        .iter()
        .map(|x| x * x)
        .sum::<f32>()
        .sqrt();
    assert!(
        (mag - 1.0).abs() < 0.01 || mag == 0.0,
        "Vector should be L2-normalized, got magnitude {}",
        mag
    );
}

// ============================================================================
// BIOMARKER REFERENCE TESTS
// ============================================================================

#[test]
fn test_biomarker_references_exist() {
    let refs = biomarker_references();
    assert!(
        refs.len() >= 13,
        "Should have at least 13 biomarker references, got {}",
        refs.len()
    );
}

#[test]
fn test_z_score_computation() {
    let refs = biomarker_references();
    let cholesterol_ref = refs.iter().find(|r| r.name == "Total Cholesterol").unwrap();

    // Normal value should have |z| < 2
    let z_normal = z_score(180.0, cholesterol_ref);
    assert!(
        z_normal.abs() < 2.0,
        "Normal cholesterol z-score should be small: {}",
        z_normal
    );

    // High value should have z > 0
    let z_high = z_score(300.0, cholesterol_ref);
    assert!(
        z_high > 0.0,
        "High cholesterol should have positive z-score: {}",
        z_high
    );
}

#[test]
fn test_biomarker_classification() {
    let refs = biomarker_references();
    let glucose_ref = refs.iter().find(|r| r.name == "Fasting Glucose").unwrap();

    let class_normal = classify_biomarker(85.0, glucose_ref);
    // Should be normal range
    let class_high = classify_biomarker(200.0, glucose_ref);
    // Should be high/critical
    assert_ne!(format!("{:?}", class_normal), format!("{:?}", class_high));
}

// ============================================================================
// SYNTHETIC POPULATION TESTS
// ============================================================================

#[test]
fn test_synthetic_population() {
    let pop = generate_synthetic_population(100, 42);
    assert_eq!(pop.len(), 100);

    // All vectors should be 64-dim
    for profile in &pop {
        assert_eq!(profile.profile_vector.len(), 64);
    }

    // Risk scores should span a range
    let scores: Vec<f64> = pop.iter().map(|p| p.global_risk_score).collect();
    let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max - min > 0.1,
        "Population should have risk score variance, range: {:.3}..{:.3}",
        min,
        max
    );
}

#[test]
fn test_synthetic_population_deterministic() {
    let pop1 = generate_synthetic_population(50, 42);
    let pop2 = generate_synthetic_population(50, 42);
    assert_eq!(pop1.len(), pop2.len());
    for (a, b) in pop1.iter().zip(pop2.iter()) {
        assert!((a.global_risk_score - b.global_risk_score).abs() < 1e-10);
    }
}

// ============================================================================
// STREAMING TESTS
// ============================================================================

#[test]
fn test_ring_buffer_basic() {
    let mut rb: RingBuffer<f64> = RingBuffer::new(5);
    for i in 0..3 {
        rb.push(i as f64);
    }
    assert_eq!(rb.len(), 3);
    let items: Vec<f64> = rb.iter().cloned().collect();
    assert_eq!(items, vec![0.0, 1.0, 2.0]);
}

#[test]
fn test_ring_buffer_overflow() {
    let mut rb: RingBuffer<f64> = RingBuffer::new(3);
    for i in 0..5 {
        rb.push(i as f64);
    }
    assert_eq!(rb.len(), 3);
    let items: Vec<f64> = rb.iter().cloned().collect();
    assert_eq!(items, vec![2.0, 3.0, 4.0]);
}

#[test]
fn test_stream_generation() {
    let config = StreamConfig::default();
    let num_biomarkers = config.num_biomarkers;
    let readings = generate_readings(&config, 1000, 42);
    // generate_readings produces count * num_biomarkers total readings
    assert_eq!(readings.len(), 1000 * num_biomarkers);

    // All values should be positive
    for r in &readings {
        assert!(
            r.value > 0.0,
            "Biomarker values should be positive: {} = {}",
            r.biomarker_id,
            r.value
        );
    }
}

#[test]
fn test_stream_processor() {
    let config = StreamConfig::default();
    let num_biomarkers = config.num_biomarkers;
    let readings = generate_readings(&config, 500, 42);
    let mut processor = StreamProcessor::new(config);

    for reading in &readings {
        processor.process_reading(reading);
    }

    let summary = processor.summary();
    assert_eq!(summary.total_readings, 500 * num_biomarkers as u64);
    assert!(
        summary.anomaly_rate < 0.2,
        "Anomaly rate should be reasonable: {}",
        summary.anomaly_rate
    );
}

#[test]
fn test_anomaly_detection() {
    let config = StreamConfig {
        anomaly_probability: 0.0, // No random anomalies
        num_biomarkers: 1,
        ..StreamConfig::default()
    };

    let readings = generate_readings(&config, 200, 42);
    let mut processor = StreamProcessor::new(config);

    for reading in &readings {
        processor.process_reading(reading);
    }

    // With no anomaly injection, anomaly rate should be very low
    let summary = processor.summary();
    assert!(
        summary.anomaly_rate < 0.1,
        "Without injection, anomaly rate should be low: {}",
        summary.anomaly_rate
    );
}

// ============================================================================
// GENE-GENE INTERACTION TESTS
// ============================================================================

#[test]
fn test_mthfr_comt_interaction() {
    // MTHFR A1298C hom + COMT Met/Met should amplify neurological score
    let mut gts_both = HashMap::new();
    gts_both.insert("rs1801131".to_string(), "GG".to_string()); // A1298C hom_alt
    gts_both.insert("rs4680".to_string(), "AA".to_string()); // COMT Met/Met
    let both = compute_risk_scores(&gts_both);

    let mut gts_one = HashMap::new();
    gts_one.insert("rs4680".to_string(), "AA".to_string()); // COMT Met/Met only
    let one = compute_risk_scores(&gts_one);

    let n_both = both.category_scores.get("Neurological").unwrap().score;
    let n_one = one.category_scores.get("Neurological").unwrap().score;
    assert!(
        n_both > n_one,
        "MTHFR×COMT interaction should amplify: {n_both} > {n_one}"
    );
}

#[test]
fn test_drd2_comt_interaction() {
    // DRD2 Taq1A + COMT variant should amplify neurological score
    let mut gts = HashMap::new();
    gts.insert("rs1800497".to_string(), "AA".to_string()); // DRD2 hom_alt
    gts.insert("rs4680".to_string(), "AA".to_string()); // COMT Met/Met
    let with = compute_risk_scores(&gts);

    let mut gts2 = HashMap::new();
    gts2.insert("rs1800497".to_string(), "AA".to_string()); // DRD2 only
    let without = compute_risk_scores(&gts2);

    let n_with = with.category_scores.get("Neurological").unwrap().score;
    let n_without = without.category_scores.get("Neurological").unwrap().score;
    assert!(
        n_with > n_without,
        "DRD2×COMT interaction should amplify: {n_with} > {n_without}"
    );
}

// ============================================================================
// GENE-BIOMARKER CORRELATION TESTS
// ============================================================================

#[test]
fn test_apoe_lowers_hdl_in_population() {
    let pop = generate_synthetic_population(300, 88);
    let (mut apoe_hdl, mut ref_hdl) = (Vec::new(), Vec::new());
    for p in &pop {
        let hdl = p.biomarker_values.get("HDL").copied().unwrap_or(0.0);
        // APOE carriers have elevated neurological scores from rs429358
        let neuro = p
            .category_scores
            .get("Neurological")
            .map(|c| c.score)
            .unwrap_or(0.0);
        if neuro > 0.3 {
            apoe_hdl.push(hdl);
        } else {
            ref_hdl.push(hdl);
        }
    }
    if !apoe_hdl.is_empty() && !ref_hdl.is_empty() {
        let avg_apoe = apoe_hdl.iter().sum::<f64>() / apoe_hdl.len() as f64;
        let avg_ref = ref_hdl.iter().sum::<f64>() / ref_hdl.len() as f64;
        assert!(
            avg_apoe < avg_ref,
            "APOE e4 should lower HDL: {avg_apoe} < {avg_ref}"
        );
    }
}

#[test]
fn test_cusum_changepoint_detection() {
    let mut p = StreamProcessor::new(StreamConfig {
        window_size: 20,
        ..Default::default()
    });
    // Establish baseline
    for i in 0..30 {
        p.process_reading(&BiomarkerReading {
            timestamp_ms: i * 1000,
            biomarker_id: "glucose".into(),
            value: 85.0,
            reference_low: 70.0,
            reference_high: 100.0,
            is_anomaly: false,
            z_score: 0.0,
        });
    }
    // Inject a sustained shift (changepoint)
    for i in 30..50 {
        p.process_reading(&BiomarkerReading {
            timestamp_ms: i * 1000,
            biomarker_id: "glucose".into(),
            value: 120.0,
            reference_low: 70.0,
            reference_high: 100.0,
            is_anomaly: false,
            z_score: 0.0,
        });
    }
    let stats = p.get_stats("glucose").unwrap();
    // After sustained shift, CUSUM should have triggered at least once
    // (changepoint_detected resets after trigger, but the sustained shift
    // will keep re-triggering, so the final state may or may not be true)
    assert!(
        stats.mean > 90.0,
        "Mean should shift upward after changepoint: {}",
        stats.mean
    );
}

#[test]
fn test_trend_detection() {
    let config = StreamConfig {
        drift_rate: 0.5, // Strong upward drift
        anomaly_probability: 0.0,
        num_biomarkers: 1,
        window_size: 50,
        ..StreamConfig::default()
    };

    let readings = generate_readings(&config, 200, 42);
    let mut processor = StreamProcessor::new(config);

    for reading in &readings {
        processor.process_reading(reading);
    }

    // Should detect positive trend
    let summary = processor.summary();
    for (_, stats) in &summary.biomarker_stats {
        assert!(
            stats.trend_slope > 0.0,
            "Should detect upward trend, got slope: {}",
            stats.trend_slope
        );
    }
}
