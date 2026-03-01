//! Deterministic chaos seeding using π digits
//!
//! π digits are deterministic but appear random. This makes π perfect for:
//! - Deterministic jitter
//! - Tie-breaking
//! - Sampling order
//! - Agent scheduling
//! - Micro-LoRA update ordering
//!
//! You get pseudo-randomness without RNG state, clocks, or entropy sources.
//! Same input, same behavior, always.
//!
//! That is gold for witness-logged systems.

use super::constants::PI_DIGITS;
use std::f32::consts::PI;

/// π-based deterministic chaos generator
#[derive(Debug, Clone)]
pub struct PiChaos {
    /// Current position in π digit stream
    position: usize,
    /// Scale factor for jitter
    jitter_scale: f32,
    /// Extended digit buffer (for longer sequences)
    extended_buffer: Vec<u8>,
}

impl PiChaos {
    /// Create a new π chaos generator
    pub fn new() -> Self {
        Self {
            position: 0,
            jitter_scale: 0.001, // Default: small jitter
            extended_buffer: PI_DIGITS.to_vec(),
        }
    }

    /// Create with custom jitter scale
    pub fn with_jitter_scale(mut self, scale: f32) -> Self {
        self.jitter_scale = scale;
        self
    }

    /// Get deterministic jitter for an index
    pub fn jitter(&self, index: usize) -> f32 {
        let digit_idx = index % PI_DIGITS.len();
        let digit = PI_DIGITS[digit_idx] as f32;

        // Map digit (0-9) to jitter range
        (digit - 4.5) / 9.0 * self.jitter_scale
    }

    /// Get jitter vector for a range of indices
    pub fn jitter_vector(&self, start: usize, count: usize) -> Vec<f32> {
        (start..(start + count)).map(|i| self.jitter(i)).collect()
    }

    /// Get next π digit in sequence
    pub fn next_digit(&mut self) -> u8 {
        let digit = self.extended_buffer[self.position];
        self.position = (self.position + 1) % self.extended_buffer.len();
        digit
    }

    /// Get next float in [0, 1) from π digits
    pub fn next_float(&mut self) -> f32 {
        // Use 3 digits for ~10 bits of precision
        let d1 = self.next_digit() as f32;
        let d2 = self.next_digit() as f32;
        let d3 = self.next_digit() as f32;

        (d1 * 100.0 + d2 * 10.0 + d3) / 1000.0
    }

    /// Get next integer in [0, max)
    pub fn next_int(&mut self, max: usize) -> usize {
        if max == 0 {
            return 0;
        }
        let f = self.next_float();
        (f * max as f32) as usize % max
    }

    /// Reset to beginning of π sequence
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Seed at specific position
    pub fn seed(&mut self, position: usize) {
        self.position = position % self.extended_buffer.len();
    }

    /// Generate deterministic permutation of indices
    pub fn permutation(&mut self, n: usize) -> Vec<usize> {
        let mut indices: Vec<usize> = (0..n).collect();

        // Fisher-Yates shuffle with π randomness
        for i in (1..n).rev() {
            let j = self.next_int(i + 1);
            indices.swap(i, j);
        }

        indices
    }

    /// Get scheduling order for n agents
    pub fn schedule_order(&self, n: usize, round: usize) -> Vec<usize> {
        let mut chaos = self.clone();
        chaos.seed(round * n);
        chaos.permutation(n)
    }
}

impl Default for PiChaos {
    fn default() -> Self {
        Self::new()
    }
}

/// Deterministic jitter generator for tie-breaking
#[derive(Debug, Clone)]
pub struct DeterministicJitter {
    /// Base jitter magnitude
    magnitude: f32,
    /// π chaos source
    chaos: PiChaos,
}

impl DeterministicJitter {
    /// Create a new jitter generator
    pub fn new(magnitude: f32) -> Self {
        Self {
            magnitude,
            chaos: PiChaos::new().with_jitter_scale(magnitude),
        }
    }

    /// Add jitter to a value
    pub fn apply(&self, value: f32, index: usize) -> f32 {
        value + self.chaos.jitter(index)
    }

    /// Add jitter to a vector
    pub fn apply_vector(&self, values: &[f32]) -> Vec<f32> {
        values
            .iter()
            .enumerate()
            .map(|(i, &v)| self.apply(v, i))
            .collect()
    }

