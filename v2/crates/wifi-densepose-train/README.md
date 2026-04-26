# wifi-densepose-train

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-train.svg)](https://crates.io/crates/wifi-densepose-train)
[![Documentation](https://docs.rs/wifi-densepose-train/badge.svg)](https://docs.rs/wifi-densepose-train)
[![License](https://img.shields.io/crates/l/wifi-densepose-train.svg)](LICENSE)

Complete training pipeline for WiFi-DensePose, integrated with all five ruvector crates.

## Overview

`wifi-densepose-train` provides everything needed to train the WiFi-to-DensePose model: dataset
loading, subcarrier interpolation, loss functions, evaluation metrics, and the training loop
orchestrator. It supports both the MM-Fi dataset (NeurIPS 2023) and deterministic synthetic data
for reproducible experiments.

Without the `tch-backend` feature the crate still provides the dataset, configuration, and
subcarrier interpolation APIs needed for data preprocessing and proof verification.

## Features

- **MM-Fi dataset loader** -- Reads the MM-Fi multimodal dataset (NeurIPS 2023) from disk with
  memory-mapped `.npy` files.
- **Synthetic dataset** -- Deterministic, fixed-seed CSI generation for unit tests and proofs.
- **Subcarrier interpolation** -- 114 -> 56 subcarrier compression via `ruvector-solver` sparse
  interpolation with variance-based selection.
- **Loss functions** (`tch-backend`) -- Pose estimation losses including MSE, OKS, and combined
  multi-task loss.
- **Metrics** (`tch-backend`) -- PCKh, OKS-AP, and per-keypoint evaluation with
  `ruvector-mincut`-based person matching.
- **Training orchestrator** (`tch-backend`) -- Full training loop with learning rate scheduling,
  gradient clipping, checkpointing, and reproducible proofs.
- **All 5 ruvector crates** -- `ruvector-mincut`, `ruvector-attn-mincut`,
  `ruvector-temporal-tensor`, `ruvector-solver`, and `ruvector-attention` integrated across
  dataset loading, metrics, and model attention.

### Feature flags

| Flag          | Default | Description                            |
|---------------|---------|----------------------------------------|
| `tch-backend` | no      | Enable PyTorch training via `tch-rs`   |
| `cuda`        | no      | CUDA GPU acceleration (implies `tch`)  |

### Binaries

| Binary             | Description                              |
|--------------------|------------------------------------------|
| `train`            | Main training entry point                |
| `verify-training`  | Proof verification (requires `tch-backend`) |

## Quick Start

```rust
use wifi_densepose_train::config::TrainingConfig;
use wifi_densepose_train::dataset::{SyntheticCsiDataset, SyntheticConfig, CsiDataset};

// Build and validate config
let config = TrainingConfig::default();
config.validate().expect("config is valid");

// Create a synthetic dataset (deterministic, fixed-seed)
let syn_cfg = SyntheticConfig::default();
let dataset = SyntheticCsiDataset::new(200, syn_cfg);

// Load one sample
let sample = dataset.get(0).unwrap();
println!("amplitude shape: {:?}", sample.amplitude.shape());
```

## Architecture

```text
wifi-densepose-train/src/
  lib.rs            -- Re-exports, VERSION
  config.rs         -- TrainingConfig, hyperparameters, validation
  dataset.rs        -- CsiDataset trait, MmFiDataset, SyntheticCsiDataset, DataLoader
  error.rs          -- TrainError, ConfigError, DatasetError, SubcarrierError
  subcarrier.rs     -- interpolate_subcarriers (114->56), variance-based selection
  losses.rs         -- (tch) MSE, OKS, multi-task loss        [feature-gated]
  metrics.rs        -- (tch) PCKh, OKS-AP, person matching     [feature-gated]
  model.rs          -- (tch) Model definition with attention    [feature-gated]
  proof.rs          -- (tch) Deterministic training proofs      [feature-gated]
  trainer.rs        -- (tch) Training loop orchestrator         [feature-gated]
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-signal`](../wifi-densepose-signal) | Signal preprocessing consumed by dataset loaders |
| [`wifi-densepose-nn`](../wifi-densepose-nn) | Inference engine that loads trained models |
| [`ruvector-mincut`](https://crates.io/crates/ruvector-mincut) | Person matching in metrics |
| [`ruvector-attn-mincut`](https://crates.io/crates/ruvector-attn-mincut) | Attention-weighted graph cuts |
| [`ruvector-temporal-tensor`](https://crates.io/crates/ruvector-temporal-tensor) | Compressed CSI buffering in datasets |
| [`ruvector-solver`](https://crates.io/crates/ruvector-solver) | Sparse subcarrier interpolation |
| [`ruvector-attention`](https://crates.io/crates/ruvector-attention) | Spatial attention in model |

## License

MIT OR Apache-2.0
