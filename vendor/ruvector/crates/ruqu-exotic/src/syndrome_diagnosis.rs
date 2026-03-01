//! # Syndrome Diagnosis -- QEC-Based System Fault Localization
//!
//! Treats AI system components as a graph. Injects artificial faults into
//! a quantum encoding of the system. Extracts syndrome patterns using QEC logic
//! (CNOT parity checks on ancilla qubits) to localize which component is fragile.
//!
//! This is **structural fault localization**, not log analysis. Multiple rounds
//! of fault injection build statistical fragility profiles and detect components
//! that propagate faults beyond their direct neighborhood.

use ruqu_core::error::QuantumError;
use ruqu_core::gate::Gate;
use ruqu_core::state::QuantumState;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// A system component (node in the diagnostic graph)
#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub health: f64, // 1.0 = healthy, 0.0 = failed
}

/// An edge between two components (dependency/connection)
#[derive(Debug, Clone)]
pub struct Connection {
    pub from: usize,
    pub to: usize,
    pub strength: f64, // coupling strength
}

/// Configuration for syndrome-based diagnosis
pub struct DiagnosisConfig {
    pub fault_injection_rate: f64,
    pub num_rounds: usize,
    pub seed: u64,
}

/// Result of a single diagnostic round
#[derive(Debug)]
pub struct DiagnosticRound {
    pub syndrome: Vec<bool>,
    pub injected_faults: Vec<usize>,
}

/// Overall diagnosis result
#[derive(Debug)]
pub struct DiagnosisResult {
    pub rounds: Vec<DiagnosticRound>,
    /// Fragility score per component: how often it shows up in syndrome patterns
    pub fragility_scores: Vec<(String, f64)>,
    /// Most fragile component
    pub weakest_component: Option<String>,
    /// Components that propagate faults (appear in syndromes without being directly faulted)
    pub fault_propagators: Vec<String>,
}

/// System graph for syndrome-based diagnosis.
///
/// Maps a system topology to a quantum circuit:
/// - Each component becomes a data qubit
/// - Each connection becomes an ancilla qubit for parity measurement
/// - Total qubits = num_components + num_connections (must be <= 25)
pub struct SystemDiagnostics {
    components: Vec<Component>,
    connections: Vec<Connection>,
}

impl SystemDiagnostics {
    pub fn new(components: Vec<Component>, connections: Vec<Connection>) -> Self {
        Self {
            components,
            connections,
        }
    }

    /// Run a single diagnostic round:
    /// 1. Encode components as qubits (healthy=|0>, degraded=Ry rotation)
    /// 2. Inject random faults (X gates on selected qubits)
    /// 3. Extract syndrome using ancilla qubits measuring parity of connected pairs
    pub fn run_round(
        &self,
        config: &DiagnosisConfig,
        round_seed: u64,
    ) -> Result<DiagnosticRound, QuantumError> {
        let num_components = self.components.len();
        let num_connections = self.connections.len();
        let total_qubits = (num_components + num_connections) as u32;

        // Limit check
        if total_qubits > 25 {
            return Err(QuantumError::QubitLimitExceeded {
                requested: total_qubits,
                maximum: 25,
            });
        }

        let mut state = QuantumState::new_with_seed(total_qubits, round_seed)?;
        let mut rng = StdRng::seed_from_u64(round_seed);

        // 1. Encode component health as rotation from |0>
        for (i, comp) in self.components.iter().enumerate() {
            let angle = std::f64::consts::FRAC_PI_2 * (1.0 - comp.health);
            if angle.abs() > 1e-15 {
                state.apply_gate(&Gate::Ry(i as u32, angle))?;
            }
        }

        // 2. Inject faults
        let mut injected = Vec::new();
        for i in 0..num_components {
            if rng.gen::<f64>() < config.fault_injection_rate {
                state.apply_gate(&Gate::X(i as u32))?;
                injected.push(i);
            }
        }

        // 3. For each connection, use an ancilla qubit to check parity
        let mut syndrome = Vec::with_capacity(num_connections);
        for (ci, conn) in self.connections.iter().enumerate() {
            let ancilla = (num_components + ci) as u32;
            state.reset_qubit(ancilla)?;
            state.apply_gate(&Gate::CNOT(conn.from as u32, ancilla))?;
            state.apply_gate(&Gate::CNOT(conn.to as u32, ancilla))?;
            let outcome = state.measure(ancilla)?;
            syndrome.push(outcome.result);
        }

        Ok(DiagnosticRound {
            syndrome,
            injected_faults: injected,
        })
    }

    /// Run full diagnosis: multiple rounds of fault injection + syndrome extraction.
    /// Accumulates statistics to identify fragile and fault-propagating components.
    pub fn diagnose(&self, config: &DiagnosisConfig) -> Result<DiagnosisResult, QuantumError> {
        let mut rounds = Vec::new();
        let mut fault_counts = vec![0usize; self.components.len()];
        let mut syndrome_counts = vec![0usize; self.components.len()];

        for round in 0..config.num_rounds {
            let round_seed = config.seed.wrapping_add(round as u64 * 1000);
            let result = self.run_round(config, round_seed)?;

            // Count which components were directly faulted
            for &idx in &result.injected_faults {
                fault_counts[idx] += 1;
            }

            // Count which components appear in triggered syndromes
            for (ci, &fired) in result.syndrome.iter().enumerate() {
                if fired {
                    let conn = &self.connections[ci];
                    syndrome_counts[conn.from] += 1;
                    syndrome_counts[conn.to] += 1;
                }
            }

            rounds.push(result);
        }

        // Compute fragility scores (syndrome appearances / total rounds)
        let fragility_scores: Vec<(String, f64)> = self
            .components
            .iter()
            .enumerate()
            .map(|(i, c)| {
                (
                    c.id.clone(),
                    syndrome_counts[i] as f64 / config.num_rounds as f64,
                )
            })
            .collect();

        let weakest = fragility_scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(name, _)| name.clone());

