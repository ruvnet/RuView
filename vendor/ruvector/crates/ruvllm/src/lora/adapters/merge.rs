//! Adapter Merging and Composition
//!
//! This module provides utilities for:
//! - Merging multiple adapters with weights
//! - Hot-swapping adapters at runtime
//! - Adapter composition (combining adapters for multi-task scenarios)
//! - Interpolation between adapters

use crate::error::{Result, RuvLLMError};
use crate::lora::adapters::LoraConfig;
use crate::lora::micro_lora::{LoraAdapter, MicroLoRA, MicroLoraConfig, TargetModule};
use ndarray::Array2;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Strategy for merging adapters
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Average all adapter weights equally
    Average,
    /// Weighted sum of adapter weights
    WeightedSum,
    /// SLERP (Spherical Linear Interpolation) between two adapters
    Slerp,
    /// TIES merging (Trim, Elect, Merge)
    Ties,
    /// DARE (Drop And REscale) merging
    Dare,
    /// Task arithmetic (add/subtract task vectors)
    TaskArithmetic,
}

/// Configuration for adapter merging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConfig {
    /// Merge strategy
    pub strategy: MergeStrategy,
    /// Adapter weights (for weighted strategies)
    pub weights: HashMap<String, f32>,
    /// Interpolation factor (for SLERP, 0.0 = first, 1.0 = second)
    pub interpolation: f32,
    /// Density parameter (for TIES/DARE)
    pub density: f32,
    /// Normalize weights after merge
    pub normalize: bool,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            strategy: MergeStrategy::WeightedSum,
            weights: HashMap::new(),
            interpolation: 0.5,
            density: 0.5,
            normalize: true,
        }
    }
}

impl MergeConfig {
    /// Create config for averaging
    pub fn average() -> Self {
        Self {
            strategy: MergeStrategy::Average,
            ..Default::default()
        }
    }

    /// Create config for weighted sum
    pub fn weighted(weights: HashMap<String, f32>) -> Self {
        Self {
            strategy: MergeStrategy::WeightedSum,
            weights,
            ..Default::default()
        }
    }

    /// Create config for SLERP interpolation
    pub fn slerp(factor: f32) -> Self {
        Self {
            strategy: MergeStrategy::Slerp,
            interpolation: factor,
            ..Default::default()
        }
    }

    /// Create config for TIES merging
    pub fn ties(density: f32) -> Self {
        Self {
            strategy: MergeStrategy::Ties,
            density,
            ..Default::default()
        }
    }
}

/// Adapter merger
pub struct AdapterMerger {
    config: MergeConfig,
}

impl AdapterMerger {
    /// Create a new merger
    pub fn new(config: MergeConfig) -> Self {
        Self { config }
    }

    /// Merge multiple adapters into a single adapter
    pub fn merge(
        &self,
        adapters: &[(String, MicroLoRA)],
        output_config: &LoraConfig,
        hidden_dim: usize,
    ) -> Result<MicroLoRA> {
        if adapters.is_empty() {
            return Err(RuvLLMError::Config("No adapters to merge".to_string()));
        }

        match self.config.strategy {
            MergeStrategy::Average => self.merge_average(adapters, output_config, hidden_dim),
            MergeStrategy::WeightedSum => self.merge_weighted(adapters, output_config, hidden_dim),
            MergeStrategy::Slerp => self.merge_slerp(adapters, output_config, hidden_dim),
            MergeStrategy::Ties => self.merge_ties(adapters, output_config, hidden_dim),
            MergeStrategy::Dare => self.merge_dare(adapters, output_config, hidden_dim),
            MergeStrategy::TaskArithmetic => {
                self.merge_task_arithmetic(adapters, output_config, hidden_dim)
            }
        }
    }

