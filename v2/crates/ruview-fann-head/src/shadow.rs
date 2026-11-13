//! Shadow scoring: a candidate next to the incumbent, without authority
//! (ADR-425 D4).
//!
//! The candidate sees exactly the feature vector the incumbent scored. Its
//! output is a diagnostic record. There is no path from a [`ShadowScore`] to a
//! published presence result: only the existing owner qualification and
//! selection flow can make a model live. A candidate that fails, or whose
//! context no longer matches, yields [`CandidateOutcome::Unavailable`]; it never
//! falls back to a guess and never disturbs the incumbent's result.

use serde::Serialize;

use crate::{decide, FannHead, HeadContext, HeadError, HeadScorer, LinearHead, PresenceDecision};

/// What the candidate produced for one observation.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CandidateOutcome {
    Scored {
        probability: f32,
        decision: PresenceDecision,
    },
    Unavailable {
        reason: HeadError,
    },
}

/// One observation scored by both heads. Borrows its identifiers, so scoring
/// does not allocate; serialise it or copy the fields to keep it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ShadowScore<'a> {
    /// Caller's identity for the admitted observation (cohort / frame ids).
    pub observation_id: &'a str,
    pub incumbent_probability: f32,
    pub incumbent_decision: PresenceDecision,
    pub candidate: CandidateOutcome,
    /// Digest of the candidate that produced this score.
    pub candidate_digest: &'a str,
    /// `None` when the candidate was unavailable.
    pub decisions_agree: Option<bool>,
}

/// Running totals for a shadow session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ShadowCounters {
    pub scored: u64,
    pub unavailable: u64,
    pub agree: u64,
    pub disagree: u64,
}

/// Scores a candidate beside the incumbent for one fixed context.
#[derive(Debug)]
pub struct ShadowScorer<'a> {
    incumbent: &'a LinearHead,
    candidate: &'a FannHead,
    scorer: HeadScorer,
    counters: ShadowCounters,
}

impl<'a> ShadowScorer<'a> {
    /// Both heads must take the same features, and the candidate must have been
    /// fitted for exactly `context`.
    pub fn new(
        incumbent: &'a LinearHead,
        candidate: &'a FannHead,
        context: &HeadContext,
    ) -> Result<Self, HeadError> {
        candidate.require_context(context)?;
        if incumbent.features() != candidate.features() {
            return Err(HeadError::FeatureCount {
                expected: incumbent.features(),
                actual: candidate.features(),
            });
        }
        Ok(Self {
            incumbent,
            candidate,
            scorer: candidate.scorer()?,
            counters: ShadowCounters::default(),
        })
    }

    pub fn counters(&self) -> ShadowCounters {
        self.counters
    }

    /// Score one admitted observation. An incumbent failure is the caller's
    /// existing `Unavailable` path and is returned as an error; a candidate
    /// failure is recorded and does not affect the incumbent.
    pub fn score<'s>(
        &'s mut self,
        observation_id: &'s str,
        features: &[f32],
    ) -> Result<ShadowScore<'s>, HeadError> {
        let incumbent_probability = self.incumbent.probability(features)?;
        let incumbent_decision = decide(incumbent_probability);
        let candidate = match self.candidate.score(&mut self.scorer, features) {
            Ok(probability) => CandidateOutcome::Scored {
                probability,
                decision: decide(probability),
            },
            Err(reason) => CandidateOutcome::Unavailable { reason },
        };
        let decisions_agree = match &candidate {
            CandidateOutcome::Scored { decision, .. } => Some(*decision == incumbent_decision),
            CandidateOutcome::Unavailable { .. } => None,
        };
        match decisions_agree {
            Some(true) => {
                self.counters.scored += 1;
                self.counters.agree += 1;
            }
            Some(false) => {
                self.counters.scored += 1;
                self.counters.disagree += 1;
            }
            None => self.counters.unavailable += 1,
        }
        Ok(ShadowScore {
            observation_id,
            incumbent_probability,
            incumbent_decision,
            candidate,
            candidate_digest: self.candidate.digest(),
            decisions_agree,
        })
    }
}

/// Totals for a candidate shadowing a decision made outside this crate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ExternalCounters {
    pub scored: u64,
    pub unavailable: u64,
    pub agree: u64,
    pub disagree: u64,
    /// The candidate abstained: counted as neither agreement nor disagreement.
    pub candidate_abstain: u64,
}

/// A candidate shadowing a presence decision it does not own, e.g. the
/// sensing server's per-node classification (ADR-426 D5). It owns its head and
/// scratch, so a server can hold one for its lifetime. Like [`ShadowScorer`],
/// nothing it returns can become a published result.
#[derive(Debug)]
pub struct ExternalShadow {
    candidate: FannHead,
    scorer: HeadScorer,
    counters: ExternalCounters,
}

impl ExternalShadow {
    /// The candidate must have been fitted for exactly `context`.
    pub fn new(candidate: FannHead, context: &HeadContext) -> Result<Self, HeadError> {
        candidate.require_context(context)?;
        let scorer = candidate.scorer()?;
        Ok(Self {
            candidate,
            scorer,
            counters: ExternalCounters::default(),
        })
    }

    pub fn counters(&self) -> ExternalCounters {
        self.counters
    }

    pub fn digest(&self) -> &str {
        self.candidate.digest()
    }

    pub fn features(&self) -> usize {
        self.candidate.features()
    }

    /// Score one observation the owner already decided as `incumbent_occupied`.
    pub fn score(&mut self, features: &[f32], incumbent_occupied: bool) -> CandidateOutcome {
        match self.candidate.score(&mut self.scorer, features) {
            Ok(probability) => {
                let decision = decide(probability);
                self.counters.scored += 1;
                match decision {
                    PresenceDecision::Abstain => self.counters.candidate_abstain += 1,
                    PresenceDecision::Occupied if incumbent_occupied => self.counters.agree += 1,
                    PresenceDecision::Empty if !incumbent_occupied => self.counters.agree += 1,
                    _ => self.counters.disagree += 1,
                }
                CandidateOutcome::Scored {
                    probability,
                    decision,
                }
            }
            Err(reason) => {
                self.counters.unavailable += 1;
                CandidateOutcome::Unavailable { reason }
            }
        }
    }
}
