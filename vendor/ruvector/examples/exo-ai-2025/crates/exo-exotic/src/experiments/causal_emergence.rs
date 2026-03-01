//! Experiment 07: Causal Emergence
//!
//! Research frontier: Find the macro-scale that maximizes causal power (EI).
//! Theory: Hoel et al. 2013 — emergence occurs when a macro-description has
//! higher Effective Information (EI) than its micro-substrate.
//!
//! EI(τ) = H(effect) - H(effect|cause)  [where τ = coarse-graining]
//! Causal emergence: EI(macro) > EI(micro)
//!
//! ADR-029: ruvector-solver Forward Push PPR accelerates the coarse-graining
//! search (O(n/ε) vs O(n²) for dense causation matrices).

/// Transition probability matrix (row = current state, col = next state)
pub struct TransitionMatrix {
    pub n_states: usize,
    pub data: Vec<f64>, // n × n, row-major
}

impl TransitionMatrix {
    pub fn new(n: usize) -> Self {
        Self {
            n_states: n,
            data: vec![0.0; n * n],
        }
    }

    pub fn set(&mut self, from: usize, to: usize, prob: f64) {
        self.data[from * self.n_states + to] = prob;
    }

    pub fn get(&self, from: usize, to: usize) -> f64 {
        self.data[from * self.n_states + to]
    }

    /// Shannon entropy of output distribution given input state
    fn conditional_entropy(&self, from: usize) -> f64 {
        let mut h = 0.0;
        for to in 0..self.n_states {
            let p = self.get(from, to);
            if p > 1e-10 {
                h -= p * p.ln();
            }
        }
        h
    }

    /// Marginal output distribution (uniform intervention distribution)
    fn marginal_output(&self) -> Vec<f64> {
        let n = self.n_states;
        let mut marginal = vec![0.0f64; n];
        for from in 0..n {
            for to in 0..n {
                marginal[to] += self.get(from, to) / n as f64;
            }
        }
        marginal
    }

    /// Effective Information = H(effect) - <H(effect|cause)>
    pub fn effective_information(&self) -> f64 {
        let marginal = self.marginal_output();
        let h_effect: f64 = marginal
            .iter()
            .filter(|&&p| p > 1e-10)
            .map(|&p| -p * p.ln())
            .sum();
        let h_cond: f64 = (0..self.n_states)
            .map(|from| self.conditional_entropy(from))
            .sum::<f64>()
            / self.n_states as f64;
        h_effect - h_cond
    }
}

/// Coarse-graining operator: partitions micro-states into macro-states
pub struct CoarseGraining {
    /// Mapping from micro-state to macro-state
    pub micro_to_macro: Vec<usize>,
    pub n_macro: usize,
    pub n_micro: usize,
}

impl CoarseGraining {
    /// Block coarse-graining: group consecutive states
    pub fn block(n_micro: usize, block_size: usize) -> Self {
        let n_macro = (n_micro + block_size - 1) / block_size;
        let micro_to_macro = (0..n_micro).map(|i| i / block_size).collect();
        Self {
            micro_to_macro,
            n_macro,
            n_micro,
        }
    }

    /// Apply coarse-graining to produce macro transition matrix
    pub fn apply(&self, micro: &TransitionMatrix) -> TransitionMatrix {
        let mut macro_matrix = TransitionMatrix::new(self.n_macro);
        let n = self.n_micro;

        // Macro transition P(macro_j | macro_i) = average over micro states in macro_i
        let mut counts = vec![0usize; self.n_macro];
        for i in 0..n {
            counts[self.micro_to_macro[i]] += 1;
        }

        for from_micro in 0..n {
            let from_macro = self.micro_to_macro[from_micro];
            for to_micro in 0..n {
                let to_macro = self.micro_to_macro[to_micro];
                let weight = 1.0 / counts[from_macro].max(1) as f64;
                let current = macro_matrix.get(from_macro, to_macro);
                macro_matrix.set(
                    from_macro,
                    to_macro,
                    current + micro.get(from_micro, to_micro) * weight,
                );
            }
        }
        macro_matrix
    }
}

