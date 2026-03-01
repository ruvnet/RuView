//! # Learned Index Structures
//!
//! Experimental learned indexes using neural networks to approximate data distribution.
//! Based on Recursive Model Index (RMI) concept with bounded error correction.

use crate::error::{Result, RuvectorError};
use crate::types::VectorId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trait for learned index structures
pub trait LearnedIndex {
    /// Predict position for a key
    fn predict(&self, key: &[f32]) -> Result<usize>;

    /// Insert a key-value pair
    fn insert(&mut self, key: Vec<f32>, value: VectorId) -> Result<()>;

    /// Search for a key
    fn search(&self, key: &[f32]) -> Result<Option<VectorId>>;

    /// Get index statistics
    fn stats(&self) -> IndexStats;
}

/// Statistics for learned indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStats {
    pub total_entries: usize,
    pub model_size_bytes: usize,
    pub avg_error: f32,
    pub max_error: usize,
}

/// Simple linear model for CDF approximation
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LinearModel {
    weights: Vec<f32>,
    bias: f32,
}

impl LinearModel {
    fn new(dimensions: usize) -> Self {
        Self {
            weights: vec![0.0; dimensions],
            bias: 0.0,
        }
    }

    fn predict(&self, input: &[f32]) -> f32 {
        let mut result = self.bias;
        for (w, x) in self.weights.iter().zip(input.iter()) {
            result += w * x;
        }
        result.max(0.0)
    }

    fn train_simple(&mut self, data: &[(Vec<f32>, usize)]) {
        if data.is_empty() {
            return;
        }

        // Simple least squares approximation
        let n = data.len() as f32;
        let dim = self.weights.len();

        // Reset weights
        self.weights.fill(0.0);
        self.bias = 0.0;

        // Compute means
        let mut mean_x = vec![0.0; dim];
        let mut mean_y = 0.0;

        for (x, y) in data {
            for (i, &val) in x.iter().enumerate() {
                mean_x[i] += val;
            }
            mean_y += *y as f32;
        }

        for val in mean_x.iter_mut() {
            *val /= n;
        }
        mean_y /= n;

        // Simple linear regression for first dimension
        if dim > 0 {
            let mut numerator = 0.0;
            let mut denominator = 0.0;

            for (x, y) in data {
                let x_diff = x[0] - mean_x[0];
                let y_diff = *y as f32 - mean_y;
                numerator += x_diff * y_diff;
                denominator += x_diff * x_diff;
            }

            if denominator.abs() > 1e-10 {
                self.weights[0] = numerator / denominator;
            }
            self.bias = mean_y - self.weights[0] * mean_x[0];
        }
    }
}

/// Recursive Model Index (RMI)
/// Multi-stage neural models making coarse-then-fine predictions
pub struct RecursiveModelIndex {
    /// Root model for coarse prediction
    root_model: LinearModel,
    /// Second-level models for fine prediction
    leaf_models: Vec<LinearModel>,
    /// Sorted data with error correction
    data: Vec<(Vec<f32>, VectorId)>,
    /// Error bounds for binary search fallback
    max_error: usize,
    /// Dimensions of vectors
    dimensions: usize,
}

impl RecursiveModelIndex {
    /// Create a new RMI with specified number of leaf models
    pub fn new(dimensions: usize, num_leaf_models: usize) -> Self {
        let leaf_models = (0..num_leaf_models)
            .map(|_| LinearModel::new(dimensions))
            .collect();

        Self {
            root_model: LinearModel::new(dimensions),
            leaf_models,
            data: Vec::new(),
            max_error: 100,
            dimensions,
        }
    }

