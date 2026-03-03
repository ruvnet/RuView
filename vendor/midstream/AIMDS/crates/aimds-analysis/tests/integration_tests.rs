//! Integration tests for AIMDS analysis layer

use aimds_analysis::*;
use aimds_core::types::PromptInput;
use std::collections::HashMap;

#[tokio::test]
async fn test_behavioral_analysis_performance() {
    let analyzer = BehavioralAnalyzer::new(10).unwrap();

    // Generate test sequence
    let sequence: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.1).sin()).collect();

    let start = std::time::Instant::now();
    let score = analyzer.analyze_behavior(&sequence).await.unwrap();
    let duration = start.elapsed();

    // Should complete in <100ms (target: 87ms + overhead)
    assert!(duration.as_millis() < 100, "Duration: {:?}", duration);

    // Without baseline, should be normal
    assert!(!score.is_anomalous);
}

#[tokio::test]
async fn test_baseline_training_and_detection() {
    let analyzer = BehavioralAnalyzer::new(5).unwrap();

    // Train with normal patterns (need at least 100 points = 5 dimensions * 100 rows)
    let training_sequences: Vec<Vec<f64>> = (0..5)
        .map(|i| {
            (0..500).map(|j| ((i + j) as f64 * 0.1).sin()).collect()
        })
        .collect();

    analyzer.train_baseline(training_sequences).await.unwrap();
    assert_eq!(analyzer.baseline_count(), 5);

    // Test with similar pattern (should be normal)
    let normal_sequence: Vec<f64> = (0..500).map(|i| (i as f64 * 0.1).sin()).collect();
    let normal_score = analyzer.analyze_behavior(&normal_sequence).await.unwrap();

    // Test with anomalous pattern
    let anomalous_sequence: Vec<f64> = (0..500).map(|i| {
        if i % 20 < 10 {
            (i as f64 * 0.1).sin()
        } else {
            (i as f64 * 0.1).sin() * 10.0 // Spike
        }
    }).collect();
    let anomalous_score = analyzer.analyze_behavior(&anomalous_sequence).await.unwrap();

    // Anomalous should have higher score
    assert!(anomalous_score.score >= normal_score.score);
}

#[tokio::test]
async fn test_policy_verification() {
    let mut verifier = PolicyVerifier::new().unwrap();

    // Add security policies
    let auth_policy = SecurityPolicy::new(
        "auth_required",
        "All actions must be authenticated",
        "G authenticated"
    ).with_severity(0.9);

    verifier.add_policy(auth_policy);

    assert_eq!(verifier.policy_count(), 1);
    assert_eq!(verifier.enabled_count(), 1);

    // Create test prompt input
    let input = PromptInput::new("test prompt".to_string());

    let start = std::time::Instant::now();
    let result = verifier.verify_policy(&input).await.unwrap();
    let duration = start.elapsed();

    // Should complete in <500ms (target: 423ms + overhead)
    assert!(duration.as_millis() < 500, "Duration: {:?}", duration);

    // With empty policies or simplified check, should pass
    assert!(result.verified);
}

#[tokio::test]
async fn test_ltl_checker_globally() {
    let checker = LTLChecker::new();
    let mut trace = Trace::new();

    // All states have "safe" property
    for _i in 0..10 {
        let mut props = HashMap::new();
        props.insert("safe".to_string(), true);
        trace.add_state(props);
    }

    let formula = LTLFormula::parse("G safe").unwrap();
    assert!(checker.check_formula(&formula, &trace));
}

#[tokio::test]
async fn test_ltl_checker_finally() {
    let checker = LTLChecker::new();
    let mut trace = Trace::new();

    // Eventually "goal" is reached
    for i in 0..10 {
        let mut props = HashMap::new();
        props.insert("goal".to_string(), i == 5);
        trace.add_state(props);
    }

    let formula = LTLFormula::parse("F goal").unwrap();
    assert!(checker.check_formula(&formula, &trace));
}

