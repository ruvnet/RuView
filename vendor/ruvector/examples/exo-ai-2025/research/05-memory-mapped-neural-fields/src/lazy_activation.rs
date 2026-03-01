// Lazy Activation Evaluation for Neural Networks
// Only loads weights from storage when actually needed for computation

use crate::mmap_neural_field::MmapNeuralField;
use std::sync::Arc;

/// Activation state for neural network layers
#[derive(Clone, Debug)]
pub enum ActivationState {
    /// On disk, not in memory
    Cold { addr: u64, size: usize },

    /// Memory-mapped, not yet accessed
    Warm { addr: u64, size: usize },

    /// In DRAM, actively used
    Hot { data: Vec<f32> },
}

impl ActivationState {
    pub fn memory_usage(&self) -> usize {
        match self {
            ActivationState::Cold { .. } => 0,
            ActivationState::Warm { .. } => 0,
            ActivationState::Hot { data } => data.len() * std::mem::size_of::<f32>(),
        }
    }

    pub fn is_hot(&self) -> bool {
        matches!(self, ActivationState::Hot { .. })
    }
}

/// Lazy neural network layer with on-demand weight loading
pub struct LazyLayer {
    /// Layer weights
    weights: ActivationState,

    /// Bias terms
    bias: ActivationState,

    /// Input dimension
    input_dim: usize,

    /// Output dimension
    output_dim: usize,

    /// Reference to neural field storage
    storage: Arc<MmapNeuralField>,

    /// Access counter for eviction policy
    access_count: usize,

    /// Last access timestamp (for LRU)
    last_access: std::time::Instant,
}

impl LazyLayer {
    /// Create new lazy layer
    pub fn new(
        weights_addr: u64,
        bias_addr: u64,
        input_dim: usize,
        output_dim: usize,
        storage: Arc<MmapNeuralField>,
    ) -> Self {
        let weights_size = input_dim * output_dim;
        let bias_size = output_dim;

        Self {
            weights: ActivationState::Cold {
                addr: weights_addr,
                size: weights_size,
            },
            bias: ActivationState::Cold {
                addr: bias_addr,
                size: bias_size,
            },
            input_dim,
            output_dim,
            storage,
            access_count: 0,
            last_access: std::time::Instant::now(),
        }
    }

    /// Ensure weights are hot (loaded into DRAM)
    fn ensure_weights_hot(&mut self) -> std::io::Result<()> {
        if !self.weights.is_hot() {
            let (addr, size) = match self.weights {
                ActivationState::Cold { addr, size } | ActivationState::Warm { addr, size } => {
                    (addr, size)
                }
                ActivationState::Hot { .. } => return Ok(()),
            };

            // Load from storage
            let data = self.storage.read(addr, size)?;

            // Transition to hot
            self.weights = ActivationState::Hot { data };
        }

        Ok(())
    }

    /// Ensure bias is hot
    fn ensure_bias_hot(&mut self) -> std::io::Result<()> {
        if !self.bias.is_hot() {
            let (addr, size) = match self.bias {
                ActivationState::Cold { addr, size } | ActivationState::Warm { addr, size } => {
                    (addr, size)
                }
                ActivationState::Hot { .. } => return Ok(()),
            };

            let data = self.storage.read(addr, size)?;
            self.bias = ActivationState::Hot { data };
        }

        Ok(())
    }

    /// Forward pass with lazy weight loading
    ///
    /// # Arguments
    /// * `input` - Input activations (length = input_dim)
    ///
    /// # Returns
    /// Output activations (length = output_dim)
    pub fn forward(&mut self, input: &[f32]) -> std::io::Result<Vec<f32>> {
        assert_eq!(input.len(), self.input_dim, "Input dimension mismatch");

        // Demand-page weights into memory
        self.ensure_weights_hot()?;
        self.ensure_bias_hot()?;

        // Extract hot data
        let weights = match &self.weights {
            ActivationState::Hot { data } => data,
            _ => unreachable!(),
        };

        let bias = match &self.bias {
            ActivationState::Hot { data } => data,
            _ => unreachable!(),
        };

        // Compute matrix-vector multiplication: output = weights * input + bias
        let mut output = vec![0.0f32; self.output_dim];

        for i in 0..self.output_dim {
            let row_start = i * self.input_dim;
            let row_end = row_start + self.input_dim;
            let weight_row = &weights[row_start..row_end];

            let sum: f32 = weight_row
                .iter()
                .zip(input.iter())
                .map(|(w, x)| w * x)
                .sum();

            output[i] = sum + bias[i];
        }

        // Update access tracking
        self.touch();

        Ok(output)
    }

