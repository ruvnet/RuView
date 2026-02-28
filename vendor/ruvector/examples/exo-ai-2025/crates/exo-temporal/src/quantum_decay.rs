//! Quantum Decay Memory Eviction — ADR-029 temporal memory extension.
//!
//! Replaces hard TTL expiry with T1/T2-inspired decoherence-based eviction.
//! Patterns decohere with time constants proportional to their retrieval
//! frequency and IIT Φ value — high-Φ, often-retrieved patterns have longer
//! coherence times (Φ-stabilized memory).
//!
//! Key insight: T2 < T1 always (dephasing faster than relaxation), matching
//! the empirical observation that memory detail fades before memory existence.

use std::time::{Duration, Instant};

/// Per-pattern decoherence state
#[derive(Debug, Clone)]
pub struct PatternDecoherence {
    /// Pattern id
    pub id: u64,
    /// T1 relaxation time (energy/existence decay)
    pub t1: Duration,
    /// T2 dephasing time (detail/coherence decay)
    pub t2: Duration,
    /// Initial creation time
    pub created_at: Instant,
    /// Last retrieval time (refreshes coherence)
    pub last_retrieved: Instant,
    /// Φ value at creation — high Φ → longer coherence
    pub phi: f64,
    /// Retrieval count (higher count → refreshed T1)
    pub retrieval_count: u32,
}

impl PatternDecoherence {
    pub fn new(id: u64, phi: f64) -> Self {
        let now = Instant::now();
        // Base times: T1 = 60s, T2 = 30s (T2 < T1 always)
        // Φ-scaling: high Φ extends both times
        let phi_factor = (1.0 + phi * 0.5).min(10.0); // max 10x extension
        let t1 = Duration::from_millis((60_000.0 * phi_factor) as u64);
        let t2 = Duration::from_millis((30_000.0 * phi_factor) as u64);
        Self {
            id,
            t1,
            t2,
            created_at: now,
            last_retrieved: now,
            phi,
            retrieval_count: 0,
        }
    }

    /// Refresh coherence on retrieval (use-dependent plasticity analog)
    pub fn refresh(&mut self) {
        self.last_retrieved = Instant::now();
        self.retrieval_count += 1;
        // Hebbian refreshing: each retrieval extends T2 by 10%
        self.t2 = Duration::from_millis(
            (self.t2.as_millis() as f64 * 1.1).min(self.t1.as_millis() as f64) as u64,
        );
    }

    /// Current T2 coherence amplitude (1.0 = fully coherent, 0.0 = decoherent)
    pub fn coherence_amplitude(&self) -> f64 {
        let elapsed = self.last_retrieved.elapsed().as_millis() as f64;
        let t2_ms = self.t2.as_millis() as f64;
        (-elapsed / t2_ms).exp().max(0.0)
    }

    /// Current T1 existence probability (1.0 = exists, 0.0 = relaxed/forgotten)
    pub fn existence_probability(&self) -> f64 {
        let elapsed = self.created_at.elapsed().as_millis() as f64;
        let t1_ms = self.t1.as_millis() as f64;
        (-elapsed / t1_ms).exp().max(0.0)
    }

    /// Combined decoherence score for eviction decisions.
    /// Low score → candidate for eviction.
    pub fn decoherence_score(&self) -> f64 {
        self.coherence_amplitude() * self.existence_probability()
    }

    /// Should this pattern be evicted?
    pub fn should_evict(&self, threshold: f64) -> bool {
        self.decoherence_score() < threshold
    }
}

/// Quantum decay memory manager: tracks decoherence for a pool of patterns
pub struct QuantumDecayPool {
    pub patterns: Vec<PatternDecoherence>,
    /// Eviction threshold (patterns below this decoherence score are evicted)
    pub eviction_threshold: f64,
    /// Maximum pool size (hard cap)
    pub max_size: usize,
}