    /// Average merging
    fn merge_average(
        &self,
        adapters: &[(String, MicroLoRA)],
        output_config: &LoraConfig,
        hidden_dim: usize,
    ) -> Result<MicroLoRA> {
        let micro_config = output_config.to_micro_lora_config(hidden_dim)?;
        let merged = MicroLoRA::new(micro_config);

        let n = adapters.len() as f32;

        for module in &output_config.target_modules {
            let merged_adapter = merged
                .get_adapter(module)
                .ok_or_else(|| RuvLLMError::NotFound(format!("Module {:?} not found", module)))?;
            let mut merged_adapter = merged_adapter.write();

            // Average all adapter weights
            for (_name, lora) in adapters {
                if let Some(adapter) = lora.get_adapter(module) {
                    let adapter = adapter.read();

                    // Add to merged weights
                    for i in 0..merged_adapter.lora_a.nrows() {
                        for j in 0..merged_adapter.lora_a.ncols() {
                            merged_adapter.lora_a[[i, j]] += adapter.lora_a[[i, j]] / n;
                        }
                    }

                    for i in 0..merged_adapter.lora_b.nrows() {
                        for j in 0..merged_adapter.lora_b.ncols() {
                            merged_adapter.lora_b[[i, j]] += adapter.lora_b[[i, j]] / n;
                        }
                    }
                }
            }
        }

        Ok(merged)
    }

    /// Weighted sum merging
    fn merge_weighted(
        &self,
        adapters: &[(String, MicroLoRA)],
        output_config: &LoraConfig,
        hidden_dim: usize,
    ) -> Result<MicroLoRA> {
        let micro_config = output_config.to_micro_lora_config(hidden_dim)?;
        let merged = MicroLoRA::new(micro_config);

        // Normalize weights
        let total_weight: f32 = adapters
            .iter()
            .map(|(name, _)| self.config.weights.get(name).copied().unwrap_or(1.0))
            .sum();

        for module in &output_config.target_modules {
            let merged_adapter = merged
                .get_adapter(module)
                .ok_or_else(|| RuvLLMError::NotFound(format!("Module {:?} not found", module)))?;
            let mut merged_adapter = merged_adapter.write();

            // Weighted sum
            for (name, lora) in adapters {
                let weight = self.config.weights.get(name).copied().unwrap_or(1.0);
                let normalized_weight = if self.config.normalize {
                    weight / total_weight
                } else {
                    weight
                };

                if let Some(adapter) = lora.get_adapter(module) {
                    let adapter = adapter.read();

                    for i in 0..merged_adapter.lora_a.nrows() {
                        for j in 0..merged_adapter.lora_a.ncols() {
                            merged_adapter.lora_a[[i, j]] +=
                                adapter.lora_a[[i, j]] * normalized_weight;
                        }
                    }

                    for i in 0..merged_adapter.lora_b.nrows() {
                        for j in 0..merged_adapter.lora_b.ncols() {
                            merged_adapter.lora_b[[i, j]] +=
                                adapter.lora_b[[i, j]] * normalized_weight;
                        }
                    }
                }
            }
        }

        Ok(merged)
    }

    /// SLERP (Spherical Linear Interpolation) between two adapters
    fn merge_slerp(
        &self,
        adapters: &[(String, MicroLoRA)],
        output_config: &LoraConfig,
        hidden_dim: usize,
    ) -> Result<MicroLoRA> {
        if adapters.len() != 2 {
            return Err(RuvLLMError::Config(
                "SLERP requires exactly 2 adapters".to_string(),
            ));
        }

        let micro_config = output_config.to_micro_lora_config(hidden_dim)?;
        let merged = MicroLoRA::new(micro_config);

        let t = self.config.interpolation;
        let (_, lora_a) = &adapters[0];
        let (_, lora_b) = &adapters[1];

        for module in &output_config.target_modules {
            let merged_adapter = merged
                .get_adapter(module)
                .ok_or_else(|| RuvLLMError::NotFound(format!("Module {:?} not found", module)))?;
            let mut merged_adapter = merged_adapter.write();

            let adapter_a = lora_a.get_adapter(module).ok_or_else(|| {
                RuvLLMError::NotFound(format!("Module {:?} not found in first adapter", module))
            })?;
            let adapter_b = lora_b.get_adapter(module).ok_or_else(|| {
                RuvLLMError::NotFound(format!("Module {:?} not found in second adapter", module))
            })?;

            let adapter_a = adapter_a.read();
            let adapter_b = adapter_b.read();

            // SLERP for A matrix
            self.slerp_matrix(
                &adapter_a.lora_a,
                &adapter_b.lora_a,
                t,
                &mut merged_adapter.lora_a,
            );

            // SLERP for B matrix
            self.slerp_matrix(
                &adapter_a.lora_b,
                &adapter_b.lora_b,
                t,
                &mut merged_adapter.lora_b,
            );
        }

        Ok(merged)
    }

