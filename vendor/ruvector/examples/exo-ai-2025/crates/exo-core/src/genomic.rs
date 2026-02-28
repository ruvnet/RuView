//! Genomic integration — ADR-029 bridge from ruDNA .rvdna to EXO-AI patterns.
//!
//! .rvdna files contain pre-computed:
//! - 64-dim health risk profiles (HealthProfile64)
//! - 512-dim GNN protein embeddings
//! - k-mer vectors
//! - polygenic risk scores
//! - Horvath epigenetic clock (353 CpG sites → biological age)
//!
//! This module provides:
//! 1. RvDnaPattern: a genomic pattern for EXO-AI memory
//! 2. HorvathClock: biological age → SubstrateTime mapping
//! 3. PharmacogenomicWeights: gene variants → synaptic weight modifiers
//! 4. GenomicPatternStore: in-memory store with Phi-weighted recall

/// A genomic pattern compatible with EXO-AI memory substrate.
/// Derived from .rvdna sequence data via the ruDNA pipeline.
#[derive(Debug, Clone)]
pub struct RvDnaPattern {
    /// Unique pattern identifier (from sequence hash)
    pub id: u64,
    /// 64-dimensional health risk profile embedding
    pub health_embedding: [f32; 64],
    /// Polygenic risk score (0.0–1.0, higher = higher risk)
    pub polygenic_risk: f32,
    /// Estimated biological age via Horvath clock (years)
    pub biological_age: f32,
    /// Chronological age at sample collection (years)
    pub chronological_age: f32,
    /// Sample identifier hash
    pub sample_hash: [u8; 32],
    /// Neurotransmitter-relevant gene activity scores
    pub neuro_profile: NeurotransmitterProfile,
}

/// Neurotransmitter-relevant gene activity (relevant for cognitive substrate)
#[derive(Debug, Clone, Default)]
pub struct NeurotransmitterProfile {
    /// Dopamine pathway activity (DRD2, COMT, SLC6A3) — 0.0–1.0
    pub dopamine: f32,
    /// Serotonin pathway activity (SLC6A4, MAOA, TPH2) — 0.0–1.0
    pub serotonin: f32,
    /// GABA/Glutamate balance (GRIN2A, GABRA1, SLC1A2) — 0.0–1.0
    pub gaba_glutamate_ratio: f32,
    /// Neuroplasticity score (BDNF, NRXN1, SHANK3) — 0.0–1.0
    pub plasticity_score: f32,
    /// Circadian regulation (PER1, CLOCK, ARNTL) — 0.0–1.0
    pub circadian_regulation: f32,
}

impl NeurotransmitterProfile {
    /// Overall neuronal excitability score for IIT Φ weighting
    pub fn excitability_score(&self) -> f32 {
        (self.dopamine * 0.3
            + self.serotonin * 0.2
            + self.gaba_glutamate_ratio * 0.2
            + self.plasticity_score * 0.3)
            .clamp(0.0, 1.0)
    }

    /// Circadian phase offset (maps to Kuramoto phase in NeuromorphicBackend)
    pub fn circadian_phase_rad(&self) -> f32 {
        self.circadian_regulation * 2.0 * std::f32::consts::PI
    }
}

/// Horvath epigenetic clock — maps biological age to cognitive substrate time.
/// Based on 353 CpG site methylation levels (Horvath 2013, Genome Biology).
pub struct HorvathClock {
    /// Intercept from Horvath's original regression
    pub intercept: f64,
    /// Age transformation function
    adult_age_transform: f64,
}

impl HorvathClock {
    pub fn new() -> Self {
        Self {
            intercept: 0.696,
            adult_age_transform: 20.0,
        }
    }

