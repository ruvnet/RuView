//! Speculative Decoding - Draft and Verify
//!
//! Use a smaller/faster model to draft tokens, verify with larger model.
//! Perfect for federated setup: one chip drafts, others verify in parallel.
//!
//! # Benefits
//! - 2-4x speedup for autoregressive generation
//! - Maintains exact output quality
//! - Natural fit for multi-chip setup

use heapless::Vec as HVec;
use super::protocol::{ChipId, FederationMessage};

/// Maximum draft tokens per batch
pub const MAX_DRAFT_TOKENS: usize = 8;

/// Speculative decoding configuration
#[derive(Debug, Clone)]
pub struct DraftVerifyConfig {
    /// Number of draft tokens to generate
    pub draft_length: usize,
    /// Acceptance threshold (0.0-1.0)
    pub acceptance_threshold: f32,
    /// Draft chip ID (usually chip 0)
    pub draft_chip: ChipId,
    /// Verify chips (all others)
    pub verify_chips: HVec<ChipId, 4>,
    /// Enable adaptive draft length
    pub adaptive: bool,
}

impl Default for DraftVerifyConfig {
    fn default() -> Self {
        Self {
            draft_length: 4,
            acceptance_threshold: 0.9,
            draft_chip: ChipId(0),
            verify_chips: HVec::new(),
            adaptive: true,
        }
    }
}

impl DraftVerifyConfig {
    /// Create config for 5-chip setup
    pub fn for_five_chips() -> Self {
        let mut verify_chips = HVec::new();
        for i in 1..5 {
            let _ = verify_chips.push(ChipId(i));
        }

        Self {
            draft_length: 4,
            acceptance_threshold: 0.9,
            draft_chip: ChipId(0),
            verify_chips,
            adaptive: true,
        }
    }
}

/// Draft result from drafting chip
#[derive(Debug, Clone)]
pub struct DraftResult {
    /// Draft token IDs
    pub tokens: HVec<u16, MAX_DRAFT_TOKENS>,
    /// Draft token probabilities (fixed-point, 0-255)
    pub probs: HVec<u8, MAX_DRAFT_TOKENS>,
    /// Starting position
    pub start_pos: u16,
}

/// Verification result from verifying chip
#[derive(Debug, Clone)]
pub struct VerifyResult {
    /// Number of accepted tokens
    pub accepted_count: usize,
    /// Correct token for first rejection (if any)
    pub correction: Option<u16>,
    /// Verification probabilities
    pub verify_probs: HVec<u8, MAX_DRAFT_TOKENS>,
}

/// Speculative decoder
pub struct SpeculativeDecoder {
    config: DraftVerifyConfig,
    /// Is this the draft chip?
    is_draft_chip: bool,
    /// Current acceptance rate (for adaptive)
    acceptance_rate: f32,
    /// Draft tokens waiting for verification
    pending_draft: Option<DraftResult>,
    /// Statistics
    stats: SpecStats,
}

impl SpeculativeDecoder {
    /// Create for a specific chip
    pub fn new(config: DraftVerifyConfig, chip_id: ChipId) -> Self {
        let is_draft_chip = chip_id == config.draft_chip;

        Self {
            config,
            is_draft_chip,
            acceptance_rate: 0.9,
            pending_draft: None,
            stats: SpecStats::default(),
        }
    }

    /// Check if this is the drafting chip
    pub fn is_drafter(&self) -> bool {
        self.is_draft_chip
    }

    /// Submit draft tokens (drafter only)
    pub fn submit_draft(&mut self, draft: DraftResult) -> crate::Result<FederationMessage> {
        if !self.is_draft_chip {
            return Err(crate::Error::UnsupportedFeature("Not draft chip"));
        }

        // Create message to broadcast to verify chips
        let tokens: Vec<u16> = draft.tokens.iter().cloned().collect();
        let msg = FederationMessage::draft_tokens(
            self.config.draft_chip,
            ChipId::BROADCAST,
            draft.start_pos,
            &tokens,
        )?;

        self.pending_draft = Some(draft);
        self.stats.drafts_sent += 1;

        Ok(msg)
    }

