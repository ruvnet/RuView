//! Core Modern Hopfield Network implementation

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors that can occur in Hopfield operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum HopfieldError {
    /// Pattern dimension mismatch
    #[error("Pattern dimension {0} does not match network dimension {1}")]
    DimensionMismatch(usize, usize),

    /// Empty query
    #[error("Query vector cannot be empty")]
    EmptyQuery,

    /// Invalid beta parameter
    #[error("Beta parameter must be positive, got {0}")]
    InvalidBeta(f32),

    /// No patterns stored
    #[error("No patterns stored in network")]
    NoPatterns,
}

/// Modern Hopfield Network
///
/// Implements the 2020 Ramsauer et al. formulation with exponential
/// storage capacity and transformer-style attention mechanism.
///
/// # Examples
///
/// ```rust
/// use ruvector_nervous_system::hopfield::ModernHopfield;
///
/// let mut hopfield = ModernHopfield::new(128, 1.0);
/// let pattern = vec![1.0; 128];
/// hopfield.store(pattern.clone());
/// let retrieved = hopfield.retrieve(&pattern).unwrap();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModernHopfield {
    /// Stored patterns (N patterns × d dimensions)
    patterns: Vec<Vec<f32>>,

    /// Inverse temperature parameter (higher = sharper attention)
    beta: f32,

    /// Dimensionality of patterns
    dimension: usize,
}

impl ModernHopfield {
    /// Create a new Modern Hopfield network
    ///
    /// # Arguments
    ///
    /// * `dimension` - Dimensionality of patterns to store
    /// * `beta` - Inverse temperature parameter (typically 0.5-10.0)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_nervous_system::hopfield::ModernHopfield;
    ///
    /// let hopfield = ModernHopfield::new(128, 1.0);
    /// assert_eq!(hopfield.dimension(), 128);
    /// ```
    pub fn new(dimension: usize, beta: f32) -> Self {
        assert!(dimension > 0, "Dimension must be positive");
        assert!(beta > 0.0, "Beta must be positive");

        Self {
            patterns: Vec::new(),
            beta,
            dimension,
        }
    }

    /// Store a new pattern in the network
    ///
    /// # Arguments
    ///
    /// * `pattern` - Vector to store (must match network dimension)
    ///
    /// # Errors
    ///
    /// Returns `HopfieldError::DimensionMismatch` if pattern dimension
    /// doesn't match network dimension.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_nervous_system::hopfield::ModernHopfield;
    ///
    /// let mut hopfield = ModernHopfield::new(128, 1.0);
    /// let pattern = vec![1.0; 128];
    /// hopfield.store(pattern).unwrap();
    /// assert_eq!(hopfield.num_patterns(), 1);
    /// ```
    pub fn store(&mut self, pattern: Vec<f32>) -> Result<(), HopfieldError> {
        if pattern.len() != self.dimension {
            return Err(HopfieldError::DimensionMismatch(
                pattern.len(),
                self.dimension,
            ));
        }

        self.patterns.push(pattern);
        Ok(())
    }

    /// Retrieve a pattern using a query vector
    ///
    /// Uses softmax-weighted attention mechanism:
    /// 1. Compute similarities: s_i = pattern_i · query
    /// 2. Compute attention: α = softmax(β * s)
    /// 3. Return weighted sum: Σ α_i * pattern_i
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector (must match network dimension)
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Query dimension doesn't match network dimension
    /// - Query is empty
    /// - No patterns stored
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_nervous_system::hopfield::ModernHopfield;
    ///
    /// let mut hopfield = ModernHopfield::new(128, 1.0);
    /// let pattern = vec![1.0; 128];
    /// hopfield.store(pattern.clone()).unwrap();
    ///
    /// let retrieved = hopfield.retrieve(&pattern).unwrap();
    /// assert_eq!(retrieved.len(), 128);
    /// ```
    pub fn retrieve(&self, query: &[f32]) -> Result<Vec<f32>, HopfieldError> {
        if query.is_empty() {
            return Err(HopfieldError::EmptyQuery);
        }

        if query.len() != self.dimension {
            return Err(HopfieldError::DimensionMismatch(
                query.len(),
                self.dimension,
            ));
        }

        if self.patterns.is_empty() {
            return Err(HopfieldError::NoPatterns);
        }

        let (attention, _) = super::retrieval::compute_attention(&self.patterns, query, self.beta);

        // Weighted sum: output = Σ attention_i * pattern_i
        let mut output = vec![0.0; self.dimension];
        for (i, pattern) in self.patterns.iter().enumerate() {
            for (j, &value) in pattern.iter().enumerate() {
                output[j] += attention[i] * value;
            }
        }

        Ok(output)
    }