#[tokio::test]
async fn test_ltl_counterexample() {
    let checker = LTLChecker::new();
    let mut trace = Trace::new();

    // Not all states are "safe"
    for i in 0..5 {
        let mut props = HashMap::new();
        props.insert("safe".to_string(), i < 3);
        trace.add_state(props);
    }

    let formula = LTLFormula::parse("G safe").unwrap();
    assert!(!checker.check_formula(&formula, &trace));

    // Should generate counterexample
    let counterexample = checker.generate_counterexample(&formula, &trace);
    assert!(counterexample.is_some());
}

#[tokio::test]
async fn test_full_analysis_performance() {
    let engine = AnalysisEngine::new(10).unwrap();

    // Test sequence
    let sequence: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.1).sin()).collect();
    let input = PromptInput::new("test input".to_string());

    let start = std::time::Instant::now();
    let result = engine.analyze_full(&sequence, &input).await.unwrap();
    let duration = start.elapsed();

    // Combined analysis should complete in <520ms
    assert!(duration.as_millis() < 520, "Duration: {:?}", duration);
    // Duration should be approximately equal (within 10ms)
    assert!((result.duration.as_millis() as i64 - duration.as_millis() as i64).abs() < 10,
            "Result duration: {:?}, actual duration: {:?}", result.duration, duration);
}

#[tokio::test]
async fn test_threat_level_calculation() {
    // Create anomalous result
    let full_analysis = FullAnalysis {
        behavior: AnomalyScore {
            score: 0.8,
            is_anomalous: true,
            confidence: 0.95,
        },
        policy: VerificationResult {
            verified: false,
            confidence: 0.9,
            violations: vec!["unauthorized".to_string()],
            proof: None,
        },
        duration: std::time::Duration::from_millis(150),
    };

    assert!(full_analysis.is_threat());

    let threat_level = full_analysis.threat_level();
    assert!(threat_level > 0.6, "Threat level: {}", threat_level);
    assert!(threat_level <= 1.0, "Threat level: {}", threat_level);
}

#[tokio::test]
async fn test_safe_analysis() {
    // Create safe result
    let full_analysis = FullAnalysis {
        behavior: AnomalyScore {
            score: 0.1,
            is_anomalous: false,
            confidence: 0.95,
        },
        policy: VerificationResult {
            verified: true,
            confidence: 0.99,
            violations: Vec::new(),
            proof: None,
        },
        duration: std::time::Duration::from_millis(80),
    };

    assert!(!full_analysis.is_threat());

    let threat_level = full_analysis.threat_level();
    assert_eq!(threat_level, 0.0, "Threat level should be 0 for safe analysis");
}

#[tokio::test]
async fn test_policy_enable_disable() {
    let mut verifier = PolicyVerifier::new().unwrap();

    let policy = SecurityPolicy::new(
        "test_policy",
        "Test policy",
        "G true"
    );

    verifier.add_policy(policy);
    assert_eq!(verifier.enabled_count(), 1);

    verifier.disable_policy("test_policy").unwrap();
    assert_eq!(verifier.enabled_count(), 0);

    verifier.enable_policy("test_policy").unwrap();
    assert_eq!(verifier.enabled_count(), 1);
}

#[tokio::test]
async fn test_threshold_adjustment() {
    let analyzer = BehavioralAnalyzer::new(10).unwrap();

    assert!((analyzer.threshold() - 0.75).abs() < 1e-6);

    analyzer.set_threshold(0.9);
    assert!((analyzer.threshold() - 0.9).abs() < 1e-6);

    // Threshold should be clamped to [0, 1]
    analyzer.set_threshold(1.5);
    assert!((analyzer.threshold() - 1.0).abs() < 1e-6);

    analyzer.set_threshold(-0.5);
    assert!((analyzer.threshold() - 0.0).abs() < 1e-6);
}

#[tokio::test]
async fn test_multiple_sequential_analyses() {
    let engine = AnalysisEngine::new(10).unwrap();

    // Run multiple analyses sequentially
    for i in 0..5 {
        let sequence: Vec<f64> = (0..1000).map(|j| ((i + j) as f64 * 0.1).sin()).collect();
        let input = PromptInput::new(format!("test {}", i));

        let result = engine.analyze_full(&sequence, &input).await;
        assert!(result.is_ok());
    }
}
