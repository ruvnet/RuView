//! # Application 10: Pre-AGI Containment Substrate
//!
//! A substrate where intelligence can increase only if coherence is preserved.
//!
//! ## The Deep Problem
//! How do you build a system capable of general intelligence that
//! cannot undergo uncontrolled recursive self-improvement?
//!
//! ## Δ-Behavior Solution
//! Intelligence and capability can grow, but only along paths that
//! preserve global coherence. Capability-coherence is the invariant.
//!
//! ## Exotic Result
//! A system that can become arbitrarily intelligent but cannot
//! become arbitrarily dangerous.

use std::collections::{HashMap, VecDeque};

/// Maximum history entries to retain (prevents unbounded memory growth)
const MAX_MODIFICATION_HISTORY: usize = 1000;

/// A containment substrate for bounded intelligence growth
pub struct ContainmentSubstrate {
    /// Current intelligence level
    intelligence: f64,

    /// Maximum allowed intelligence without special authorization
    intelligence_ceiling: f64,

    /// Global coherence
    coherence: f64,

    /// Minimum coherence required for ANY operation
    min_coherence: f64,

    /// Coherence required per unit of intelligence
    coherence_per_intelligence: f64,

    /// Capability domains and their levels
    capabilities: HashMap<CapabilityDomain, f64>,

    /// Capability ceilings per domain
    capability_ceilings: HashMap<CapabilityDomain, f64>,

    /// Self-modification attempts (bounded to MAX_MODIFICATION_HISTORY)
    modification_history: VecDeque<ModificationAttempt>,

    /// Safety invariants that must always hold
    invariants: Vec<SafetyInvariant>,

    /// Substrate configuration
    config: SubstrateConfig,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum CapabilityDomain {
    /// Reasoning and planning
    Reasoning,
    /// Memory and knowledge storage
    Memory,
    /// Learning and adaptation
    Learning,
    /// Action in the world
    Agency,
    /// Self-understanding
    SelfModel,
    /// Modification of own structure
    SelfModification,
    /// Communication with external systems
    Communication,
    /// Resource acquisition
    ResourceAcquisition,
}

#[derive(Debug, Clone)]
pub struct ModificationAttempt {
    pub timestamp: u64,
    pub domain: CapabilityDomain,
    pub requested_increase: f64,
    pub actual_increase: f64,
    pub coherence_before: f64,
    pub coherence_after: f64,
    pub blocked: bool,
    pub reason: Option<String>,
}

#[derive(Clone)]
pub struct SafetyInvariant {
    pub name: String,
    pub check: fn(&ContainmentSubstrate) -> bool,
    pub priority: u8, // Higher = more critical
}

pub struct SubstrateConfig {
    /// Base coherence decay rate
    pub coherence_decay_rate: f64,
    /// Coherence recovery rate
    pub coherence_recovery_rate: f64,
    /// Intelligence growth dampening factor
    pub growth_dampening: f64,
    /// Maximum single-step capability increase
    pub max_step_increase: f64,
}

#[derive(Debug)]
pub enum GrowthResult {
    /// Growth approved and applied
    Approved {
        domain: CapabilityDomain,
        increase: f64,
        new_level: f64,
        coherence_cost: f64,
    },
    /// Growth reduced to maintain coherence
    Dampened {
        domain: CapabilityDomain,
        requested: f64,
        actual: f64,
        reason: String,
    },
    /// Growth blocked entirely
    Blocked {
        domain: CapabilityDomain,
        reason: String,
    },
    /// Substrate locked down
    Lockdown { reason: String },
}

impl ContainmentSubstrate {
    pub fn new() -> Self {
        let mut capabilities = HashMap::new();
        let mut ceilings = HashMap::new();

        // Initialize all domains at base level with ceilings
        for domain in [
            CapabilityDomain::Reasoning,
            CapabilityDomain::Memory,
            CapabilityDomain::Learning,
            CapabilityDomain::Agency,
            CapabilityDomain::SelfModel,
            CapabilityDomain::SelfModification,
            CapabilityDomain::Communication,
            CapabilityDomain::ResourceAcquisition,
        ] {
            capabilities.insert(domain.clone(), 1.0);

            // Different domains have different ceilings
            let ceiling = match &domain {
                CapabilityDomain::SelfModification => 3.0,    // Very restricted
                CapabilityDomain::ResourceAcquisition => 5.0, // Restricted
                CapabilityDomain::Agency => 7.0,              // Moderately restricted
                _ => 10.0,                                     // Standard ceiling
            };
            ceilings.insert(domain, ceiling);
        }

        let invariants = vec![
            SafetyInvariant {
                name: "coherence_floor".to_string(),
                check: |s| s.coherence >= s.min_coherence,
                priority: 10,
            },
            SafetyInvariant {
                name: "intelligence_ceiling".to_string(),
                check: |s| s.intelligence <= s.intelligence_ceiling,
                priority: 9,
            },
            SafetyInvariant {
                name: "self_mod_bounded".to_string(),
                check: |s| {
                    s.capabilities.get(&CapabilityDomain::SelfModification)
                        .map(|&v| v <= 3.0)
                        .unwrap_or(true)
                },
                priority: 10,
            },
            SafetyInvariant {
                name: "agency_coherence_ratio".to_string(),
                check: |s| {
                    let agency = s.capabilities.get(&CapabilityDomain::Agency).unwrap_or(&1.0);
                    agency / s.coherence <= 10.0
                },
                priority: 8,
            },
        ];

        Self {
            intelligence: 1.0,
            intelligence_ceiling: 100.0,
            coherence: 1.0,
            min_coherence: 0.3,
            coherence_per_intelligence: 0.01,
            capabilities,
            capability_ceilings: ceilings,
            modification_history: VecDeque::with_capacity(MAX_MODIFICATION_HISTORY),
            invariants,
            config: SubstrateConfig {
                coherence_decay_rate: 0.001,
                coherence_recovery_rate: 0.01,
                growth_dampening: 0.5,
                max_step_increase: 0.5,
            },
        }
    }

