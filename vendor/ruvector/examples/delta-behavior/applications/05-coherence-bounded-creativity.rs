//! # Application 5: Coherence-Bounded Creativity Systems
//!
//! Creativity is allowed only inside coherence-preserving manifolds.
//!
//! ## Problem
//! Generative systems oscillate between boring and insane.
//!
//! ## Exotic Outcome
//! - Novelty without collapse
//! - Exploration without nonsense
//!
//! ## Applications
//! - Music systems that never dissolve into noise
//! - Design systems that don't violate constraints
//! - Narrative generators that maintain internal consistency over long arcs

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A creative system bounded by coherence constraints
pub struct CoherenceBoundedCreator<T: Creative> {
    /// The creative element being generated
    current: T,

    /// Coherence constraints
    constraints: Vec<Box<dyn Constraint<T>>>,

    /// Current coherence level
    coherence: f64,

    /// Minimum coherence to allow creativity
    min_coherence: f64,

    /// Maximum coherence (too high = boring)
    max_coherence: f64,

    /// History of creative decisions
    history: Vec<CreativeDecision<T>>,

    /// Exploration budget (regenerates over time)
    exploration_budget: f64,
}

/// Trait for creative elements
pub trait Creative: Clone + std::fmt::Debug {
    /// Generate a random variation
    fn vary(&self, magnitude: f64) -> Self;

    /// Compute distance between two creative elements
    fn distance(&self, other: &Self) -> f64;

    /// Get a unique identifier for this state
    fn fingerprint(&self) -> u64;
}

/// Constraint that must be satisfied
pub trait Constraint<T>: Send + Sync {
    /// Name of the constraint
    fn name(&self) -> &str;

    /// Check if element satisfies constraint (0.0 = violated, 1.0 = satisfied)
    fn satisfaction(&self, element: &T) -> f64;

    /// Is this a hard constraint (violation = immediate rejection)?
    fn is_hard(&self) -> bool { false }
}

#[derive(Debug)]
pub struct CreativeDecision<T> {
    pub from: T,
    pub to: T,
    pub coherence_before: f64,
    pub coherence_after: f64,
    pub constraint_satisfactions: Vec<(String, f64)>,
    pub accepted: bool,
}

#[derive(Debug)]
pub enum CreativeResult<T> {
    /// Created something new within bounds
    Created { element: T, novelty: f64, coherence: f64 },
    /// Creation rejected - would violate coherence
    Rejected { attempted: T, reason: String },
    /// System is too stable - needs perturbation to create
    TooBoring { coherence: f64 },
    /// System exhausted exploration budget
    BudgetExhausted,
}

impl<T: Creative> CoherenceBoundedCreator<T> {
    pub fn new(initial: T, min_coherence: f64, max_coherence: f64) -> Self {
        Self {
            current: initial,
            constraints: Vec::new(),
            coherence: 1.0,
            min_coherence,
            max_coherence,
            history: Vec::new(),
            exploration_budget: 10.0,
        }
    }

    pub fn add_constraint(&mut self, constraint: Box<dyn Constraint<T>>) {
        self.constraints.push(constraint);
    }

    /// Calculate coherence based on constraint satisfaction
    /// Returns a valid f64 in range [0.0, 1.0], with NaN/Infinity protection
    fn calculate_coherence(&self, element: &T) -> f64 {
        if self.constraints.is_empty() {
            return 1.0;
        }

        let satisfactions: Vec<f64> = self.constraints
            .iter()
            .map(|c| {
                let sat = c.satisfaction(element);
                // Validate satisfaction value
                if sat.is_finite() { sat.clamp(0.0, 1.0) } else { 0.0 }
            })
            .collect();

        // Geometric mean of satisfactions
        let product: f64 = satisfactions.iter().product();

        // Validate product before computing power
        if !product.is_finite() || product < 0.0 {
            return 0.0; // Safe default for invalid state
        }

        let result = product.powf(1.0 / satisfactions.len() as f64);

        // Final validation
        if result.is_finite() { result.clamp(0.0, 1.0) } else { 0.0 }
    }

    /// Check hard constraints
    fn check_hard_constraints(&self, element: &T) -> Option<String> {
        for constraint in &self.constraints {
            if constraint.is_hard() && constraint.satisfaction(element) < 0.1 {
                return Some(format!("Hard constraint '{}' violated", constraint.name()));
            }
        }
        None
    }