    /// Perform SLERP on a matrix
    fn slerp_matrix(&self, a: &Array2<f32>, b: &Array2<f32>, t: f32, output: &mut Array2<f32>) {
        // Simple linear interpolation (full SLERP requires quaternion math)
        for i in 0..a.nrows() {
            for j in 0..a.ncols() {
                output[[i, j]] = a[[i, j]] * (1.0 - t) + b[[i, j]] * t;
            }
        }
    }

    /// TIES merging (Trim, Elect, Merge)
    fn merge_ties(
        &self,
        adapters: &[(String, MicroLoRA)],
        output_config: &LoraConfig,
        hidden_dim: usize,
    ) -> Result<MicroLoRA> {
        let micro_config = output_config.to_micro_lora_config(hidden_dim)?;
        let merged = MicroLoRA::new(micro_config);

        for module in &output_config.target_modules {
            let merged_adapter = merged
                .get_adapter(module)
                .ok_or_else(|| RuvLLMError::NotFound(format!("Module {:?} not found", module)))?;
            let mut merged_adapter = merged_adapter.write();

            // Collect all values for each position
            let mut values_a: Vec<Vec<f32>> =
                vec![vec![]; merged_adapter.lora_a.nrows() * merged_adapter.lora_a.ncols()];
            let mut values_b: Vec<Vec<f32>> =
                vec![vec![]; merged_adapter.lora_b.nrows() * merged_adapter.lora_b.ncols()];

            for (_name, lora) in adapters {
                if let Some(adapter) = lora.get_adapter(module) {
                    let adapter = adapter.read();

                    for i in 0..adapter.lora_a.nrows() {
                        for j in 0..adapter.lora_a.ncols() {
                            let idx = i * adapter.lora_a.ncols() + j;
                            values_a[idx].push(adapter.lora_a[[i, j]]);
                        }
                    }

                    for i in 0..adapter.lora_b.nrows() {
                        for j in 0..adapter.lora_b.ncols() {
                            let idx = i * adapter.lora_b.ncols() + j;
                            values_b[idx].push(adapter.lora_b[[i, j]]);
                        }
                    }
                }
            }

            // Trim, Elect, Merge for A
            for i in 0..merged_adapter.lora_a.nrows() {
                for j in 0..merged_adapter.lora_a.ncols() {
                    let idx = i * merged_adapter.lora_a.ncols() + j;
                    merged_adapter.lora_a[[i, j]] = self.ties_aggregate(&values_a[idx]);
                }
            }

            // Trim, Elect, Merge for B
            for i in 0..merged_adapter.lora_b.nrows() {
                for j in 0..merged_adapter.lora_b.ncols() {
                    let idx = i * merged_adapter.lora_b.ncols() + j;
                    merged_adapter.lora_b[[i, j]] = self.ties_aggregate(&values_b[idx]);
                }
            }
        }

        Ok(merged)
    }

