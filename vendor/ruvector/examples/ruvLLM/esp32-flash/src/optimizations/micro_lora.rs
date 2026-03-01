//! MicroLoRA - Tiny Low-Rank Adaptation for ESP32

use heapless::Vec as HVec;
use crate::QuantParams;

pub const MAX_LORA_RANK: usize = 2;
pub const MAX_LORA_DIM: usize = 64;

#[derive(Debug, Clone, Copy)]
pub struct LoRAConfig {
    pub rank: usize,
    pub dim: usize,
    pub scale: i8,
    pub frozen: bool,
}

impl Default for LoRAConfig {
    fn default() -> Self {
        Self { rank: 1, dim: 32, scale: 8, frozen: true }
    }
}

pub struct MicroLoRA {
    a_weights: HVec<i8, { MAX_LORA_DIM * MAX_LORA_RANK }>,
    b_weights: HVec<i8, { MAX_LORA_RANK * MAX_LORA_DIM }>,
    config: LoRAConfig,
    intermediate: [i32; MAX_LORA_RANK],
}

impl MicroLoRA {
    pub fn new(config: LoRAConfig, seed: u32) -> crate::Result<Self> {
        if config.rank > MAX_LORA_RANK || config.dim > MAX_LORA_DIM {
            return Err(crate::Error::InvalidModel("LoRA dimensions too large"));
        }

        let mut a_weights = HVec::new();
        let mut b_weights = HVec::new();
        let mut rng = seed;

        for _ in 0..(config.dim * config.rank) {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            a_weights.push((((rng >> 16) & 0x3F) as i16 - 32) as i8)
                .map_err(|_| crate::Error::BufferOverflow)?;
        }

        for _ in 0..(config.rank * config.dim) {
            b_weights.push(0).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self { a_weights, b_weights, config, intermediate: [0; MAX_LORA_RANK] })
    }

    pub fn from_weights(config: LoRAConfig, a: &[i8], b: &[i8]) -> crate::Result<Self> {
        let mut a_vec = HVec::new();
        let mut b_vec = HVec::new();
        for &w in a { a_vec.push(w).map_err(|_| crate::Error::BufferOverflow)?; }
        for &w in b { b_vec.push(w).map_err(|_| crate::Error::BufferOverflow)?; }
        Ok(Self { a_weights: a_vec, b_weights: b_vec, config, intermediate: [0; MAX_LORA_RANK] })
    }

    #[inline]
    pub fn apply(&mut self, input: &[i8], output: &mut [i32]) {
        let (dim, rank, scale) = (self.config.dim, self.config.rank, self.config.scale as i32);

        for r in 0..rank {
            let mut sum: i32 = 0;
            for d in 0..dim {
                sum += input[d] as i32 * self.a_weights[d * rank + r] as i32;
            }
            self.intermediate[r] = sum >> 4;
        }

        for d in 0..dim {
            let mut sum: i32 = 0;
            for r in 0..rank {
                sum += self.intermediate[r] * self.b_weights[r * dim + d] as i32;
            }
            output[d] += (sum * scale) >> 8;
        }
    }

    pub fn memory_size(&self) -> usize { self.a_weights.len() + self.b_weights.len() }
}

pub struct LoRAStack<const NUM_LAYERS: usize> {
    adapters: [Option<MicroLoRA>; NUM_LAYERS],
    active_count: usize,
}

impl<const NUM_LAYERS: usize> LoRAStack<NUM_LAYERS> {
    pub fn new() -> Self {
        Self { adapters: core::array::from_fn(|_| None), active_count: 0 }
    }

    pub fn add_adapter(&mut self, layer: usize, adapter: MicroLoRA) -> crate::Result<()> {
        if layer >= NUM_LAYERS { return Err(crate::Error::InvalidModel("Layer out of range")); }
        self.adapters[layer] = Some(adapter);
        self.active_count += 1;
        Ok(())
    }

    pub fn get(&mut self, layer: usize) -> Option<&mut MicroLoRA> {
        self.adapters.get_mut(layer).and_then(|a| a.as_mut())
    }

    pub fn total_memory(&self) -> usize {
        self.adapters.iter().filter_map(|a| a.as_ref()).map(|a| a.memory_size()).sum()
    }
}

impl<const N: usize> Default for LoRAStack<N> {
    fn default() -> Self { Self::new() }
}