    /// Predict biological age from methylation levels (simplified model)
    /// Full model uses 353 CpG sites — this uses a compressed 10-site proxy
    pub fn predict_age(&self, methylation_proxy: &[f32]) -> f32 {
        if methylation_proxy.is_empty() {
            return 30.0;
        }
        // Anti-correlated sites accelerate aging; correlated sites decelerate
        let signal: f64 = methylation_proxy
            .iter()
            .enumerate()
            .map(|(i, &m)| {
                // Alternating positive/negative weights (simplified from full model)
                let w = if i % 2 == 0 { 1.5 } else { -0.8 };
                w * m as f64
            })
            .sum::<f64>()
            / methylation_proxy.len() as f64;

        // Horvath transformation: anti-log transform for age > 20
        let transformed = self.intercept + signal;
        if transformed < 0.0 {
            (self.adult_age_transform * 2.0_f64.powf(transformed) - 1.0) as f32
        } else {
            (self.adult_age_transform * (transformed + 1.0)) as f32
        }
    }

    /// Compute age acceleration (biological - chronological)
    pub fn age_acceleration(&self, methylation: &[f32], chronological_age: f32) -> f32 {
        let bio_age = self.predict_age(methylation);
        bio_age - chronological_age
    }
}

impl Default for HorvathClock {
    fn default() -> Self {
        Self::new()
    }
}

/// Pharmacogenomic weight modifiers for IIT Φ computation.
/// Maps gene variants to synaptic weight scaling factors.
pub struct PharmacogenomicWeights {
    #[allow(dead_code)]
    clock: HorvathClock,
}

impl PharmacogenomicWeights {
    pub fn new() -> Self {
        Self {
            clock: HorvathClock::new(),
        }
    }

    /// Compute Φ-weighting factor from neurotransmitter profile.
    /// Higher excitability + high plasticity → higher Φ weight (more consciousness).
    pub fn phi_weight(&self, neuro: &NeurotransmitterProfile) -> f64 {
        let excit = neuro.excitability_score() as f64;
        let plastic = neuro.plasticity_score as f64;
        // Φ ∝ excitability × plasticity (both needed for high integrated information)
        (1.0 + 3.0 * excit * plastic).min(5.0)
    }

    /// Connection weight scaling for IIT substrate.
    /// Maps gene activity to network edge weights.
    pub fn connection_weight_scale(&self, neuro: &NeurotransmitterProfile) -> f32 {
        let da_effect = 1.0 + 0.5 * neuro.dopamine; // Dopamine increases connection strength
        let gaba_effect = 1.0 - 0.3 * neuro.gaba_glutamate_ratio; // GABA inhibits
        (da_effect * gaba_effect).clamp(0.3, 2.5)
    }

    /// Age-dependent memory decay rate (young = slower decay, old = faster)
    pub fn memory_decay_rate(&self, bio_age: f32) -> f64 {
        // Logistic: fast decay for >50, slow for <30
        1.0 / (1.0 + (-0.1 * (bio_age as f64 - 40.0)).exp())
    }
}

impl Default for PharmacogenomicWeights {
    fn default() -> Self {
        Self::new()
    }
}

/// In-memory genomic pattern store with pharmacogenomic-weighted retrieval
pub struct GenomicPatternStore {
    patterns: Vec<RvDnaPattern>,
    weights: PharmacogenomicWeights,
}

#[derive(Debug)]
pub struct GenomicSearchResult {
    pub id: u64,
    pub similarity: f32,
    pub phi_weight: f64,
    pub weighted_score: f64,
}