    /// SIMD-accelerated forward pass (AVX2)
    ///
    /// Requires CPU with AVX2 support
    #[cfg(target_arch = "x86_64")]
    pub fn forward_simd(&mut self, input: &[f32]) -> std::io::Result<Vec<f32>> {
        use std::arch::x86_64::*;

        assert_eq!(input.len(), self.input_dim);

        self.ensure_weights_hot()?;
        self.ensure_bias_hot()?;

        let weights = match &self.weights {
            ActivationState::Hot { data } => data,
            _ => unreachable!(),
        };

        let bias = match &self.bias {
            ActivationState::Hot { data } => data,
            _ => unreachable!(),
        };

        let mut output = vec![0.0f32; self.output_dim];

        unsafe {
            for i in 0..self.output_dim {
                let row_start = i * self.input_dim;
                let row_end = row_start + self.input_dim;
                let weight_row = &weights[row_start..row_end];

                let mut sum = _mm256_setzero_ps();

                // Process 8 elements at a time
                let mut j = 0;
                while j + 8 <= self.input_dim {
                    let w = _mm256_loadu_ps(&weight_row[j]);
                    let x = _mm256_loadu_ps(&input[j]);
                    sum = _mm256_fmadd_ps(w, x, sum);
                    j += 8;
                }

                // Horizontal sum
                let sum_array: [f32; 8] = std::mem::transmute(sum);
                let mut total: f32 = sum_array.iter().sum();

                // Handle remaining elements
                for k in j..self.input_dim {
                    total += weight_row[k] * input[k];
                }

                output[i] = total + bias[i];
            }
        }

        self.touch();
        Ok(output)
    }

    /// Evict weights from DRAM (transition to cold)
    pub fn evict(&mut self) {
        let (weights_addr, weights_size) = match self.weights {
            ActivationState::Hot { .. } => {
                if let ActivationState::Cold { addr, size } | ActivationState::Warm { addr, size } =
                    self.weights
                {
                    (addr, size)
                } else {
                    // Extract addr/size from current state
                    return; // Skip if already cold
                }
            }
            _ => return,
        };

        let (bias_addr, bias_size) = match self.bias {
            ActivationState::Hot { .. } => {
                if let ActivationState::Cold { addr, size } | ActivationState::Warm { addr, size } =
                    self.bias
                {
                    (addr, size)
                } else {
                    return;
                }
            }
            _ => return,
        };

        // Note: In real implementation, we'd flush dirty data to storage here
        self.weights = ActivationState::Cold {
            addr: weights_addr,
            size: weights_size,
        };

        self.bias = ActivationState::Cold {
            addr: bias_addr,
            size: bias_size,
        };
    }

    /// Mark as recently used (for LRU eviction)
    fn touch(&mut self) {
        self.last_access = std::time::Instant::now();
        self.access_count += 1;
    }

    /// Get memory usage
    pub fn memory_usage(&self) -> usize {
        self.weights.memory_usage() + self.bias.memory_usage()
    }

    /// Get age (seconds since last access)
    pub fn age(&self) -> u64 {
        self.last_access.elapsed().as_secs()
    }

    /// Get access count
    pub fn access_count(&self) -> usize {
        self.access_count
    }
}

/// Multi-layer neural network with lazy evaluation
pub struct LazyNetwork {
    layers: Vec<LazyLayer>,
    storage: Arc<MmapNeuralField>,
    max_memory: usize,
}

impl LazyNetwork {
    /// Create new lazy network
    pub fn new(storage: Arc<MmapNeuralField>, max_memory: usize) -> Self {
        Self {
            layers: Vec::new(),
            storage,
            max_memory,
        }
    }

    /// Add layer to network
    pub fn add_layer(
        &mut self,
        weights_addr: u64,
        bias_addr: u64,
        input_dim: usize,
        output_dim: usize,
    ) {
        let layer = LazyLayer::new(
            weights_addr,
            bias_addr,
            input_dim,
            output_dim,
            self.storage.clone(),
        );
        self.layers.push(layer);
    }

    /// Forward pass through entire network
    pub fn forward(&mut self, mut input: Vec<f32>) -> std::io::Result<Vec<f32>> {
        // Check memory pressure before processing
        self.manage_memory();

        // Process each layer
        let num_layers = self.layers.len();
        for i in 0..num_layers {
            input = self.layers[i].forward(&input)?;

            // Optionally apply activation function (e.g., ReLU)
            input.iter_mut().for_each(|x| *x = x.max(0.0));

            // Check memory after each layer (every 3 layers to reduce overhead)
            if i % 3 == 0 {
                self.manage_memory();
            }
        }

        Ok(input)
    }

    /// SIMD-accelerated forward pass
    #[cfg(target_arch = "x86_64")]
    pub fn forward_simd(&mut self, mut input: Vec<f32>) -> std::io::Result<Vec<f32>> {
        self.manage_memory();

        // Process each layer
        let num_layers = self.layers.len();
        for i in 0..num_layers {
            input = self.layers[i].forward_simd(&input)?;

            // ReLU activation
            input.iter_mut().for_each(|x| *x = x.max(0.0));

            // Check memory periodically
            if i % 3 == 0 {
                self.manage_memory();
            }
        }

        Ok(input)
    }