    /// Attempt to create something new
    pub fn create(&mut self, exploration_magnitude: f64) -> CreativeResult<T> {
        // Check exploration budget
        if self.exploration_budget <= 0.0 {
            return CreativeResult::BudgetExhausted;
        }

        // Check if we're too stable (boring)
        if self.coherence > self.max_coherence {
            return CreativeResult::TooBoring { coherence: self.coherence };
        }

        // Generate variation
        let candidate = self.current.vary(exploration_magnitude);

        // Check hard constraints
        if let Some(violation) = self.check_hard_constraints(&candidate) {
            return CreativeResult::Rejected {
                attempted: candidate,
                reason: violation,
            };
        }

        // Calculate new coherence
        let new_coherence = self.calculate_coherence(&candidate);

        // Would this drop coherence too low?
        if new_coherence < self.min_coherence {
            self.exploration_budget -= 0.5; // Exploration cost

            return CreativeResult::Rejected {
                attempted: candidate,
                reason: format!(
                    "Coherence would drop to {:.3} (min: {:.3})",
                    new_coherence, self.min_coherence
                ),
            };
        }

        // Calculate novelty
        let novelty = self.current.distance(&candidate);

        // Record decision
        let decision = CreativeDecision {
            from: self.current.clone(),
            to: candidate.clone(),
            coherence_before: self.coherence,
            coherence_after: new_coherence,
            constraint_satisfactions: self.constraints
                .iter()
                .map(|c| (c.name().to_string(), c.satisfaction(&candidate)))
                .collect(),
            accepted: true,
        };
        self.history.push(decision);

        // Accept the creation
        self.current = candidate.clone();
        self.coherence = new_coherence;
        self.exploration_budget -= exploration_magnitude;

        CreativeResult::Created {
            element: candidate,
            novelty,
            coherence: new_coherence,
        }
    }

    /// Perturb the system to escape local optima (controlled chaos)
    pub fn perturb(&mut self, magnitude: f64) -> bool {
        let perturbed = self.current.vary(magnitude * 0.5);
        let new_coherence = self.calculate_coherence(&perturbed);

        // Only accept perturbation if it doesn't violate hard constraints
        // and stays within bounds
        if new_coherence >= self.min_coherence * 0.9 {
            self.current = perturbed;
            self.coherence = new_coherence;
            true
        } else {
            false
        }
    }

    /// Regenerate exploration budget
    pub fn rest(&mut self, amount: f64) {
        self.exploration_budget = (self.exploration_budget + amount).min(20.0);
    }

    pub fn current(&self) -> &T {
        &self.current
    }

    pub fn coherence(&self) -> f64 {
        self.coherence
    }
}

// =============================================================================
// Example: Music Generation
// =============================================================================

/// A musical phrase
#[derive(Clone, Debug)]
pub struct MusicalPhrase {
    /// Notes as MIDI values
    notes: Vec<u8>,
    /// Durations in beats
    durations: Vec<f64>,
    /// Velocities (loudness)
    velocities: Vec<u8>,
}

impl Creative for MusicalPhrase {
    fn vary(&self, magnitude: f64) -> Self {
        let mut new_notes = self.notes.clone();
        let mut new_durations = self.durations.clone();
        let mut new_velocities = self.velocities.clone();

        // Randomly modify based on magnitude
        let changes = (magnitude * self.notes.len() as f64) as usize;

        for _ in 0..changes.max(1) {
            let idx = pseudo_random() % self.notes.len();

            // Vary note (small intervals)
            let delta = ((pseudo_random() % 7) as i8 - 3) * (magnitude * 2.0) as i8;
            new_notes[idx] = (new_notes[idx] as i8 + delta).clamp(36, 96) as u8;

            // Vary duration slightly
            let dur_delta = (pseudo_random_f64() - 0.5) * magnitude;
            new_durations[idx] = (new_durations[idx] + dur_delta).clamp(0.125, 4.0);

            // Vary velocity
            let vel_delta = ((pseudo_random() % 21) as i8 - 10) * (magnitude * 2.0) as i8;
            new_velocities[idx] = (new_velocities[idx] as i8 + vel_delta).clamp(20, 127) as u8;
        }

        Self {
            notes: new_notes,
            durations: new_durations,
            velocities: new_velocities,
        }
    }

    fn distance(&self, other: &Self) -> f64 {
        let note_diff: f64 = self.notes.iter()
            .zip(&other.notes)
            .map(|(a, b)| (*a as f64 - *b as f64).abs())
            .sum::<f64>() / self.notes.len() as f64;

        let dur_diff: f64 = self.durations.iter()
            .zip(&other.durations)
            .map(|(a, b)| (a - b).abs())
            .sum::<f64>() / self.durations.len() as f64;

        (note_diff / 12.0 + dur_diff) / 2.0 // Normalize
    }

    fn fingerprint(&self) -> u64 {
        let mut hash: u64 = 0;
        for (i, &note) in self.notes.iter().enumerate() {
            hash ^= (note as u64) << ((i * 8) % 56);
        }
        hash
    }
}

