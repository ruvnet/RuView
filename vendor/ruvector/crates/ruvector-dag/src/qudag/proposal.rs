//! Pattern Proposal System

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternProposal {
    pub pattern_vector: Vec<f32>,
    pub metadata: serde_json::Value,
    pub quality_score: f64,
    pub noise_epsilon: Option<f64>, // Differential privacy
}

impl PatternProposal {
    pub fn new(pattern_vector: Vec<f32>, metadata: serde_json::Value, quality_score: f64) -> Self {
        Self {
            pattern_vector,
            metadata,
            quality_score,
            noise_epsilon: None,
        }
    }

    pub fn with_differential_privacy(mut self, epsilon: f64) -> Self {
        self.noise_epsilon = Some(epsilon);
        // Add Laplace noise to pattern
        self.add_laplace_noise(epsilon);
        self
    }

    fn add_laplace_noise(&mut self, epsilon: f64) {
        let scale = 1.0 / epsilon;
        for v in &mut self.pattern_vector {
            // Simple approximation of Laplace noise
            let u: f64 = rand::random::<f64>() - 0.5;
            let noise = -scale * u.signum() * (1.0 - 2.0 * u.abs()).ln();
            *v += noise as f32;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pending,
    Voting,
    Accepted,
    Rejected,
    Finalized,
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProposalStatus::Pending => write!(f, "pending"),
            ProposalStatus::Voting => write!(f, "voting"),
            ProposalStatus::Accepted => write!(f, "accepted"),
            ProposalStatus::Rejected => write!(f, "rejected"),
            ProposalStatus::Finalized => write!(f, "finalized"),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ProposalResult {
    pub proposal_id: String,
    pub status: ProposalStatus,
    pub votes_for: u64,
    pub votes_against: u64,
    pub finalized_at: Option<std::time::SystemTime>,
}