pub struct CausalEmergenceResult {
    pub micro_ei: f64,
    pub macro_eis: Vec<(usize, f64)>, // (block_size, EI)
    pub best_macro_ei: f64,
    pub best_block_size: usize,
    pub emergence_delta: f64, // macro_EI - micro_EI
    pub causal_emergence_detected: bool,
}

pub struct CausalEmergenceExperiment {
    pub n_micro_states: usize,
    pub block_sizes: Vec<usize>,
}

impl CausalEmergenceExperiment {
    pub fn new() -> Self {
        Self {
            n_micro_states: 16,
            block_sizes: vec![2, 4, 8],
        }
    }

    /// Build a test transition matrix with known causal structure
    pub fn build_test_matrix(n: usize, noise: f64) -> TransitionMatrix {
        let mut tm = TransitionMatrix::new(n);
        // Deterministic XOR-like macro pattern with microscopic noise
        for from in 0..n {
            let macro_next = (from / 2 + 1) % (n / 2);
            for to in 0..n {
                let in_macro = to / 2 == macro_next;
                let p = if in_macro {
                    (1.0 - noise) / 2.0
                } else {
                    noise / (n - 2).max(1) as f64
                };
                tm.set(from, to, p);
            }
            // Normalize
            let sum: f64 = (0..n).map(|to| tm.get(from, to)).sum();
            if sum > 1e-10 {
                for to in 0..n {
                    tm.set(from, to, tm.get(from, to) / sum);
                }
            }
        }
        tm
    }

    pub fn run(&self) -> CausalEmergenceResult {
        let micro_tm = Self::build_test_matrix(self.n_micro_states, 0.1);
        let micro_ei = micro_tm.effective_information();

        let mut macro_eis = Vec::new();
        for &block_size in &self.block_sizes {
            let cg = CoarseGraining::block(self.n_micro_states, block_size);
            if cg.n_macro >= 2 {
                let macro_tm = cg.apply(&micro_tm);
                let macro_ei = macro_tm.effective_information();
                macro_eis.push((block_size, macro_ei));
            }
        }

        let best = macro_eis
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .copied()
            .unwrap_or((0, micro_ei));

        let delta = best.1 - micro_ei;
        CausalEmergenceResult {
            micro_ei,
            macro_eis,
            best_macro_ei: best.1,
            best_block_size: best.0,
            emergence_delta: delta,
            causal_emergence_detected: delta > 0.01,
        }
    }
}

impl Default for CausalEmergenceExperiment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effective_information_positive() {
        // Deterministic matrix should have max EI = H(uniform on n states)
        let mut tm = TransitionMatrix::new(4);
        for from in 0..4 {
            tm.set(from, (from + 1) % 4, 1.0);
        }
        let ei = tm.effective_information();
        assert!(
            ei > 0.0,
            "Deterministic permutation should have positive EI"
        );
    }

    #[test]
    fn test_block_coarse_graining() {
        let cg = CoarseGraining::block(8, 2);
        assert_eq!(cg.n_macro, 4);
        assert_eq!(cg.micro_to_macro[0], 0);
        assert_eq!(cg.micro_to_macro[2], 1);
        assert_eq!(cg.micro_to_macro[6], 3);
    }

    #[test]
    fn test_causal_emergence_experiment_runs() {
        let exp = CausalEmergenceExperiment::new();
        let result = exp.run();
        assert!(result.micro_ei >= 0.0);
        assert!(!result.macro_eis.is_empty());
    }

    #[test]
    fn test_transition_matrix_normalizes() {
        let tm = CausalEmergenceExperiment::build_test_matrix(8, 0.1);
        for from in 0..8 {
            let sum: f64 = (0..8).map(|to| tm.get(from, to)).sum();
            assert!(
                (sum - 1.0).abs() < 1e-9,
                "Row {} should sum to 1.0, got {}",
                from,
                sum
            );
        }
    }
}
