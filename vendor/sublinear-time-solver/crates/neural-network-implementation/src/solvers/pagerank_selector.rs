//! PageRank-based active sample selection for training
//!
//! This module implements the PageRank-based active learning strategy
//! that selects the most valuable training samples for the neural network.

use crate::{
    config::ActiveSelectionConfig,
    error::{Result, TemporalNeuralError},
    solvers::InferenceReadyTrait,
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// PageRank-based active sample selector
///
/// Uses k-NN graphs and PageRank scoring to identify the most valuable
/// training samples, focusing on regions where the model is uncertain
/// or making large errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRankSelector {
    /// Configuration
    config: ActiveSelectionConfig,
    /// k-NN graph adjacency matrix
    graph: Option<DMatrix<f64>>,
    /// Sample embeddings (features from last layer)
    embeddings: Vec<DVector<f64>>,
    /// Sample errors for scoring
    sample_errors: Vec<f64>,
    /// Sample importance scores
    importance_scores: Vec<f64>,
    /// Selected sample indices
    selected_indices: HashSet<usize>,
    /// PageRank scores
    pagerank_scores: Vec<f64>,
    /// Statistics
    stats: SelectorStatistics,
    /// Ready for inference flag
    inference_ready: bool,
}

/// Statistics tracked by the selector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorStatistics {
    /// Total number of samples processed
    pub total_samples: usize,
    /// Number of selections made
    pub selections_made: usize,
    /// Average error of selected samples
    pub avg_selected_error: f64,
    /// Average error of non-selected samples
    pub avg_nonselected_error: f64,
    /// Graph construction time in milliseconds
    pub graph_construction_time_ms: f64,
    /// PageRank computation time in milliseconds
    pub pagerank_computation_time_ms: f64,
    /// Selection time in milliseconds
    pub selection_time_ms: f64,
}

