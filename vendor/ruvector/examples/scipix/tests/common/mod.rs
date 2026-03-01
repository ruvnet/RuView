// Common test utilities
//
// Provides shared functionality for integration tests

pub mod images;
pub mod latex;
pub mod metrics;
pub mod server;
pub mod types;

// Re-export commonly used types and functions
pub use images::{generate_fraction, generate_integral, generate_simple_equation, generate_symbol};
pub use latex::{calculate_similarity, expressions_match, normalize};
pub use metrics::{calculate_bleu, calculate_cer, calculate_wer};
pub use server::TestServer;
pub use types::{CacheStats, OutputFormat, ProcessingOptions, ProcessingResult};
