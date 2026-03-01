//! MinCut-Inspired Layer Pruning

use heapless::Vec as HVec;

pub const MAX_PRUNING_UNITS: usize = 64;
pub const MAX_MASK_WORDS: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct PruningConfig {
    pub target_sparsity: f32,
    pub importance_threshold: i8,
    pub structured: bool,
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self { target_sparsity: 0.5, importance_threshold: 8, structured: true }
    }
}

#[derive(Debug, Clone)]
pub struct PruningMask<const N: usize> {
    pub mask: HVec<u32, MAX_MASK_WORDS>,
    pub size: usize,
    pub pruned_count: usize,
}

impl<const N: usize> PruningMask<N> {
    pub fn new(size: usize) -> crate::Result<Self> {
        let num_words = (size + 31) / 32;
        let mut mask = HVec::new();
        for i in 0..num_words {
            let bits = if i == num_words - 1 && size % 32 != 0 {
                (1u32 << (size % 32)) - 1
            } else {
                u32::MAX
            };
            mask.push(bits).map_err(|_| crate::Error::BufferOverflow)?;
        }
        Ok(Self { mask, size, pruned_count: 0 })
    }

    #[inline]
    pub fn is_kept(&self, idx: usize) -> bool {
        let word = idx / 32;
        let bit = idx % 32;
        (self.mask.get(word).copied().unwrap_or(0) >> bit) & 1 == 1
    }

    pub fn prune(&mut self, idx: usize) {
        if idx < self.size && self.is_kept(idx) {
            let word = idx / 32;
            let bit = idx % 32;
            if let Some(w) = self.mask.get_mut(word) {
                *w &= !(1 << bit);
                self.pruned_count += 1;
            }
        }
    }

    pub fn sparsity(&self) -> f32 { self.pruned_count as f32 / self.size as f32 }
}

pub struct LayerPruner {
    config: PruningConfig,
    importance_scores: HVec<i16, MAX_PRUNING_UNITS>,
}

impl LayerPruner {
    pub fn new(config: PruningConfig) -> Self {
        Self { config, importance_scores: HVec::new() }
    }

    pub fn compute_magnitude_importance(&mut self, weights: &[i8]) {
        self.importance_scores.clear();
        for &w in weights.iter().take(MAX_PRUNING_UNITS) {
            let _ = self.importance_scores.push((w as i16).abs());
        }
    }

    pub fn create_mask<const N: usize>(&self, size: usize) -> crate::Result<PruningMask<N>> {
        let mut mask = PruningMask::new(size)?;
        let threshold = self.compute_threshold(size);
        for (idx, &score) in self.importance_scores.iter().enumerate() {
            if score < threshold { mask.prune(idx); }
        }
        Ok(mask)
    }

    fn compute_threshold(&self, size: usize) -> i16 {
        let target = (size as f32 * self.config.target_sparsity) as usize;
        if target == 0 || self.importance_scores.is_empty() { return 0; }

        let mut sorted: HVec<i16, MAX_PRUNING_UNITS> = self.importance_scores.clone();
        for i in 0..sorted.len() {
            for j in 0..sorted.len() - 1 - i {
                if sorted[j] > sorted[j + 1] { sorted.swap(j, j + 1); }
            }
        }
        sorted.get(target.min(sorted.len() - 1)).copied().unwrap_or(0)
    }

    pub fn apply_mask<const N: usize>(&self, weights: &mut [i8], mask: &PruningMask<N>) {
        for (idx, weight) in weights.iter_mut().enumerate() {
            if !mask.is_kept(idx) { *weight = 0; }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PruningStats {
    pub total_weights: usize,
    pub pruned_weights: usize,
    pub sparsity: f32,
    pub memory_saved: usize,
}

pub struct MinCutScorer {
    input_flow: HVec<i32, MAX_PRUNING_UNITS>,
    output_flow: HVec<i32, MAX_PRUNING_UNITS>,
}

impl MinCutScorer {
    pub fn new() -> Self {
        Self { input_flow: HVec::new(), output_flow: HVec::new() }
    }

    pub fn compute_edge_importance(&mut self, weights: &[i8], input_dim: usize, output_dim: usize)
        -> HVec<i16, MAX_PRUNING_UNITS>
    {
        self.input_flow.clear();
        self.output_flow.clear();

        for in_idx in 0..input_dim.min(MAX_PRUNING_UNITS) {
            let flow: i32 = (0..output_dim).map(|out_idx| {
                let w_idx = out_idx * input_dim + in_idx;
                if w_idx < weights.len() { (weights[w_idx] as i32).abs() } else { 0 }
            }).sum();
            let _ = self.input_flow.push(flow);
        }

        for out_idx in 0..output_dim.min(MAX_PRUNING_UNITS) {
            let flow: i32 = (0..input_dim).map(|in_idx| {
                let w_idx = out_idx * input_dim + in_idx;
                if w_idx < weights.len() { (weights[w_idx] as i32).abs() } else { 0 }
            }).sum();
            let _ = self.output_flow.push(flow);
        }

        let mut importance: HVec<i16, MAX_PRUNING_UNITS> = HVec::new();
        for out_idx in 0..output_dim.min(self.output_flow.len()) {
            for in_idx in 0..input_dim.min(self.input_flow.len()) {
                let w_idx = out_idx * input_dim + in_idx;
                if w_idx < weights.len() && importance.len() < MAX_PRUNING_UNITS {
                    let w = (weights[w_idx] as i32).abs();
                    let bottleneck = self.input_flow[in_idx].min(self.output_flow[out_idx]);
                    let _ = importance.push(((w * bottleneck) >> 10) as i16);
                }
            }
        }
        importance
    }
}

impl Default for MinCutScorer {
    fn default() -> Self { Self::new() }
}
