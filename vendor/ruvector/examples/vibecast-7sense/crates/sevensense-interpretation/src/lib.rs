//! # sevensense-interpretation
//!
//! LLM-powered interpretation for the 7sense bioacoustics platform.
//!
//! This crate provides:
//! - Natural language report generation
//! - Conservation insights
//! - Anomaly explanation
//! - Multi-language support
//!
//! ## Architecture
//!
//! ```text
//! sevensense-interpretation
//! ├── reports/          # Report generation
//! ├── insights/         # Conservation insights
//! ├── prompts/          # Prompt templates
//! └── providers/        # LLM provider integrations
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

// TODO: Implement interpretation modules
// - reports: Structured report generation
// - insights: Ecological pattern detection
// - prompts: Template management
// - providers: Claude, GPT-4, local models

/// Crate version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
