use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};
use console::style;

use super::commands::OcrResult;

/// Print a summary of a single OCR result
pub fn print_ocr_summary(result: &OcrResult) {
    println!("\n{}", style("OCR Processing Summary").bold().cyan());
    println!("{}", style("─".repeat(60)).dim());

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Property").fg(Color::Cyan),
            Cell::new("Value").fg(Color::Green),
        ]);

    table.add_row(vec![
        Cell::new("File"),
        Cell::new(result.file.display().to_string()),
    ]);

    table.add_row(vec![
        Cell::new("Confidence"),
        Cell::new(format!("{:.2}%", result.confidence * 100.0))
            .fg(confidence_color(result.confidence)),
    ]);

    table.add_row(vec![
        Cell::new("Processing Time"),
        Cell::new(format!("{}ms", result.processing_time_ms)),
    ]);

    if let Some(latex) = &result.latex {
        table.add_row(vec![
            Cell::new("LaTeX"),
            Cell::new(if latex.len() > 50 {
                format!("{}...", &latex[..50])
            } else {
                latex.clone()
            }),
        ]);
    }

    if !result.errors.is_empty() {
        table.add_row(vec![
            Cell::new("Errors").fg(Color::Red),
            Cell::new(result.errors.len().to_string()).fg(Color::Red),
        ]);
    }

    println!("{table}");

    if !result.errors.is_empty() {
        println!("\n{}", style("Errors:").bold().red());
        for (i, error) in result.errors.iter().enumerate() {
            println!("  {}. {}", i + 1, style(error).red());
        }
    }

    println!();
}

/// Print a summary of batch processing results
pub fn print_batch_summary(passed: &[OcrResult], failed: &[OcrResult], threshold: f64) {
    println!("\n{}", style("Batch Processing Summary").bold().cyan());
    println!("{}", style("═".repeat(60)).dim());

    let total = passed.len() + failed.len();
    let avg_confidence = if !passed.is_empty() {
        passed.iter().map(|r| r.confidence).sum::<f64>() / passed.len() as f64
    } else {
        0.0
    };
    let total_time: u64 = passed.iter().map(|r| r.processing_time_ms).sum();
    let avg_time = if !passed.is_empty() {
        total_time / passed.len() as u64
    } else {
        0
    };

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![
            Cell::new("Metric").fg(Color::Cyan),
            Cell::new("Value").fg(Color::Green),
        ]);

    table.add_row(vec![Cell::new("Total Files"), Cell::new(total.to_string())]);

    table.add_row(vec![
        Cell::new("Passed").fg(Color::Green),
        Cell::new(format!(
            "{} ({:.1}%)",
            passed.len(),
            (passed.len() as f64 / total as f64) * 100.0
        ))
        .fg(Color::Green),
    ]);

    table.add_row(vec![
        Cell::new("Failed").fg(Color::Red),
        Cell::new(format!(
            "{} ({:.1}%)",
            failed.len(),
            (failed.len() as f64 / total as f64) * 100.0
        ))
        .fg(if failed.is_empty() {
            Color::Green
        } else {
            Color::Red
        }),
    ]);

    table.add_row(vec![
        Cell::new("Threshold"),
        Cell::new(format!("{:.2}%", threshold * 100.0)),
    ]);

    table.add_row(vec![
        Cell::new("Avg Confidence"),
        Cell::new(format!("{:.2}%", avg_confidence * 100.0)).fg(confidence_color(avg_confidence)),
    ]);

    table.add_row(vec![
        Cell::new("Avg Processing Time"),
        Cell::new(format!("{}ms", avg_time)),
    ]);

    table.add_row(vec![
        Cell::new("Total Processing Time"),
        Cell::new(format!("{:.2}s", total_time as f64 / 1000.0)),
    ]);

    println!("{table}");

    if !failed.is_empty() {
        println!("\n{}", style("Failed Files:").bold().red());

        let mut failed_table = Table::new();
        failed_table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![
                Cell::new("#").fg(Color::Cyan),
                Cell::new("File").fg(Color::Cyan),
                Cell::new("Confidence").fg(Color::Cyan),
            ]);

        for (i, result) in failed.iter().enumerate() {
            failed_table.add_row(vec![
                Cell::new((i + 1).to_string()),
                Cell::new(result.file.display().to_string()),
                Cell::new(format!("{:.2}%", result.confidence * 100.0)).fg(Color::Red),
            ]);
        }

        println!("{failed_table}");
    }

    // Summary statistics
    println!("\n{}", style("Statistics:").bold().cyan());

    if !passed.is_empty() {
        let confidences: Vec<f64> = passed.iter().map(|r| r.confidence).collect();
        let min_confidence = confidences.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_confidence = confidences
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max);

        println!(
            "  Min confidence: {}",
            style(format!("{:.2}%", min_confidence * 100.0)).green()
        );
        println!(
            "  Max confidence: {}",
            style(format!("{:.2}%", max_confidence * 100.0)).green()
        );

        let times: Vec<u64> = passed.iter().map(|r| r.processing_time_ms).collect();
        let min_time = times.iter().min().unwrap_or(&0);
        let max_time = times.iter().max().unwrap_or(&0);

        println!("  Min processing time: {}ms", style(min_time).cyan());
        println!("  Max processing time: {}ms", style(max_time).cyan());
    }

    println!();
}

/// Get color based on confidence value
fn confidence_color(confidence: f64) -> Color {
    if confidence >= 0.9 {
        Color::Green
    } else if confidence >= 0.7 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Create a progress bar style for batch processing
pub fn create_progress_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::default_bar()
        .template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
        )
        .unwrap()
        .progress_chars("█▓▒░  ")
}

/// Create a spinner style for individual file processing
pub fn create_spinner_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::default_spinner()
        .template("{spinner:.cyan} {msg}")
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
}