        // Fault propagators: components that appear in syndromes more often
        // than they were directly faulted
        let fault_propagators: Vec<String> = self
            .components
            .iter()
            .enumerate()
            .filter(|(i, _)| syndrome_counts[*i] > fault_counts[*i] + config.num_rounds / 4)
            .map(|(_, c)| c.id.clone())
            .collect();

        Ok(DiagnosisResult {
            rounds,
            fragility_scores,
            weakest_component: weakest,
            fault_propagators,
        })
    }

    /// Get the number of components
    pub fn num_components(&self) -> usize {
        self.components.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_linear_system(n: usize) -> (Vec<Component>, Vec<Connection>) {
        let components: Vec<Component> = (0..n)
            .map(|i| Component {
                id: format!("comp_{}", i),
                health: 1.0,
            })
            .collect();
        let connections: Vec<Connection> = (0..n.saturating_sub(1))
            .map(|i| Connection {
                from: i,
                to: i + 1,
                strength: 1.0,
            })
            .collect();
        (components, connections)
    }

    #[test]
    fn test_new_system() {
        let (comps, conns) = make_linear_system(5);
        let diag = SystemDiagnostics::new(comps, conns);
        assert_eq!(diag.num_components(), 5);
    }

    #[test]
    fn test_qubit_limit_check() {
        // 15 components + 14 connections = 29 > 25
        let (comps, conns) = make_linear_system(15);
        let diag = SystemDiagnostics::new(comps, conns);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.0,
            num_rounds: 1,
            seed: 42,
        };
        let result = diag.run_round(&config, 42);
        assert!(result.is_err());
    }

    #[test]
    fn test_single_round_no_faults() {
        let (comps, conns) = make_linear_system(5);
        let diag = SystemDiagnostics::new(comps, conns);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.0,
            num_rounds: 1,
            seed: 42,
        };
        let round = diag.run_round(&config, 42).unwrap();
        // No faults injected -> no syndrome should fire (all healthy components
        // are in |0>, parity checks should agree)
        assert!(round.injected_faults.is_empty());
        assert!(round.syndrome.iter().all(|&s| !s));
    }

    #[test]
    fn test_diagnose_no_faults() {
        let (comps, conns) = make_linear_system(5);
        let diag = SystemDiagnostics::new(comps, conns);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.0,
            num_rounds: 10,
            seed: 42,
        };
        let result = diag.diagnose(&config).unwrap();
        assert_eq!(result.rounds.len(), 10);
        // No faults -> all fragility scores should be 0
        for (_, score) in &result.fragility_scores {
            assert!((*score - 0.0).abs() < 1e-10);
        }
    }

    #[test]
    fn test_diagnose_with_faults() {
        let (comps, conns) = make_linear_system(5);
        let diag = SystemDiagnostics::new(comps, conns);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.5,
            num_rounds: 20,
            seed: 123,
        };
        let result = diag.diagnose(&config).unwrap();
        assert_eq!(result.rounds.len(), 20);
        assert_eq!(result.fragility_scores.len(), 5);
        // At least some syndromes should fire with 50% fault rate
        let total_fired: usize = result
            .rounds
            .iter()
            .map(|r| r.syndrome.iter().filter(|&&s| s).count())
            .sum();
        assert!(total_fired > 0);
    }

    #[test]
    fn test_weakest_component_identified() {
        let (comps, conns) = make_linear_system(5);
        let diag = SystemDiagnostics::new(comps, conns);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.3,
            num_rounds: 50,
            seed: 999,
        };
        let result = diag.diagnose(&config).unwrap();
        // Should identify a weakest component
        assert!(result.weakest_component.is_some());
    }

    #[test]
    fn test_degraded_components() {
        // One component is already degraded
        let mut comps: Vec<Component> = (0..5)
            .map(|i| Component {
                id: format!("comp_{}", i),
                health: 1.0,
            })
            .collect();
        comps[2].health = 0.3; // Component 2 is degraded
        let conns: Vec<Connection> = (0..4)
            .map(|i| Connection {
                from: i,
                to: i + 1,
                strength: 1.0,
            })
            .collect();
        let diag = SystemDiagnostics::new(comps, conns);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.0,
            num_rounds: 1,
            seed: 42,
        };
        // Should run without error
        let round = diag.run_round(&config, 42).unwrap();
        assert_eq!(round.syndrome.len(), 4);
    }

    #[test]
    fn test_max_qubit_boundary() {
        // 13 components + 12 connections = 25 qubits (exactly at limit)
        let (comps, conns) = make_linear_system(13);
        let diag = SystemDiagnostics::new(comps, conns);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.0,
            num_rounds: 1,
            seed: 42,
        };
        let result = diag.run_round(&config, 42);
        assert!(result.is_ok());
    }

    #[test]
    fn test_empty_connections() {
        // Components with no connections
        let comps = vec![
            Component {
                id: "a".into(),
                health: 1.0,
            },
            Component {
                id: "b".into(),
                health: 1.0,
            },
        ];
        let diag = SystemDiagnostics::new(comps, vec![]);
        let config = DiagnosisConfig {
            fault_injection_rate: 0.0,
            num_rounds: 5,
            seed: 42,
        };
        let result = diag.diagnose(&config).unwrap();
        // No connections -> no syndrome bits
        for round in &result.rounds {
            assert!(round.syndrome.is_empty());
        }
    }
}
