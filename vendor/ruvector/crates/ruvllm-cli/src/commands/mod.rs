//! CLI command implementations for RuvLLM
//!
//! This module contains all the subcommand implementations:
//! - `download` - Download models from HuggingFace Hub
//! - `list` - List available and downloaded models
//! - `info` - Show detailed model information
//! - `serve` - Start an OpenAI-compatible inference server
//! - `chat` - Interactive chat mode
//! - `benchmark` - Run performance benchmarks
//! - `quantize` - Quantize models to GGUF format

pub mod benchmark;
pub mod chat;
pub mod download;
pub mod info;
pub mod list;
pub mod quantize;
pub mod serve;
