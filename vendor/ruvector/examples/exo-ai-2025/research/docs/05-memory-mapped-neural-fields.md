# 05 - Memory-Mapped Neural Fields

## Overview

Petabyte-scale neural field storage using memory-mapped files with lazy activation, enabling neural networks that exceed RAM capacity while maintaining fast access patterns.

## Key Innovation

**Lazy Neural Activation**: Only load and compute neural activations when accessed, with intelligent prefetching based on access patterns.

```rust
pub struct MMapNeuralField {
    /// Memory-mapped file handle
    mmap: Mmap,
    /// Field dimensions
    shape: Vec<usize>,
    /// Activation cache (LRU)
    cache: LruCache<usize, Vec<f32>>,
    /// Prefetch predictor
    prefetcher: PrefetchPredictor,
}
```

## Architecture

```
┌─────────────────────────────────────────┐
│         Application Layer               │
│  ┌─────────────────────────────────┐   │
│  │  field.activate(x, y, z)        │   │
│  └─────────────────────────────────┘   │
├─────────────────────────────────────────┤
│         Cache Layer (LRU)               │
│  ┌─────────────────────────────────┐   │
│  │  Hot: Recently accessed regions │   │
│  │  Warm: Prefetched regions       │   │
│  │  Cold: On-disk only             │   │
│  └─────────────────────────────────┘   │
├─────────────────────────────────────────┤
│         Memory Map Layer                │
│  ┌─────────────────────────────────┐   │
│  │  Virtual Address Space          │   │
│  │  Backed by file on disk         │   │
│  │  OS manages paging              │   │
│  └─────────────────────────────────┘   │
├─────────────────────────────────────────┤
│         Storage Layer                   │
│  ┌─────────────────────────────────┐   │
│  │  NVMe SSD / Distributed FS      │   │
│  │  Chunked for parallel access    │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

## Lazy Activation

```rust
impl LazyActivation {
    /// Get activation, loading from disk if needed
    pub fn get(&mut self, index: usize) -> &[f32] {
        // Check cache first
        if let Some(cached) = self.cache.get(&index) {
            return cached;
        }

        // Load from memory map
        let offset = index * self.element_size;
        let slice = &self.mmap[offset..offset + self.element_size];

        // Parse and cache
        let activation: Vec<f32> = slice.chunks(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();

        self.cache.put(index, activation);

        // Trigger prefetch for likely next accesses
        self.prefetcher.predict_and_fetch(index);

        self.cache.get(&index).unwrap()
    }
}
```

## Tiered Memory Hierarchy

```rust
pub struct TieredMemory {
    /// L1: GPU HBM (fastest, smallest)
    l1_gpu: Vec<f32>,
    /// L2: CPU RAM
    l2_ram: Vec<f32>,
    /// L3: NVMe SSD (memory-mapped)
    l3_ssd: MMapNeuralField,
    /// L4: Network storage
    l4_network: Option<NetworkStorage>,
}

impl TieredMemory {
    pub fn get(&mut self, index: usize) -> &[f32] {
        // Check each tier
        if let Some(val) = self.l1_gpu.get(index) {
            return val;
        }
        if let Some(val) = self.l2_ram.get(index) {
            // Promote to L1
            self.promote_to_l1(index, val);
            return val;
        }
        // Load from L3, promote through tiers
        let val = self.l3_ssd.get(index);
        self.promote_to_l2(index, val);
        val
    }
}
```

## Prefetch Predictor

```rust
pub struct PrefetchPredictor {
    /// Access history for pattern detection
    history: VecDeque<usize>,
    /// Detected stride patterns
    strides: Vec<isize>,
    /// Prefetch queue
    queue: VecDeque<usize>,
}

impl PrefetchPredictor {
    pub fn predict_and_fetch(&mut self, current: usize) {
        self.history.push_back(current);

        // Detect stride pattern
        if self.history.len() >= 3 {
            let stride1 = self.history[self.history.len()-1] as isize
                        - self.history[self.history.len()-2] as isize;
            let stride2 = self.history[self.history.len()-2] as isize
                        - self.history[self.history.len()-3] as isize;

            if stride1 == stride2 {
                // Consistent stride detected
                let next = (current as isize + stride1) as usize;
                self.queue.push_back(next);
            }
        }

        // Issue prefetch for queued items
        for &idx in &self.queue {
            self.async_prefetch(idx);
        }
    }
}
```

## Performance

| Tier | Capacity | Latency | Bandwidth |
|------|----------|---------|-----------|
| L1 GPU | 80GB | 1μs | 2TB/s |
| L2 RAM | 1TB | 100ns | 200GB/s |
| L3 SSD | 100TB | 10μs | 7GB/s |
| L4 Net | 1PB | 1ms | 100Gb/s |

| Operation | Cold | Warm | Hot |
|-----------|------|------|-----|
| Single access | 10μs | 100ns | 1μs |
| Batch 1K | 50μs | 5μs | 50μs |
| Sequential scan | 7GB/s | 200GB/s | 2TB/s |

## Usage

```rust
use memory_mapped_neural_fields::{MMapNeuralField, TieredMemory};

// Create petabyte-scale field
let field = MMapNeuralField::create(
    "/data/neural_field.bin",
    &[1_000_000, 1_000_000, 256], // 1M x 1M x 256
)?;

// Access with lazy loading
let activation = field.activate(500_000, 500_000, 0);

// Use tiered memory for optimal performance
let mut tiered = TieredMemory::new(field);
for region in regions_of_interest {
    let activations = tiered.batch_get(&region);
    process(activations);
}
```

## Petabyte Example

```rust
// 1 petabyte neural field
let field = MMapNeuralField::create(
    "/mnt/distributed/brain.bin",
    &[
        86_000_000_000, // 86 billion neurons
        1_000,          // 1000 features per neuron
    ],
)?;

// Access specific neuron
let neuron_42b = field.get(42_000_000_000);
```

## References

- Memory-Mapped Files: POSIX mmap, Windows MapViewOfFile
- Prefetching: "Effective Prefetching for Disk I/O Requests" (USENIX)
- Tiered Storage: "Auto-tiering for High-Performance Storage Systems"
