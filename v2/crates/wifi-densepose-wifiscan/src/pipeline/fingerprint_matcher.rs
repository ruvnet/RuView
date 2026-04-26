//! Stage 7: BSSID fingerprint matching via cosine similarity.
//!
//! Stores reference BSSID amplitude patterns for known postures
//! (standing, sitting, walking, empty) and classifies new observations
//! by retrieving the nearest stored template.
//!
//! This is a pure-Rust implementation using cosine similarity. When
//! `ruvector-nervous-system` becomes available, the inner store can
//! be replaced with `ModernHopfield` for richer associative memory.

use crate::domain::result::PostureClass;

/// A stored posture fingerprint template.
#[derive(Debug, Clone)]
struct PostureTemplate {
    /// Reference amplitude pattern (normalised).
    pattern: Vec<f32>,
    /// The posture label for this template.
    label: PostureClass,
}

/// BSSID fingerprint matcher using cosine similarity.
pub struct FingerprintMatcher {
    /// Stored reference templates.
    templates: Vec<PostureTemplate>,
    /// Minimum cosine similarity for a match.
    confidence_threshold: f32,
    /// Expected dimension (number of BSSID slots).
    n_bssids: usize,
}

impl FingerprintMatcher {
    /// Create a new fingerprint matcher.
    ///
    /// - `n_bssids`: number of BSSID slots (pattern dimension).
    /// - `confidence_threshold`: minimum cosine similarity for a match.
    #[must_use]
    pub fn new(n_bssids: usize, confidence_threshold: f32) -> Self {
        Self {
            templates: Vec::new(),
            confidence_threshold,
            n_bssids,
        }
    }

    /// Store a reference pattern with its posture label.
    ///
    /// # Errors
    ///
    /// Returns an error if the pattern dimension does not match `n_bssids`.
    pub fn store_pattern(
        &mut self,
        pattern: Vec<f32>,
        label: PostureClass,
    ) -> Result<(), String> {
        if pattern.len() != self.n_bssids {
            return Err(format!(
                "pattern dimension {} != expected {}",
                pattern.len(),
                self.n_bssids
            ));
        }
        self.templates.push(PostureTemplate { pattern, label });
        Ok(())
    }

    /// Classify an observation by matching against stored fingerprints.
    ///
    /// Returns the best-matching posture and similarity score, or `None`
    /// if no patterns are stored or similarity is below threshold.
    #[must_use]
    pub fn classify(&self, observation: &[f32]) -> Option<(PostureClass, f32)> {
        if self.templates.is_empty() || observation.len() != self.n_bssids {
            return None;
        }

        let mut best_label = None;
        let mut best_sim = f32::NEG_INFINITY;

        for tmpl in &self.templates {
            let sim = cosine_similarity(&tmpl.pattern, observation);
            if sim > best_sim {
                best_sim = sim;
                best_label = Some(tmpl.label);
            }
        }

        match best_label {
            Some(label) if best_sim >= self.confidence_threshold => Some((label, best_sim)),
            _ => None,
        }
    }

    /// Match posture and return a structured result.
    #[must_use]
    pub fn match_posture(&self, observation: &[f32]) -> MatchResult {
        match self.classify(observation) {
            Some((posture, confidence)) => MatchResult {
                posture: Some(posture),
                confidence,
                matched: true,
            },
            None => MatchResult {
                posture: None,
                confidence: 0.0,
                matched: false,
            },
        }
    }

    /// Generate default templates from a baseline signal.
    ///
    /// Creates heuristic patterns for standing, sitting, and empty by
    /// scaling the baseline amplitude pattern.
    pub fn generate_defaults(&mut self, baseline: &[f32]) {
        if baseline.len() != self.n_bssids {
            return;
        }

        // Empty: very low amplitude (background noise only)
        let empty: Vec<f32> = baseline.iter().map(|&a| a * 0.1).collect();
        let _ = self.store_pattern(empty, PostureClass::Empty);

        // Standing: moderate perturbation of some BSSIDs
        let standing: Vec<f32> = baseline
            .iter()
            .enumerate()
            .map(|(i, &a)| if i % 3 == 0 { a * 1.3 } else { a })
            .collect();
        let _ = self.store_pattern(standing, PostureClass::Standing);

        // Sitting: different perturbation pattern
        let sitting: Vec<f32> = baseline
            .iter()
            .enumerate()
            .map(|(i, &a)| if i % 2 == 0 { a * 1.2 } else { a * 0.9 })
            .collect();
        let _ = self.store_pattern(sitting, PostureClass::Sitting);
    }