    /// Manage memory by evicting cold layers
    fn manage_memory(&mut self) {
        let total_memory: usize = self.layers.iter().map(|l| l.memory_usage()).sum();

        if total_memory > self.max_memory {
            // Collect layer indices and ages
            let mut layer_ages: Vec<_> = self
                .layers
                .iter()
                .enumerate()
                .map(|(i, l)| (i, l.age()))
                .collect();

            // Sort by age (descending - oldest first)
            layer_ages.sort_by_key(|(_, age)| std::cmp::Reverse(*age));

            // Evict oldest layers until under memory limit
            for (idx, _) in layer_ages {
                let current_total: usize = self.layers.iter().map(|l| l.memory_usage()).sum();
                if current_total <= self.max_memory {
                    break;
                }
                self.layers[idx].evict();
            }
        }
    }

    /// Get total memory usage
    pub fn total_memory(&self) -> usize {
        self.layers.iter().map(|l| l.memory_usage()).sum()
    }

    /// Get statistics
    pub fn stats(&self) -> NetworkStats {
        let total_layers = self.layers.len();
        let hot_layers = self.layers.iter().filter(|l| l.weights.is_hot()).count();
        let total_memory = self.total_memory();

        NetworkStats {
            total_layers,
            hot_layers,
            total_memory,
            max_memory: self.max_memory,
        }
    }
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_layers: usize,
    pub hot_layers: usize,
    pub total_memory: usize,
    pub max_memory: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mmap_neural_field::MmapNeuralField;
    use tempfile::NamedTempFile;

    #[test]
    fn test_lazy_layer() {
        let temp = NamedTempFile::new().unwrap();
        let storage = Arc::new(MmapNeuralField::new(temp.path(), 1024 * 1024, Some(4096)).unwrap());

        // Write some test weights
        let weights = vec![1.0f32; 100]; // 10x10 matrix
        let bias = vec![0.5f32; 10];

        storage.write(0, &weights).unwrap();
        storage.write(400, &bias).unwrap();

        // Create lazy layer
        let mut layer = LazyLayer::new(0, 400, 10, 10, storage);

        // Initially cold
        assert!(!layer.weights.is_hot());

        // Forward pass should load weights
        let input = vec![1.0f32; 10];
        let output = layer.forward(&input).unwrap();

        // Now hot
        assert!(layer.weights.is_hot());
        assert_eq!(output.len(), 10);

        // Each output should be sum(weights) + bias = 10*1.0 + 0.5 = 10.5
        assert!((output[0] - 10.5).abs() < 1e-5);
    }

    #[test]
    fn test_lazy_network() {
        let temp = NamedTempFile::new().unwrap();
        let storage = Arc::new(MmapNeuralField::new(temp.path(), 1024 * 1024, Some(4096)).unwrap());

        // Create 3-layer network: 10 -> 20 -> 10 -> 5
        let mut network = LazyNetwork::new(storage.clone(), 10 * 1024); // 10 KB limit

        // Initialize weights (just use ones for testing)
        let w1 = vec![1.0f32; 10 * 20];
        let b1 = vec![0.1f32; 20];
        storage.write(0, &w1).unwrap();
        storage.write(800, &b1).unwrap();

        let w2 = vec![0.5f32; 20 * 10];
        let b2 = vec![0.2f32; 10];
        storage.write(880, &w2).unwrap();
        storage.write(1680, &b2).unwrap();

        let w3 = vec![0.25f32; 10 * 5];
        let b3 = vec![0.3f32; 5];
        storage.write(1720, &w3).unwrap();
        storage.write(1920, &b3).unwrap();

        network.add_layer(0, 800, 10, 20);
        network.add_layer(880, 1680, 20, 10);
        network.add_layer(1720, 1920, 10, 5);

        // Forward pass
        let input = vec![1.0f32; 10];
        let output = network.forward(input).unwrap();

        assert_eq!(output.len(), 5);
    }

    #[test]
    fn test_eviction() {
        let temp = NamedTempFile::new().unwrap();
        let storage = Arc::new(MmapNeuralField::new(temp.path(), 1024 * 1024, Some(4096)).unwrap());

        let weights = vec![1.0f32; 100];
        let bias = vec![0.5f32; 10];
        storage.write(0, &weights).unwrap();
        storage.write(400, &bias).unwrap();

        let mut layer = LazyLayer::new(0, 400, 10, 10, storage);

        // Load weights
        let input = vec![1.0f32; 10];
        let _ = layer.forward(&input).unwrap();

        assert!(layer.memory_usage() > 0);

        // Evict
        layer.evict();

        assert_eq!(layer.memory_usage(), 0);
        assert!(!layer.weights.is_hot());
    }
}