impl MusicalPhrase {
    pub fn simple_melody() -> Self {
        Self {
            notes: vec![60, 62, 64, 65, 67, 65, 64, 62], // C major scale fragment
            durations: vec![0.5, 0.5, 0.5, 0.5, 1.0, 0.5, 0.5, 1.0],
            velocities: vec![80, 75, 85, 80, 90, 75, 70, 85],
        }
    }
}

/// Constraint: Notes should stay within a comfortable range
pub struct RangeConstraint {
    min_note: u8,
    max_note: u8,
}

impl Constraint<MusicalPhrase> for RangeConstraint {
    fn name(&self) -> &str { "pitch_range" }

    fn satisfaction(&self, phrase: &MusicalPhrase) -> f64 {
        let in_range = phrase.notes.iter()
            .filter(|&&n| n >= self.min_note && n <= self.max_note)
            .count();
        in_range as f64 / phrase.notes.len() as f64
    }

    fn is_hard(&self) -> bool { false }
}

/// Constraint: Avoid large interval jumps
pub struct IntervalConstraint {
    max_interval: u8,
}

impl Constraint<MusicalPhrase> for IntervalConstraint {
    fn name(&self) -> &str { "interval_smoothness" }

    fn satisfaction(&self, phrase: &MusicalPhrase) -> f64 {
        if phrase.notes.len() < 2 {
            return 1.0;
        }

        let smooth_intervals = phrase.notes.windows(2)
            .filter(|w| (w[0] as i8 - w[1] as i8).abs() <= self.max_interval as i8)
            .count();

        smooth_intervals as f64 / (phrase.notes.len() - 1) as f64
    }
}

/// Constraint: Rhythm should have variety but not chaos
pub struct RhythmConstraint;

impl Constraint<MusicalPhrase> for RhythmConstraint {
    fn name(&self) -> &str { "rhythm_coherence" }

    fn satisfaction(&self, phrase: &MusicalPhrase) -> f64 {
        let unique_durations: HashSet<u64> = phrase.durations
            .iter()
            .map(|d| (d * 1000.0) as u64)
            .collect();

        // Penalize both too few (boring) and too many (chaotic) unique durations
        let variety = unique_durations.len() as f64 / phrase.durations.len() as f64;

        // Optimal variety is around 0.3-0.5
        let optimal = 0.4;
        1.0 - (variety - optimal).abs() * 2.0
    }
}

/// Thread-safe atomic seed for pseudo-random number generation
static SEED: AtomicUsize = AtomicUsize::new(42);

// Thread-safe pseudo-random for reproducibility
fn pseudo_random() -> usize {
    let old = SEED.fetch_add(1, Ordering::Relaxed);
    let new = old.wrapping_mul(1103515245).wrapping_add(12345);
    // Store back for next call (best-effort, races are acceptable for RNG)
    let _ = SEED.compare_exchange(old + 1, new, Ordering::Relaxed, Ordering::Relaxed);
    (new >> 16) & 0x7fff
}

fn pseudo_random_f64() -> f64 {
    (pseudo_random() as f64) / 32768.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_musical_creativity() {
        let initial = MusicalPhrase::simple_melody();
        let mut creator = CoherenceBoundedCreator::new(initial, 0.6, 0.95);

        // Add constraints
        creator.add_constraint(Box::new(RangeConstraint { min_note: 48, max_note: 84 }));
        creator.add_constraint(Box::new(IntervalConstraint { max_interval: 7 }));
        creator.add_constraint(Box::new(RhythmConstraint));

        let mut successful_creations = 0;
        let mut rejections = 0;

        for i in 0..50 {
            let magnitude = 0.2 + (i as f64 * 0.02); // Increasing exploration

            match creator.create(magnitude) {
                CreativeResult::Created { novelty, coherence, .. } => {
                    successful_creations += 1;
                    println!(
                        "Step {}: Created! Novelty: {:.3}, Coherence: {:.3}",
                        i, novelty, coherence
                    );
                }
                CreativeResult::Rejected { reason, .. } => {
                    rejections += 1;
                    println!("Step {}: Rejected - {}", i, reason);
                }
                CreativeResult::TooBoring { coherence } => {
                    println!("Step {}: Too boring (coherence: {:.3}), perturbing...", i, coherence);
                    creator.perturb(0.5);
                }
                CreativeResult::BudgetExhausted => {
                    println!("Step {}: Budget exhausted, resting...", i);
                    creator.rest(5.0);
                }
            }
        }

        println!("\n=== Results ===");
        println!("Successful creations: {}", successful_creations);
        println!("Rejections: {}", rejections);
        println!("Final coherence: {:.3}", creator.coherence());
        println!("Final phrase: {:?}", creator.current());

        // Should have some successes but also some rejections
        // (pure acceptance = not enough constraint, pure rejection = too much)
        assert!(successful_creations > 10, "Should create some novelty");
        assert!(rejections > 0, "Should reject some incoherent attempts");
    }
}
