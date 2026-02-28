use anyhow::{Context, Result};
use clap::Args;
use glob::glob;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

use super::{OcrConfig, OcrResult};
use crate::cli::{output, Cli, OutputFormat};

/// Process multiple files in batch mode
#[derive(Args, Debug, Clone)]
pub struct BatchArgs {
    /// Input pattern (glob) or directory
    #[arg(value_name = "PATTERN", help = "Input pattern (glob) or directory")]
    pub pattern: String,

    /// Output directory for results
    #[arg(
        short,
        long,
        value_name = "DIR",
        help = "Output directory for results (default: stdout as JSON array)"
    )]
    pub output: Option<PathBuf>,

    /// Number of parallel workers
    #[arg(
        short,
        long,
        default_value = "4",
        help = "Number of parallel processing workers"
    )]
    pub parallel: usize,

    /// Minimum confidence threshold (0.0 to 1.0)
    #[arg(
        short = 't',
        long,
        default_value = "0.7",
        help = "Minimum confidence threshold for results"
    )]
    pub threshold: f64,

    /// Continue on errors
    #[arg(
        short = 'c',
        long,
        help = "Continue processing even if some files fail"
    )]
    pub continue_on_error: bool,

    /// Maximum retry attempts per file
    #[arg(
        short = 'r',
        long,
        default_value = "2",
        help = "Maximum retry attempts per file on failure"
    )]
    pub max_retries: usize,

    /// Save individual results as separate files
    #[arg(long, help = "Save each result as a separate file (requires --output)")]
    pub separate_files: bool,

    /// Recursive directory search
    #[arg(short = 'R', long, help = "Recursively search directories")]
    pub recursive: bool,
}

pub async fn execute(args: BatchArgs, cli: &Cli) -> Result<()> {
    info!("Starting batch processing with pattern: {}", args.pattern);

    // Load configuration
    let config = Arc::new(load_config(cli.config.as_ref())?);

    // Expand pattern to file list
    let files = collect_files(&args)?;

    if files.is_empty() {
        anyhow::bail!("No files found matching pattern: {}", args.pattern);
    }

    info!("Found {} files to process", files.len());

    // Create output directory if needed
    if let Some(output_dir) = &args.output {
        std::fs::create_dir_all(output_dir).context("Failed to create output directory")?;
    }

    // Process files in parallel with progress bars
    let results = process_files_parallel(files, &args, &config, cli.quiet).await?;

    // Filter by confidence threshold
    let (passed, failed): (Vec<_>, Vec<_>) = results
        .into_iter()
        .partition(|r| r.confidence >= args.threshold);

    info!(
        "Processing complete: {} passed, {} failed threshold",
        passed.len(),
        failed.len()
    );

    // Save or display results
    if let Some(output_dir) = &args.output {
        save_results(&passed, output_dir, &cli.format, args.separate_files)?;

        if !cli.quiet {
            println!("Results saved to: {}", output_dir.display());
        }
    } else {
        // Output as JSON array to stdout
        let json = serde_json::to_string_pretty(&passed).context("Failed to serialize results")?;
        println!("{}", json);
    }

    // Display summary
    if !cli.quiet {
        output::print_batch_summary(&passed, &failed, args.threshold);
    }

    // Return error if any files failed and continue_on_error is false
    if !failed.is_empty() && !args.continue_on_error {
        anyhow::bail!("{} files failed confidence threshold", failed.len());
    }

    Ok(())
}

fn collect_files(args: &BatchArgs) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let path = PathBuf::from(&args.pattern);

    if path.is_dir() {
        // Directory mode
        let pattern = if args.recursive {
            format!("{}/**/*", args.pattern)
        } else {
            format!("{}/*", args.pattern)
        };

        for entry in glob(&pattern).context("Failed to read glob pattern")? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        files.push(path);
                    }
                }
                Err(e) => warn!("Failed to read entry: {}", e),
            }
        }
    } else {
        // Glob pattern mode
        for entry in glob(&args.pattern).context("Failed to read glob pattern")? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        files.push(path);
                    }
                }
                Err(e) => warn!("Failed to read entry: {}", e),
            }
        }
    }

    Ok(files)
}