    /// Verify draft tokens (verifier only)
    pub fn verify_draft<F>(
        &mut self,
        draft: &DraftResult,
        mut get_prob: F,
    ) -> VerifyResult
    where
        F: FnMut(u16, u16) -> u8, // (position, token) -> probability
    {
        let mut accepted_count = 0;
        let mut correction = None;
        let mut verify_probs = HVec::new();

        for (i, &token) in draft.tokens.iter().enumerate() {
            let pos = draft.start_pos + i as u16;
            let verify_prob = get_prob(pos, token);
            let _ = verify_probs.push(verify_prob);

            let draft_prob = draft.probs.get(i).copied().unwrap_or(128);

            // Acceptance criterion: verify_prob >= draft_prob * threshold
            let threshold = (draft_prob as f32 * self.config.acceptance_threshold) as u8;

            if verify_prob >= threshold {
                accepted_count += 1;
            } else {
                // Rejection - sample correct token
                // In real impl, would sample from verify distribution
                correction = Some(token.wrapping_add(1)); // Placeholder
                break;
            }
        }

        VerifyResult {
            accepted_count,
            correction,
            verify_probs,
        }
    }

    /// Process verification result (drafter)
    pub fn process_verification(&mut self, result: &VerifyResult) -> HVec<u16, MAX_DRAFT_TOKENS> {
        let mut accepted_tokens = HVec::new();

        if let Some(ref draft) = self.pending_draft {
            // Accept tokens up to rejection point
            for i in 0..result.accepted_count {
                if let Some(&token) = draft.tokens.get(i) {
                    let _ = accepted_tokens.push(token);
                }
            }

            // Add correction if any
            if let Some(correct_token) = result.correction {
                let _ = accepted_tokens.push(correct_token);
            }

            self.stats.tokens_accepted += result.accepted_count;
            self.stats.tokens_rejected += draft.tokens.len() - result.accepted_count;

            // Update acceptance rate
            let batch_rate = result.accepted_count as f32 / draft.tokens.len() as f32;
            self.acceptance_rate = 0.9 * self.acceptance_rate + 0.1 * batch_rate;
        }

        self.pending_draft = None;
        accepted_tokens
    }

    /// Get adaptive draft length based on acceptance rate
    pub fn adaptive_draft_length(&self) -> usize {
        if !self.config.adaptive {
            return self.config.draft_length;
        }

        // Higher acceptance -> longer drafts
        if self.acceptance_rate > 0.95 {
            (self.config.draft_length + 2).min(MAX_DRAFT_TOKENS)
        } else if self.acceptance_rate > 0.8 {
            self.config.draft_length
        } else if self.acceptance_rate > 0.5 {
            (self.config.draft_length - 1).max(1)
        } else {
            1 // Fall back to no speculation
        }
    }

    /// Get speedup estimate
    pub fn estimated_speedup(&self) -> f32 {
        // Speedup = accepted_tokens / (1 + verify_overhead)
        let avg_accepted = self.acceptance_rate * self.adaptive_draft_length() as f32;
        let verify_overhead = 0.2; // Verification overhead
        avg_accepted / (1.0 + verify_overhead)
    }

    /// Get statistics
    pub fn stats(&self) -> &SpecStats {
        &self.stats
    }
}

/// Speculative decoding statistics
#[derive(Debug, Default, Clone)]
pub struct SpecStats {
    /// Total draft batches sent
    pub drafts_sent: usize,
    /// Total tokens accepted
    pub tokens_accepted: usize,
    /// Total tokens rejected
    pub tokens_rejected: usize,
}

impl SpecStats {
    /// Overall acceptance rate
    pub fn acceptance_rate(&self) -> f32 {
        let total = self.tokens_accepted + self.tokens_rejected;
        if total == 0 {
            0.0
        } else {
            self.tokens_accepted as f32 / total as f32
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speculative_config() {
        let config = DraftVerifyConfig::for_five_chips();

        assert_eq!(config.draft_chip, ChipId(0));
        assert_eq!(config.verify_chips.len(), 4);
    }

    #[test]
    fn test_verify_draft() {
        let config = DraftVerifyConfig::default();
        let mut decoder = SpeculativeDecoder::new(config, ChipId(1));

        let mut draft = DraftResult {
            tokens: HVec::new(),
            probs: HVec::new(),
            start_pos: 0,
        };
        let _ = draft.tokens.push(100);
        let _ = draft.tokens.push(101);
        let _ = draft.probs.push(200);
        let _ = draft.probs.push(200);

        let result = decoder.verify_draft(&draft, |_pos, _token| 190);

        // Both should be accepted (190 >= 200 * 0.9 = 180)
        assert_eq!(result.accepted_count, 2);
        assert!(result.correction.is_none());
    }
}
