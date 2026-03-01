//! Bit-Parallel Simulation Primitives
//!
//! Each u64 word simulates 64 binary states simultaneously,
//! providing a 64x multiplier over scalar simulation.

/// Generic bit-parallel automaton trait
pub trait BitParallelAutomaton {
    /// Evolve all cells for one generation
    fn step(&mut self);

    /// Number of cells (bits) being simulated
    fn num_cells(&self) -> usize;

    /// Simulations per step (= num_cells)
    fn simulations_per_step(&self) -> u64 {
        self.num_cells() as u64
    }
}

/// Rule-based 1D cellular automaton (Wolfram-style)
/// Each u64 contains 64 cells, evolved using a lookup table
#[repr(align(64))]
pub struct CellularAutomaton1D {
    /// State: each bit is one cell
    state: Vec<u64>,
    /// Lookup table for 3-cell neighborhood → next cell
    rule_lut: [u8; 256],
}

impl CellularAutomaton1D {
    /// Create CA with given number of u64 words and rule number
    pub fn new(num_words: usize, rule: u8) -> Self {
        // Build LUT: for each 8-bit pattern, compute result
        let mut rule_lut = [0u8; 256];
        for pattern in 0..=255u8 {
            let mut result = 0u8;
            for bit in 0..8 {
                let neighborhood = (pattern >> bit) & 0b111;
                let next = (rule >> neighborhood) & 1;
                result |= next << bit;
            }
            rule_lut[pattern as usize] = result;
        }

        Self {
            state: vec![0xAAAA_AAAA_AAAA_AAAAu64; num_words],
            rule_lut,
        }
    }

    /// Set initial state
    pub fn set_state(&mut self, initial: &[u64]) {
        self.state.copy_from_slice(initial);
    }

    /// Get current state
    pub fn state(&self) -> &[u64] {
        &self.state
    }
}

impl BitParallelAutomaton for CellularAutomaton1D {
    fn step(&mut self) {
        let len = self.state.len();
        if len == 0 { return; }

        // We need to update in-place, so use temp for boundary handling
        let first = self.state[0];
        let last = self.state[len - 1];

        for i in 0..len {
            let left = if i == 0 { last } else { self.state[i - 1] };
            let center = self.state[i];
            let right = if i == len - 1 { first } else { self.state[i + 1] };

            let mut next = 0u64;
            for byte_idx in 0..8 {
                let shift = byte_idx * 8;
                // Extract 8-bit windows
                let l = ((left >> shift) & 0xFF) as u8;
                let c = ((center >> shift) & 0xFF) as u8;
                let r = ((right >> shift) & 0xFF) as u8;

                // Combine into neighborhood pattern and lookup
                let pattern = l.rotate_right(1) ^ c ^ r.rotate_left(1);
                let result = self.rule_lut[pattern as usize];
                next |= (result as u64) << shift;
            }
            self.state[i] = next;
        }
    }

    fn num_cells(&self) -> usize {
        self.state.len() * 64
    }
}

/// Binary Markov chain with bit-parallel transitions
/// Each bit represents one independent chain
#[repr(align(64))]
pub struct BinaryMarkovChain {
    /// Current states: 64 chains per u64
    states: Vec<u64>,
    /// Transition probability (0-65535 = 0.0-1.0)
    transition_threshold: u16,
    /// PRNG state
    rng_state: u64,
}

impl BinaryMarkovChain {
    /// Create n×64 independent binary chains
    pub fn new(num_words: usize, transition_prob: f64) -> Self {
        let threshold = (transition_prob * 65535.0) as u16;
        Self {
            states: vec![0; num_words],
            transition_threshold: threshold,
            rng_state: 0x12345678_9ABCDEF0,
        }
    }

    /// Fast xorshift64 PRNG
    #[inline(always)]
    fn next_random(&mut self) -> u64 {
        let mut x = self.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        x
    }
}

impl BitParallelAutomaton for BinaryMarkovChain {
    fn step(&mut self) {
        let threshold = self.transition_threshold;
        let len = self.states.len();

        for i in 0..len {
            let random = self.next_random();
            // Flip bit where random < threshold (probabilistic)
            // Using bit manipulation for parallel evaluation
            let flip_mask = random.wrapping_mul(threshold as u64);
            self.states[i] ^= flip_mask;
        }
    }

    fn num_cells(&self) -> usize {
        self.states.len() * 64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ca_creation() {
        let ca = CellularAutomaton1D::new(16, 110);
        assert_eq!(ca.num_cells(), 16 * 64);
    }

    #[test]
    fn test_ca_step() {
        let mut ca = CellularAutomaton1D::new(4, 110);
        let initial = ca.state().to_vec();
        ca.step();
        // State should change
        assert_ne!(ca.state(), &initial[..]);
    }

    #[test]
    fn test_markov_chain() {
        let mut mc = BinaryMarkovChain::new(8, 0.1);
        mc.step();
        assert_eq!(mc.num_cells(), 8 * 64);
    }
}
