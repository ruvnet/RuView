//! # Interference Search
//!
//! Concepts interfere during retrieval. Each concept can exist in a
//! superposition of multiple meanings, each with a complex amplitude.
//! When a search context is applied, the amplitudes interfere --
//! meanings aligned with the context get constructively boosted,
//! while misaligned meanings destructively cancel.
//!
//! This replaces simple cosine reranking with a quantum-inspired
//! interference model where polysemous concepts naturally resolve
//! to context-appropriate meanings.

use ruqu_core::types::Complex;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single meaning within a superposition: a label, an embedding, and a
/// complex amplitude.
#[derive(Debug, Clone)]
pub struct Meaning {
    pub label: String,
    pub embedding: Vec<f64>,
    pub amplitude: Complex,
}

/// A concept in superposition of multiple meanings.
#[derive(Debug, Clone)]
pub struct ConceptSuperposition {
    pub concept_id: String,
    pub meanings: Vec<Meaning>,
}

/// Score for a single meaning after interference with a context.
#[derive(Debug, Clone)]
pub struct InterferenceScore {
    pub label: String,
    pub probability: f64,
    pub amplitude: Complex,
}

/// A concept with its interference-computed relevance score.
#[derive(Debug, Clone)]
pub struct ConceptScore {
    pub concept_id: String,
    pub relevance: f64,
    pub dominant_meaning: String,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl ConceptSuperposition {
    /// Create a uniform superposition: all meanings get equal amplitude
    /// with zero phase.
    pub fn uniform(concept_id: &str, meanings: Vec<(String, Vec<f64>)>) -> Self {
        let n = meanings.len();
        let amp = if n > 0 { 1.0 / (n as f64).sqrt() } else { 0.0 };
        let meanings = meanings
            .into_iter()
            .map(|(label, embedding)| Meaning {
                label,
                embedding,
                amplitude: Complex::new(amp, 0.0),
            })
            .collect();
        Self {
            concept_id: concept_id.to_string(),
            meanings,
        }
    }

    /// Create a superposition with explicit complex amplitudes.
    pub fn with_amplitudes(concept_id: &str, meanings: Vec<(String, Vec<f64>, Complex)>) -> Self {
        let meanings = meanings
            .into_iter()
            .map(|(label, embedding, amplitude)| Meaning {
                label,
                embedding,
                amplitude,
            })
            .collect();
        Self {
            concept_id: concept_id.to_string(),
            meanings,
        }
    }

    /// Compute interference scores for each meaning given a context embedding.
    ///
    /// For each meaning, the context modifies the amplitude:
    ///   effective_amplitude = original_amplitude * (1 + similarity(meaning, context))
    ///
    /// Meanings aligned with the context get amplified; orthogonal meanings
    /// stay the same; opposing meanings get attenuated.
    ///
    /// Returns scores sorted by probability (descending).
    pub fn interfere(&self, context: &[f64]) -> Vec<InterferenceScore> {
        let mut scores: Vec<InterferenceScore> = self
            .meanings
            .iter()
            .map(|m| {
                let sim = cosine_similarity(&m.embedding, context);
                // Scale amplitude by (1 + sim). For sim in [-1, 1], this gives
                // a factor in [0, 2]. Negative similarity attenuates.
                let scale = (1.0 + sim).max(0.0);
                let effective = m.amplitude * scale;
                InterferenceScore {
                    label: m.label.clone(),
                    probability: effective.norm_sq(),
                    amplitude: effective,
                }
            })
            .collect();

        scores.sort_by(|a, b| {
            b.probability
                .partial_cmp(&a.probability)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scores
    }

    /// Collapse the superposition to a single meaning by sampling from
    /// the interference-weighted probability distribution.
    pub fn collapse(&self, context: &[f64], seed: u64) -> String {
        let scores = self.interfere(context);
        let total: f64 = scores.iter().map(|s| s.probability).sum();
        if total < 1e-15 {
            // Degenerate case: return first meaning if available
            return scores.first().map(|s| s.label.clone()).unwrap_or_default();
        }

        let mut rng = StdRng::seed_from_u64(seed);
        let r: f64 = rng.gen::<f64>() * total;
        let mut cumulative = 0.0;
        for score in &scores {
            cumulative += score.probability;
            if r <= cumulative {
                return score.label.clone();
            }
        }
        scores.last().map(|s| s.label.clone()).unwrap_or_default()
    }

    /// Return the dominant meaning: the one with the largest |amplitude|^2
    /// (before any context is applied).
    pub fn dominant(&self) -> Option<&Meaning> {
        self.meanings.iter().max_by(|a, b| {
            a.amplitude
                .norm_sq()
                .partial_cmp(&b.amplitude.norm_sq())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Run an interference search across multiple concepts, ranking them by
/// relevance to the given query context.
///
/// Returns concepts sorted by relevance (descending).
pub fn interference_search(
    concepts: &[ConceptSuperposition],
    context: &[f64],
) -> Vec<ConceptScore> {
    let mut results: Vec<ConceptScore> = concepts
        .iter()
        .map(|concept| {
            let scores = concept.interfere(context);
            let relevance: f64 = scores.iter().map(|s| s.probability).sum();
            let dominant_meaning = scores.first().map(|s| s.label.clone()).unwrap_or_default();
            ConceptScore {
                concept_id: concept.concept_id.clone(),
                relevance,
                dominant_meaning,
            }
        })
        .collect();

    results.sort_by(|a, b| {
        b.relevance
            .partial_cmp(&a.relevance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let len = a.len().min(b.len());
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;
    for i in 0..len {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom < 1e-15 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}