impl GenomicPatternStore {
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
            weights: PharmacogenomicWeights::new(),
        }
    }

    pub fn insert(&mut self, pattern: RvDnaPattern) {
        self.patterns.push(pattern);
    }

    /// Cosine similarity between health embeddings
    fn cosine_similarity(a: &[f32; 64], b: &[f32; 64]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-8);
        dot / (na * nb)
    }

    /// Search with pharmacogenomic Φ-weighting
    pub fn search(&self, query: &RvDnaPattern, k: usize) -> Vec<GenomicSearchResult> {
        let mut results: Vec<GenomicSearchResult> = self
            .patterns
            .iter()
            .map(|p| {
                let sim = Self::cosine_similarity(&query.health_embedding, &p.health_embedding);
                let phi_w = self.weights.phi_weight(&p.neuro_profile);
                GenomicSearchResult {
                    id: p.id,
                    similarity: sim,
                    phi_weight: phi_w,
                    weighted_score: sim as f64 * phi_w,
                }
            })
            .collect();
        results.sort_unstable_by(|a, b| {
            b.weighted_score
                .partial_cmp(&a.weighted_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(k);
        results
    }

    pub fn len(&self) -> usize {
        self.patterns.len()
    }
}

impl Default for GenomicPatternStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a test pattern from synthetic data (for testing without actual .rvdna files)
pub fn synthetic_rvdna_pattern(id: u64, seed: u64) -> RvDnaPattern {
    let mut health = [0.0f32; 64];
    let mut s = seed.wrapping_mul(0x9e3779b97f4a7c15);
    for h in health.iter_mut() {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *h = (s >> 33) as f32 / (u32::MAX as f32);
    }
    let neuro = NeurotransmitterProfile {
        dopamine: (seed as f32 * 0.1) % 1.0,
        serotonin: ((seed + 1) as f32 * 0.15) % 1.0,
        gaba_glutamate_ratio: 0.5,
        plasticity_score: ((seed + 2) as f32 * 0.07) % 1.0,
        circadian_regulation: ((seed + 3) as f32 * 0.13) % 1.0,
    };
    RvDnaPattern {
        id,
        health_embedding: health,
        polygenic_risk: (seed as f32 * 0.003) % 1.0,
        biological_age: 20.0 + (seed as f32 * 0.5) % 40.0,
        chronological_age: 25.0 + (seed as f32 * 0.4) % 35.0,
        sample_hash: [0u8; 32],
        neuro_profile: neuro,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horvath_clock_adult_age() {
        let clock = HorvathClock::new();
        let methylation = vec![0.5f32; 10];
        let age = clock.predict_age(&methylation);
        assert!(
            age > 0.0 && age < 120.0,
            "Biological age should be in [0, 120]: {}",
            age
        );
    }

    #[test]
    fn test_phi_weight_scales_with_excitability() {
        let weights = PharmacogenomicWeights::new();
        let low_neuro = NeurotransmitterProfile {
            dopamine: 0.1,
            serotonin: 0.1,
            gaba_glutamate_ratio: 0.1,
            plasticity_score: 0.1,
            circadian_regulation: 0.5,
        };
        let high_neuro = NeurotransmitterProfile {
            dopamine: 0.9,
            serotonin: 0.8,
            gaba_glutamate_ratio: 0.5,
            plasticity_score: 0.9,
            circadian_regulation: 0.5,
        };
        let low_phi = weights.phi_weight(&low_neuro);
        let high_phi = weights.phi_weight(&high_neuro);
        assert!(
            high_phi > low_phi,
            "High excitability should yield higher Φ weight"
        );
    }

    #[test]
    fn test_genomic_store_search() {
        let mut store = GenomicPatternStore::new();
        for i in 0..10u64 {
            store.insert(synthetic_rvdna_pattern(i, i * 13));
        }
        let query = synthetic_rvdna_pattern(0, 0);
        let results = store.search(&query, 3);
        assert!(!results.is_empty());
        assert!(
            results[0].weighted_score >= results.last().map(|r| r.weighted_score).unwrap_or(0.0)
        );
    }

    #[test]
    fn test_neuro_circadian_phase() {
        let neuro = NeurotransmitterProfile {
            circadian_regulation: 0.5,
            ..Default::default()
        };
        let phase = neuro.circadian_phase_rad();
        assert!(phase >= 0.0 && phase <= 2.0 * std::f32::consts::PI);
    }
}