    /// Build the index from data
    pub fn build(&mut self, mut data: Vec<(Vec<f32>, VectorId)>) -> Result<()> {
        if data.is_empty() {
            return Err(RuvectorError::InvalidInput(
                "Cannot build index from empty data".into(),
            ));
        }

        if data[0].0.is_empty() {
            return Err(RuvectorError::InvalidInput(
                "Cannot build index from vectors with zero dimensions".into(),
            ));
        }

        if self.leaf_models.is_empty() {
            return Err(RuvectorError::InvalidInput(
                "Cannot build index with zero leaf models".into(),
            ));
        }

        // Sort data by first dimension (simple heuristic)
        data.sort_by(|a, b| {
            a.0[0]
                .partial_cmp(&b.0[0])
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let n = data.len();

        // Train root model to predict leaf model index
        let root_training_data: Vec<(Vec<f32>, usize)> = data
            .iter()
            .enumerate()
            .map(|(i, (key, _))| {
                let leaf_idx = (i * self.leaf_models.len()) / n;
                (key.clone(), leaf_idx)
            })
            .collect();

        self.root_model.train_simple(&root_training_data);

        // Train each leaf model
        let num_leaf_models = self.leaf_models.len();
        let chunk_size = n / num_leaf_models;
        for (i, model) in self.leaf_models.iter_mut().enumerate() {
            let start = i * chunk_size;
            let end = if i == num_leaf_models - 1 {
                n
            } else {
                (i + 1) * chunk_size
            };

            if start < n {
                let leaf_data: Vec<(Vec<f32>, usize)> = data[start..end.min(n)]
                    .iter()
                    .enumerate()
                    .map(|(j, (key, _))| (key.clone(), start + j))
                    .collect();

                model.train_simple(&leaf_data);
            }
        }

        self.data = data;
        Ok(())
    }
}

impl LearnedIndex for RecursiveModelIndex {
    fn predict(&self, key: &[f32]) -> Result<usize> {
        if key.len() != self.dimensions {
            return Err(RuvectorError::InvalidInput(
                "Key dimensions mismatch".into(),
            ));
        }

        if self.leaf_models.is_empty() {
            return Err(RuvectorError::InvalidInput(
                "Index not built: no leaf models available".into(),
            ));
        }

        if self.data.is_empty() {
            return Err(RuvectorError::InvalidInput(
                "Index not built: no data available".into(),
            ));
        }

        // Root model predicts leaf model
        let leaf_idx = self.root_model.predict(key) as usize;
        let leaf_idx = leaf_idx.min(self.leaf_models.len() - 1);

        // Leaf model predicts position
        let pos = self.leaf_models[leaf_idx].predict(key) as usize;
        let pos = pos.min(self.data.len().saturating_sub(1));

        Ok(pos)
    }

    fn insert(&mut self, key: Vec<f32>, value: VectorId) -> Result<()> {
        // For simplicity, append and mark for rebuild
        // Production implementation would use incremental updates
        self.data.push((key, value));
        Ok(())
    }

    fn search(&self, key: &[f32]) -> Result<Option<VectorId>> {
        if self.data.is_empty() {
            return Ok(None);
        }

        let predicted_pos = self.predict(key)?;

        // Binary search around predicted position with error bound
        let start = predicted_pos.saturating_sub(self.max_error);
        let end = (predicted_pos + self.max_error).min(self.data.len());

        for i in start..end {
            if self.data[i].0 == key {
                return Ok(Some(self.data[i].1.clone()));
            }
        }

        Ok(None)
    }

    fn stats(&self) -> IndexStats {
        let model_size = std::mem::size_of_val(&self.root_model)
            + self.leaf_models.len() * std::mem::size_of::<LinearModel>();

        // Compute average prediction error
        let mut total_error = 0.0;
        let mut max_error = 0;

        for (i, (key, _)) in self.data.iter().enumerate() {
            if let Ok(pred_pos) = self.predict(key) {
                let error = i.abs_diff(pred_pos);
                total_error += error as f32;
                max_error = max_error.max(error);
            }
        }

        let avg_error = if !self.data.is_empty() {
            total_error / self.data.len() as f32
        } else {
            0.0
        };

        IndexStats {
            total_entries: self.data.len(),
            model_size_bytes: model_size,
            avg_error,
            max_error,
        }
    }
}

/// Hybrid index combining learned index for static data with HNSW for dynamic updates
pub struct HybridIndex {
    /// Learned index for static segment
    learned: RecursiveModelIndex,
    /// Dynamic updates buffer
    dynamic_buffer: HashMap<Vec<u8>, VectorId>,
    /// Threshold for rebuilding learned index
    rebuild_threshold: usize,
}

impl HybridIndex {
    /// Create a new hybrid index
    pub fn new(dimensions: usize, num_leaf_models: usize, rebuild_threshold: usize) -> Self {
        Self {
            learned: RecursiveModelIndex::new(dimensions, num_leaf_models),
            dynamic_buffer: HashMap::new(),
            rebuild_threshold,
        }
    }

    /// Build the learned portion from static data
    pub fn build_static(&mut self, data: Vec<(Vec<f32>, VectorId)>) -> Result<()> {
        self.learned.build(data)
    }

    /// Check if rebuild is needed
    pub fn needs_rebuild(&self) -> bool {
        self.dynamic_buffer.len() >= self.rebuild_threshold
    }

    /// Rebuild learned index incorporating dynamic updates
    pub fn rebuild(&mut self) -> Result<()> {
        let mut all_data: Vec<(Vec<f32>, VectorId)> = self.learned.data.clone();

        for (key_bytes, value) in &self.dynamic_buffer {
            let (key, _): (Vec<f32>, usize) =
                bincode::decode_from_slice(key_bytes, bincode::config::standard())
                    .map_err(|e| RuvectorError::SerializationError(e.to_string()))?;
            all_data.push((key, value.clone()));
        }

        self.learned.build(all_data)?;
        self.dynamic_buffer.clear();
        Ok(())
    }

    fn serialize_key(key: &[f32]) -> Vec<u8> {
        bincode::encode_to_vec(key, bincode::config::standard()).unwrap_or_default()
    }
}

impl LearnedIndex for HybridIndex {
    fn predict(&self, key: &[f32]) -> Result<usize> {
        self.learned.predict(key)
    }

    fn insert(&mut self, key: Vec<f32>, value: VectorId) -> Result<()> {
        let key_bytes = Self::serialize_key(&key);
        self.dynamic_buffer.insert(key_bytes, value);
        Ok(())
    }

    fn search(&self, key: &[f32]) -> Result<Option<VectorId>> {
        // Check dynamic buffer first
        let key_bytes = Self::serialize_key(key);
        if let Some(value) = self.dynamic_buffer.get(&key_bytes) {
            return Ok(Some(value.clone()));
        }

        // Fall back to learned index
        self.learned.search(key)
    }

    fn stats(&self) -> IndexStats {
        let mut stats = self.learned.stats();
        stats.total_entries += self.dynamic_buffer.len();
        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_model() {
        let mut model = LinearModel::new(2);
        let data = vec![
            (vec![0.0, 0.0], 0),
            (vec![1.0, 1.0], 10),
            (vec![2.0, 2.0], 20),
        ];

        model.train_simple(&data);

        let pred = model.predict(&[1.5, 1.5]);
        assert!(pred >= 0.0 && pred <= 30.0);
    }

    #[test]
    fn test_rmi_build() {
        let mut rmi = RecursiveModelIndex::new(2, 4);

        let data: Vec<(Vec<f32>, VectorId)> = (0..100)
            .map(|i| {
                let x = i as f32 / 100.0;
                (vec![x, x * x], i.to_string())
            })
            .collect();

        rmi.build(data).unwrap();

        let stats = rmi.stats();
        assert_eq!(stats.total_entries, 100);
        assert!(stats.avg_error < 50.0); // Should have reasonable error
    }

    #[test]
    fn test_rmi_search() {
        let mut rmi = RecursiveModelIndex::new(1, 2);

        let data = vec![
            (vec![0.0], "0".to_string()),
            (vec![0.5], "1".to_string()),
            (vec![1.0], "2".to_string()),
        ];

        rmi.build(data).unwrap();

        let result = rmi.search(&[0.5]).unwrap();
        assert_eq!(result, Some("1".to_string()));
    }

    #[test]
    fn test_hybrid_index() {
        let mut hybrid = HybridIndex::new(1, 2, 10);

        let static_data = vec![(vec![0.0], "0".to_string()), (vec![1.0], "1".to_string())];
        hybrid.build_static(static_data).unwrap();

        // Add dynamic updates
        hybrid.insert(vec![2.0], "2".to_string()).unwrap();

        assert_eq!(hybrid.search(&[2.0]).unwrap(), Some("2".to_string()));
        assert_eq!(hybrid.search(&[0.0]).unwrap(), Some("0".to_string()));
    }
}
