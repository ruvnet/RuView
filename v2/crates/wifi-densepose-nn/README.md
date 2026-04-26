# wifi-densepose-nn

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-nn.svg)](https://crates.io/crates/wifi-densepose-nn)
[![Documentation](https://docs.rs/wifi-densepose-nn/badge.svg)](https://docs.rs/wifi-densepose-nn)
[![License](https://img.shields.io/crates/l/wifi-densepose-nn.svg)](LICENSE)

Multi-backend neural network inference for WiFi-based DensePose estimation.

## Overview

`wifi-densepose-nn` provides the inference engine that maps processed WiFi CSI features to
DensePose body surface predictions. It supports three backends -- ONNX Runtime (default),
PyTorch via `tch-rs`, and Candle -- so models can run on CPU, CUDA GPU, or TensorRT depending
on the deployment target.

The crate implements two key neural components:

- **DensePose Head** -- Predicts 24 body part segmentation masks and per-part UV coordinate
  regression.
- **Modality Translator** -- Translates CSI feature embeddings into visual feature space,
  bridging the domain gap between WiFi signals and image-based pose estimation.

## Features

- **ONNX Runtime backend** (default) -- Load and run `.onnx` models with CPU or GPU execution
  providers.
- **PyTorch backend** (`tch-backend`) -- Native PyTorch inference via libtorch FFI.
- **Candle backend** (`candle-backend`) -- Pure-Rust inference with `candle-core` and
  `candle-nn`.
- **CUDA acceleration** (`cuda`) -- GPU execution for supported backends.
- **TensorRT optimization** (`tensorrt`) -- INT8/FP16 optimized inference via ONNX Runtime.
- **Batched inference** -- Process multiple CSI frames in a single forward pass.
- **Model caching** -- Memory-mapped model weights via `memmap2`.

### Feature flags

| Flag              | Default | Description                         |
|-------------------|---------|-------------------------------------|
| `onnx`            | yes     | ONNX Runtime backend                |
| `tch-backend`     | no      | PyTorch (tch-rs) backend            |
| `candle-backend`  | no      | Candle pure-Rust backend            |
| `cuda`            | no      | CUDA GPU acceleration               |
| `tensorrt`        | no      | TensorRT via ONNX Runtime           |
| `all-backends`    | no      | Enable onnx + tch + candle together |

## Quick Start

```rust
use wifi_densepose_nn::{InferenceEngine, DensePoseConfig, OnnxBackend};

// Create inference engine with ONNX backend
let config = DensePoseConfig::default();
let backend = OnnxBackend::from_file("model.onnx")?;
let engine = InferenceEngine::new(backend, config)?;

// Run inference on a CSI feature tensor
let input = ndarray::Array4::zeros((1, 256, 64, 64));
let output = engine.infer(&input)?;

println!("Body parts: {}", output.body_parts.shape()[1]); // 24
```

## Architecture

```text
wifi-densepose-nn/src/
  lib.rs          -- Re-exports, constants (NUM_BODY_PARTS=24), prelude
  densepose.rs    -- DensePoseHead, DensePoseConfig, DensePoseOutput
  inference.rs    -- Backend trait, InferenceEngine, InferenceOptions
  onnx.rs         -- OnnxBackend, OnnxSession (feature-gated)
  tensor.rs       -- Tensor, TensorShape utilities
  translator.rs   -- ModalityTranslator (CSI -> visual space)
  error.rs        -- NnError, NnResult
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-core`](../wifi-densepose-core) | Foundation types and `NeuralInference` trait |
| [`wifi-densepose-signal`](../wifi-densepose-signal) | Produces CSI features consumed by inference |
| [`wifi-densepose-train`](../wifi-densepose-train) | Trains the models this crate loads |
| [`ort`](https://crates.io/crates/ort) | ONNX Runtime Rust bindings |
| [`tch`](https://crates.io/crates/tch) | PyTorch Rust bindings |
| [`candle-core`](https://crates.io/crates/candle-core) | Hugging Face pure-Rust ML framework |

## License

MIT OR Apache-2.0