impl QuantumDecayPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            patterns: Vec::with_capacity(max_size),
            eviction_threshold: 0.1,
            max_size,
        }
    }

    /// Register a pattern with its Φ value.
    pub fn register(&mut self, id: u64, phi: f64) {
        if self.patterns.len() >= self.max_size {
            self.evict_weakest();
        }
        self.patterns.push(PatternDecoherence::new(id, phi));
    }

    /// Record retrieval — refreshes coherence.
    pub fn on_retrieve(&mut self, id: u64) {
        if let Some(p) = self.patterns.iter_mut().find(|p| p.id == id) {
            p.refresh();
        }
    }

    /// Get decoherence-weighted score for search results.
    pub fn weighted_score(&self, id: u64, base_score: f64) -> f64 {
        self.patterns
            .iter()
            .find(|p| p.id == id)
            .map(|p| base_score * (0.3 + 0.7 * p.decoherence_score()))
            .unwrap_or(base_score * 0.5) // Unknown patterns get 50% weight
    }

    /// Evict decoherent patterns, return count evicted.
    pub fn evict_decoherent(&mut self) -> usize {
        let threshold = self.eviction_threshold;
        let before = self.patterns.len();
        self.patterns.retain(|p| !p.should_evict(threshold));
        before - self.patterns.len()
    }

    /// Evict the weakest pattern (lowest decoherence score).
    fn evict_weakest(&mut self) {
        if let Some(idx) = self
            .patterns
            .iter()
            .enumerate()
            .min_by(|a, b| {
                a.1.decoherence_score()
                    .partial_cmp(&b.1.decoherence_score())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(i, _)| i)
        {
            self.patterns.remove(idx);
        }
    }

    pub fn len(&self) -> usize {
        self.patterns.len()
    }
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Statistics for monitoring
    pub fn stats(&self) -> DecayPoolStats {
        if self.patterns.is_empty() {
            return DecayPoolStats::default();
        }
        let scores: Vec<f64> = self
            .patterns
            .iter()
            .map(|p| p.decoherence_score())
            .collect();
        let mean = scores.iter().sum::<f64>() / scores.len() as f64;
        let min = scores.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        DecayPoolStats {
            count: self.patterns.len(),
            mean_score: mean,
            min_score: min,
            max_score: max,
        }
    }
}

#[derive(Debug, Default)]
pub struct DecayPoolStats {
    pub count: usize,
    pub mean_score: f64,
    pub min_score: f64,
    pub max_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phi_extends_coherence_time() {
        let low_phi = PatternDecoherence::new(0, 0.1);
        let high_phi = PatternDecoherence::new(1, 5.0);
        // High Φ pattern should have longer T1 and T2
        assert!(high_phi.t1 > low_phi.t1, "High Φ should extend T1");
        assert!(high_phi.t2 > low_phi.t2, "High Φ should extend T2");
    }

    #[test]
    fn test_t2_less_than_t1() {
        let pattern = PatternDecoherence::new(0, 1.0);
        assert!(
            pattern.t2 <= pattern.t1,
            "T2 must never exceed T1 (physical constraint)"
        );
    }

    #[test]
    fn test_retrieval_refreshes_coherence() {
        let mut pattern = PatternDecoherence::new(0, 1.0);
        let initial_t2 = pattern.t2;
        pattern.refresh();
        assert!(pattern.t2 >= initial_t2, "Retrieval should not decrease T2");
        assert_eq!(pattern.retrieval_count, 1);
    }

    #[test]
    fn test_pool_evicts_decoherent() {
        let mut pool = QuantumDecayPool::new(100);
        // Add pattern with very short T2 (will decohere fast)
        let mut fast_decoh = PatternDecoherence::new(99, 0.0001);
        fast_decoh.t1 = Duration::from_micros(1);
        fast_decoh.t2 = Duration::from_micros(1);
        pool.patterns.push(fast_decoh);
        // High-Φ pattern should survive
        pool.register(1, 10.0);
        std::thread::sleep(Duration::from_millis(5));
        let evicted = pool.evict_decoherent();
        assert!(evicted > 0, "Fast-decoherent pattern should be evicted");
        assert!(
            pool.patterns.iter().any(|p| p.id == 1),
            "High-Φ pattern should survive"
        );
    }

    #[test]
    fn test_decoherence_weighted_score() {
        let mut pool = QuantumDecayPool::new(10);
        pool.register(5, 2.0);
        let weighted = pool.weighted_score(5, 1.0);
        // Should be between 0.3 and 1.0 (decoherence_score is in [0,1])
        assert!(
            weighted > 0.0 && weighted <= 1.0,
            "Weighted score should be in (0,1]"
        );
    }
}