    /// TIES aggregation: trim small values, elect by sign, merge by mean
    fn ties_aggregate(&self, values: &[f32]) -> f32 {
        if values.is_empty() {
            return 0.0;
        }

        // Calculate threshold for trimming
        let abs_values: Vec<f32> = values.iter().map(|v| v.abs()).collect();
        let max_abs = abs_values.iter().copied().fold(0.0f32, f32::max);
        let threshold = max_abs * (1.0 - self.config.density);

        // Trim
        let trimmed: Vec<f32> = values
            .iter()
            .copied()
            .filter(|v| v.abs() >= threshold)
            .collect();

        if trimmed.is_empty() {
            return 0.0;
        }

        // Elect by sign (majority voting)
        let pos_count = trimmed.iter().filter(|&&v| v > 0.0).count();
        let neg_count = trimmed.len() - pos_count;

        let elected: Vec<f32> = if pos_count > neg_count {
            trimmed.iter().copied().filter(|&v| v > 0.0).collect()
        } else if neg_count > pos_count {
            trimmed.iter().copied().filter(|&v| v < 0.0).collect()
        } else {
            trimmed
        };

        // Merge by mean
        elected.iter().sum::<f32>() / elected.len() as f32
    }

    /// DARE merging (Drop And REscale)
    fn merge_dare(
        &self,
        adapters: &[(String, MicroLoRA)],
        output_config: &LoraConfig,
        hidden_dim: usize,
    ) -> Result<MicroLoRA> {
        use rand::rngs::StdRng;
        use rand::{Rng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(42);

        let micro_config = output_config.to_micro_lora_config(hidden_dim)?;
        let merged = MicroLoRA::new(micro_config);

        for module in &output_config.target_modules {
            let merged_adapter = merged
                .get_adapter(module)
                .ok_or_else(|| RuvLLMError::NotFound(format!("Module {:?} not found", module)))?;
            let mut merged_adapter = merged_adapter.write();

            let rescale = 1.0 / (1.0 - self.config.density);

            for (_name, lora) in adapters {
                if let Some(adapter) = lora.get_adapter(module) {
                    let adapter = adapter.read();

                    // Drop and rescale A
                    for i in 0..merged_adapter.lora_a.nrows() {
                        for j in 0..merged_adapter.lora_a.ncols() {
                            if rng.gen::<f32>() > self.config.density {
                                merged_adapter.lora_a[[i, j]] += adapter.lora_a[[i, j]] * rescale;
                            }
                        }
                    }

                    // Drop and rescale B
                    for i in 0..merged_adapter.lora_b.nrows() {
                        for j in 0..merged_adapter.lora_b.ncols() {
                            if rng.gen::<f32>() > self.config.density {
                                merged_adapter.lora_b[[i, j]] += adapter.lora_b[[i, j]] * rescale;
                            }
                        }
                    }
                }
            }

            // Average
            let n = adapters.len() as f32;
            merged_adapter.lora_a.mapv_inplace(|v| v / n);
            merged_adapter.lora_b.mapv_inplace(|v| v / n);
        }

        Ok(merged)
    }

    /// Task arithmetic merging
    fn merge_task_arithmetic(
        &self,
        adapters: &[(String, MicroLoRA)],
        output_config: &LoraConfig,
        hidden_dim: usize,
    ) -> Result<MicroLoRA> {
        // Similar to weighted sum but allows negative weights
        self.merge_weighted(adapters, output_config, hidden_dim)
    }
}

/// Hot-swap manager for runtime adapter switching
pub struct HotSwapManager {
    /// Currently active adapter
    active: Option<MicroLoRA>,
    /// Standby adapter being prepared
    standby: Option<MicroLoRA>,
    /// Swap in progress flag
    swapping: bool,
}

impl HotSwapManager {
    /// Create a new hot-swap manager
    pub fn new() -> Self {
        Self {
            active: None,
            standby: None,
            swapping: false,
        }
    }

    /// Set the active adapter
    pub fn set_active(&mut self, adapter: MicroLoRA) {
        self.active = Some(adapter);
    }

    /// Prepare a new adapter in standby
    pub fn prepare_standby(&mut self, adapter: MicroLoRA) {
        self.standby = Some(adapter);
    }

