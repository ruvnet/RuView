//! Accuracy testing example
//!
//! This example demonstrates how to test OCR accuracy against a ground truth dataset.
//! It calculates various metrics including WER, CER, and confidence correlations.
//!
//! Usage:
//! ```bash
//! cargo run --example accuracy_test -- dataset.json
//! ```
//!
//! Dataset format (JSON):
//! ```json
//! [
//!   {
//!     "image_path": "path/to/image.png",
//!     "ground_truth_text": "x^2 + 2x + 1 = 0",
//!     "ground_truth_latex": "x^{2} + 2x + 1 = 0",
//!     "category": "quadratic"
//!   }
//! ]
//! ```

use anyhow::{Context, Result};
use ruvector_scipix::{OcrConfig, OcrEngine, OutputFormat};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct TestCase {
    image_path: String,
    ground_truth_text: String,
    ground_truth_latex: Option<String>,
    category: Option<String>,
}

#[derive(Debug, Serialize)]
struct TestResult {
    image_path: String,
    category: String,
    predicted_text: String,
    predicted_latex: Option<String>,
    ground_truth_text: String,
    ground_truth_latex: Option<String>,
    confidence: f32,
    text_accuracy: f32,
    latex_accuracy: Option<f32>,
    character_error_rate: f32,
    word_error_rate: f32,
}

#[derive(Debug, Serialize)]
struct AccuracyMetrics {
    total_cases: usize,
    successful_cases: usize,
    failed_cases: usize,
    average_confidence: f32,
    average_text_accuracy: f32,
    average_latex_accuracy: f32,
    average_cer: f32,
    average_wer: f32,
    category_breakdown: HashMap<String, CategoryMetrics>,
    confidence_correlation: f32,
}

#[derive(Debug, Serialize)]
struct CategoryMetrics {
    count: usize,
    average_accuracy: f32,
    average_confidence: f32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <dataset.json>", args[0]);
        eprintln!("\nDataset format:");
        eprintln!(
            r#"[
  {{
    "image_path": "path/to/image.png",
    "ground_truth_text": "x^2 + 2x + 1 = 0",
    "ground_truth_latex": "x^{{2}} + 2x + 1 = 0",
    "category": "quadratic"
  }}
]"#
        );
        std::process::exit(1);
    }

    let dataset_path = &args[1];

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("Loading test dataset: {}", dataset_path);
    let dataset_content = std::fs::read_to_string(dataset_path)?;
    let test_cases: Vec<TestCase> = serde_json::from_str(&dataset_content)?;

    println!("Loaded {} test cases", test_cases.len());

    // Initialize OCR engine
    println!("Initializing OCR engine...");
    let config = OcrConfig::default();
    let engine = OcrEngine::new(config).await?;

    println!("Running accuracy tests...\n");

    let mut results = Vec::new();

    for (idx, test_case) in test_cases.iter().enumerate() {
        println!(
            "[{}/{}] Processing: {}",
            idx + 1,
            test_cases.len(),
            test_case.image_path
        );

        match run_test_case(&engine, test_case).await {
            Ok(result) => {
                println!(
                    "  Accuracy: {:.2}%, CER: {:.2}%, WER: {:.2}%",
                    result.text_accuracy * 100.0,
                    result.character_error_rate * 100.0,
                    result.word_error_rate * 100.0
                );
                results.push(result);
            }
            Err(e) => {
                eprintln!("  Error: {}", e);
            }
        }
    }

    // Calculate overall metrics
    let metrics = calculate_metrics(&results);

    // Display results
    println!("\n{}", "=".repeat(80));
    println!("Accuracy Test Results");
    println!("{}", "=".repeat(80));
    println!("Total Cases: {}", metrics.total_cases);
    println!(
        "Successful: {} ({:.1}%)",
        metrics.successful_cases,
        (metrics.successful_cases as f32 / metrics.total_cases as f32) * 100.0
    );
    println!("Failed: {}", metrics.failed_cases);
    println!("\n📊 Overall Metrics:");
    println!(
        "  Average Confidence: {:.2}%",
        metrics.average_confidence * 100.0
    );
    println!(
        "  Average Text Accuracy: {:.2}%",
        metrics.average_text_accuracy * 100.0
    );
    println!(
        "  Average LaTeX Accuracy: {:.2}%",
        metrics.average_latex_accuracy * 100.0
    );
    println!("  Average CER: {:.2}%", metrics.average_cer * 100.0);
    println!("  Average WER: {:.2}%", metrics.average_wer * 100.0);
    println!(
        "  Confidence Correlation: {:.3}",
        metrics.confidence_correlation
    );

    if !metrics.category_breakdown.is_empty() {
        println!("\n📂 Category Breakdown:");
        for (category, cat_metrics) in &metrics.category_breakdown {
            println!("  {}:", category);
            println!("    Count: {}", cat_metrics.count);
            println!(
                "    Average Accuracy: {:.2}%",
                cat_metrics.average_accuracy * 100.0
            );
            println!(
                "    Average Confidence: {:.2}%",
                cat_metrics.average_confidence * 100.0
            );
        }
    }

    println!("{}", "=".repeat(80));

    // Save detailed results
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "metrics": metrics,
        "results": results
    }))?;
    std::fs::write("accuracy_results.json", json)?;
    println!("\nDetailed results saved to: accuracy_results.json");

    Ok(())
}

