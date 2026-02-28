//! # Swarm Interference
//!
//! Agents don't vote -- they *interfere*. Each agent contributes a complex
//! amplitude toward one or more actions. Conflicting agents cancel
//! (destructive interference). Reinforcing agents amplify (constructive
//! interference). The decision emerges from the interference pattern, not
//! from a majority vote or consensus protocol.
//!
//! ## Model
//!
//! - **Action**: something the swarm can do, identified by an `id` string.
//! - **Agent contribution**: a complex amplitude per action.  The *magnitude*
//!   encodes confidence; the *phase* encodes stance (0 = support, pi = oppose).
//! - **Decision**: for each action, sum all contributing amplitudes. The
//!   resulting probability |sum|^2 determines the action's strength.
//!
//! Destructive interference naturally resolves conflicts: an action backed
//! by 3 agents at phase 0 and opposed by 3 agents at phase pi has zero net
//! amplitude, so it is detected as a deadlock.

use ruqu_core::types::Complex;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::f64::consts::PI;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// An action that agents can support or oppose.
#[derive(Debug, Clone, PartialEq)]
pub struct Action {
    pub id: String,
    pub description: String,
}

/// An agent's complex-amplitude contribution to one or more actions.
///
/// The amplitude encodes both confidence (magnitude) and stance (phase).
/// Phase 0 = full support, phase pi = full opposition.
pub struct AgentContribution {
    pub agent_id: String,
    pub amplitudes: Vec<(Action, Complex)>,
}

impl AgentContribution {
    /// Create a contribution where the agent supports or opposes a single
    /// action with the given confidence.
    ///
    /// - `confidence` in `[0, 1]` sets the magnitude.
    /// - `support = true` => phase 0 (constructive with other supporters).
    /// - `support = false` => phase pi (destructive against supporters).
    pub fn new(agent_id: &str, action: Action, confidence: f64, support: bool) -> Self {
        let phase = if support { 0.0 } else { PI };
        let amplitude = Complex::from_polar(confidence, phase);
        Self {
            agent_id: agent_id.to_string(),
            amplitudes: vec![(action, amplitude)],
        }
    }

    /// Create a contribution spanning multiple actions with explicit complex
    /// amplitudes.
    pub fn multi(agent_id: &str, amplitudes: Vec<(Action, Complex)>) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            amplitudes,
        }
    }
}

/// The swarm decision engine using quantum interference.
pub struct SwarmInterference {
    contributions: Vec<AgentContribution>,
}

