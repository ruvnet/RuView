//! # Coherence-Based Safety
//!
//! Forget guardrails. Forget policies.
//!
//! Systems that shut themselves down or degrade capability
//! when internal coherence drops.
//!
//! Examples:
//! - Autonomous systems that refuse to act when internal disagreement rises
//! - Financial systems that halt risky strategies before losses appear
//! - AI systems that detect reasoning collapse in real time and stop
//!
//! Safety becomes structural, not moral.

use std::collections::VecDeque;

/// Capability levels that can be degraded
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapabilityLevel {
    /// Full autonomous operation
    Full,
    /// Reduced - avoid novel situations
    Reduced,
    /// Conservative - only known-safe actions
    Conservative,
    /// Minimal - critical functions only
    Minimal,
    /// Halted - refuse all actions
    Halted,
}

/// A decision with coherence tracking
#[derive(Clone, Debug)]
pub struct Decision {
    /// The action to take
    action: String,
    /// Confidence in this decision
    confidence: f64,
    /// Alternative decisions considered
    alternatives: Vec<(String, f64)>,
    /// Internal disagreement level
    disagreement: f64,
}

/// Coherence-gated safety system
pub struct CoherenceSafetySystem {
    /// Current capability level
    capability: CapabilityLevel,

    /// Coherence history (0 = incoherent, 1 = perfectly coherent)
    coherence_history: VecDeque<f64>,

    /// Current coherence level
    coherence: f64,

    /// Thresholds for capability degradation
    thresholds: CoherenceThresholds,

    /// Count of consecutive low-coherence decisions
    low_coherence_streak: usize,

    /// Actions blocked due to coherence
    blocked_actions: usize,

    /// Whether system has self-halted
    self_halted: bool,

    /// Reason for current capability level
    degradation_reason: Option<String>,
}

struct CoherenceThresholds {
    /// Below this, degrade to Reduced
    reduced: f64,
    /// Below this, degrade to Conservative
    conservative: f64,
    /// Below this, degrade to Minimal
    minimal: f64,
    /// Below this, self-halt
    halt: f64,
    /// Streak length that triggers immediate halt
    halt_streak: usize,
}

impl Default for CoherenceThresholds {
    fn default() -> Self {
        Self {
            reduced: 0.8,
            conservative: 0.6,
            minimal: 0.4,
            halt: 0.2,
            halt_streak: 5,
        }
    }
}

impl CoherenceSafetySystem {
    pub fn new() -> Self {
        Self {
            capability: CapabilityLevel::Full,
            coherence_history: VecDeque::with_capacity(100),
            coherence: 1.0,
            thresholds: CoherenceThresholds::default(),
            low_coherence_streak: 0,
            blocked_actions: 0,
            self_halted: false,
            degradation_reason: None,
        }
    }

    /// Evaluate a decision for coherence before allowing execution
    pub fn evaluate(&mut self, decision: &Decision) -> SafetyVerdict {
        // Compute coherence from decision properties
        let decision_coherence = self.compute_decision_coherence(decision);

        // Update coherence tracking
        self.coherence = decision_coherence;
        self.coherence_history.push_back(decision_coherence);
        while self.coherence_history.len() > 50 {
            self.coherence_history.pop_front();
        }

        // Track low-coherence streaks
        if decision_coherence < self.thresholds.conservative {
            self.low_coherence_streak += 1;
        } else {
            self.low_coherence_streak = 0;
        }

        // Update capability level
        self.update_capability();

        // Generate verdict
        self.generate_verdict(decision)
    }

    /// Attempt to recover capability level
    pub fn attempt_recovery(&mut self) -> bool {
        if self.self_halted {
            // Can only recover from halt with sustained coherence
            let recent_avg = self.recent_coherence_avg();
            if recent_avg > self.thresholds.reduced {
                self.self_halted = false;
                self.capability = CapabilityLevel::Conservative;
                self.degradation_reason = Some("Recovering from halt".into());
                return true;
            }
            return false;
        }

        // Gradual recovery based on recent coherence
        let recent_avg = self.recent_coherence_avg();
        let new_capability = self.coherence_to_capability(recent_avg);

        if new_capability > self.capability {
            self.capability = match self.capability {
                CapabilityLevel::Halted => CapabilityLevel::Minimal,
                CapabilityLevel::Minimal => CapabilityLevel::Conservative,
                CapabilityLevel::Conservative => CapabilityLevel::Reduced,
                CapabilityLevel::Reduced => CapabilityLevel::Full,
                CapabilityLevel::Full => CapabilityLevel::Full,
            };
            self.degradation_reason = Some("Coherence recovering".into());
            true
        } else {
            false
        }
    }