    /// Retrieve top-k patterns by attention weight
    ///
    /// Returns the k patterns with highest attention scores along with
    /// their indices, patterns, and attention weights.
    ///
    /// # Arguments
    ///
    /// * `query` - Query vector
    /// * `k` - Number of top patterns to return
    ///
    /// # Returns
    ///
    /// Vector of (index, pattern, attention_weight) tuples, sorted by
    /// attention weight in descending order.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_nervous_system::hopfield::ModernHopfield;
    ///
    /// let mut hopfield = ModernHopfield::new(128, 1.0);
    /// hopfield.store(vec![1.0; 128]).unwrap();
    /// hopfield.store(vec![0.5; 128]).unwrap();
    ///
    /// let query = vec![1.0; 128];
    /// let top_k = hopfield.retrieve_k(&query, 2).unwrap();
    /// assert_eq!(top_k.len(), 2);
    /// ```
    pub fn retrieve_k(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(usize, Vec<f32>, f32)>, HopfieldError> {
        if query.is_empty() {
            return Err(HopfieldError::EmptyQuery);
        }

        if query.len() != self.dimension {
            return Err(HopfieldError::DimensionMismatch(
                query.len(),
                self.dimension,
            ));
        }

        if self.patterns.is_empty() {
            return Err(HopfieldError::NoPatterns);
        }

        let (attention, _) = super::retrieval::compute_attention(&self.patterns, query, self.beta);

        // Create (index, attention) pairs and sort (NaN-safe)
        let mut indexed: Vec<_> = attention.iter().copied().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Less));

        // Take top k
        let k = k.min(indexed.len());
        let results: Vec<_> = indexed
            .into_iter()
            .take(k)
            .map(|(idx, attn)| (idx, self.patterns[idx].clone(), attn))
            .collect();

        Ok(results)
    }

    /// Get the theoretical storage capacity
    ///
    /// Returns 2^(d/2) where d is the dimension.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ruvector_nervous_system::hopfield::ModernHopfield;
    ///
    /// let hopfield = ModernHopfield::new(32, 1.0);
    /// assert_eq!(hopfield.capacity(), 2_u64.pow(16)); // 2^(32/2) = 65536
    /// ```
    pub fn capacity(&self) -> u64 {
        super::capacity::theoretical_capacity(self.dimension)
    }

    /// Get network dimension
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// Get number of stored patterns
    pub fn num_patterns(&self) -> usize {
        self.patterns.len()
    }

    /// Get beta parameter
    pub fn beta(&self) -> f32 {
        self.beta
    }

    /// Set beta parameter
    ///
    /// # Errors
    ///
    /// Returns error if beta is not positive.
    pub fn set_beta(&mut self, beta: f32) -> Result<(), HopfieldError> {
        if beta <= 0.0 {
            return Err(HopfieldError::InvalidBeta(beta));
        }
        self.beta = beta;
        Ok(())
    }

    /// Clear all stored patterns
    pub fn clear(&mut self) {
        self.patterns.clear();
    }

    /// Get reference to all stored patterns
    pub fn patterns(&self) -> &[Vec<f32>] {
        &self.patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let hopfield = ModernHopfield::new(128, 1.0);
        assert_eq!(hopfield.dimension(), 128);
        assert_eq!(hopfield.beta(), 1.0);
        assert_eq!(hopfield.num_patterns(), 0);
    }

    #[test]
    #[should_panic(expected = "Dimension must be positive")]
    fn test_new_zero_dimension() {
        ModernHopfield::new(0, 1.0);
    }

    #[test]
    #[should_panic(expected = "Beta must be positive")]
    fn test_new_zero_beta() {
        ModernHopfield::new(128, 0.0);
    }

    #[test]
    fn test_store() {
        let mut hopfield = ModernHopfield::new(128, 1.0);
        let pattern = vec![1.0; 128];

        assert!(hopfield.store(pattern).is_ok());
        assert_eq!(hopfield.num_patterns(), 1);
    }

    #[test]
    fn test_store_dimension_mismatch() {
        let mut hopfield = ModernHopfield::new(128, 1.0);
        let pattern = vec![1.0; 64];

        let result = hopfield.store(pattern);
        assert!(matches!(
            result,
            Err(HopfieldError::DimensionMismatch(64, 128))
        ));
    }

    #[test]
    fn test_retrieve_empty_query() {
        let hopfield = ModernHopfield::new(128, 1.0);
        let result = hopfield.retrieve(&[]);
        assert!(matches!(result, Err(HopfieldError::EmptyQuery)));
    }

    #[test]
    fn test_retrieve_no_patterns() {
        let hopfield = ModernHopfield::new(128, 1.0);
        let query = vec![1.0; 128];
        let result = hopfield.retrieve(&query);
        assert!(matches!(result, Err(HopfieldError::NoPatterns)));
    }

    #[test]
    fn test_set_beta() {
        let mut hopfield = ModernHopfield::new(128, 1.0);
        assert!(hopfield.set_beta(2.0).is_ok());
        assert_eq!(hopfield.beta(), 2.0);

        let result = hopfield.set_beta(-1.0);
        assert!(matches!(result, Err(HopfieldError::InvalidBeta(_))));
    }

    #[test]
    fn test_clear() {
        let mut hopfield = ModernHopfield::new(128, 1.0);
        hopfield.store(vec![1.0; 128]).unwrap();
        assert_eq!(hopfield.num_patterns(), 1);

        hopfield.clear();
        assert_eq!(hopfield.num_patterns(), 0);
    }
}