impl PageRankSelector {
    /// Create a new PageRank selector
    pub fn new(config: &ActiveSelectionConfig) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            graph: None,
            embeddings: Vec::new(),
            sample_errors: Vec::new(),
            importance_scores: Vec::new(),
            selected_indices: HashSet::new(),
            pagerank_scores: Vec::new(),
            stats: SelectorStatistics {
                total_samples: 0,
                selections_made: 0,
                avg_selected_error: 0.0,
                avg_nonselected_error: 0.0,
                graph_construction_time_ms: 0.0,
                pagerank_computation_time_ms: 0.0,
                selection_time_ms: 0.0,
            },
            inference_ready: false,
        })
    }

    /// Add samples with their embeddings and errors
    pub fn add_samples(
        &mut self,
        embeddings: &[DVector<f64>],
        errors: &[f64],
    ) -> Result<()> {
        if embeddings.len() != errors.len() {
            return Err(TemporalNeuralError::DataError {
                message: "Embeddings and errors length mismatch".to_string(),
                context: Some(format!("embeddings: {}, errors: {}", embeddings.len(), errors.len())),
            });
        }

        // Add to internal storage
        self.embeddings.extend_from_slice(embeddings);
        self.sample_errors.extend_from_slice(errors);
        self.stats.total_samples = self.embeddings.len();

        // Invalidate graph since we have new samples
        self.graph = None;
        self.pagerank_scores.clear();
        self.importance_scores.clear();

        Ok(())
    }

    /// Build k-NN graph from current embeddings
    pub fn build_graph(&mut self) -> Result<()> {
        if self.embeddings.is_empty() {
            return Err(TemporalNeuralError::DataError {
                message: "No embeddings available for graph construction".to_string(),
                context: None,
            });
        }

        let start_time = std::time::Instant::now();
        let n = self.embeddings.len();
        let k = self.config.k as usize;

        // Initialize adjacency matrix
        let mut adjacency = DMatrix::zeros(n, n);

        // Build k-NN graph
        for i in 0..n {
            // Compute distances to all other samples
            let mut distances: Vec<(usize, f64)> = (0..n)
                .filter(|&j| j != i)
                .map(|j| {
                    let dist = self.compute_distance(&self.embeddings[i], &self.embeddings[j]);
                    (j, dist)
                })
                .collect();

            // Sort by distance and take k nearest neighbors
            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            let neighbors: Vec<usize> = distances
                .into_iter()
                .take(k)
                .map(|(idx, _)| idx)
                .collect();

            // Add edges to adjacency matrix
            for &neighbor in &neighbors {
                // Use Gaussian similarity as edge weight
                let dist = self.compute_distance(&self.embeddings[i], &self.embeddings[neighbor]);
                let weight = (-dist * dist / (2.0 * 0.1)).exp(); // σ = 0.1
                adjacency[(i, neighbor)] = weight;
            }
        }

        // Make graph symmetric
        for i in 0..n {
            for j in 0..n {
                let avg_weight = (adjacency[(i, j)] + adjacency[(j, i)]) / 2.0;
                adjacency[(i, j)] = avg_weight;
                adjacency[(j, i)] = avg_weight;
            }
        }

        self.graph = Some(adjacency);
        self.stats.graph_construction_time_ms = start_time.elapsed().as_millis() as f64;

        Ok(())
    }

    /// Compute PageRank scores with error-based personalization
    pub fn compute_pagerank(&mut self) -> Result<()> {
        if self.graph.is_none() {
            self.build_graph()?;
        }

        let graph = self.graph.as_ref().unwrap();
        let n = graph.nrows();

        if n == 0 {
            return Ok(());
        }

        let start_time = std::time::Instant::now();

        // Create personalization vector based on recent errors
        let personalization = self.create_error_personalization_vector()?;

        // Compute PageRank using power iteration
        let pagerank_scores = self.power_iteration_pagerank(graph, &personalization)?;

        self.pagerank_scores = pagerank_scores;
        self.stats.pagerank_computation_time_ms = start_time.elapsed().as_millis() as f64;

        Ok(())
    }

    /// Select active samples based on PageRank scores
    pub fn select_samples(&mut self) -> Result<Vec<usize>> {
        if self.pagerank_scores.is_empty() {
            self.compute_pagerank()?;
        }

        let start_time = std::time::Instant::now();
        let n_samples = self.config.samples_per_epoch as usize;
        let total_samples = self.embeddings.len();

        if n_samples >= total_samples {
            // Select all samples if we need more than available
            let selected: Vec<usize> = (0..total_samples).collect();
            self.selected_indices = selected.iter().cloned().collect();
            return Ok(selected);
        }

        // Combine PageRank scores with diversity to avoid clustering
        let mut combined_scores = Vec::new();
        for i in 0..total_samples {
            let pagerank_score = self.pagerank_scores.get(i).copied().unwrap_or(0.0);
            let error_score = self.sample_errors.get(i).copied().unwrap_or(0.0);
            let diversity_score = self.compute_diversity_score(i)?;

            let combined_score =
                self.config.error_weight * error_score +
                (1.0 - self.config.error_weight) * pagerank_score +
                self.config.diversity_weight * diversity_score;

            combined_scores.push((i, combined_score));
        }

        // Sort by combined score (descending)
        combined_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Select top samples with diversity constraints
        let selected = self.select_with_diversity_constraint(&combined_scores, n_samples)?;

        self.selected_indices = selected.iter().cloned().collect();
        self.stats.selections_made += 1;
        self.stats.selection_time_ms = start_time.elapsed().as_millis() as f64;

        // Update statistics
        self.update_selection_statistics(&selected)?;

        Ok(selected)
    }

    /// Create error-based personalization vector for PageRank
    fn create_error_personalization_vector(&self) -> Result<DVector<f64>> {
        let n = self.sample_errors.len();
        if n == 0 {
            return Err(TemporalNeuralError::DataError {
                message: "No sample errors available".to_string(),
                context: None,
            });
        }

        // Create personalization vector: higher error = higher probability
        let mut personalization = DVector::zeros(n);
        let max_error = self.sample_errors.iter().fold(0.0f64, |a, &b| a.max(b));

        if max_error > 0.0 {
            for (i, &error) in self.sample_errors.iter().enumerate() {
                personalization[i] = error / max_error;
            }
        } else {
            // Uniform if no errors
            personalization.fill(1.0 / n as f64);
        }

        // Normalize
        let sum = personalization.sum();
        if sum > 0.0 {
            personalization /= sum;
        } else {
            personalization.fill(1.0 / n as f64);
        }

        Ok(personalization)
    }

    /// Compute PageRank using power iteration
    fn power_iteration_pagerank(
        &self,
        graph: &DMatrix<f64>,
        personalization: &DVector<f64>,
    ) -> Result<Vec<f64>> {
        let n = graph.nrows();
        let damping = 0.85;
        let tolerance = self.config.pagerank_eps;
        let max_iterations = 100;

        // Normalize graph to transition matrix
        let mut transition = graph.clone();
        for i in 0..n {
            let row_sum: f64 = transition.row(i).sum();
            if row_sum > 1e-12 {
                for j in 0..n {
                    transition[(i, j)] /= row_sum;
                }
            } else {
                // Uniform transition for isolated nodes
                for j in 0..n {
                    transition[(i, j)] = 1.0 / n as f64;
                }
            }
        }

        // Initialize PageRank vector
        let mut pagerank = DVector::from_element(n, 1.0 / n as f64);

        // Power iteration
        for _ in 0..max_iterations {
            let old_pagerank = pagerank.clone();

            // PageRank update: PR = (1-d)/N + d * T^T * PR + (1-d) * personalization
            pagerank = &transition.transpose() * &old_pagerank * damping +
                      personalization * (1.0 - damping);

            // Check convergence
            let diff = (&pagerank - &old_pagerank).norm();
            if diff < tolerance {
                break;
            }
        }

        Ok(pagerank.data.as_vec().clone())
    }

    /// Compute diversity score for a sample
    fn compute_diversity_score(&self, sample_idx: usize) -> Result<f64> {
        if self.selected_indices.is_empty() {
            return Ok(1.0); // Maximum diversity if no samples selected yet
        }

        let sample_embedding = &self.embeddings[sample_idx];
        let mut min_distance = f64::INFINITY;

        // Find minimum distance to already selected samples
        for &selected_idx in &self.selected_indices {
            let distance = self.compute_distance(sample_embedding, &self.embeddings[selected_idx]);
            min_distance = min_distance.min(distance);
        }

        // Diversity score is minimum distance (higher = more diverse)
        Ok(min_distance)
    }

    /// Select samples with diversity constraint
    fn select_with_diversity_constraint(
        &self,
        scored_samples: &[(usize, f64)],
        n_samples: usize,
    ) -> Result<Vec<usize>> {
        let mut selected = Vec::new();
        let mut selected_set = HashSet::new();

        for &(idx, _score) in scored_samples {
            if selected.len() >= n_samples {
                break;
            }

            // Check diversity constraint
            if self.meets_diversity_constraint(idx, &selected_set)? {
                selected.push(idx);
                selected_set.insert(idx);
            }
        }

        // Fill remaining slots if we don't have enough diverse samples
        for &(idx, _score) in scored_samples {
            if selected.len() >= n_samples {
                break;
            }
            if !selected_set.contains(&idx) {
                selected.push(idx);
                selected_set.insert(idx);
            }
        }

        Ok(selected)
    }

    /// Check if sample meets diversity constraint
    fn meets_diversity_constraint(
        &self,
        sample_idx: usize,
        selected_indices: &HashSet<usize>,
    ) -> Result<bool> {
        if selected_indices.is_empty() {
            return Ok(true);
        }

        let min_diversity_distance = 0.1; // Minimum distance threshold
        let sample_embedding = &self.embeddings[sample_idx];

        for &selected_idx in selected_indices {
            let distance = self.compute_distance(sample_embedding, &self.embeddings[selected_idx]);
            if distance < min_diversity_distance {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Compute distance between two embeddings
    fn compute_distance(&self, a: &DVector<f64>, b: &DVector<f64>) -> f64 {
        (a - b).norm()
    }

    /// Update selection statistics
    fn update_selection_statistics(&mut self, selected: &[usize]) -> Result<()> {
        if selected.is_empty() {
            return Ok(());
        }

        // Compute average error of selected samples
        let selected_errors: Vec<f64> = selected
            .iter()
            .map(|&idx| self.sample_errors.get(idx).copied().unwrap_or(0.0))
            .collect();

        self.stats.avg_selected_error = selected_errors.iter().sum::<f64>() / selected_errors.len() as f64;

        // Compute average error of non-selected samples
        let non_selected_errors: Vec<f64> = (0..self.sample_errors.len())
            .filter(|idx| !selected.contains(idx))
            .map(|idx| self.sample_errors[idx])
            .collect();

        if !non_selected_errors.is_empty() {
            self.stats.avg_nonselected_error = non_selected_errors.iter().sum::<f64>() / non_selected_errors.len() as f64;
        }

        Ok(())
    }

    /// Get selection statistics
    pub fn get_statistics(&self) -> &SelectorStatistics {
        &self.stats
    }

    /// Clear all stored data
    pub fn clear_data(&mut self) {
        self.embeddings.clear();
        self.sample_errors.clear();
        self.importance_scores.clear();
        self.selected_indices.clear();
        self.pagerank_scores.clear();
        self.graph = None;
        self.stats.total_samples = 0;
    }

    /// Get memory usage estimate
    pub fn estimate_memory_usage(&self) -> usize {
        let embeddings_size = self.embeddings.len() *
            self.embeddings.get(0).map_or(0, |e| e.len()) *
            std::mem::size_of::<f64>();

        let graph_size = self.graph.as_ref().map_or(0, |g| g.len() * std::mem::size_of::<f64>());

        let other_vecs_size = (self.sample_errors.len() +
                              self.importance_scores.len() +
                              self.pagerank_scores.len()) * std::mem::size_of::<f64>();

        std::mem::size_of::<Self>() + embeddings_size + graph_size + other_vecs_size
    }
}

impl InferenceReadyTrait for PageRankSelector {
    fn prepare_for_inference(&mut self) -> Result<()> {
        // Clear training-specific data to save memory
        self.clear_data();
        self.inference_ready = true;
        Ok(())
    }

    fn is_inference_ready(&self) -> bool {
        self.inference_ready
    }

    fn memory_usage(&self) -> usize {
        self.estimate_memory_usage()
    }

    fn reset(&mut self) -> Result<()> {
        self.clear_data();
        self.stats = SelectorStatistics {
            total_samples: 0,
            selections_made: 0,
            avg_selected_error: 0.0,
            avg_nonselected_error: 0.0,
            graph_construction_time_ms: 0.0,
            pagerank_computation_time_ms: 0.0,
            selection_time_ms: 0.0,
        };
        self.inference_ready = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> ActiveSelectionConfig {
        ActiveSelectionConfig {
            k: 5,
            pagerank_eps: 0.01,
            samples_per_epoch: 10,
            error_weight: 0.7,
            diversity_weight: 0.3,
        }
    }

    fn create_test_embeddings() -> Vec<DVector<f64>> {
        (0..20)
            .map(|i| {
                DVector::from_vec(vec![
                    i as f64 / 10.0,
                    (i as f64 / 10.0).sin(),
                    (i as f64 / 10.0).cos(),
                ])
            })
            .collect()
    }

    fn create_test_errors() -> Vec<f64> {
        (0..20).map(|i| (i as f64 / 20.0) + 0.1).collect()
    }

    #[test]
    fn test_selector_creation() {
        let config = create_test_config();
        let selector = PageRankSelector::new(&config).unwrap();

        assert_eq!(selector.config.k, 5);
        assert_eq!(selector.config.samples_per_epoch, 10);
        assert!(selector.embeddings.is_empty());
    }

    #[test]
    fn test_add_samples() {
        let config = create_test_config();
        let mut selector = PageRankSelector::new(&config).unwrap();

        let embeddings = create_test_embeddings();
        let errors = create_test_errors();

        selector.add_samples(&embeddings, &errors).unwrap();

        assert_eq!(selector.stats.total_samples, 20);
        assert_eq!(selector.embeddings.len(), 20);
        assert_eq!(selector.sample_errors.len(), 20);
    }

    #[test]
    fn test_graph_construction() {
        let config = create_test_config();
        let mut selector = PageRankSelector::new(&config).unwrap();

        let embeddings = create_test_embeddings();
        let errors = create_test_errors();
        selector.add_samples(&embeddings, &errors).unwrap();

        selector.build_graph().unwrap();

        assert!(selector.graph.is_some());
        let graph = selector.graph.as_ref().unwrap();
        assert_eq!(graph.shape(), (20, 20));
        assert!(selector.stats.graph_construction_time_ms > 0.0);
    }

    #[test]
    fn test_pagerank_computation() {
        let config = create_test_config();
        let mut selector = PageRankSelector::new(&config).unwrap();

        let embeddings = create_test_embeddings();
        let errors = create_test_errors();
        selector.add_samples(&embeddings, &errors).unwrap();

        selector.compute_pagerank().unwrap();

        assert_eq!(selector.pagerank_scores.len(), 20);
        assert!(selector.stats.pagerank_computation_time_ms > 0.0);

        // Check that scores sum approximately to 1
        let sum: f64 = selector.pagerank_scores.iter().sum();
        assert!((sum - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_sample_selection() {
        let config = create_test_config();
        let mut selector = PageRankSelector::new(&config).unwrap();

        let embeddings = create_test_embeddings();
        let errors = create_test_errors();
        selector.add_samples(&embeddings, &errors).unwrap();

        let selected = selector.select_samples().unwrap();

        assert_eq!(selected.len(), 10); // Should select requested number
        assert!(selector.stats.selection_time_ms > 0.0);
        assert!(selector.stats.avg_selected_error >= 0.0);
    }

    #[test]
    fn test_diversity_constraint() {
        let config = create_test_config();
        let mut selector = PageRankSelector::new(&config).unwrap();

        // Create embeddings with some very similar ones
        let mut embeddings = vec![DVector::from_vec(vec![0.0, 0.0, 0.0])];
        embeddings.push(DVector::from_vec(vec![0.001, 0.001, 0.001])); // Very similar
        embeddings.push(DVector::from_vec(vec![1.0, 1.0, 1.0])); // Different

        let errors = vec![1.0, 1.0, 0.1]; // First two have high error
        selector.add_samples(&embeddings, &errors).unwrap();

        let selected = selector.select_samples().unwrap();

        // Should not select both very similar samples
        assert!(selected.len() <= 2);
    }

    #[test]
    fn test_inference_preparation() {
        let config = create_test_config();
        let mut selector = PageRankSelector::new(&config).unwrap();

        let embeddings = create_test_embeddings();
        let errors = create_test_errors();
        selector.add_samples(&embeddings, &errors).unwrap();

        let memory_before = selector.memory_usage();

        selector.prepare_for_inference().unwrap();

        assert!(selector.is_inference_ready());
        assert!(selector.embeddings.is_empty()); // Should clear training data

        let memory_after = selector.memory_usage();
        assert!(memory_after < memory_before); // Should use less memory
    }
}