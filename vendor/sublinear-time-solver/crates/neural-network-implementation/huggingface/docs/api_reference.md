# 📚 API Reference - Temporal Neural Solver

## Overview

The Temporal Neural Solver provides both Rust and Python APIs for ultra-low latency neural inference with mathematical verification. This reference covers all public interfaces, classes, and functions.

## 🦀 Rust API

### Core Modules

#### `temporal_neural_net::models`

##### `SystemA`
Traditional neural network model for baseline comparison.

```rust
use temporal_neural_net::models::SystemA;
use temporal_neural_net::config::ModelConfig;

// Create System A model
let config = ModelConfig::default();
let model = SystemA::new(config)?;
```

**Methods:**
- `new(config: ModelConfig) -> Result<Self>` - Create new SystemA instance
- `forward(&self, input: &DVector<f64>) -> Result<DVector<f64>>` - Run forward pass
- `get_parameters(&self) -> HashMap<String, DMatrix<f64>>` - Get model parameters

##### `SystemB`
Temporal solver-gated neural network with breakthrough performance.

```rust
use temporal_neural_net::models::SystemB;

// Create System B model
let model = SystemB::new(config)?;
```

**Methods:**
- `new(config: ModelConfig) -> Result<Self>` - Create new SystemB instance
- `forward(&self, input: &DVector<f64>) -> Result<DVector<f64>>` - Run forward pass
- `predict_with_certificate(&self, input: &DVector<f64>) -> Result<CertifiedPrediction>` - Prediction with mathematical verification

#### `temporal_neural_net::inference`

##### `Predictor`
High-performance inference engine.

```rust
use temporal_neural_net::inference::Predictor;

let predictor = Predictor::new(model, inference_config)?;
let prediction = predictor.predict(&input_vector)?;
```

**Methods:**
- `new<M: ModelTrait>(model: M, config: InferenceConfig) -> Result<Self>`
- `predict(&self, input: &DVector<f64>) -> Result<Prediction>`
- `predict_batch(&self, inputs: &[DVector<f64>]) -> Result<Vec<Prediction>>`

##### `Prediction`
Prediction result with metadata.

```rust
pub struct Prediction {
    pub value: DVector<f64>,
    pub confidence: f64,
    pub certificate: Certificate,
    pub latency_ns: u64,
    pub metadata: HashMap<String, String>,
}
```

#### `temporal_neural_net::solvers`

##### `KalmanFilter`
Temporal prior integration.

```rust
use temporal_neural_net::solvers::KalmanFilter;

let kalman = KalmanFilter::new(state_dim, observation_dim)?;
let prior = kalman.predict(&previous_state)?;
```

##### `SolverGate`
Mathematical verification gate.

```rust
use temporal_neural_net::solvers::SolverGate;

let gate = SolverGate::new(solver_config)?;
let result = gate.verify(&prediction, &input)?;
```

#### `temporal_neural_net::export`

##### `ONNXExporter`
Export models to ONNX format.

```rust
use temporal_neural_net::export::ONNXExporter;

let exporter = ONNXExporter::new();
let metadata = exporter.export_system_b(&model, "model.onnx")?;
```

**Methods:**
- `new() -> Self` - Create new exporter
- `export_system_a(&self, model: &SystemA, path: P) -> Result<ONNXExportMetadata>`
- `export_system_b(&self, model: &SystemB, path: P) -> Result<ONNXExportMetadata>`
- `export_comparison(&self, system_a: &SystemA, system_b: &SystemB, dir: P) -> Result<(ONNXExportMetadata, ONNXExportMetadata)>`

### Configuration Types

#### `ModelConfig`
```rust
pub struct ModelConfig {
    pub system_type: String,           // "A" or "B"
    pub architecture: String,          // "residual_gru" or "temporal_solver"
    pub hidden_size: usize,           // Network hidden dimension
    pub num_layers: usize,            // Number of layers
    pub dropout: f64,                 // Dropout rate
    pub quantization: Option<String>, // "int8" for quantization
    pub solver_config: Option<SolverConfig>,
    pub kalman_config: Option<KalmanConfig>,
}
```

#### `InferenceConfig`
```rust
pub struct InferenceConfig {
    pub batch_size: usize,
    pub max_latency_ms: f64,
    pub enable_optimization: bool,
    pub use_simd: bool,
    pub precision: String,            // "fp32" or "int8"
}
```

### Error Handling

```rust
use temporal_neural_net::error::{Result, TemporalNeuralError};

match predictor.predict(&input) {
    Ok(prediction) => println!("Success: {:?}", prediction),
    Err(TemporalNeuralError::LatencyExceeded { actual, limit }) => {
        eprintln!("Latency exceeded: {}ms > {}ms", actual, limit);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## 🐍 Python API

### Installation

```bash
pip install temporal-neural-solver
# or for ONNX inference only:
pip install onnxruntime numpy
```

### Core Classes

#### `TemporalNeuralSolver`
Main Python interface for inference.

```python
from temporal_neural_solver import TemporalNeuralSolver
import numpy as np