async fn process_files_parallel(
    files: Vec<PathBuf>,
    args: &BatchArgs,
    config: &Arc<OcrConfig>,
    quiet: bool,
) -> Result<Vec<OcrResult>> {
    let semaphore = Arc::new(Semaphore::new(args.parallel));
    let multi_progress = Arc::new(MultiProgress::new());

    let overall_progress = if !quiet {
        let pb = multi_progress.add(ProgressBar::new(files.len() as u64));
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        Some(pb)
    } else {
        None
    };

    let mut handles = Vec::new();

    for (_idx, file) in files.into_iter().enumerate() {
        let semaphore = semaphore.clone();
        let config = config.clone();
        let multi_progress = multi_progress.clone();
        let overall_progress = overall_progress.clone();
        let max_retries = args.max_retries;

        let handle = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();

            let file_progress = if !quiet {
                let pb = multi_progress.insert_before(
                    &overall_progress.as_ref().unwrap(),
                    ProgressBar::new_spinner(),
                );
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("{spinner:.green} {msg}")
                        .unwrap(),
                );
                pb.set_message(format!("[{}] Processing...", file.display()));
                Some(pb)
            } else {
                None
            };

            let result = process_with_retry(&file, &config, max_retries).await;

            if let Some(pb) = &file_progress {
                match &result {
                    Ok(r) => pb.finish_with_message(format!(
                        "[{}] ✓ Confidence: {:.2}%",
                        file.display(),
                        r.confidence * 100.0
                    )),
                    Err(e) => {
                        pb.finish_with_message(format!("[{}] ✗ Error: {}", file.display(), e))
                    }
                }
            }

            if let Some(pb) = &overall_progress {
                pb.inc(1);
            }

            result
        });

        handles.push(handle);
    }

    // Wait for all tasks to complete
    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(Ok(result)) => results.push(result),
            Ok(Err(e)) => error!("Processing failed: {}", e),
            Err(e) => error!("Task panicked: {}", e),
        }
    }

    if let Some(pb) = overall_progress {
        pb.finish_with_message("Batch processing complete");
    }

    Ok(results)
}

async fn process_with_retry(
    file: &PathBuf,
    config: &OcrConfig,
    max_retries: usize,
) -> Result<OcrResult> {
    let mut attempts = 0;
    let mut last_error = None;

    while attempts <= max_retries {
        match process_single_file(file, config).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                attempts += 1;
                last_error = Some(e);

                if attempts <= max_retries {
                    debug!("Retry {}/{} for {}", attempts, max_retries, file.display());
                    tokio::time::sleep(tokio::time::Duration::from_millis(100 * attempts as u64))
                        .await;
                }
            }
        }
    }

    Err(last_error.unwrap())
}

async fn process_single_file(file: &PathBuf, _config: &OcrConfig) -> Result<OcrResult> {
    // TODO: Implement actual OCR processing
    // For now, return a mock result

    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Simulate varying confidence
    let confidence = 0.7 + (rand::random::<f64>() * 0.3);

    Ok(OcrResult {
        file: file.clone(),
        text: format!("OCR text from {}", file.display()),
        latex: Some(format!(r"\text{{Content from {}}}", file.display())),
        confidence,
        processing_time_ms: 50,
        errors: Vec::new(),
    })
}

fn save_results(
    results: &[OcrResult],
    output_dir: &PathBuf,
    format: &OutputFormat,
    separate_files: bool,
) -> Result<()> {
    if separate_files {
        // Save each result as a separate file
        for (idx, result) in results.iter().enumerate() {
            let filename = format!(
                "result_{:04}.{}",
                idx,
                match format {
                    OutputFormat::Json => "json",
                    OutputFormat::Latex => "tex",
                    OutputFormat::Markdown => "md",
                    OutputFormat::MathMl => "xml",
                    OutputFormat::Text => "txt",
                }
            );

            let output_path = output_dir.join(filename);
            let content = format_single_result(result, format)?;

            std::fs::write(&output_path, content)
                .context(format!("Failed to write {}", output_path.display()))?;
        }
    } else {
        // Save all results as a single file
        let filename = format!(
            "results.{}",
            match format {
                OutputFormat::Json => "json",
                OutputFormat::Latex => "tex",
                OutputFormat::Markdown => "md",
                OutputFormat::MathMl => "xml",
                OutputFormat::Text => "txt",
            }
        );

        let output_path = output_dir.join(filename);
        let content = format_batch_results(results, format)?;

        std::fs::write(&output_path, content).context("Failed to write results file")?;
    }

    Ok(())
}

fn format_single_result(result: &OcrResult, format: &OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(result).context("Failed to serialize result")
        }
        OutputFormat::Text => Ok(result.text.clone()),
        OutputFormat::Latex => Ok(result.latex.clone().unwrap_or_else(|| result.text.clone())),
        OutputFormat::Markdown => Ok(format!("# {}\n\n{}\n", result.file.display(), result.text)),
        OutputFormat::MathMl => Ok(format!(
            "<math xmlns=\"http://www.w3.org/1998/Math/MathML\">\n  {}\n</math>",
            result.text
        )),
    }
}

fn format_batch_results(results: &[OcrResult], format: &OutputFormat) -> Result<String> {
    match format {
        OutputFormat::Json => {
            serde_json::to_string_pretty(results).context("Failed to serialize results")
        }
        _ => {
            let mut output = String::new();
            for result in results {
                output.push_str(&format_single_result(result, format)?);
                output.push_str("\n\n---\n\n");
            }
            Ok(output)
        }
    }
}

fn load_config(config_path: Option<&PathBuf>) -> Result<OcrConfig> {
    if let Some(path) = config_path {
        let content = std::fs::read_to_string(path).context("Failed to read config file")?;
        toml::from_str(&content).context("Failed to parse config file")
    } else {
        Ok(OcrConfig::default())
    }
}
