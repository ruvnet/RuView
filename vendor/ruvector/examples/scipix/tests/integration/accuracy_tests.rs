// Accuracy validation tests
//
// Tests OCR accuracy against Im2latex-100k subset and calculates CER, WER, BLEU

use super::*;
use tokio;

#[tokio::test]
async fn test_accuracy_simple_expressions() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    let test_cases = vec![
        ("x + 1", "x + 1"),
        ("2x - 3", "2x - 3"),
        ("a = b", "a = b"),
        ("f(x)", "f(x)"),
        ("y^2", "y^2"),
    ];

    let mut total_cer = 0.0;
    let mut correct = 0;

    for (equation, expected) in test_cases.iter() {
        let image = images::generate_simple_equation(equation);
        let path = format!("/tmp/accuracy_simple_{}.png", equation.replace(' ', "_"));
        image.save(&path).unwrap();

        let result = test_server
            .process_image(&path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        let cer = metrics::calculate_cer(expected, &result.latex);
        total_cer += cer;

        if latex::normalize(&result.latex) == latex::normalize(expected) {
            correct += 1;
        }

        println!(
            "Equation: {} | CER: {:.4} | Got: {}",
            equation, cer, result.latex
        );
    }

    let avg_cer = total_cer / test_cases.len() as f64;
    let accuracy = correct as f64 / test_cases.len() as f64;

    println!(
        "Simple expressions - Avg CER: {:.4}, Accuracy: {:.2}%",
        avg_cer,
        accuracy * 100.0
    );

    assert!(avg_cer < 0.05, "Average CER too high: {:.4}", avg_cer);
    assert!(
        accuracy > 0.90,
        "Accuracy too low: {:.2}%",
        accuracy * 100.0
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_accuracy_im2latex_subset() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Load Im2latex-100k test subset (sample)
    let test_cases = load_im2latex_test_subset(50); // Test 50 samples

    let mut cer_sum = 0.0;
    let mut wer_sum = 0.0;
    let mut bleu_sum = 0.0;
    let mut exact_matches = 0;

    for (i, case) in test_cases.iter().enumerate() {
        // Generate or load image
        let image_path = case.image_path.clone();

        let result = test_server
            .process_image(&image_path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        // Calculate metrics
        let cer = metrics::calculate_cer(&case.ground_truth, &result.latex);
        let wer = metrics::calculate_wer(&case.ground_truth, &result.latex);
        let bleu = metrics::calculate_bleu(&case.ground_truth, &result.latex, 4);

        cer_sum += cer;
        wer_sum += wer;
        bleu_sum += bleu;

        if latex::normalize(&result.latex) == latex::normalize(&case.ground_truth) {
            exact_matches += 1;
        }

        if i % 10 == 0 {
            println!("Processed {}/{} samples", i + 1, test_cases.len());
        }
    }

    let count = test_cases.len() as f64;
    let avg_cer = cer_sum / count;
    let avg_wer = wer_sum / count;
    let avg_bleu = bleu_sum / count;
    let exact_match_rate = exact_matches as f64 / count;

    println!("\nIm2latex subset results:");
    println!("  Average CER: {:.4}", avg_cer);
    println!("  Average WER: {:.4}", avg_wer);
    println!("  Average BLEU: {:.2}", avg_bleu);
    println!("  Exact match rate: {:.2}%", exact_match_rate * 100.0);

    // Assert quality thresholds
    assert!(avg_cer < 0.03, "CER too high: {:.4}", avg_cer);
    assert!(avg_bleu > 80.0, "BLEU too low: {:.2}", avg_bleu);

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_accuracy_fractions() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    let test_cases = vec![
        ((1, 2), r"\frac{1}{2}"),
        ((3, 4), r"\frac{3}{4}"),
        ((5, 6), r"\frac{5}{6}"),
        ((10, 3), r"\frac{10}{3}"),
    ];

    let mut correct = 0;

    for ((num, den), expected) in test_cases.iter() {
        let image = images::generate_fraction(*num, *den);
        let path = format!("/tmp/frac_{}_{}.png", num, den);
        image.save(&path).unwrap();

        let result = test_server
            .process_image(&path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        if latex::expressions_match(&result.latex, expected) {
            correct += 1;
        } else {
            println!(
                "Fraction {}/{} - Expected: {}, Got: {}",
                num, den, expected, result.latex
            );
        }
    }

    let accuracy = correct as f64 / test_cases.len() as f64;
    println!("Fraction accuracy: {:.2}%", accuracy * 100.0);

    assert!(
        accuracy >= 0.85,
        "Fraction accuracy too low: {:.2}%",
        accuracy * 100.0
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_accuracy_special_symbols() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    let test_cases = vec![
        (r"\alpha", r"\alpha"),
        (r"\beta", r"\beta"),
        (r"\sum", r"\sum"),
        (r"\int", r"\int"),
        (r"\pi", r"\pi"),
        (r"\infty", r"\infty"),
    ];

    let mut correct = 0;

    for (symbol, expected) in test_cases.iter() {
        let image = images::generate_symbol(symbol);
        let path = format!("/tmp/symbol_{}.png", symbol.replace('\\', ""));
        image.save(&path).unwrap();

        let result = test_server
            .process_image(&path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        if result.latex.contains(expected) {
            correct += 1;
        } else {
            println!(
                "Symbol {} - Expected to contain: {}, Got: {}",
                symbol, expected, result.latex
            );
        }
    }

    let accuracy = correct as f64 / test_cases.len() as f64;
    println!("Special symbol accuracy: {:.2}%", accuracy * 100.0);

    assert!(
        accuracy >= 0.80,
        "Symbol accuracy too low: {:.2}%",
        accuracy * 100.0
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_accuracy_regression_detection() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Load baseline results
    let baseline = load_baseline_results();

    // Run same test cases
    let test_cases = load_regression_test_cases();

    let mut regressions = Vec::new();

    for case in test_cases.iter() {
        let result = test_server
            .process_image(&case.image_path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        // Compare with baseline
        if let Some(baseline_result) = baseline.get(&case.id) {
            let current_cer = metrics::calculate_cer(&case.ground_truth, &result.latex);
            let baseline_cer = baseline_result.cer;

            // Check for regression (10% threshold)
            if current_cer > baseline_cer * 1.10 {
                regressions.push((
                    case.id.clone(),
                    baseline_cer,
                    current_cer,
                    baseline_result.latex.clone(),
                    result.latex.clone(),
                ));
            }
        }
    }

    if !regressions.is_empty() {
        println!("Regressions detected:");
        for (id, baseline_cer, current_cer, baseline_latex, current_latex) in &regressions {
            println!("  {} - CER: {:.4} -> {:.4}", id, baseline_cer, current_cer);
            println!("    Baseline: {}", baseline_latex);
            println!("    Current:  {}", current_latex);
        }
    }

    assert!(
        regressions.is_empty(),
        "Found {} regressions",
        regressions.len()
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_accuracy_confidence_calibration() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    let test_cases = load_calibration_test_cases();

    let mut high_conf_correct = 0;
    let mut high_conf_total = 0;
    let mut low_conf_correct = 0;
    let mut low_conf_total = 0;

    for case in test_cases.iter() {
        let result = test_server
            .process_image(&case.image_path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        let is_correct = latex::normalize(&result.latex) == latex::normalize(&case.ground_truth);

        if result.confidence > 0.9 {
            high_conf_total += 1;
            if is_correct {
                high_conf_correct += 1;
            }
        } else if result.confidence < 0.7 {
            low_conf_total += 1;
            if is_correct {
                low_conf_correct += 1;
            }
        }
    }

    let high_conf_accuracy = if high_conf_total > 0 {
        high_conf_correct as f64 / high_conf_total as f64
    } else {
        1.0
    };

    let low_conf_accuracy = if low_conf_total > 0 {
        low_conf_correct as f64 / low_conf_total as f64
    } else {
        0.0
    };

    println!("Confidence calibration:");
    println!(
        "  High confidence (>0.9): {:.2}% accuracy ({}/{})",
        high_conf_accuracy * 100.0,
        high_conf_correct,
        high_conf_total
    );
    println!(
        "  Low confidence (<0.7): {:.2}% accuracy ({}/{})",
        low_conf_accuracy * 100.0,
        low_conf_correct,
        low_conf_total
    );

    // High confidence should correlate with high accuracy
    assert!(
        high_conf_accuracy > 0.95,
        "High confidence predictions should be very accurate"
    );

    test_server.shutdown().await;
}

// Helper functions and types

#[derive(Debug, Clone)]
struct TestCase {
    id: String,
    image_path: String,
    ground_truth: String,
}

#[derive(Debug, Clone)]
struct BaselineResult {
    latex: String,
    cer: f64,
}

fn load_im2latex_test_subset(count: usize) -> Vec<TestCase> {
    // Load or generate Im2latex test subset
    // For now, generate synthetic test cases
    (0..count)
        .map(|i| {
            let eq = match i % 5 {
                0 => format!("x^{}", i),
                1 => format!("a + {}", i),
                2 => format!(r"\frac{{{}}}{{{}}}", i, i + 1),
                3 => format!("{}x + {}", i, i * 2),
                _ => format!("y = {}x", i),
            };

            let image = images::generate_simple_equation(&eq);
            let path = format!("/tmp/im2latex_{}.png", i);
            image.save(&path).unwrap();

            TestCase {
                id: format!("im2latex_{}", i),
                image_path: path,
                ground_truth: eq,
            }
        })
        .collect()
}

fn load_regression_test_cases() -> Vec<TestCase> {
    // Load regression test cases from file or generate
    vec![
        TestCase {
            id: "reg_001".to_string(),
            image_path: "/tmp/reg_001.png".to_string(),
            ground_truth: "x + y".to_string(),
        },
        // Add more test cases...
    ]
}

fn load_baseline_results() -> std::collections::HashMap<String, BaselineResult> {
    // Load baseline results from file
    let mut baseline = std::collections::HashMap::new();

    baseline.insert(
        "reg_001".to_string(),
        BaselineResult {
            latex: "x + y".to_string(),
            cer: 0.0,
        },
    );

    baseline
}

fn load_calibration_test_cases() -> Vec<TestCase> {
    // Generate test cases with varying difficulty for confidence calibration
    let mut cases = Vec::new();

    // Easy cases
    for i in 0..10 {
        let eq = format!("x + {}", i);
        let image = images::generate_simple_equation(&eq);
        let path = format!("/tmp/calib_easy_{}.png", i);
        image.save(&path).unwrap();

        cases.push(TestCase {
            id: format!("calib_easy_{}", i),
            image_path: path,
            ground_truth: eq,
        });
    }

    // Hard cases (noisy)
    for i in 0..10 {
        let eq = format!("y^{}", i);
        let mut image = images::generate_simple_equation(&eq);
        images::add_noise(&mut image, 0.2);
        let path = format!("/tmp/calib_hard_{}.png", i);
        image.save(&path).unwrap();

        cases.push(TestCase {
            id: format!("calib_hard_{}", i),
            image_path: path,
            ground_truth: eq,
        });
    }

    cases
}
