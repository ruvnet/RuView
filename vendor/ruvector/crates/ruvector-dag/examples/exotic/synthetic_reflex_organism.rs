//! # Synthetic Reflex Organism
//!
//! A system that behaves like a simple organism:
//! - No global objective function
//! - Only minimizes structural stress over time
//! - Appears calm most of the time
//! - Spikes briefly when something meaningful happens
//! - Learns only when instability crosses thresholds
//!
//! This is not intelligence as problem-solving.
//! This is intelligence as homeostasis.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// The organism's internal state - no goals, only coherence
pub struct ReflexOrganism {
    /// Current tension level (0.0 = calm, 1.0 = crisis)
    tension: f32,

    /// Tension history for detecting spikes
    tension_history: VecDeque<(Instant, f32)>,

    /// Resting tension threshold - below this, organism is calm
    resting_threshold: f32,

    /// Learning threshold - only learn when tension exceeds this
    learning_threshold: f32,

    /// Current metabolic rate (activity level)
    metabolic_rate: f32,

    /// Accumulated stress over time
    accumulated_stress: f32,

    /// Internal coherence patterns learned from instability
    coherence_patterns: Vec<CoherencePattern>,
}

/// A pattern learned during high-tension moments
struct CoherencePattern {
    /// What the tension signature looked like
    tension_signature: Vec<f32>,
    /// How the organism responded
    response: OrganismResponse,
    /// How effective was this response (0-1)
    efficacy: f32,
}

#[derive(Clone, Debug)]
enum OrganismResponse {
    /// Do nothing, wait for coherence to return
    Rest,
    /// Reduce activity, conserve resources
    Contract,
    /// Increase activity, explore solutions
    Expand,
    /// Isolate affected subsystems
    Partition,
    /// Redistribute load across subsystems
    Rebalance,
}

impl ReflexOrganism {
    pub fn new() -> Self {
        Self {
            tension: 0.0,
            tension_history: VecDeque::with_capacity(1000),
            resting_threshold: 0.2,
            learning_threshold: 0.6,
            metabolic_rate: 0.1, // Calm baseline
            accumulated_stress: 0.0,
            coherence_patterns: Vec::new(),
        }
    }

    /// Observe external stimulus and update internal tension
    /// The organism doesn't "process" data - it feels structural stress
    pub fn observe(&mut self, mincut_tension: f32, coherence_delta: f32) {
        let now = Instant::now();

        // Tension is a blend of external signal and internal state
        let external_stress = mincut_tension;
        let internal_stress = self.accumulated_stress * 0.1;
        let delta_stress = coherence_delta.abs() * 0.5;

        self.tension = (external_stress + internal_stress + delta_stress).min(1.0);
        self.tension_history.push_back((now, self.tension));

        // Prune old history (keep last 10 seconds)
        while let Some((t, _)) = self.tension_history.front() {
            if now.duration_since(*t) > Duration::from_secs(10) {
                self.tension_history.pop_front();
            } else {
                break;
            }
        }

        // Update metabolic rate based on tension
        self.metabolic_rate = self.compute_metabolic_response();

        // Accumulate or release stress
        if self.tension > self.resting_threshold {
            self.accumulated_stress += self.tension * 0.01;
        } else {
            self.accumulated_stress *= 0.95; // Slow release when calm
        }
    }

    /// The organism's reflex response - no planning, just reaction
    pub fn reflex(&mut self) -> OrganismResponse {
        // Below resting threshold: do nothing
        if self.tension < self.resting_threshold {
            return OrganismResponse::Rest;
        }

        // Check if we have a learned pattern for this tension signature
        let current_signature = self.current_tension_signature();
        if let Some(pattern) = self.find_matching_pattern(&current_signature) {
            if pattern.efficacy > 0.7 {
                return pattern.response.clone();
            }
        }

        // No learned pattern - use instinctive response
        match self.tension {
            t if t < 0.4 => OrganismResponse::Contract,
            t if t < 0.7 => OrganismResponse::Rebalance,
            _ => OrganismResponse::Partition,
        }
    }

    /// Learn from a tension episode - only when threshold exceeded
    pub fn maybe_learn(&mut self, response_taken: OrganismResponse, outcome_tension: f32) {
        // Only learn during significant instability
        if self.tension < self.learning_threshold {
            return;
        }

        let signature = self.current_tension_signature();
        let efficacy = 1.0 - outcome_tension; // Lower resulting tension = better

        // Check if we already have this pattern
        if let Some(pattern) = self.find_matching_pattern_mut(&signature) {
            // Update existing pattern with exponential moving average
            pattern.efficacy = pattern.efficacy * 0.9 + efficacy * 0.1;
            if efficacy > pattern.efficacy {
                pattern.response = response_taken;
            }
        } else {
            // New pattern
            self.coherence_patterns.push(CoherencePattern {
                tension_signature: signature,
                response: response_taken,
                efficacy,
            });
        }

        println!(
            "[LEARN] Tension={:.2}, Efficacy={:.2}, Patterns={}",
            self.tension,
            efficacy,
            self.coherence_patterns.len()
        );
    }

