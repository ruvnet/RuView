pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Scipix CLI - OCR and mathematical content processing
#[derive(Parser, Debug)]
#[command(
    name = "scipix-cli",
    version,
    about = "A Rust-based CLI for Scipix OCR processing",
    long_about = "Process images with OCR, extract mathematical formulas, and convert to LaTeX or other formats.\n\n\
                  Supports single file processing, batch operations, and API server mode."
)]
pub struct Cli {
    /// Path to configuration file
    #[arg(
        short,
        long,
        global = true,
        env = "MATHPIX_CONFIG",
        help = "Path to configuration file"
    )]
    pub config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(
        short,
        long,
        global = true,
        help = "Enable verbose logging (DEBUG level)"
    )]
    pub verbose: bool,

    /// Suppress all non-error output
    #[arg(
        short,
        long,
        global = true,
        conflicts_with = "verbose",
        help = "Suppress all non-error output"
    )]
    pub quiet: bool,

    /// Output format (json, text, latex, markdown)
    #[arg(
        short,
        long,
        global = true,
        default_value = "text",
        help = "Output format for results"
    )]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Process a single image or file with OCR
    Ocr(commands::ocr::OcrArgs),

    /// Process multiple files in batch mode
    Batch(commands::batch::BatchArgs),

    /// Start the API server
    Serve(commands::serve::ServeArgs),

    /// Start the MCP (Model Context Protocol) server for AI integration
    Mcp(commands::mcp::McpArgs),

    /// Manage configuration
    Config(commands::config::ConfigArgs),

    /// Diagnose environment and optimize configuration
    Doctor(commands::doctor::DoctorArgs),

    /// Show version information
    Version,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell)
        #[arg(value_enum)]
        shell: Option<clap_complete::Shell>,
    },
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum OutputFormat {
    /// Plain text output
    Text,
    /// JSON output
    Json,
    /// LaTeX format
    Latex,
    /// Markdown format
    Markdown,
    /// MathML format
    MathMl,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Latex => write!(f, "latex"),
            OutputFormat::Markdown => write!(f, "markdown"),
            OutputFormat::MathMl => write!(f, "mathml"),
        }
    }
}