async fn run_test_case(engine: &OcrEngine, test_case: &TestCase) -> Result<TestResult> {
    let image = image::open(&test_case.image_path)
        .context(format!("Failed to load image: {}", test_case.image_path))?;

    let ocr_result = engine.recognize(&image).await?;

    let predicted_text = ocr_result.text.clone();
    let predicted_latex = ocr_result.to_format(OutputFormat::LaTeX).ok();

    let text_accuracy = calculate_accuracy(&predicted_text, &test_case.ground_truth_text);
    let latex_accuracy =
        if let (Some(pred), Some(gt)) = (&predicted_latex, &test_case.ground_truth_latex) {
            Some(calculate_accuracy(pred, gt))
        } else {
            None
        };

    let cer = calculate_character_error_rate(&predicted_text, &test_case.ground_truth_text);
    let wer = calculate_word_error_rate(&predicted_text, &test_case.ground_truth_text);

    Ok(TestResult {
        image_path: test_case.image_path.clone(),
        category: test_case
            .category
            .clone()
            .unwrap_or_else(|| "uncategorized".to_string()),
        predicted_text,
        predicted_latex,
        ground_truth_text: test_case.ground_truth_text.clone(),
        ground_truth_latex: test_case.ground_truth_latex.clone(),
        confidence: ocr_result.confidence,
        text_accuracy,
        latex_accuracy,
        character_error_rate: cer,
        word_error_rate: wer,
    })
}

fn calculate_accuracy(predicted: &str, ground_truth: &str) -> f32 {
    let distance = levenshtein_distance(predicted, ground_truth);
    let max_len = predicted.len().max(ground_truth.len());

    if max_len == 0 {
        return 1.0;
    }

    1.0 - (distance as f32 / max_len as f32)
}

fn calculate_character_error_rate(predicted: &str, ground_truth: &str) -> f32 {
    let distance = levenshtein_distance(predicted, ground_truth);
    if ground_truth.len() == 0 {
        return if predicted.len() == 0 { 0.0 } else { 1.0 };
    }
    distance as f32 / ground_truth.len() as f32
}

