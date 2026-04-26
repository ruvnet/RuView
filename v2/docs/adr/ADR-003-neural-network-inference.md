# ADR-003: Neural Network Inference Strategy

## Status
Accepted

## Context
The WiFi-DensePose system requires neural network inference for:
1. Modality translation (CSI → visual features)
2. DensePose estimation (body part segmentation + UV mapping)

We need to select an inference strategy that supports pre-trained models and multiple backends.

## Decision
We will implement a multi-backend inference engine:

### Primary Backend: ONNX Runtime (`ort` crate)
- Load pre-trained PyTorch models exported to ONNX
- GPU acceleration via CUDA/TensorRT
- Cross-platform support

### Alternative Backends (Feature-gated)
- `tch-rs`: PyTorch C++ bindings
- `candle`: Pure Rust ML framework

### Architecture
```rust
pub trait Backend: Send + Sync {
    fn load_model(&mut self, path: &Path) -> NnResult<()>;
    fn run(&self, inputs: HashMap<String, Tensor>) -> NnResult<HashMap<String, Tensor>>;
    fn input_specs(&self) -> Vec<TensorSpec>;
    fn output_specs(&self) -> Vec<TensorSpec>;
}
```

### Feature Flags
```toml
[features]
default = ["onnx"]
onnx = ["ort"]
tch-backend = ["tch"]
candle-backend = ["candle-core", "candle-nn"]
cuda = ["ort/cuda"]
tensorrt = ["ort/tensorrt"]
```

## Consequences

### Positive
- Use existing trained models (no retraining)
- Multiple backend options for different deployments
- GPU acceleration when available
- Feature flags minimize binary size

### Negative
- ONNX model conversion required
- ort crate pulls in C++ dependencies
- tch requires libtorch installation