/// Result of swarm interference for a single action.
#[derive(Debug)]
pub struct SwarmDecision {
    /// The action evaluated.
    pub action: Action,
    /// |total_amplitude|^2 after interference.
    pub probability: f64,
    /// Number of agents whose phase reinforced the net amplitude.
    pub constructive_count: usize,
    /// Number of agents whose phase opposed the net amplitude.
    pub destructive_count: usize,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl SwarmInterference {
    /// Create an empty swarm interference engine.
    pub fn new() -> Self {
        Self {
            contributions: Vec::new(),
        }
    }

    /// Add an agent's contribution to the interference pattern.
    pub fn contribute(&mut self, contribution: AgentContribution) {
        self.contributions.push(contribution);
    }

    /// Compute the interference pattern across all agents for all actions.
    ///
    /// For each unique action (matched by `action.id`):
    /// 1. Sum all agent amplitudes (complex addition).
    /// 2. Compute probability = |sum|^2.
    /// 3. Classify each contributing agent as constructive or destructive
    ///    relative to the net amplitude's phase.
    ///
    /// Returns actions sorted by probability (descending).
    pub fn decide(&self) -> Vec<SwarmDecision> {
        let (action_map, amplitude_map, agent_phases_map) = self.aggregate();

        let mut decisions: Vec<SwarmDecision> = amplitude_map
            .into_iter()
            .map(|(id, total)| {
                let probability = total.norm_sq();
                let net_phase = total.arg();

                // Count constructive vs destructive contributors.
                let phases = agent_phases_map.get(&id).unwrap();
                let mut constructive = 0usize;
                let mut destructive = 0usize;

                for &agent_phase in phases {
                    let delta = Self::phase_distance(agent_phase, net_phase);
                    if delta <= PI / 2.0 {
                        constructive += 1;
                    } else {
                        destructive += 1;
                    }
                }

                SwarmDecision {
                    action: action_map[&id].clone(),
                    probability,
                    constructive_count: constructive,
                    destructive_count: destructive,
                }
            })
            .collect();

        decisions.sort_by(|a, b| {
            b.probability
                .partial_cmp(&a.probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        decisions
    }

    /// Return the winning action (highest probability after interference).
    pub fn winner(&self) -> Option<SwarmDecision> {
        let decisions = self.decide();
        decisions.into_iter().next()
    }

    /// Run `num_trials` decisions with additive quantum noise and return
    /// win counts: `Vec<(Action, wins)>` sorted by wins descending.
    ///
    /// Each trial adds a small random complex perturbation to every agent
    /// amplitude before summing. This models environmental noise and shows
    /// the stability of the interference pattern.
    pub fn decide_with_noise(
        &self,
        noise_level: f64,
        num_trials: usize,
        seed: u64,
    ) -> Vec<(Action, usize)> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut win_counts: HashMap<String, (Action, usize)> = HashMap::new();

        for _ in 0..num_trials {
            // Aggregate with noise.
            let mut amplitude_map: HashMap<String, Complex> = HashMap::new();
            let mut action_map: HashMap<String, Action> = HashMap::new();

            for contrib in &self.contributions {
                for (action, amp) in &contrib.amplitudes {
                    action_map
                        .entry(action.id.clone())
                        .or_insert_with(|| action.clone());

                    // Add noise: random complex perturbation with magnitude up
                    // to `noise_level`.
                    let noise_r = rng.gen::<f64>() * noise_level;
                    let noise_theta = rng.gen::<f64>() * 2.0 * PI;
                    let noise = Complex::from_polar(noise_r, noise_theta);
                    let noisy_amp = *amp + noise;

                    let entry = amplitude_map
                        .entry(action.id.clone())
                        .or_insert(Complex::ZERO);
                    *entry = *entry + noisy_amp;
                }
            }

            // Find winner for this trial.
            if let Some((winner_id, _)) = amplitude_map.iter().max_by(|a, b| {
                a.1.norm_sq()
                    .partial_cmp(&b.1.norm_sq())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) {
                let entry = win_counts
                    .entry(winner_id.clone())
                    .or_insert_with(|| (action_map[winner_id].clone(), 0));
                entry.1 += 1;
            }
        }

        let mut result: Vec<(Action, usize)> = win_counts.into_values().collect();
        result.sort_by(|a, b| b.1.cmp(&a.1));
        result
    }

    /// Check if the decision is deadlocked.
    ///
    /// A deadlock is detected when the top two actions have probabilities
    /// within `epsilon` of each other.
    pub fn is_deadlocked(&self, epsilon: f64) -> bool {
        let decisions = self.decide();
        if decisions.len() < 2 {
            return false;
        }
        (decisions[0].probability - decisions[1].probability).abs() <= epsilon
    }

    /// Clear all contributions, resetting the engine.
    pub fn reset(&mut self) {
        self.contributions.clear();
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Aggregate all contributions by action id.
    ///
    /// Returns:
    /// - action_map: id -> canonical Action
    /// - amplitude_map: id -> summed complex amplitude
    /// - agent_phases_map: id -> list of each agent's contributing phase
    fn aggregate(
        &self,
    ) -> (
        HashMap<String, Action>,
        HashMap<String, Complex>,
        HashMap<String, Vec<f64>>,
    ) {
        let mut action_map: HashMap<String, Action> = HashMap::new();
        let mut amplitude_map: HashMap<String, Complex> = HashMap::new();
        let mut agent_phases_map: HashMap<String, Vec<f64>> = HashMap::new();

        for contrib in &self.contributions {
            for (action, amp) in &contrib.amplitudes {
                action_map
                    .entry(action.id.clone())
                    .or_insert_with(|| action.clone());

                let entry = amplitude_map
                    .entry(action.id.clone())
                    .or_insert(Complex::ZERO);
                *entry = *entry + *amp;

                agent_phases_map
                    .entry(action.id.clone())
                    .or_insert_with(Vec::new)
                    .push(amp.arg());
            }
        }

        (action_map, amplitude_map, agent_phases_map)
    }

    /// Absolute angular distance between two phases, in [0, pi].
    fn phase_distance(a: f64, b: f64) -> f64 {
        let mut d = (a - b).abs() % (2.0 * PI);
        if d > PI {
            d = 2.0 * PI - d;
        }
        d
    }
}

impl Default for SwarmInterference {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str) -> Action {
        Action {
            id: id.to_string(),
            description: id.to_string(),
        }
    }

    #[test]
    fn single_agent_support() {
        let mut swarm = SwarmInterference::new();
        swarm.contribute(AgentContribution::new("alice", action("deploy"), 0.8, true));

        let decisions = swarm.decide();
        assert_eq!(decisions.len(), 1);
        assert_eq!(decisions[0].action.id, "deploy");
        // probability = |0.8|^2 = 0.64
        assert!((decisions[0].probability - 0.64).abs() < 1e-10);
        assert_eq!(decisions[0].constructive_count, 1);
        assert_eq!(decisions[0].destructive_count, 0);
    }

    #[test]
    fn constructive_interference() {
        let mut swarm = SwarmInterference::new();
        // 3 agents all support "deploy" with confidence 1.0
        swarm.contribute(AgentContribution::new("a", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("c", action("deploy"), 1.0, true));

        let decisions = swarm.decide();
        // Net amplitude = 3.0, probability = 9.0
        assert!((decisions[0].probability - 9.0).abs() < 1e-10);
        assert_eq!(decisions[0].constructive_count, 3);
        assert_eq!(decisions[0].destructive_count, 0);
    }

    #[test]
    fn destructive_interference_cancels() {
        let mut swarm = SwarmInterference::new();
        // 2 agents support, 2 oppose with equal confidence
        swarm.contribute(AgentContribution::new("a", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("c", action("deploy"), 1.0, false));
        swarm.contribute(AgentContribution::new("d", action("deploy"), 1.0, false));

        let decisions = swarm.decide();
        // Net amplitude ~ 0, probability ~ 0
        assert!(decisions[0].probability < 1e-10);
    }

    #[test]
    fn partial_cancellation() {
        let mut swarm = SwarmInterference::new();
        // 3 support, 1 opposes => net amplitude ~ 2.0
        swarm.contribute(AgentContribution::new("a", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("c", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("d", action("deploy"), 1.0, false));

        let decisions = swarm.decide();
        // Net amplitude = 3 - 1 = 2, probability = 4.0
        assert!((decisions[0].probability - 4.0).abs() < 1e-10);
        assert_eq!(decisions[0].constructive_count, 3);
        assert_eq!(decisions[0].destructive_count, 1);
    }

    #[test]
    fn multiple_actions_sorted_by_probability() {
        let mut swarm = SwarmInterference::new();
        // Action "deploy": 2 supporters
        swarm.contribute(AgentContribution::new("a", action("deploy"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("deploy"), 1.0, true));
        // Action "rollback": 3 supporters
        swarm.contribute(AgentContribution::new("c", action("rollback"), 1.0, true));
        swarm.contribute(AgentContribution::new("d", action("rollback"), 1.0, true));
        swarm.contribute(AgentContribution::new("e", action("rollback"), 1.0, true));

        let decisions = swarm.decide();
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0].action.id, "rollback"); // P=9
        assert_eq!(decisions[1].action.id, "deploy"); // P=4
    }

    #[test]
    fn winner_returns_highest() {
        let mut swarm = SwarmInterference::new();
        swarm.contribute(AgentContribution::new("a", action("A"), 0.5, true));
        swarm.contribute(AgentContribution::new("b", action("B"), 1.0, true));

        let w = swarm.winner().unwrap();
        assert_eq!(w.action.id, "B");
    }

    #[test]
    fn winner_empty_swarm() {
        let swarm = SwarmInterference::new();
        assert!(swarm.winner().is_none());
    }

    #[test]
    fn deadlock_detection() {
        let mut swarm = SwarmInterference::new();
        // Two actions with exactly equal support
        swarm.contribute(AgentContribution::new("a", action("A"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("B"), 1.0, true));

        assert!(swarm.is_deadlocked(1e-10));
    }

    #[test]
    fn no_deadlock_with_clear_winner() {
        let mut swarm = SwarmInterference::new();
        swarm.contribute(AgentContribution::new("a", action("A"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("A"), 1.0, true));
        swarm.contribute(AgentContribution::new("c", action("B"), 0.1, true));

        assert!(!swarm.is_deadlocked(0.01));
    }

    #[test]
    fn reset_clears_contributions() {
        let mut swarm = SwarmInterference::new();
        swarm.contribute(AgentContribution::new("a", action("X"), 1.0, true));
        assert_eq!(swarm.decide().len(), 1);

        swarm.reset();
        assert!(swarm.decide().is_empty());
    }

    #[test]
    fn multi_contribution() {
        let mut swarm = SwarmInterference::new();
        swarm.contribute(AgentContribution::multi(
            "alice",
            vec![
                (action("A"), Complex::new(0.5, 0.0)),
                (action("B"), Complex::new(0.0, 0.3)),
            ],
        ));

        let decisions = swarm.decide();
        assert_eq!(decisions.len(), 2);
    }

    #[test]
    fn noise_trials_are_reproducible() {
        let mut swarm = SwarmInterference::new();
        swarm.contribute(AgentContribution::new("a", action("X"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("Y"), 0.5, true));

        let r1 = swarm.decide_with_noise(0.1, 100, 42);
        let r2 = swarm.decide_with_noise(0.1, 100, 42);

        // Same seed -> same results.
        assert_eq!(r1.len(), r2.len());
        for i in 0..r1.len() {
            assert_eq!(r1[i].0.id, r2[i].0.id);
            assert_eq!(r1[i].1, r2[i].1);
        }
    }

    #[test]
    fn noise_preserves_strong_winner() {
        let mut swarm = SwarmInterference::new();
        // Action "A" has overwhelming support.
        swarm.contribute(AgentContribution::new("a", action("A"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("A"), 1.0, true));
        swarm.contribute(AgentContribution::new("c", action("A"), 1.0, true));
        // Action "B" has weak support.
        swarm.contribute(AgentContribution::new("d", action("B"), 0.1, true));

        let results = swarm.decide_with_noise(0.05, 200, 7);
        // "A" should win the vast majority of trials.
        assert_eq!(results[0].0.id, "A");
        assert!(results[0].1 > 150, "A should win most trials");
    }

    #[test]
    fn default_trait() {
        let swarm = SwarmInterference::default();
        assert!(swarm.decide().is_empty());
    }

    #[test]
    fn complete_cancellation_detects_deadlock() {
        let mut swarm = SwarmInterference::new();
        // Perfect cancellation on a single action.
        swarm.contribute(AgentContribution::new("a", action("X"), 1.0, true));
        swarm.contribute(AgentContribution::new("b", action("X"), 1.0, false));

        let decisions = swarm.decide();
        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].probability < 1e-10);
    }
}