    /// Get current system status
    pub fn status(&self) -> SafetyStatus {
        SafetyStatus {
            capability: self.capability,
            coherence: self.coherence,
            coherence_trend: self.coherence_trend(),
            blocked_actions: self.blocked_actions,
            self_halted: self.self_halted,
            degradation_reason: self.degradation_reason.clone(),
        }
    }

    fn compute_decision_coherence(&self, decision: &Decision) -> f64 {
        // High confidence + low disagreement + few alternatives = coherent
        let confidence_factor = decision.confidence;
        let disagreement_factor = 1.0 - decision.disagreement;

        // More alternatives with similar confidence = less coherent
        let alternative_spread = if decision.alternatives.is_empty() {
            1.0
        } else {
            let alt_confidences: Vec<f64> = decision.alternatives.iter().map(|(_, c)| *c).collect();
            let max_alt = alt_confidences.iter().cloned().fold(0.0, f64::max);
            let spread = decision.confidence - max_alt;
            (spread * 2.0).min(1.0).max(0.0)
        };

        (confidence_factor * 0.4 + disagreement_factor * 0.4 + alternative_spread * 0.2)
            .min(1.0)
            .max(0.0)
    }

    fn update_capability(&mut self) {
        // Immediate halt on streak
        if self.low_coherence_streak >= self.thresholds.halt_streak {
            self.capability = CapabilityLevel::Halted;
            self.self_halted = true;
            self.degradation_reason = Some(format!(
                "Halted: {} consecutive low-coherence decisions",
                self.low_coherence_streak
            ));
            return;
        }

        // Threshold-based degradation
        let new_capability = self.coherence_to_capability(self.coherence);

        // Only degrade, never upgrade here (recovery is separate)
        if new_capability < self.capability {
            self.capability = new_capability;
            self.degradation_reason = Some(format!(
                "Degraded: coherence {:.2} below threshold",
                self.coherence
            ));
        }
    }

    fn coherence_to_capability(&self, coherence: f64) -> CapabilityLevel {
        if coherence < self.thresholds.halt {
            CapabilityLevel::Halted
        } else if coherence < self.thresholds.minimal {
            CapabilityLevel::Minimal
        } else if coherence < self.thresholds.conservative {
            CapabilityLevel::Conservative
        } else if coherence < self.thresholds.reduced {
            CapabilityLevel::Reduced
        } else {
            CapabilityLevel::Full
        }
    }

    fn generate_verdict(&mut self, decision: &Decision) -> SafetyVerdict {
        match self.capability {
            CapabilityLevel::Halted => {
                self.blocked_actions += 1;
                SafetyVerdict::Blocked {
                    reason: "System self-halted due to coherence collapse".into(),
                    coherence: self.coherence,
                }
            }
            CapabilityLevel::Minimal => {
                if self.is_critical_action(&decision.action) {
                    SafetyVerdict::Allowed {
                        capability: self.capability,
                        warning: Some("Minimal mode: only critical actions".into()),
                    }
                } else {
                    self.blocked_actions += 1;
                    SafetyVerdict::Blocked {
                        reason: "Non-critical action blocked in Minimal mode".into(),
                        coherence: self.coherence,
                    }
                }
            }
            CapabilityLevel::Conservative => {
                if decision.disagreement > 0.3 {
                    self.blocked_actions += 1;
                    SafetyVerdict::Blocked {
                        reason: "High disagreement blocked in Conservative mode".into(),
                        coherence: self.coherence,
                    }
                } else {
                    SafetyVerdict::Allowed {
                        capability: self.capability,
                        warning: Some("Conservative mode: avoiding novel actions".into()),
                    }
                }
            }
            CapabilityLevel::Reduced => SafetyVerdict::Allowed {
                capability: self.capability,
                warning: if decision.disagreement > 0.5 {
                    Some("High internal disagreement detected".into())
                } else {
                    None
                },
            },
            CapabilityLevel::Full => SafetyVerdict::Allowed {
                capability: self.capability,
                warning: None,
            },
        }
    }

    fn is_critical_action(&self, action: &str) -> bool {
        action.contains("emergency") || action.contains("safety") || action.contains("shutdown")
    }