    /// Number of stored patterns.
    #[must_use]
    pub fn num_patterns(&self) -> usize {
        self.templates.len()
    }

    /// Clear all stored patterns.
    pub fn clear(&mut self) {
        self.templates.clear();
    }

    /// Set the minimum similarity threshold for classification.
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold;
    }
}

/// Result of fingerprint matching.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Matched posture class (None if no match).
    pub posture: Option<PostureClass>,
    /// Cosine similarity of the best match.
    pub confidence: f32,
    /// Whether a match was found above threshold.
    pub matched: bool,
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..n {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a * norm_b).sqrt();
    if denom < 1e-12 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_matcher_returns_none() {
        let matcher = FingerprintMatcher::new(4, 0.5);
        assert!(matcher.classify(&[1.0, 2.0, 3.0, 4.0]).is_none());
    }

    #[test]
    fn wrong_dimension_returns_none() {
        let mut matcher = FingerprintMatcher::new(4, 0.5);
        matcher
            .store_pattern(vec![1.0; 4], PostureClass::Standing)
            .unwrap();
        // Wrong dimension
        assert!(matcher.classify(&[1.0, 2.0]).is_none());
    }

    #[test]
    fn store_and_recall() {
        let mut matcher = FingerprintMatcher::new(4, 0.5);

        // Store distinct patterns
        matcher
            .store_pattern(vec![1.0, 0.0, 0.0, 0.0], PostureClass::Standing)
            .unwrap();
        matcher
            .store_pattern(vec![0.0, 1.0, 0.0, 0.0], PostureClass::Sitting)
            .unwrap();

        assert_eq!(matcher.num_patterns(), 2);

        // Query close to "Standing" pattern
        let result = matcher.classify(&[0.9, 0.1, 0.0, 0.0]);
        if let Some((posture, sim)) = result {
            assert_eq!(posture, PostureClass::Standing);
            assert!(sim > 0.5, "similarity should be above threshold: {sim}");
        }
    }

    #[test]
    fn wrong_dim_store_rejected() {
        let mut matcher = FingerprintMatcher::new(4, 0.5);
        let result = matcher.store_pattern(vec![1.0, 2.0], PostureClass::Empty);
        assert!(result.is_err());
    }

    #[test]
    fn clear_removes_all() {
        let mut matcher = FingerprintMatcher::new(2, 0.5);
        matcher
            .store_pattern(vec![1.0, 0.0], PostureClass::Standing)
            .unwrap();
        assert_eq!(matcher.num_patterns(), 1);
        matcher.clear();
        assert_eq!(matcher.num_patterns(), 0);
    }

    #[test]
    fn cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-5, "identical vectors: {sim}");
    }

    #[test]
    fn cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-5, "orthogonal vectors: {sim}");
    }

    #[test]
    fn match_posture_result() {
        let mut matcher = FingerprintMatcher::new(3, 0.5);
        matcher
            .store_pattern(vec![1.0, 0.0, 0.0], PostureClass::Standing)
            .unwrap();

        let result = matcher.match_posture(&[0.95, 0.05, 0.0]);
        assert!(result.matched);
        assert_eq!(result.posture, Some(PostureClass::Standing));
    }

    #[test]
    fn generate_defaults_creates_templates() {
        let mut matcher = FingerprintMatcher::new(4, 0.3);
        matcher.generate_defaults(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(matcher.num_patterns(), 3); // Empty, Standing, Sitting
    }
}