    /// Break tie between equal values using index-based jitter
    pub fn break_tie(&self, value: f32, indices: &[usize]) -> usize {
        indices
            .iter()
            .copied()
            .max_by(|&a, &b| {
                let ja = self.chaos.jitter(a);
                let jb = self.chaos.jitter(b);
                ja.partial_cmp(&jb).unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap_or(0)
    }
}

/// π-based scheduler for deterministic agent/task ordering
#[derive(Debug, Clone)]
pub struct PiScheduler {
    /// Number of agents/tasks
    num_items: usize,
    /// Current round
    round: usize,
    /// π chaos source
    chaos: PiChaos,
    /// Priority weights (optional)
    weights: Option<Vec<f32>>,
}

impl PiScheduler {
    /// Create a new scheduler
    pub fn new(num_items: usize) -> Self {
        Self {
            num_items,
            round: 0,
            chaos: PiChaos::new(),
            weights: None,
        }
    }

    /// Set priority weights
    pub fn with_weights(mut self, weights: Vec<f32>) -> Self {
        assert_eq!(weights.len(), self.num_items);
        self.weights = Some(weights);
        self
    }

    /// Get execution order for current round
    pub fn get_order(&self) -> Vec<usize> {
        self.chaos.schedule_order(self.num_items, self.round)
    }

    /// Get weighted execution order
    pub fn get_weighted_order(&self) -> Vec<usize> {
        let mut order = self.get_order();

        if let Some(ref weights) = self.weights {
            // Sort by weight, using π jitter for tie-breaking
            order.sort_by(|&a, &b| {
                let wa = weights[a] + self.chaos.jitter(a) * 0.001;
                let wb = weights[b] + self.chaos.jitter(b) * 0.001;
                wb.partial_cmp(&wa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        order
    }

    /// Advance to next round
    pub fn next_round(&mut self) {
        self.round += 1;
    }

    /// Reset to round 0
    pub fn reset(&mut self) {
        self.round = 0;
    }

    /// Get item for micro-LoRA update based on π sequence
    pub fn get_lora_update_order(&self, round: usize) -> Vec<usize> {
        // For LoRA, we want a different permutation that prioritizes
        // items with higher impact (measured by weights)
        let base_order = self.chaos.schedule_order(self.num_items, round);

        if let Some(ref weights) = self.weights {
            // Interleave high-weight and low-weight items
            let mut sorted_by_weight: Vec<(usize, f32)> =
                base_order.iter().map(|&i| (i, weights[i])).collect();
            sorted_by_weight
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let mut result = Vec::with_capacity(self.num_items);
            let high_priority = &sorted_by_weight[..self.num_items / 2];
            let low_priority = &sorted_by_weight[self.num_items / 2..];

            let mut h = 0;
            let mut l = 0;
            for i in 0..self.num_items {
                if i % 3 < 2 && h < high_priority.len() {
                    result.push(high_priority[h].0);
                    h += 1;
                } else if l < low_priority.len() {
                    result.push(low_priority[l].0);
                    l += 1;
                } else if h < high_priority.len() {
                    result.push(high_priority[h].0);
                    h += 1;
                }
            }
            result
        } else {
            base_order
        }
    }

    /// Get sampling indices for mini-batch
    pub fn sample_indices(&mut self, batch_size: usize, total: usize) -> Vec<usize> {
        let mut chaos = self.chaos.clone();
        chaos.seed(self.round * total);
        let perm = chaos.permutation(total);
        perm.into_iter().take(batch_size.min(total)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pi_chaos_deterministic() {
        let chaos1 = PiChaos::new();
        let chaos2 = PiChaos::new();

        // Same index = same jitter
        assert_eq!(chaos1.jitter(0), chaos2.jitter(0));
        assert_eq!(chaos1.jitter(42), chaos2.jitter(42));
    }

    #[test]
    fn test_pi_chaos_different_indices() {
        let chaos = PiChaos::new();

        let j0 = chaos.jitter(0);
        let j1 = chaos.jitter(1);
        let j2 = chaos.jitter(2);

        // Different indices should have different jitter
        // (except by chance if same π digit)
        assert!(j0 != j1 || j1 != j2);
    }

    #[test]
    fn test_pi_chaos_next_float() {
        let mut chaos = PiChaos::new();

        let f1 = chaos.next_float();
        let f2 = chaos.next_float();

        // Should be in [0, 1)
        assert!(f1 >= 0.0 && f1 < 1.0);
        assert!(f2 >= 0.0 && f2 < 1.0);

        // Reset should give same sequence
        chaos.reset();
        assert_eq!(chaos.next_float(), f1);
    }

    #[test]
    fn test_pi_chaos_permutation() {
        let mut chaos = PiChaos::new();
        let perm = chaos.permutation(10);

        // Should contain all elements
        assert_eq!(perm.len(), 10);
        let mut sorted = perm.clone();
        sorted.sort();
        assert_eq!(sorted, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn test_pi_chaos_permutation_deterministic() {
        let mut chaos1 = PiChaos::new();
        let mut chaos2 = PiChaos::new();

        let perm1 = chaos1.permutation(20);
        let perm2 = chaos2.permutation(20);

        assert_eq!(perm1, perm2);
    }

    #[test]
    fn test_deterministic_jitter() {
        let jitter = DeterministicJitter::new(0.01);

        let values = vec![1.0, 1.0, 1.0, 1.0];
        let jittered = jitter.apply_vector(&values);

        // All original values were same, but jittered should differ
        let unique: std::collections::HashSet<_> =
            jittered.iter().map(|x| (x * 10000.0) as i32).collect();
        assert!(unique.len() > 1);
    }

    #[test]
    fn test_pi_scheduler() {
        let scheduler = PiScheduler::new(5);
        let order1 = scheduler.get_order();

        assert_eq!(order1.len(), 5);
        let mut sorted = order1.clone();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn test_pi_scheduler_rounds() {
        let mut scheduler = PiScheduler::new(5);
        let order_r0 = scheduler.get_order();

        scheduler.next_round();
        let order_r1 = scheduler.get_order();

        // Different rounds may have different orders
        // (not guaranteed but likely with π digits)
        // Just check both are valid permutations
        assert_eq!(order_r0.len(), 5);
        assert_eq!(order_r1.len(), 5);
    }

    #[test]
    fn test_pi_scheduler_weighted() {
        let weights = vec![1.0, 0.5, 2.0, 0.1, 1.5];
        let scheduler = PiScheduler::new(5).with_weights(weights);
        let order = scheduler.get_weighted_order();

        // Highest weight (index 2) should be early
        let pos_2 = order.iter().position(|&x| x == 2).unwrap();
        assert!(pos_2 < 3, "High weight item should be scheduled early");
    }

    #[test]
    fn test_schedule_order_deterministic() {
        let chaos = PiChaos::new();
        let order1 = chaos.schedule_order(10, 5);
        let order2 = chaos.schedule_order(10, 5);
        assert_eq!(order1, order2);
    }
}
