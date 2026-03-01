//! SONA Learning Loops
//!
//! Three-tier temporal learning architecture:
//! - Loop A (Instant): Per-request trajectory recording and micro-LoRA updates
//! - Loop B (Background): Hourly pattern extraction and base LoRA updates
//! - Loop C (Deep): Weekly dream consolidation and full EWC++ update

pub mod background;
pub mod coordinator;
pub mod instant;

pub use background::BackgroundLoop;
pub use coordinator::LoopCoordinator;
pub use instant::InstantLoop;
