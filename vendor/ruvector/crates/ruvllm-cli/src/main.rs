//! RuvLLM CLI - Model Management and Inference for Apple Silicon
//!
//! A command-line interface for downloading, managing, and running LLM models
//! optimized for Mac M4 Pro and other Apple Silicon devices.
//!
//! ## Commands
//!
//! - `ruvllm download <model>` - Download model from HuggingFace Hub
//! - `ruvllm list` - List available/downloaded models
//! - `ruvllm info <model>` - Show model information
//! - `ruvllm serve <model>` - Start inference server
//! - `ruvllm chat <model>` - Interactive chat mode
//! - `ruvllm benchmark <model>` - Run performance benchmarks
//! - `ruvllm quantize <model>` - Quantize model to GGUF format

use clap::{Parser, Subcommand};
use colored::Colorize;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
mod models;

use commands::{benchmark, chat, download, info, list, quantize, serve};

/// RuvLLM - High-performance LLM inference for Apple Silicon
#[derive(Parser)]
#[command(name = "ruvllm")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Custom cache directory for models
    #[arg(long, global = true, env = "RUVLLM_CACHE_DIR")]
    cache_dir: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download a model from HuggingFace Hub
    #[command(alias = "dl")]
    Download {
        /// Model identifier (HuggingFace model ID or alias)
        ///
        /// Aliases: qwen, mistral, phi, llama
        model: String,

        /// Quantization format (q4k, q8, f16, none)
        #[arg(short, long, default_value = "q4k")]
        quantization: String,

        /// Force re-download even if model exists
        #[arg(short, long)]
        force: bool,

        /// Specific revision/branch to download
        #[arg(long)]
        revision: Option<String>,
    },

    /// List available and downloaded models
    #[command(alias = "ls")]
    List {
        /// Show only downloaded models
        #[arg(short, long)]
        downloaded: bool,

        /// Show detailed information
        #[arg(short, long)]
        long: bool,
    },

    /// Show detailed model information
    Info {
        /// Model identifier or alias
        model: String,
    },

    /// Start an OpenAI-compatible inference server
    Serve {
        /// Model to serve
        model: String,

        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to bind to
        #[arg(short, long, default_value = "8080")]
        port: u16,

        /// Maximum concurrent requests
        #[arg(long, default_value = "4")]
        max_concurrent: usize,

        /// Maximum context length
        #[arg(long, default_value = "4096")]
        max_context: usize,

        /// Quantization format
        #[arg(short, long, default_value = "q4k")]
        quantization: String,
    },

    /// Interactive chat mode
    Chat {
        /// Model to use for chat
        model: String,

        /// System prompt
        #[arg(short, long)]
        system: Option<String>,

        /// Maximum tokens to generate per response
        #[arg(long, default_value = "512")]
        max_tokens: usize,

        /// Temperature for sampling (0.0 = deterministic)
        #[arg(short, long, default_value = "0.7")]
        temperature: f32,

        /// Quantization format
        #[arg(short, long, default_value = "q4k")]
        quantization: String,

        /// Enable speculative decoding with a draft model
        ///
        /// Provide the draft model path/ID. Recommended pairings:
        /// - Qwen2.5-14B + Qwen2.5-0.5B
        /// - Mistral-7B + TinyLlama-1.1B
        /// - Llama-3.2-3B + Llama-3.2-1B
        #[arg(long)]
        speculative: Option<String>,

        /// Number of speculative tokens to generate ahead (2-8)
        #[arg(long, default_value = "4")]
        speculative_lookahead: usize,
    },

    /// Run performance benchmarks
    #[command(alias = "bench")]
    Benchmark {
        /// Model to benchmark
        model: String,

        /// Number of warmup iterations
        #[arg(long, default_value = "3")]
        warmup: usize,

        /// Number of benchmark iterations
        #[arg(short, long, default_value = "10")]
        iterations: usize,

        /// Prompt length for benchmarking
        #[arg(long, default_value = "128")]
        prompt_length: usize,

        /// Generation length for benchmarking
        #[arg(long, default_value = "64")]
        gen_length: usize,

        /// Quantization format
        #[arg(short, long, default_value = "q4k")]
        quantization: String,

        /// Output format (text, json, csv)
        #[arg(long, default_value = "text")]
        format: String,
    },

    /// Quantize a model to GGUF format
    ///
    /// Supports Q4_K_M (4-bit), Q5_K_M (5-bit), and Q8_0 (8-bit) quantization.
    /// Optimized for Apple Neural Engine (ANE) inference on M4 Pro.
    ///
    /// Examples:
    ///   ruvllm quantize --model qwen-0.5b --output ruvltra-small-q4.gguf --quant q4_k_m
    ///   ruvllm quantize --model ./model.safetensors --quant q8_0 --ane-optimize
    #[command(alias = "quant")]
    Quantize {
        /// Model to quantize (path or HuggingFace ID)
        #[arg(short, long)]
        model: String,

        /// Output file path (default: <model>-<quant>.gguf)
        #[arg(short, long, default_value = "")]
        output: String,

        /// Quantization format: q4_k_m, q5_k_m, q8_0, f16
        ///
        /// Memory estimates for 0.5B model:
        /// - q4_k_m: ~300 MB (best quality/size tradeoff)
        /// - q5_k_m: ~375 MB (higher quality)
        /// - q8_0:   ~500 MB (near-lossless)
        #[arg(short, long, default_value = "q4_k_m")]
        quant: String,

        /// Enable ANE-optimized weight layouts (16-byte aligned, tiled)
        #[arg(long, default_value = "true")]
        ane_optimize: bool,

        /// Keep embedding layer in FP16 (recommended for quality)
        #[arg(long, default_value = "true")]
        keep_embed_fp16: bool,

        /// Keep output/LM head layer in FP16 (recommended for quality)
        #[arg(long, default_value = "true")]
        keep_output_fp16: bool,

        /// Show detailed progress and statistics
        #[arg(long)]
        verbose: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| log_level.into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    // Set up colored output
    if cli.no_color {
        colored::control::set_override(false);
    }

    // Get cache directory
    let cache_dir = cli.cache_dir.unwrap_or_else(|| {
        dirs::cache_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("ruvllm")
            .to_string_lossy()
            .to_string()
    });

    // Execute command
    let result = match cli.command {
        Commands::Download {
            model,
            quantization,
            force,
            revision,
        } => {
            download::run(
                &model,
                &quantization,
                force,
                revision.as_deref(),
                &cache_dir,
            )
            .await
        }

        Commands::List { downloaded, long } => list::run(downloaded, long, &cache_dir).await,

        Commands::Info { model } => info::run(&model, &cache_dir).await,

        Commands::Serve {
            model,
            host,
            port,
            max_concurrent,
            max_context,
            quantization,
        } => {
            serve::run(
                &model,
                &host,
                port,
                max_concurrent,
                max_context,
                &quantization,
                &cache_dir,
            )
            .await
        }

        Commands::Chat {
            model,
            system,
            max_tokens,
            temperature,
            quantization,
            speculative,
            speculative_lookahead,
        } => {
            chat::run(
                &model,
                system.as_deref(),
                max_tokens,
                temperature,
                &quantization,
                &cache_dir,
                speculative.as_deref(),
                speculative_lookahead,
            )
            .await
        }

        Commands::Benchmark {
            model,
            warmup,
            iterations,
            prompt_length,
            gen_length,
            quantization,
            format,
        } => {
            benchmark::run(
                &model,
                warmup,
                iterations,
                prompt_length,
                gen_length,
                &quantization,
                &format,
                &cache_dir,
            )
            .await
        }

        Commands::Quantize {
            model,
            output,
            quant,
            ane_optimize,
            keep_embed_fp16,
            keep_output_fp16,
            verbose,
        } => {
            quantize::run(
                &model,
                &output,
                &quant,
                ane_optimize,
                keep_embed_fp16,
                keep_output_fp16,
                verbose,
                &cache_dir,
            )
            .await
        }
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }

    Ok(())
}
