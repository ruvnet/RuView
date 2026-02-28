//! Multi-criteria utility-based action selection.
//!
//! The [`DecisionEngine`] scores candidate actions using a weighted
//! combination of reward, risk, energy cost, and novelty to select the
//! best option for the current context.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A candidate action with associated attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionOption {
    pub name: String,
    pub reward: f64,
    pub risk: f64,
    pub energy_cost: f64,
    pub novelty: f64,
}

/// Weights and parameters controlling the decision engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionConfig {
    /// How strongly the engine penalises risky actions (>= 0).
    pub risk_aversion: f64,
    /// How strongly the engine penalises energy expenditure (>= 0).
    pub energy_weight: f64,
    /// How strongly the engine rewards novel/exploratory actions (>= 0).
    pub curiosity_weight: f64,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            risk_aversion: 1.0,
            energy_weight: 0.5,
            curiosity_weight: 0.2,
        }
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Evaluates candidate actions and selects the one with the highest utility.
#[derive(Debug, Clone)]
pub struct DecisionEngine {
    config: DecisionConfig,
}

impl DecisionEngine {
    /// Create a new engine with the given configuration.
    pub fn new(config: DecisionConfig) -> Self {
        Self { config }
    }

    /// Compute the utility of a single action option.
    pub fn utility(&self, option: &ActionOption) -> f64 {
        option.reward
            - self.config.risk_aversion * option.risk
            - self.config.energy_weight * option.energy_cost
            + self.config.curiosity_weight * option.novelty
    }

    /// Evaluate all options and return the index and utility of the best one.
    ///
    /// Returns `None` when the slice is empty.
    pub fn evaluate(&self, options: &[ActionOption]) -> Option<(usize, f64)> {
        if options.is_empty() {
            return None;
        }

        let mut best_idx = 0;
        let mut best_util = f64::NEG_INFINITY;

        for (i, opt) in options.iter().enumerate() {
            let u = self.utility(opt);
            if u > best_util {
                best_util = u;
                best_idx = i;
            }
        }

        Some((best_idx, best_util))
    }

    /// Read-only access to the configuration.
    pub fn config(&self) -> &DecisionConfig {
        &self.config
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_option(name: &str, reward: f64, risk: f64, energy: f64, novelty: f64) -> ActionOption {
        ActionOption {
            name: name.into(),
            reward,
            risk,
            energy_cost: energy,
            novelty,
        }
    }

    #[test]
    fn test_single_option() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let options = vec![make_option("go", 1.0, 0.0, 0.0, 0.0)];
        let (idx, util) = engine.evaluate(&options).unwrap();
        assert_eq!(idx, 0);
        assert!((util - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_empty_options() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        assert!(engine.evaluate(&[]).is_none());
    }

    #[test]
    fn test_risk_penalty() {
        let engine = DecisionEngine::new(DecisionConfig {
            risk_aversion: 2.0,
            energy_weight: 0.0,
            curiosity_weight: 0.0,
        });
        let options = vec![
            make_option("safe", 1.0, 0.0, 0.0, 0.0),
            make_option("risky", 2.0, 1.0, 0.0, 0.0),
        ];
        let (idx, _) = engine.evaluate(&options).unwrap();
        assert_eq!(idx, 0); // safe: 1.0, risky: 2.0 - 2.0*1.0 = 0.0
    }

    #[test]
    fn test_curiosity_bonus() {
        let engine = DecisionEngine::new(DecisionConfig {
            risk_aversion: 0.0,
            energy_weight: 0.0,
            curiosity_weight: 5.0,
        });
        let options = vec![
            make_option("boring", 1.0, 0.0, 0.0, 0.0),
            make_option("novel", 1.0, 0.0, 0.0, 2.0),
        ];
        let (idx, _) = engine.evaluate(&options).unwrap();
        assert_eq!(idx, 1); // novel: 1.0 + 5.0*2.0 = 11.0
    }

    #[test]
    fn test_energy_penalty() {
        let engine = DecisionEngine::new(DecisionConfig {
            risk_aversion: 0.0,
            energy_weight: 3.0,
            curiosity_weight: 0.0,
        });
        let options = vec![
            make_option("cheap", 1.0, 0.0, 0.1, 0.0),
            make_option("expensive", 1.0, 0.0, 1.0, 0.0),
        ];
        let (idx, _) = engine.evaluate(&options).unwrap();
        assert_eq!(idx, 0);
    }

    #[test]
    fn test_utility_formula() {
        let engine = DecisionEngine::new(DecisionConfig {
            risk_aversion: 1.0,
            energy_weight: 0.5,
            curiosity_weight: 0.2,
        });
        let opt = make_option("test", 10.0, 2.0, 4.0, 5.0);
        // utility = 10 - 1*2 - 0.5*4 + 0.2*5 = 10 - 2 - 2 + 1 = 7.0
        let u = engine.utility(&opt);
        assert!((u - 7.0).abs() < 1e-9);
    }

    #[test]
    fn test_multiple_options_best() {
        let engine = DecisionEngine::new(DecisionConfig::default());
        let options = vec![
            make_option("a", 0.5, 0.1, 0.1, 0.1),
            make_option("b", 5.0, 0.1, 0.1, 0.1),
            make_option("c", 2.0, 0.1, 0.1, 0.1),
        ];
        let (idx, _) = engine.evaluate(&options).unwrap();
        assert_eq!(idx, 1);
    }
}