    /// Swap standby to active (atomic operation)
    pub fn swap(&mut self) -> Result<()> {
        if self.swapping {
            return Err(RuvLLMError::Config("Swap already in progress".to_string()));
        }

        if self.standby.is_none() {
            return Err(RuvLLMError::Config(
                "No standby adapter prepared".to_string(),
            ));
        }

        self.swapping = true;

        // Atomic swap
        std::mem::swap(&mut self.active, &mut self.standby);
        self.standby = None;

        self.swapping = false;
        Ok(())
    }

    /// Get reference to active adapter
    pub fn active(&self) -> Option<&MicroLoRA> {
        self.active.as_ref()
    }

    /// Get mutable reference to active adapter
    pub fn active_mut(&mut self) -> Option<&mut MicroLoRA> {
        self.active.as_mut()
    }

    /// Check if swap is in progress
    pub fn is_swapping(&self) -> bool {
        self.swapping
    }
}

impl Default for HotSwapManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lora::adapters::RuvLtraAdapters;

    #[test]
    fn test_merge_average() {
        let adapters_cfg = RuvLtraAdapters::new();
        let lora1 = adapters_cfg.create_lora("coder", 64).unwrap();
        let lora2 = adapters_cfg.create_lora("researcher", 64).unwrap();

        let adapters = vec![
            ("coder".to_string(), lora1),
            ("researcher".to_string(), lora2),
        ];

        let config = MergeConfig::average();
        let merger = AdapterMerger::new(config);

        let merged = merger.merge(&adapters, &adapters_cfg.coder, 64).unwrap();

        assert_eq!(merged.config().rank, 16);
    }

    #[test]
    fn test_merge_weighted() {
        let adapters_cfg = RuvLtraAdapters::new();
        let lora1 = adapters_cfg.create_lora("coder", 64).unwrap();
        let lora2 = adapters_cfg.create_lora("security", 64).unwrap();

        let adapters = vec![
            ("coder".to_string(), lora1),
            ("security".to_string(), lora2),
        ];

        let mut weights = HashMap::new();
        weights.insert("coder".to_string(), 0.7);
        weights.insert("security".to_string(), 0.3);

        let config = MergeConfig::weighted(weights);
        let merger = AdapterMerger::new(config);

        let merged = merger.merge(&adapters, &adapters_cfg.coder, 64).unwrap();

        assert!(merged.is_enabled());
    }

    #[test]
    fn test_merge_slerp() {
        let adapters_cfg = RuvLtraAdapters::new();
        let lora1 = adapters_cfg.create_lora("coder", 64).unwrap();
        let lora2 = adapters_cfg.create_lora("reviewer", 64).unwrap();

        let adapters = vec![
            ("coder".to_string(), lora1),
            ("reviewer".to_string(), lora2),
        ];

        let config = MergeConfig::slerp(0.5);
        let merger = AdapterMerger::new(config);

        let merged = merger.merge(&adapters, &adapters_cfg.coder, 64).unwrap();

        assert!(merged.is_enabled());
    }

    #[test]
    fn test_hot_swap() {
        let adapters_cfg = RuvLtraAdapters::new();
        let lora1 = adapters_cfg.create_lora("coder", 64).unwrap();
        let lora2 = adapters_cfg.create_lora("security", 64).unwrap();

        let mut manager = HotSwapManager::new();

        manager.set_active(lora1);
        assert!(manager.active().is_some());

        manager.prepare_standby(lora2);
        manager.swap().unwrap();

        assert!(manager.active().is_some());
        assert!(manager.standby.is_none());
    }

    #[test]
    fn test_ties_aggregate() {
        let config = MergeConfig::ties(0.5);
        let merger = AdapterMerger::new(config);

        let values = vec![0.1, 0.2, -0.3, 0.4, -0.1];
        let result = merger.ties_aggregate(&values);

        // Should trim small values and elect by majority sign
        assert!(result.abs() > 0.0);
    }
}