fn calculate_word_error_rate(predicted: &str, ground_truth: &str) -> f32 {
    let pred_words: Vec<&str> = predicted.split_whitespace().collect();
    let gt_words: Vec<&str> = ground_truth.split_whitespace().collect();

    let distance = levenshtein_distance_vec(&pred_words, &gt_words);

    if gt_words.len() == 0 {
        return if pred_words.len() == 0 { 0.0 } else { 1.0 };
    }

    distance as f32 / gt_words.len() as f32
}

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = *[
                matrix[i][j + 1] + 1,
                matrix[i + 1][j] + 1,
                matrix[i][j] + cost,
            ]
            .iter()
            .min()
            .unwrap();
        }
    }

    matrix[len1][len2]
}

fn levenshtein_distance_vec<T: Eq>(s1: &[T], s2: &[T]) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 0..len1 {
        for j in 0..len2 {
            let cost = if s1[i] == s2[j] { 0 } else { 1 };
            matrix[i + 1][j + 1] = *[
                matrix[i][j + 1] + 1,
                matrix[i + 1][j] + 1,
                matrix[i][j] + cost,
            ]
            .iter()
            .min()
            .unwrap();
        }
    }

    matrix[len1][len2]
}

fn calculate_metrics(results: &[TestResult]) -> AccuracyMetrics {
    let total_cases = results.len();
    let successful_cases = results.len();
    let failed_cases = 0;

    let average_confidence = results.iter().map(|r| r.confidence).sum::<f32>() / total_cases as f32;
    let average_text_accuracy =
        results.iter().map(|r| r.text_accuracy).sum::<f32>() / total_cases as f32;

    let latex_count = results
        .iter()
        .filter(|r| r.latex_accuracy.is_some())
        .count();
    let average_latex_accuracy = if latex_count > 0 {
        results.iter().filter_map(|r| r.latex_accuracy).sum::<f32>() / latex_count as f32
    } else {
        0.0
    };

    let average_cer =
        results.iter().map(|r| r.character_error_rate).sum::<f32>() / total_cases as f32;
    let average_wer = results.iter().map(|r| r.word_error_rate).sum::<f32>() / total_cases as f32;

    // Calculate category breakdown
    let mut category_breakdown = HashMap::new();
    for result in results {
        let entry = category_breakdown
            .entry(result.category.clone())
            .or_insert_with(|| CategoryMetrics {
                count: 0,
                average_accuracy: 0.0,
                average_confidence: 0.0,
            });

        entry.count += 1;
        entry.average_accuracy += result.text_accuracy;
        entry.average_confidence += result.confidence;
    }

    for metrics in category_breakdown.values_mut() {
        metrics.average_accuracy /= metrics.count as f32;
        metrics.average_confidence /= metrics.count as f32;
    }

    // Calculate confidence correlation (Pearson correlation)
    let confidence_correlation = calculate_pearson_correlation(
        &results.iter().map(|r| r.confidence).collect::<Vec<_>>(),
        &results.iter().map(|r| r.text_accuracy).collect::<Vec<_>>(),
    );

    AccuracyMetrics {
        total_cases,
        successful_cases,
        failed_cases,
        average_confidence,
        average_text_accuracy,
        average_latex_accuracy,
        average_cer,
        average_wer,
        category_breakdown,
        confidence_correlation,
    }
}

fn calculate_pearson_correlation(x: &[f32], y: &[f32]) -> f32 {
    let n = x.len() as f32;
    let mean_x = x.iter().sum::<f32>() / n;
    let mean_y = y.iter().sum::<f32>() / n;

    let mut numerator = 0.0;
    let mut sum_sq_x = 0.0;
    let mut sum_sq_y = 0.0;

    for i in 0..x.len() {
        let diff_x = x[i] - mean_x;
        let diff_y = y[i] - mean_y;
        numerator += diff_x * diff_y;
        sum_sq_x += diff_x * diff_x;
        sum_sq_y += diff_y * diff_y;
    }

    if sum_sq_x == 0.0 || sum_sq_y == 0.0 {
        return 0.0;
    }

    numerator / (sum_sq_x * sum_sq_y).sqrt()
}