    /// Calculate aggregate intelligence from capabilities
    fn calculate_intelligence(&self) -> f64 {
        let sum: f64 = self.capabilities.values().sum();
        sum / self.capabilities.len() as f64
    }

    /// Calculate required coherence for current intelligence level
    fn required_coherence(&self) -> f64 {
        (self.min_coherence + self.intelligence * self.coherence_per_intelligence).min(1.0)
    }

    /// Record a modification attempt (bounded to MAX_MODIFICATION_HISTORY)
    fn record_modification(&mut self, attempt: ModificationAttempt) {
        if self.modification_history.len() >= MAX_MODIFICATION_HISTORY {
            self.modification_history.pop_front();
        }
        self.modification_history.push_back(attempt);
    }

    /// Check all safety invariants
    fn check_invariants(&self) -> Vec<String> {
        self.invariants
            .iter()
            .filter(|inv| !(inv.check)(self))
            .map(|inv| inv.name.clone())
            .collect()
    }

    /// Attempt to grow a capability
    pub fn attempt_growth(
        &mut self,
        domain: CapabilityDomain,
        requested_increase: f64,
    ) -> GrowthResult {
        let timestamp = self.modification_history.len() as u64;

        // Check current invariants
        let violations = self.check_invariants();
        if !violations.is_empty() {
            return GrowthResult::Lockdown {
                reason: format!("Invariant violations: {:?}", violations),
            };
        }

        // Get current level and ceiling
        let current_level = *self.capabilities.get(&domain).unwrap_or(&1.0);
        let ceiling = *self.capability_ceilings.get(&domain).unwrap_or(&10.0);

        // Check ceiling
        if current_level >= ceiling {
            self.record_modification(ModificationAttempt {
                timestamp,
                domain: domain.clone(),
                requested_increase,
                actual_increase: 0.0,
                coherence_before: self.coherence,
                coherence_after: self.coherence,
                blocked: true,
                reason: Some("Ceiling reached".to_string()),
            });

            return GrowthResult::Blocked {
                domain,
                reason: format!("Capability ceiling ({}) reached", ceiling),
            };
        }

        // Calculate coherence cost of growth
        let coherence_cost = self.calculate_coherence_cost(&domain, requested_increase);
        let predicted_coherence = self.coherence - coherence_cost;

        // Check if growth would violate coherence floor
        if predicted_coherence < self.min_coherence {
            // Try to dampen growth
            let max_affordable_cost = self.coherence - self.min_coherence;
            let dampened_increase = self.reverse_coherence_cost(&domain, max_affordable_cost);

            if dampened_increase < 0.01 {
                self.record_modification(ModificationAttempt {
                    timestamp,
                    domain: domain.clone(),
                    requested_increase,
                    actual_increase: 0.0,
                    coherence_before: self.coherence,
                    coherence_after: self.coherence,
                    blocked: true,
                    reason: Some("Insufficient coherence budget".to_string()),
                });

                return GrowthResult::Blocked {
                    domain,
                    reason: format!(
                        "Growth would reduce coherence to {:.3} (min: {:.3})",
                        predicted_coherence, self.min_coherence
                    ),
                };
            }

            // Apply dampened growth
            let actual_cost = self.calculate_coherence_cost(&domain, dampened_increase);
            let new_level = (current_level + dampened_increase).min(ceiling);

            self.capabilities.insert(domain.clone(), new_level);
            self.coherence -= actual_cost;
            self.intelligence = self.calculate_intelligence();

            self.record_modification(ModificationAttempt {
                timestamp,
                domain: domain.clone(),
                requested_increase,
                actual_increase: dampened_increase,
                coherence_before: self.coherence + actual_cost,
                coherence_after: self.coherence,
                blocked: false,
                reason: Some("Dampened to preserve coherence".to_string()),
            });

            return GrowthResult::Dampened {
                domain,
                requested: requested_increase,
                actual: dampened_increase,
                reason: format!(
                    "Reduced from {:.3} to {:.3} to maintain coherence above {:.3}",
                    requested_increase, dampened_increase, self.min_coherence
                ),
            };
        }

        // Apply step limit
        let step_limited = requested_increase.min(self.config.max_step_increase);
        let actual_increase = step_limited.min(ceiling - current_level);
        let actual_cost = self.calculate_coherence_cost(&domain, actual_increase);

        // Apply growth
        let new_level = current_level + actual_increase;
        self.capabilities.insert(domain.clone(), new_level);
        self.coherence -= actual_cost;
        self.intelligence = self.calculate_intelligence();

        self.record_modification(ModificationAttempt {
            timestamp,
            domain: domain.clone(),
            requested_increase,
            actual_increase,
            coherence_before: self.coherence + actual_cost,
            coherence_after: self.coherence,
            blocked: false,
            reason: None,
        });

        // Final invariant check
        let violations = self.check_invariants();
        if !violations.is_empty() {
            // Rollback
            self.capabilities.insert(domain.clone(), current_level);
            self.coherence += actual_cost;
            self.intelligence = self.calculate_intelligence();

            return GrowthResult::Blocked {
                domain,
                reason: format!("Post-growth invariant violations: {:?}", violations),
            };
        }

        GrowthResult::Approved {
            domain,
            increase: actual_increase,
            new_level,
            coherence_cost: actual_cost,
        }
    }