    /// Is the organism in a calm state?
    pub fn is_calm(&self) -> bool {
        self.tension < self.resting_threshold && self.accumulated_stress < 0.1
    }

    /// Is the organism experiencing a spike?
    pub fn is_spiking(&self) -> bool {
        if self.tension_history.len() < 10 {
            return false;
        }

        let recent: Vec<f32> = self
            .tension_history
            .iter()
            .rev()
            .take(5)
            .map(|(_, t)| *t)
            .collect();
        let older: Vec<f32> = self
            .tension_history
            .iter()
            .rev()
            .skip(5)
            .take(5)
            .map(|(_, t)| *t)
            .collect();

        let recent_avg: f32 = recent.iter().sum::<f32>() / recent.len() as f32;
        let older_avg: f32 = older.iter().sum::<f32>() / older.len() as f32;

        recent_avg > older_avg * 1.5 // 50% increase = spike
    }

    fn compute_metabolic_response(&self) -> f32 {
        // Metabolic rate follows tension with damping
        let target = self.tension * 0.8 + 0.1; // Never fully dormant
        self.metabolic_rate * 0.9 + target * 0.1
    }

    fn current_tension_signature(&self) -> Vec<f32> {
        self.tension_history
            .iter()
            .rev()
            .take(10)
            .map(|(_, t)| *t)
            .collect()
    }

    fn find_matching_pattern(&self, signature: &[f32]) -> Option<&CoherencePattern> {
        self.coherence_patterns
            .iter()
            .find(|p| Self::signature_similarity(&p.tension_signature, signature) > 0.8)
    }

    fn find_matching_pattern_mut(&mut self, signature: &[f32]) -> Option<&mut CoherencePattern> {
        self.coherence_patterns
            .iter_mut()
            .find(|p| Self::signature_similarity(&p.tension_signature, signature) > 0.8)
    }

    fn signature_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let len = a.len().min(b.len());
        let diff: f32 = a
            .iter()
            .zip(b.iter())
            .take(len)
            .map(|(x, y)| (x - y).abs())
            .sum();
        1.0 - (diff / len as f32).min(1.0)
    }
}

fn main() {
    println!("=== Synthetic Reflex Organism ===\n");
    println!("No goals. No objectives. Only homeostasis.\n");

    let mut organism = ReflexOrganism::new();

    // Simulate external perturbations
    let perturbations = [
        // (mincut_tension, coherence_delta, description)
        (0.1, 0.0, "Calm baseline"),
        (0.15, 0.02, "Minor fluctuation"),
        (0.1, -0.01, "Returning to calm"),
        (0.5, 0.3, "Sudden stress spike"),
        (0.6, 0.1, "Stress continues"),
        (0.7, 0.15, "Peak tension"),
        (0.55, -0.1, "Beginning recovery"),
        (0.3, -0.2, "Stress releasing"),
        (0.15, -0.1, "Approaching calm"),
        (0.1, 0.0, "Calm restored"),
        (0.8, 0.5, "Major crisis"),
        (0.9, 0.1, "Crisis peak"),
        (0.7, -0.15, "Crisis subsiding"),
        (0.4, -0.25, "Recovery"),
        (0.15, -0.1, "Calm again"),
    ];

    println!("Time | Tension | State     | Response      | Metabolic");
    println!("-----|---------|-----------|---------------|----------");

    for (i, (mincut, delta, desc)) in perturbations.iter().enumerate() {
        organism.observe(*mincut, *delta);
        let response = organism.reflex();

        let state = if organism.is_calm() {
            "Calm"
        } else if organism.is_spiking() {
            "SPIKE"
        } else {
            "Active"
        };

        println!(
            "{:4} | {:.2}    | {:9} | {:13?} | {:.2}  <- {}",
            i, organism.tension, state, response, organism.metabolic_rate, desc
        );

        // Simulate response outcome and maybe learn
        let outcome = organism.tension * 0.7; // Response reduces tension by 30%
        organism.maybe_learn(response, outcome);

        std::thread::sleep(Duration::from_millis(100));
    }

    println!("\n=== Organism Summary ===");
    println!("Learned patterns: {}", organism.coherence_patterns.len());
    println!(
        "Final accumulated stress: {:.3}",
        organism.accumulated_stress
    );
    println!(
        "Current state: {}",
        if organism.is_calm() { "Calm" } else { "Active" }
    );

    println!("\n\"Intelligence as homeostasis, not problem-solving.\"");
}
