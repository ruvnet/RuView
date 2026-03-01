//! Speculative Decoding - Draft and Verify

use heapless::Vec as HVec;
use super::protocol::{ChipId, FederationMessage};

pub const MAX_DRAFT_TOKENS: usize = 8;

#[derive(Debug, Clone)]
pub struct DraftVerifyConfig {
    pub draft_length: usize,
    pub acceptance_threshold: f32,
    pub draft_chip: ChipId,
    pub verify_chips: HVec<ChipId, 4>,
    pub adaptive: bool,
}

impl Default for DraftVerifyConfig {
    fn default() -> Self {
        Self { draft_length: 4, acceptance_threshold: 0.9, draft_chip: ChipId(0), verify_chips: HVec::new(), adaptive: true }
    }
}

impl DraftVerifyConfig {
    pub fn for_five_chips() -> Self {
        let mut verify_chips = HVec::new();
        for i in 1..5 { let _ = verify_chips.push(ChipId(i)); }
        Self { draft_length: 4, acceptance_threshold: 0.9, draft_chip: ChipId(0), verify_chips, adaptive: true }
    }
}

#[derive(Debug, Clone)]
pub struct DraftResult {
    pub tokens: HVec<u16, MAX_DRAFT_TOKENS>,
    pub probs: HVec<u8, MAX_DRAFT_TOKENS>,
    pub start_pos: u16,
}

#[derive(Debug, Clone)]
pub struct VerifyResult {
    pub accepted_count: usize,
    pub correction: Option<u16>,
    pub verify_probs: HVec<u8, MAX_DRAFT_TOKENS>,
}

pub struct SpeculativeDecoder {
    config: DraftVerifyConfig,
    is_draft_chip: bool,
    acceptance_rate: f32,
    pending_draft: Option<DraftResult>,
    stats: SpecStats,
}

impl SpeculativeDecoder {
    pub fn new(config: DraftVerifyConfig, chip_id: ChipId) -> Self {
        let is_draft = chip_id == config.draft_chip;
        Self { config, is_draft_chip: is_draft, acceptance_rate: 0.9, pending_draft: None, stats: SpecStats::default() }
    }

    pub fn is_drafter(&self) -> bool { self.is_draft_chip }

    pub fn submit_draft(&mut self, draft: DraftResult) -> crate::Result<FederationMessage> {
        if !self.is_draft_chip { return Err(crate::Error::UnsupportedFeature("Not draft chip")); }
        let tokens: heapless::Vec<u16, MAX_DRAFT_TOKENS> = draft.tokens.iter().cloned().collect();
        let msg = FederationMessage::draft_tokens(self.config.draft_chip, ChipId::BROADCAST, draft.start_pos, &tokens)?;
        self.pending_draft = Some(draft);
        self.stats.drafts_sent += 1;
        Ok(msg)
    }

    pub fn verify_draft<F>(&mut self, draft: &DraftResult, mut get_prob: F) -> VerifyResult
    where F: FnMut(u16, u16) -> u8
    {
        let mut accepted = 0;
        let mut correction = None;
        let mut verify_probs = HVec::new();

        for (i, &token) in draft.tokens.iter().enumerate() {
            let pos = draft.start_pos + i as u16;
            let verify_prob = get_prob(pos, token);
            let _ = verify_probs.push(verify_prob);
            let draft_prob = draft.probs.get(i).copied().unwrap_or(128);
            let threshold = (draft_prob as f32 * self.config.acceptance_threshold) as u8;

            if verify_prob >= threshold {
                accepted += 1;
            } else {
                correction = Some(token.wrapping_add(1));
                break;
            }
        }

        VerifyResult { accepted_count: accepted, correction, verify_probs }
    }

    pub fn process_verification(&mut self, result: &VerifyResult) -> HVec<u16, MAX_DRAFT_TOKENS> {
        let mut accepted_tokens = HVec::new();

        if let Some(ref draft) = self.pending_draft {
            for i in 0..result.accepted_count {
                if let Some(&token) = draft.tokens.get(i) {
                    let _ = accepted_tokens.push(token);
                }
            }
            if let Some(correct) = result.correction {
                let _ = accepted_tokens.push(correct);
            }

            self.stats.tokens_accepted += result.accepted_count;
            self.stats.tokens_rejected += draft.tokens.len() - result.accepted_count;
            let rate = result.accepted_count as f32 / draft.tokens.len() as f32;
            self.acceptance_rate = 0.9 * self.acceptance_rate + 0.1 * rate;
        }

        self.pending_draft = None;
        accepted_tokens
    }

    pub fn adaptive_draft_length(&self) -> usize {
        if !self.config.adaptive { return self.config.draft_length; }
        if self.acceptance_rate > 0.95 { (self.config.draft_length + 2).min(MAX_DRAFT_TOKENS) }
        else if self.acceptance_rate > 0.8 { self.config.draft_length }
        else if self.acceptance_rate > 0.5 { (self.config.draft_length - 1).max(1) }
        else { 1 }
    }

    pub fn estimated_speedup(&self) -> f32 {
        let avg = self.acceptance_rate * self.adaptive_draft_length() as f32;
        avg / 1.2
    }

    pub fn stats(&self) -> &SpecStats { &self.stats }
}

#[derive(Debug, Default, Clone)]
pub struct SpecStats {
    pub drafts_sent: usize,
    pub tokens_accepted: usize,
    pub tokens_rejected: usize,
}

impl SpecStats {
    pub fn acceptance_rate(&self) -> f32 {
        let total = self.tokens_accepted + self.tokens_rejected;
        if total == 0 { 0.0 } else { self.tokens_accepted as f32 / total as f32 }
    }
}
