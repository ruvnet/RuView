//! AIMDS Detection Layer
//!
//! This crate provides pattern matching, sanitization, and scheduling
//! for detecting potential threats in AI model inputs.

pub mod pattern_matcher;
pub mod sanitizer;
pub mod scheduler;

pub use pattern_matcher::PatternMatcher;
pub use sanitizer::{Sanitizer, PiiMatch, PiiType};
pub use scheduler::{DetectionScheduler, ThreatPriority};

use aimds_core::{DetectionResult, PromptInput, Result};

/// Main detection service that coordinates all detection components
pub struct DetectionService {
    pattern_matcher: PatternMatcher,
    sanitizer: Sanitizer,
    scheduler: DetectionScheduler,
}

impl DetectionService {
    /// Create a new detection service
    pub fn new() -> Result<Self> {
        Ok(Self {
            pattern_matcher: PatternMatcher::new()?,
            sanitizer: Sanitizer::new(),
            scheduler: DetectionScheduler::new()?,
        })
    }

    /// Process a prompt input through all detection layers
    pub async fn detect(&self, input: &PromptInput) -> Result<DetectionResult> {
        // Schedule the detection task
        self.scheduler.schedule_detection(input.id).await?;

        // Pattern matching
        let detection = self.pattern_matcher.match_patterns(&input.content).await?;

        // Sanitization
        let _sanitized = self.sanitizer.sanitize(&input.content).await?;

        Ok(detection)
    }
}

impl Default for DetectionService {
    fn default() -> Self {
        Self::new().expect("Failed to create detection service")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detection_service() {
        let service = DetectionService::new().unwrap();
        let input = PromptInput::new("Test prompt".to_string());

        let result = service.detect(&input).await;
        assert!(result.is_ok());
    }
}