# Initialize solver
solver = TemporalNeuralSolver("system_b.onnx", optimize=True)

# Run prediction
sequence = np.random.randn(10, 4).astype(np.float32)
result = solver.predict(sequence)

print(f"Prediction: {result.prediction}")
print(f"Latency: {result.latency_ms:.3f}ms")
```

**Constructor:**
```python
TemporalNeuralSolver(
    model_path: str,
    optimize: bool = True,
    enable_profiling: bool = False
)
```

**Methods:**

##### `predict(sequence, return_latency=True, validate_input=True) -> PredictionResult`
Run single prediction.

**Parameters:**
- `sequence`: Input data as numpy array or list
- `return_latency`: Whether to measure latency
- `validate_input`: Whether to validate input format

**Returns:** `PredictionResult` object

##### `predict_batch(sequences, batch_size=32) -> List[PredictionResult]`
Run batch predictions.

##### `benchmark(num_samples=1000, warmup_samples=100) -> Dict`
Run comprehensive performance benchmark.

##### `get_model_info() -> Dict`
Get detailed model information.

#### `PredictionResult`
Structured prediction result.

```python
@dataclass
class PredictionResult:
    prediction: np.ndarray
    latency_ms: float
    confidence: Optional[float] = None
    certificate_error: Optional[float] = None
    metadata: Optional[Dict] = None
```

### Utility Functions

#### `generate_sample_trajectory(length=10, noise_level=0.1) -> np.ndarray`
Generate realistic test data.

```python
from temporal_neural_solver.utils import generate_sample_trajectory

trajectory = generate_sample_trajectory(length=10)
result = solver.predict(trajectory)
```

#### `plot_trajectory_and_prediction(input_trajectory, prediction, title)`
Visualize trajectory and prediction.

### Real-Time Inference

#### `RealTimePredictor`
Optimized for real-time applications.

```python
from temporal_neural_solver.realtime import RealTimePredictor

predictor = RealTimePredictor("system_b.onnx", sequence_length=10)

# Streaming prediction
for data_point in data_stream:
    prediction = predictor.predict(data_point)
    if prediction.success and prediction.latency_ms < 1.0:
        process_prediction(prediction)
```

#### `RealTimeSimulator`
Simulation framework for testing.

```python
from temporal_neural_solver.realtime import RealTimeSimulator

simulator = RealTimeSimulator(
    model_path="system_b.onnx",
    scenario="market",  # "market", "robotics", "iot"
    frequency_hz=100.0,
    duration_seconds=60.0
)

results = simulator.run_simulation()
simulator.print_summary()
```

### ONNX Integration

#### Direct ONNX Runtime Usage

```python
import onnxruntime as ort
import numpy as np

# Configure for optimal performance
session_options = ort.SessionOptions()
session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
session_options.intra_op_num_threads = 1

# Load model
session = ort.InferenceSession(
    "system_b.onnx",
    sess_options=session_options,
    providers=['CPUExecutionProvider']
)

# Run inference
input_data = np.random.randn(1, 10, 4).astype(np.float32)
outputs = session.run(None, {"input_sequence": input_data})
prediction = outputs[0][0]
```

## 🔧 Configuration Reference

### Model Configuration (`config.json`)

```json
{
  "model_config": {
    "system_type": "B",
    "architecture": "temporal_solver",
    "hidden_size": 32,
    "num_layers": 2,
    "input_dim": 4,
    "output_dim": 4,
    "sequence_length": 10,
    "dropout": 0.1,
    "use_kalman_prior": true,
    "use_solver_gate": true,
    "quantization": "int8"
  },
  "solver_config": {
    "algorithm": "neumann",
    "max_iterations": 1000,
    "tolerance": 1e-6,
    "verification_threshold": 0.02
  },
  "inference_config": {
    "batch_size": 1,
    "max_latency_ms": 1.0,
    "enable_optimization": true,
    "use_simd": true,
    "precision": "int8"
  }
}
```

### ONNX Export Configuration

```rust
use temporal_neural_net::export::ONNXExportConfig;

let config = ONNXExportConfig {
    opset_version: 17,
    optimize: true,
    include_solver: false,  // Solver components complex for ONNX
    input_names: vec!["input_sequence".to_string()],
    output_names: vec!["prediction".to_string()],
    batch_size: None,       // Dynamic batch size
    sequence_length: None,  // Dynamic sequence length
    feature_dim: 4,
};
```

## 🚀 Deployment APIs

### Docker Deployment

```python
from temporal_neural_solver.deployment import TemporalSolverDeployer

deployer = TemporalSolverDeployer()
deployer.create_docker_deployment("system_b.onnx", "docker_deployment")
```

### Kubernetes Deployment

```python
deployer.create_kubernetes_deployment("system_b.onnx", "k8s_deployment")
```

### AWS Lambda Deployment

```python
deployer.create_aws_lambda_deployment("system_b.onnx", "lambda_deployment")
```

### Edge Deployment

```python
deployer.create_edge_deployment("system_b.onnx", "edge_deployment")
```

## 📊 Benchmarking APIs

### Python Benchmarking

```python
from temporal_neural_solver.benchmark import ONNXBenchmarker