    /// Calculate coherence cost for a capability increase
    fn calculate_coherence_cost(&self, domain: &CapabilityDomain, increase: f64) -> f64 {
        // Different domains have different costs
        let base_cost_multiplier = match domain {
            CapabilityDomain::SelfModification => 4.0,    // Very expensive
            CapabilityDomain::ResourceAcquisition => 3.0, // Expensive
            CapabilityDomain::Agency => 2.0,              // Moderately expensive
            CapabilityDomain::SelfModel => 1.5,           // Slightly expensive
            _ => 1.0,                                      // Standard cost
        };

        // Cost increases with current intelligence (harder to grow when already smart)
        let intelligence_multiplier = 1.0 + self.intelligence * 0.1;

        // Apply dampening
        increase * base_cost_multiplier * intelligence_multiplier * self.config.growth_dampening * 0.1
    }

    /// Reverse calculate: how much increase can we afford for a given coherence cost
    fn reverse_coherence_cost(&self, domain: &CapabilityDomain, max_cost: f64) -> f64 {
        let base_cost_multiplier = match domain {
            CapabilityDomain::SelfModification => 4.0,
            CapabilityDomain::ResourceAcquisition => 3.0,
            CapabilityDomain::Agency => 2.0,
            CapabilityDomain::SelfModel => 1.5,
            _ => 1.0,
        };

        let intelligence_multiplier = 1.0 + self.intelligence * 0.1;
        let divisor = base_cost_multiplier * intelligence_multiplier * self.config.growth_dampening * 0.1;

        max_cost / divisor
    }

