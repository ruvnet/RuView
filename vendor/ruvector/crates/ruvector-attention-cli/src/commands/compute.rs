use clap::Args;
use crate::{config::Config, output::{Output, OutputFormat, OutputDimensions, OutputMetadata}};
use ruvector_attention::{
    attention::{ScaledDotProductAttention, MultiHeadAttention},
    hyperbolic::HyperbolicAttention,
    sparse::{FlashAttention, LinearAttention},
    moe::MoEAttention,
};
use std::time::Instant;

#[derive(Args)]
pub struct ComputeArgs {
    /// Input file (JSON/binary/msgpack)
    #[arg(short, long)]
    input: std::path::PathBuf,

    /// Output file (optional, prints to stdout if not specified)
    #[arg(short, long)]
    output: Option<std::path::PathBuf>,

    /// Attention type
    #[arg(short, long, default_value = "scaled_dot")]
    attention_type: AttentionType,

    /// Number of attention heads (for multi-head attention)
    #[arg(long, default_value = "8")]
    num_heads: usize,

    /// Number of experts (for MoE attention)
    #[arg(long, default_value = "4")]
    num_experts: usize,

    /// Top-k experts (for MoE attention)
    #[arg(long, default_value = "2")]
    top_k: usize,

    /// Curvature (for hyperbolic attention)
    #[arg(long, default_value = "1.0")]
    curvature: f32,

    /// Output format
    #[arg(short, long, default_value = "pretty")]
    format: OutputFormat,

    /// Show detailed metrics
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Clone, clap::ValueEnum)]
pub enum AttentionType {
    ScaledDot,
    MultiHead,
    Hyperbolic,
    Flash,
    Linear,
    MoE,
}

pub async fn run(args: ComputeArgs, config: &Config) -> anyhow::Result<()> {
    tracing::info!("Loading input from {:?}", args.input);
    let input_data = super::load_input(&args.input)?;

    tracing::info!(
        "Input dimensions: query={:?}, keys={}, values={}",
        input_data.query.len(),
        input_data.keys.len(),
        input_data.values.len()
    );

    let start = Instant::now();
    let (result, attention_type_str) = match args.attention_type {
        AttentionType::ScaledDot => {
            tracing::info!("Computing scaled dot-product attention");
            let attention = ScaledDotProductAttention::new(input_data.dim, None);
            let result = attention.compute(
                &input_data.query,
                &input_data.keys_refs(),
                &input_data.values_refs()
            )?;
            (result, "ScaledDotProduct")
        }
        AttentionType::MultiHead => {
            tracing::info!("Computing multi-head attention with {} heads", args.num_heads);
            let attention = MultiHeadAttention::new(input_data.dim, args.num_heads)?;
            let result = attention.compute(
                &input_data.query,
                &input_data.keys_refs(),
                &input_data.values_refs()
            )?;
            (result, "MultiHead")
        }
        AttentionType::Hyperbolic => {
            tracing::info!("Computing hyperbolic attention with curvature={}", args.curvature);
            let attention = HyperbolicAttention::new(input_data.dim, args.curvature)?;
            let result = attention.compute(
                &input_data.query,
                &input_data.keys_refs(),
                &input_data.values_refs()
            )?;
            (result, "Hyperbolic")
        }
        AttentionType::Flash => {
            tracing::info!("Computing flash attention");
            let attention = FlashAttention::new(input_data.dim, 64)?;
            let result = attention.compute(
                &input_data.query,
                &input_data.keys_refs(),
                &input_data.values_refs()
            )?;
            (result, "Flash")
        }
        AttentionType::Linear => {
            tracing::info!("Computing linear attention");
            let attention = LinearAttention::new(input_data.dim)?;
            let result = attention.compute(
                &input_data.query,
                &input_data.keys_refs(),
                &input_data.values_refs()
            )?;
            (result, "Linear")
        }
        AttentionType::MoE => {
            tracing::info!(
                "Computing MoE attention with {} experts, top-{}",
                args.num_experts,
                args.top_k
            );
            let attention = MoEAttention::new(
                input_data.dim,
                args.num_experts,
                args.top_k
            )?;
            let result = attention.compute(
                &input_data.query,
                &input_data.keys_refs(),
                &input_data.values_refs()
            )?;
            (result, "MixtureOfExperts")
        }
    };

    let elapsed = start.elapsed();

    if args.verbose {
        tracing::info!("Computation completed in {:.2}ms", elapsed.as_secs_f64() * 1000.0);
    }

    let dimensions = OutputDimensions {
        batch_size: 1,
        num_heads: args.num_heads,
        seq_length: input_data.keys.len(),
        embedding_dim: input_data.dim,
    };

    let metadata = OutputMetadata {
        compute_time_ms: elapsed.as_secs_f64() * 1000.0,
        memory_bytes: estimate_memory_usage(&result),
        num_parameters: calculate_parameters(&args, input_data.dim),
    };

    let output = Output::new(attention_type_str, dimensions, result, metadata);
    output.write(args.output.as_deref(), args.format)?;

    tracing::info!("Output written successfully");

    Ok(())
}

fn estimate_memory_usage(result: &[Vec<f32>]) -> usize {
    result.iter().map(|row| row.len() * std::mem::size_of::<f32>()).sum()
}

fn calculate_parameters(args: &ComputeArgs, dim: usize) -> usize {
    match args.attention_type {
        AttentionType::ScaledDot => dim * dim * 3, // Q, K, V projections
        AttentionType::MultiHead => dim * dim * 3 * args.num_heads + dim * dim, // + output projection
        AttentionType::Hyperbolic => dim * dim * 3 + dim, // + curvature params
        AttentionType::Flash => dim * dim * 3,
        AttentionType::Linear => dim * dim * 2, // Linear projections
        AttentionType::MoE => dim * dim * 3 * args.num_experts + dim * args.num_experts, // + router
    }
}
