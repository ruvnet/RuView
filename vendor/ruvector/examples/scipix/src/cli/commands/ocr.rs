use anyhow::{Context, Result};
use clap::Args;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, info};

use super::{OcrConfig, OcrResult};
use crate::cli::{output, Cli, OutputFormat};

/// Process a single image or file with OCR
#[derive(Args, Debug, Clone)]
pub struct OcrArgs {
    /// Path to the image file to process
    #[arg(value_name = "FILE", help = "Path to the image file")]
    pub file: PathBuf,

    /// Minimum confidence threshold (0.0 to 1.0)
    #[arg(
        short = 't',
        long,
        default_value = "0.7",
        help = "Minimum confidence threshold for results"
    )]
    pub threshold: f64,

    /// Save output to file instead of stdout
    #[arg(
        short,
        long,
        value_name = "OUTPUT",
        help = "Save output to file instead of stdout"
    )]
    pub output: Option<PathBuf>,

    /// Pretty-print JSON output
    #[arg(
        short,
        long,
        help = "Pretty-print JSON output (only with --format json)"
    )]
    pub pretty: bool,

    /// Include metadata in output
    #[arg(short, long, help = "Include processing metadata in output")]
    pub metadata: bool,

    /// Force processing even if confidence is below threshold
    #[arg(
        short = 'f',
        long,
        help = "Force processing even if confidence is below threshold"
    )]
    pub force: bool,
}

pub async fn execute(args: OcrArgs, cli: &Cli) -> Result<()> {
    info!("Processing file: {}", args.file.display());

    // Validate input file
    if !args.file.exists() {
        anyhow::bail!("File not found: {}", args.file.display());
    }

    if !args.file.is_file() {
        anyhow::bail!("Not a file: {}", args.file.display());
    }

    // Load configuration
    let config = load_config(cli.config.as_ref())?;

    // Validate file extension
    if let Some(ext) = args.file.extension() {
        let ext_str = ext.to_string_lossy().to_lowercase();
        if !config.supported_extensions.contains(&ext_str) {
            anyhow::bail!(
                "Unsupported file extension: {}. Supported: {}",
                ext_str,
                config.supported_extensions.join(", ")
            );
        }
    } else {
        anyhow::bail!("File has no extension");
    }

    // Check file size
    let metadata = std::fs::metadata(&args.file).context("Failed to read file metadata")?;

    if metadata.len() as usize > config.max_image_size {
        anyhow::bail!(
            "File too large: {} bytes (max: {} bytes)",
            metadata.len(),
            config.max_image_size
        );
    }

    // Process the file
    let start = Instant::now();
    let result = process_file(&args.file, &config).await?;
    let processing_time = start.elapsed();

    debug!("Processing completed in {:?}", processing_time);

    // Check confidence threshold
    if result.confidence < args.threshold && !args.force {
        anyhow::bail!(
            "Confidence {} is below threshold {} (use --force to override)",
            result.confidence,
            args.threshold
        );
    }

    // Format and output result
    let output_content = format_result(&result, &cli.format, args.pretty, args.metadata)?;

    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &output_content).context("Failed to write output file")?;
        info!("Output saved to: {}", output_path.display());
    } else {
        println!("{}", output_content);
    }

    // Display summary if not quiet
    if !cli.quiet {
        output::print_ocr_summary(&result);
    }

    Ok(())
}

async fn process_file(file: &PathBuf, _config: &OcrConfig) -> Result<OcrResult> {
    // TODO: Implement actual OCR processing
    // For now, return a mock result

    let start = Instant::now();

    // Simulate OCR processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let processing_time = start.elapsed().as_millis() as u64;

    Ok(OcrResult {
        file: file.clone(),
        text: "Sample OCR text from image".to_string(),
        latex: Some(r"\int_0^1 x^2 \, dx = \frac{1}{3}".to_string()),
        confidence: 0.95,
        processing_time_ms: processing_time,
        errors: Vec::new(),
    })
}

fn format_result(
    result: &OcrResult,
    format: &OutputFormat,
    pretty: bool,
    include_metadata: bool,
) -> Result<String> {
    match format {
        OutputFormat::Json => if include_metadata {
            if pretty {
                serde_json::to_string_pretty(result)
            } else {
                serde_json::to_string(result)
            }
        } else {
            let simple = serde_json::json!({
                "text": result.text,
                "latex": result.latex,
                "confidence": result.confidence,
            });
            if pretty {
                serde_json::to_string_pretty(&simple)
            } else {
                serde_json::to_string(&simple)
            }
        }
        .context("Failed to serialize to JSON"),
        OutputFormat::Text => Ok(result.text.clone()),
        OutputFormat::Latex => Ok(result.latex.clone().unwrap_or_else(|| result.text.clone())),
        OutputFormat::Markdown => {
            let mut md = format!("# OCR Result\n\n{}\n", result.text);
            if let Some(latex) = &result.latex {
                md.push_str(&format!("\n## LaTeX\n\n```latex\n{}\n```\n", latex));
            }
            if include_metadata {
                md.push_str(&format!(
                    "\n---\n\nConfidence: {:.2}%\nProcessing time: {}ms\n",
                    result.confidence * 100.0,
                    result.processing_time_ms
                ));
            }
            Ok(md)
        }
        OutputFormat::MathMl => {
            // TODO: Implement MathML conversion
            Ok(format!(
                "<math xmlns=\"http://www.w3.org/1998/Math/MathML\">\n  {}\n</math>",
                result.text
            ))
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