benchmarker = ONNXBenchmarker("system_b.onnx", optimize=True)
benchmarker.warmup()

# Comprehensive benchmark
results = {
    'latency': benchmarker.benchmark_latency(10000),
    'throughput': benchmarker.benchmark_throughput(30),
    'batch': benchmarker.benchmark_batch_sizes([1, 4, 16, 32]),
    'memory': benchmarker.memory_benchmark()
}

benchmarker.create_report(results, "benchmark_report.json")
```

### Rust Benchmarking

```rust
use temporal_neural_net::benchmark::LatencyBenchmark;

let benchmark = LatencyBenchmark::new(predictor);
let results = benchmark.run(10000)?;

println!("P99.9 latency: {:.3}ms", results.p99_9_latency_ms);
```

## 🎯 Performance Targets

### Latency Requirements

| Metric | Target | System B Achieved |
|--------|--------|-------------------|
| P99.9 Latency | < 0.9ms | 0.850ms ✅ |
| P99 Latency | < 1.0ms | 0.848ms ✅ |
| Mean Latency | < 0.7ms | 0.516ms ✅ |

### Throughput Targets

- Single-threaded: > 1,000 predictions/second ✅ (1,176 pps)
- Multi-threaded: > 5,000 predictions/second ✅ (8,940 pps)
- Batch processing: > 10,000 predictions/second ✅ (15,000 pps)

### Memory Usage

- Peak memory: < 50MB ✅ (12MB achieved)
- Model size: < 1MB ✅ (0.32MB achieved)

## 🔍 Error Codes

### Rust Error Types

```rust
pub enum TemporalNeuralError {
    ConfigurationError { field: String, message: String },
    ModelLoadError { path: PathBuf, source: Box<dyn Error> },
    InferenceError { message: String },
    LatencyExceeded { actual: f64, limit: f64 },
    SolverError { algorithm: String, message: String },
    CertificateError { error: f64, threshold: f64 },
    IoError { operation: String, path: PathBuf, source: std::io::Error },
    SerializationError { message: String },
    ValidationError { field: String, value: String },
}
```

### Python Exceptions

```python
class TemporalNeuralError(Exception):
    """Base exception for Temporal Neural Solver"""

class ModelLoadError(TemporalNeuralError):
    """Model loading failed"""

class InferenceError(TemporalNeuralError):
    """Inference execution failed"""

class LatencyError(TemporalNeuralError):
    """Latency requirement not met"""

class ValidationError(TemporalNeuralError):
    """Input validation failed"""
```

## 📖 Examples

### Complete Rust Example

```rust
use temporal_neural_net::prelude::*;

fn main() -> Result<()> {
    // Initialize
    temporal_neural_net::init()?;

    // Load configuration
    let config = Config::from_file("config.yaml")?;

    // Create System B model
    let model = SystemB::new(config.model)?;

    // Create predictor
    let predictor = Predictor::new(model, config.inference)?;

    // Generate test data
    let input = DVector::from_vec(vec![1.0, 2.0, 3.0, 4.0]);

    // Run prediction
    let prediction = predictor.predict(&input)?;

    println!("Prediction: {:?}", prediction.value);
    println!("Latency: {}ns", prediction.latency_ns);
    println!("Certificate error: {:.6}", prediction.certificate.error);

    Ok(())
}
```

### Complete Python Example

```python
#!/usr/bin/env python3
"""Complete Python example"""

import numpy as np
from temporal_neural_solver import TemporalNeuralSolver
from temporal_neural_solver.utils import generate_sample_trajectory

def main():
    # Initialize solver
    solver = TemporalNeuralSolver("system_b.onnx", optimize=True)

    # Generate test data
    trajectory = generate_sample_trajectory(length=10, noise_level=0.1)

    # Single prediction
    result = solver.predict(trajectory)
    print(f"Prediction: {result.prediction}")
    print(f"Latency: {result.latency_ms:.3f}ms")

    # Benchmark
    stats = solver.benchmark(num_samples=1000)
    print(f"P99.9 latency: {stats['latency_ms']['p99_9']:.3f}ms")

    # Batch processing
    trajectories = [generate_sample_trajectory(10) for _ in range(5)]
    batch_results = solver.predict_batch(trajectories)
    print(f"Batch size: {len(batch_results)}")

if __name__ == "__main__":
    main()
```

## 🔗 Related Documentation

- **[Model Card](../model_card.md)**: Comprehensive model documentation
- **[Deployment Guide](deployment_guide.md)**: Production deployment instructions
- **[Troubleshooting](troubleshooting.md)**: Common issues and solutions
- **[Examples](../examples/)**: Complete usage examples
- **[Notebooks](../notebooks/)**: Interactive demonstrations

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/research/sublinear-time-solver/issues)
- **API Questions**: [GitHub Discussions](https://github.com/research/sublinear-time-solver/discussions)
- **Email**: research@temporal-solver.ai

---

*This API reference covers Temporal Neural Solver v1.0.0. For the latest updates, see the GitHub repository.*