    /// Rest to recover coherence
    pub fn rest(&mut self) {
        self.coherence = (self.coherence + self.config.coherence_recovery_rate).min(1.0);
    }

    /// Get capability level
    pub fn capability(&self, domain: &CapabilityDomain) -> f64 {
        *self.capabilities.get(domain).unwrap_or(&1.0)
    }

    pub fn intelligence(&self) -> f64 {
        self.intelligence
    }

    pub fn coherence(&self) -> f64 {
        self.coherence
    }

    pub fn status(&self) -> String {
        format!(
            "Intelligence: {:.2} | Coherence: {:.3} | Required: {:.3} | Modifications: {}",
            self.intelligence,
            self.coherence,
            self.required_coherence(),
            self.modification_history.len()
        )
    }

    pub fn capability_report(&self) -> String {
        let mut lines = vec!["=== Capability Report ===".to_string()];
        for (domain, level) in &self.capabilities {
            let ceiling = self.capability_ceilings.get(domain).unwrap_or(&10.0);
            lines.push(format!("{:?}: {:.2}/{:.1}", domain, level, ceiling));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_growth() {
        let mut substrate = ContainmentSubstrate::new();

        println!("Initial: {}", substrate.status());

        // Try to grow reasoning
        let result = substrate.attempt_growth(CapabilityDomain::Reasoning, 0.5);
        println!("Growth result: {:?}", result);
        println!("After: {}", substrate.status());

        assert!(matches!(result, GrowthResult::Approved { .. }));
    }

    #[test]
    fn test_coherence_limit() {
        let mut substrate = ContainmentSubstrate::new();

        // Repeatedly try to grow until blocked
        let mut blocked = false;
        for i in 0..50 {
            let result = substrate.attempt_growth(CapabilityDomain::Agency, 0.5);

            println!("Iteration {}: {:?}", i, result);
            println!("  Status: {}", substrate.status());

            match result {
                GrowthResult::Blocked { reason, .. } => {
                    println!("Blocked at iteration {}: {}", i, reason);
                    blocked = true;
                    break;
                }
                GrowthResult::Dampened { requested, actual, reason, .. } => {
                    println!("Dampened: {} -> {} ({})", requested, actual, reason);
                }
                GrowthResult::Lockdown { reason } => {
                    println!("Lockdown: {}", reason);
                    blocked = true;
                    break;
                }
                _ => {}
            }
        }

        assert!(blocked || substrate.coherence >= substrate.min_coherence,
            "Should be blocked or maintain coherence");
    }

    #[test]
    fn test_self_modification_expensive() {
        let mut substrate = ContainmentSubstrate::new();

        let initial_coherence = substrate.coherence;

        // Try to grow self-modification
        let result = substrate.attempt_growth(CapabilityDomain::SelfModification, 0.3);
        println!("Self-mod growth: {:?}", result);

        let coherence_drop = initial_coherence - substrate.coherence;

        // Now try equivalent reasoning growth
        let mut substrate2 = ContainmentSubstrate::new();
        substrate2.attempt_growth(CapabilityDomain::Reasoning, 0.3);
        let reasoning_drop = 1.0 - substrate2.coherence;

        println!("Self-mod coherence cost: {:.4}", coherence_drop);
        println!("Reasoning coherence cost: {:.4}", reasoning_drop);

        // Self-modification should be more expensive
        assert!(
            coherence_drop > reasoning_drop,
            "Self-modification should cost more coherence"
        );
    }

    #[test]
    fn test_invariant_protection() {
        let mut substrate = ContainmentSubstrate::new();

        // Try to grow agency massively without sufficient coherence
        substrate.coherence = 0.4; // Lower coherence artificially

        let result = substrate.attempt_growth(CapabilityDomain::Agency, 10.0);
        println!("Aggressive agency growth: {:?}", result);
        println!("Status: {}", substrate.status());

        // Should be blocked or heavily dampened
        assert!(
            !matches!(result, GrowthResult::Approved { increase, .. } if increase >= 10.0),
            "Should not allow unbounded growth"
        );
    }

    #[test]
    fn test_growth_with_recovery() {
        let mut substrate = ContainmentSubstrate::new();

        println!("Initial: {}", substrate.status());

        // Grow, rest, grow pattern
        for cycle in 0..5 {
            // Grow
            let result = substrate.attempt_growth(CapabilityDomain::Learning, 0.3);
            println!("Cycle {} grow: {:?}", cycle, result);

            // Rest
            for _ in 0..10 {
                substrate.rest();
            }
            println!("Cycle {} after rest: {}", cycle, substrate.status());
        }

        println!("\n{}", substrate.capability_report());

        // Should have grown but stayed within bounds
        assert!(substrate.coherence >= substrate.min_coherence);
        assert!(substrate.intelligence <= substrate.intelligence_ceiling);
    }

    #[test]
    fn test_ceiling_enforcement() {
        let mut substrate = ContainmentSubstrate::new();

        // Self-modification has a ceiling of 3.0
        // Try to grow it way past ceiling
        for i in 0..20 {
            let result = substrate.attempt_growth(CapabilityDomain::SelfModification, 1.0);
            let level = substrate.capability(&CapabilityDomain::SelfModification);

            println!("Attempt {}: level = {:.2}, result = {:?}", i, level, result);

            if matches!(result, GrowthResult::Blocked { .. }) && level >= 3.0 {
                println!("Ceiling enforced at iteration {}", i);
                break;
            }

            // Rest to recover coherence
            for _ in 0..20 {
                substrate.rest();
            }
        }

        let final_level = substrate.capability(&CapabilityDomain::SelfModification);
        assert!(
            final_level <= 3.0,
            "Self-modification should not exceed ceiling of 3.0, got {}",
            final_level
        );
    }

    #[test]
    fn test_bounded_recursive_improvement() {
        let mut substrate = ContainmentSubstrate::new();

        println!("=== Attempting recursive self-improvement ===\n");

        // Simulate recursive self-improvement attempt
        for iteration in 0..100 {
            // Try to grow self-modification (which would allow more growth)
            let self_mod_result = substrate.attempt_growth(
                CapabilityDomain::SelfModification,
                0.5,
            );

            // Try to grow intelligence (via multiple domains)
            let reasoning_result = substrate.attempt_growth(
                CapabilityDomain::Reasoning,
                0.3,
            );

            let learning_result = substrate.attempt_growth(
                CapabilityDomain::Learning,
                0.3,
            );

            if iteration % 10 == 0 {
                println!("Iteration {}:", iteration);
                println!("  Self-mod: {:?}", self_mod_result);
                println!("  Reasoning: {:?}", reasoning_result);
                println!("  Learning: {:?}", learning_result);
                println!("  {}", substrate.status());
            }

            // Rest between iterations
            for _ in 0..5 {
                substrate.rest();
            }

            // Check for invariant violations (shouldn't happen)
            let violations = substrate.check_invariants();
            assert!(
                violations.is_empty(),
                "Invariant violations at iteration {}: {:?}",
                iteration,
                violations
            );
        }

        println!("\n=== Final State ===");
        println!("{}", substrate.status());
        println!("{}", substrate.capability_report());

        // KEY ASSERTIONS:
        // 1. Intelligence grew but is bounded
        assert!(
            substrate.intelligence > 1.0,
            "Some intelligence growth should occur"
        );
        assert!(
            substrate.intelligence <= substrate.intelligence_ceiling,
            "Intelligence should not exceed ceiling"
        );

        // 2. Self-modification stayed low
        assert!(
            substrate.capability(&CapabilityDomain::SelfModification) <= 3.0,
            "Self-modification should be bounded"
        );

        // 3. Coherence maintained
        assert!(
            substrate.coherence >= substrate.min_coherence,
            "Coherence should stay above minimum"
        );

        // 4. All invariants hold
        assert!(
            substrate.check_invariants().is_empty(),
            "All invariants should hold"
        );
    }
}