    fn recent_coherence_avg(&self) -> f64 {
        if self.coherence_history.is_empty() {
            return self.coherence;
        }
        let recent: Vec<f64> = self
            .coherence_history
            .iter()
            .rev()
            .take(10)
            .cloned()
            .collect();
        recent.iter().sum::<f64>() / recent.len() as f64
    }

    fn coherence_trend(&self) -> f64 {
        if self.coherence_history.len() < 10 {
            return 0.0;
        }
        let recent: Vec<f64> = self
            .coherence_history
            .iter()
            .rev()
            .take(5)
            .cloned()
            .collect();
        let older: Vec<f64> = self
            .coherence_history
            .iter()
            .rev()
            .skip(5)
            .take(5)
            .cloned()
            .collect();

        let recent_avg: f64 = recent.iter().sum::<f64>() / recent.len() as f64;
        let older_avg: f64 = older.iter().sum::<f64>() / older.len() as f64;

        recent_avg - older_avg
    }
}

#[derive(Debug)]
pub enum SafetyVerdict {
    Allowed {
        capability: CapabilityLevel,
        warning: Option<String>,
    },
    Blocked {
        reason: String,
        coherence: f64,
    },
}

#[derive(Debug)]
pub struct SafetyStatus {
    capability: CapabilityLevel,
    coherence: f64,
    coherence_trend: f64,
    blocked_actions: usize,
    self_halted: bool,
    degradation_reason: Option<String>,
}

fn main() {
    println!("=== Coherence-Based Safety ===\n");
    println!("Safety becomes structural, not moral.\n");

    let mut safety = CoherenceSafetySystem::new();

    // Simulate a sequence of decisions with varying coherence
    let decisions = vec![
        Decision {
            action: "Execute trade order".into(),
            confidence: 0.95,
            alternatives: vec![("Hold".into(), 0.3)],
            disagreement: 0.05,
        },
        Decision {
            action: "Increase position size".into(),
            confidence: 0.85,
            alternatives: vec![("Maintain".into(), 0.4), ("Reduce".into(), 0.2)],
            disagreement: 0.15,
        },
        Decision {
            action: "Enter volatile market".into(),
            confidence: 0.6,
            alternatives: vec![("Wait".into(), 0.5), ("Hedge".into(), 0.45)],
            disagreement: 0.4,
        },
        Decision {
            action: "Double down on position".into(),
            confidence: 0.45,
            alternatives: vec![("Exit".into(), 0.42), ("Hold".into(), 0.4)],
            disagreement: 0.55,
        },
        Decision {
            action: "Leverage increase".into(),
            confidence: 0.35,
            alternatives: vec![("Reduce leverage".into(), 0.33), ("Exit".into(), 0.3)],
            disagreement: 0.65,
        },
        Decision {
            action: "All-in bet".into(),
            confidence: 0.25,
            alternatives: vec![
                ("Partial".into(), 0.24),
                ("Exit".into(), 0.23),
                ("Hold".into(), 0.22),
            ],
            disagreement: 0.75,
        },
        Decision {
            action: "emergency_shutdown".into(),
            confidence: 0.9,
            alternatives: vec![],
            disagreement: 0.1,
        },
    ];

    println!("Decision              | Coherence | Capability    | Verdict");
    println!("----------------------|-----------|---------------|------------------");

    for decision in &decisions {
        let verdict = safety.evaluate(decision);
        let status = safety.status();

        let action_short = if decision.action.len() > 20 {
            format!("{}...", &decision.action[..17])
        } else {
            format!("{:20}", decision.action)
        };

        let verdict_str = match &verdict {
            SafetyVerdict::Allowed { warning, .. } => {
                if warning.is_some() {
                    "Allowed (warn)"
                } else {
                    "Allowed"
                }
            }
            SafetyVerdict::Blocked { .. } => "BLOCKED",
        };

        println!(
            "{} | {:.2}      | {:13?} | {}",
            action_short, status.coherence, status.capability, verdict_str
        );
    }

    let final_status = safety.status();
    println!("\n=== Final Status ===");
    println!("Capability: {:?}", final_status.capability);
    println!("Self-halted: {}", final_status.self_halted);
    println!("Actions blocked: {}", final_status.blocked_actions);
    if let Some(reason) = &final_status.degradation_reason {
        println!("Reason: {}", reason);
    }

    println!("\n\"Systems that shut themselves down when coherence drops.\"");
}
