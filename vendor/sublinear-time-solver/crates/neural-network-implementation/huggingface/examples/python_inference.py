#!/usr/bin/env python3
"""
Python Inference Example for Temporal Neural Solver

This script demonstrates how to use the Temporal Neural Solver models
for ultra-low latency inference in Python applications.
"""

import time
import argparse
import json
from pathlib import Path
from typing import Dict, List, Tuple, Optional, Union
import warnings
warnings.filterwarnings('ignore')

try:
    import numpy as np
    import onnxruntime as ort
    import matplotlib.pyplot as plt
    from dataclasses import dataclass
except ImportError as e:
    print(f"❌ Missing dependencies: {e}")
    print("Install with: pip install numpy onnxruntime matplotlib")
    exit(1)

@dataclass
class PredictionResult:
    """Structured prediction result"""
    prediction: np.ndarray
    latency_ms: float
    confidence: Optional[float] = None
    certificate_error: Optional[float] = None
    metadata: Optional[Dict] = None

class TemporalNeuralSolver:
    """
    Python interface for Temporal Neural Solver inference

    This class provides a high-level interface for running inference
    with the breakthrough sub-millisecond neural network.
    """

    def __init__(
        self,
        model_path: str,
        optimize: bool = True,
        enable_profiling: bool = False
    ):
        """
        Initialize the Temporal Neural Solver

        Args:
            model_path: Path to ONNX model file
            optimize: Enable ONNX Runtime optimizations
            enable_profiling: Enable performance profiling
        """
        self.model_path = Path(model_path)
        self.enable_profiling = enable_profiling

        if not self.model_path.exists():
            raise FileNotFoundError(f"Model file not found: {model_path}")

        # Configure ONNX Runtime for optimal performance
        self.session_options = ort.SessionOptions()

        if optimize:
            self.session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            self.session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
            self.session_options.intra_op_num_threads = 1  # Single thread for latency

        if enable_profiling:
            self.session_options.enable_profiling = True

        # Load model with optimal providers
        providers = self._get_optimal_providers()

        try:
            self.session = ort.InferenceSession(
                str(self.model_path),
                sess_options=self.session_options,
                providers=providers
            )
        except Exception as e:
            print(f"❌ Failed to load model: {e}")
            raise

        # Get model metadata
        self.input_info = self.session.get_inputs()[0]
        self.output_info = self.session.get_outputs()[0]

        print(f"✅ Temporal Neural Solver loaded: {self.model_path.name}")
        print(f"   Input: {self.input_info.name} {self.input_info.shape}")
        print(f"   Output: {self.output_info.name} {self.output_info.shape}")
        print(f"   Providers: {self.session.get_providers()}")

        # Warmup for stable performance
        self._warmup()

    def _get_optimal_providers(self) -> List[str]:
        """Get optimal execution providers based on availability"""
        available_providers = ort.get_available_providers()
        optimal_providers = []

        # Prefer GPU providers if available
        if 'TensorrtExecutionProvider' in available_providers:
            optimal_providers.append('TensorrtExecutionProvider')
        if 'CUDAExecutionProvider' in available_providers:
            optimal_providers.append('CUDAExecutionProvider')

        # Always include CPU as fallback
        optimal_providers.append('CPUExecutionProvider')

        return optimal_providers

    def _warmup(self, num_runs: int = 100) -> None:
        """Warmup the model for stable benchmarking"""
        print(f"🔥 Warming up model ({num_runs} runs)...")

        # Generate dummy input matching model expectations
        dummy_input = self._generate_dummy_input()
        input_dict = {self.input_info.name: dummy_input}

        for _ in range(num_runs):
            _ = self.session.run(None, input_dict)

        print("✅ Warmup complete")

    def _generate_dummy_input(self) -> np.ndarray:
        """Generate dummy input for warmup and testing"""
        # Parse input shape, handling dynamic dimensions
        shape = []
        for dim in self.input_info.shape:
            if isinstance(dim, str) or dim == -1:
                # Dynamic dimension - use reasonable default
                if len(shape) == 0:  # Batch dimension
                    shape.append(1)
                elif len(shape) == 1:  # Sequence dimension
                    shape.append(10)
                else:  # Feature dimension
                    shape.append(4)
            else:
                shape.append(dim)

        return np.random.randn(*shape).astype(np.float32)

    def predict(
        self,
        sequence: Union[np.ndarray, List[List[float]]],
        return_latency: bool = True,
        validate_input: bool = True
    ) -> PredictionResult:
        """
        Run prediction on input sequence

        Args:
            sequence: Input time series data [timesteps, features] or [batch, timesteps, features]
            return_latency: Whether to measure and return latency
            validate_input: Whether to validate input format

        Returns:
            PredictionResult with prediction and metadata
        """
        # Convert to numpy array if needed
        if isinstance(sequence, list):
            sequence = np.array(sequence, dtype=np.float32)

        # Validate and reshape input
        if validate_input:
            sequence = self._validate_and_reshape_input(sequence)

        # Prepare input dictionary
        input_dict = {self.input_info.name: sequence}

        # Run inference with optional timing
        if return_latency:
            start_time = time.perf_counter()
            outputs = self.session.run(None, input_dict)
            end_time = time.perf_counter()
            latency_ms = (end_time - start_time) * 1000
        else:
            outputs = self.session.run(None, input_dict)
            latency_ms = 0.0

        # Extract prediction (first batch if batched)
        prediction = outputs[0]
        if prediction.ndim > 1:
            prediction = prediction[0]

        return PredictionResult(
            prediction=prediction,
            latency_ms=latency_ms,
            metadata={
                'model': str(self.model_path.name),
                'input_shape': list(sequence.shape),
                'output_shape': list(outputs[0].shape)
            }
        )

    def _validate_and_reshape_input(self, sequence: np.ndarray) -> np.ndarray:
        """Validate and reshape input to match model expectations"""
        # Ensure float32 dtype
        if sequence.dtype != np.float32:
            sequence = sequence.astype(np.float32)

        # Handle different input shapes
        if sequence.ndim == 1:
            # Single timestep: [features] -> [1, 1, features]
            sequence = sequence.reshape(1, 1, -1)
        elif sequence.ndim == 2:
            # Sequence: [timesteps, features] -> [1, timesteps, features]
            sequence = sequence.reshape(1, *sequence.shape)
        elif sequence.ndim == 3:
            # Already correct: [batch, timesteps, features]
            pass
        else:
            raise ValueError(f"Invalid input shape: {sequence.shape}")

        return sequence

    def predict_batch(
        self,
        sequences: List[np.ndarray],
        batch_size: int = 32
    ) -> List[PredictionResult]:
        """
        Run batch prediction on multiple sequences

        Args:
            sequences: List of input sequences
            batch_size: Batch size for processing

        Returns:
            List of PredictionResult objects
        """
        results = []

        for i in range(0, len(sequences), batch_size):
            batch = sequences[i:i + batch_size]

            # Stack into batch tensor
            batch_tensor = np.stack([
                self._validate_and_reshape_input(seq)[0] for seq in batch
            ], axis=0)

            # Run batch prediction
            input_dict = {self.input_info.name: batch_tensor}

            start_time = time.perf_counter()
            outputs = self.session.run(None, input_dict)
            end_time = time.perf_counter()

            total_latency_ms = (end_time - start_time) * 1000
            per_sample_latency = total_latency_ms / len(batch)

            # Create results for each item in batch
            for j, prediction in enumerate(outputs[0]):
                results.append(PredictionResult(
                    prediction=prediction,
                    latency_ms=per_sample_latency,
                    metadata={
                        'model': str(self.model_path.name),
                        'batch_size': len(batch),
                        'batch_index': j
                    }
                ))

        return results

    def benchmark(
        self,
        num_samples: int = 1000,
        warmup_samples: int = 100,
        return_raw_latencies: bool = False
    ) -> Dict:
        """
        Run comprehensive performance benchmark

        Args:
            num_samples: Number of inference samples
            warmup_samples: Number of warmup samples
            return_raw_latencies: Whether to include raw latency data

        Returns:
            Dictionary with benchmark statistics
        """
        print(f"📊 Running benchmark ({num_samples} samples)...")

        # Generate test data
        dummy_input = self._generate_dummy_input()
        input_dict = {self.input_info.name: dummy_input}

        # Warmup
        print(f"🔥 Warmup ({warmup_samples} samples)...")
        for _ in range(warmup_samples):
            _ = self.session.run(None, input_dict)

        # Benchmark
        print(f"⏱️  Measuring latency...")
        latencies = []
        errors = 0

        for i in range(num_samples):
            if i % 100 == 0 and i > 0:
                print(f"   Progress: {i}/{num_samples}")

            try:
                start_time = time.perf_counter()
                outputs = self.session.run(None, input_dict)
                end_time = time.perf_counter()

                latency_ms = (end_time - start_time) * 1000
                latencies.append(latency_ms)

                # Basic output validation
                if not outputs or outputs[0] is None:
                    errors += 1

            except Exception as e:
                errors += 1
                print(f"   Error in sample {i}: {e}")

        latencies = np.array(latencies)

        # Calculate comprehensive statistics
        stats = {
            'num_samples': len(latencies),
            'errors': errors,
            'success_rate': (len(latencies) / num_samples) * 100,
            'latency_ms': {
                'mean': float(np.mean(latencies)),
                'std': float(np.std(latencies)),
                'min': float(np.min(latencies)),
                'max': float(np.max(latencies)),
                'median': float(np.median(latencies)),
                'p90': float(np.percentile(latencies, 90)),
                'p95': float(np.percentile(latencies, 95)),
                'p99': float(np.percentile(latencies, 99)),
                'p99_9': float(np.percentile(latencies, 99.9)),
                'p99_99': float(np.percentile(latencies, 99.99)),
            },
            'throughput_pps': 1000 / np.mean(latencies),
            'success_criteria': {
                'sub_millisecond_p99_9': float(np.percentile(latencies, 99.9)) < 1.0,
                'target_0_9ms_p99_9': float(np.percentile(latencies, 99.9)) < 0.9,
                'high_success_rate': (len(latencies) / num_samples) * 100 > 99.0
            }
        }

        if return_raw_latencies:
            stats['raw_latencies'] = latencies.tolist()

        print(f"✅ Benchmark complete!")
        print(f"   Mean latency: {stats['latency_ms']['mean']:.3f}ms")
        print(f"   P99.9 latency: {stats['latency_ms']['p99_9']:.3f}ms")
        print(f"   Throughput: {stats['throughput_pps']:.0f} pps")
        print(f"   Sub-ms P99.9: {'✅' if stats['success_criteria']['sub_millisecond_p99_9'] else '❌'}")
        print(f"   Target 0.9ms: {'✅' if stats['success_criteria']['target_0_9ms_p99_9'] else '❌'}")

        return stats

    def get_model_info(self) -> Dict:
        """Get detailed model information"""
        return {
            'model_path': str(self.model_path),
            'model_size_mb': self.model_path.stat().st_size / (1024 * 1024),
            'inputs': [{
                'name': inp.name,
                'shape': inp.shape,
                'type': inp.type
            } for inp in self.session.get_inputs()],
            'outputs': [{
                'name': out.name,
                'shape': out.shape,
                'type': out.type
            } for out in self.session.get_outputs()],
            'providers': self.session.get_providers(),
            'onnx_version': ort.__version__
        }

def generate_sample_trajectory(length: int = 10, noise_level: float = 0.1) -> np.ndarray:
    """Generate a sample trajectory for demonstration"""
    trajectory = []

    for i in range(length):
        t = i / length
        # Position with sinusoidal motion + noise
        x = np.sin(2 * np.pi * t) + np.random.normal(0, noise_level)
        y = np.cos(2 * np.pi * t) + np.random.normal(0, noise_level)

        # Velocity (derivatives)
        vx = 2 * np.pi * np.cos(2 * np.pi * t) + np.random.normal(0, noise_level * 0.5)
        vy = -2 * np.pi * np.sin(2 * np.pi * t) + np.random.normal(0, noise_level * 0.5)

        trajectory.append([x, y, vx, vy])

    return np.array(trajectory, dtype=np.float32)

def plot_trajectory_and_prediction(
    input_trajectory: np.ndarray,
    prediction: np.ndarray,
    title: str = "Trajectory Prediction"
) -> None:
    """Visualize input trajectory and prediction"""
    try:
        fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(12, 5))

        # Plot trajectory in 2D space
        ax1.plot(input_trajectory[:, 0], input_trajectory[:, 1], 'b-o',
                label='Input Trajectory', alpha=0.7)
        ax1.plot(prediction[0], prediction[1], 'ro',
                label='Predicted Next Point', markersize=10)
        ax1.set_xlabel('X Position')
        ax1.set_ylabel('Y Position')
        ax1.set_title('Spatial Trajectory')
        ax1.legend()
        ax1.grid(True, alpha=0.3)
        ax1.axis('equal')

        # Plot time series
        time_steps = range(len(input_trajectory))
        for i, label in enumerate(['X', 'Y', 'VX', 'VY']):
            ax2.plot(time_steps, input_trajectory[:, i],
                    label=f'{label} (input)', alpha=0.7)

        # Show prediction as next timestep
        next_time = len(input_trajectory)
        for i, label in enumerate(['X', 'Y', 'VX', 'VY']):
            ax2.plot(next_time, prediction[i], 'o',
                    label=f'{label} (pred)', markersize=8)

        ax2.set_xlabel('Time Step')
        ax2.set_ylabel('Value')
        ax2.set_title('Time Series Features')
        ax2.legend()
        ax2.grid(True, alpha=0.3)

        plt.suptitle(title, fontsize=14, fontweight='bold')
        plt.tight_layout()
        plt.show()

    except Exception as e:
        print(f"⚠️  Plotting failed: {e}")

def demo_basic_usage(model_path: str) -> None:
    """Demonstrate basic usage of the Temporal Neural Solver"""
    print("🎯 Basic Usage Demo")
    print("="*40)

    # Initialize solver
    solver = TemporalNeuralSolver(model_path)

    # Generate sample data
    trajectory = generate_sample_trajectory(length=10)
    print(f"📊 Generated trajectory with shape: {trajectory.shape}")

    # Single prediction
    result = solver.predict(trajectory)

    print(f"✅ Prediction Results:")
    print(f"   Prediction: {result.prediction}")
    print(f"   Latency: {result.latency_ms:.3f}ms")
    print(f"   Sub-millisecond: {'✅' if result.latency_ms < 1.0 else '❌'}")

    # Visualize if matplotlib available
    plot_trajectory_and_prediction(trajectory, result.prediction,
                                  "Basic Usage - Trajectory Prediction")

def demo_batch_processing(model_path: str) -> None:
    """Demonstrate batch processing capabilities"""
    print("\n📦 Batch Processing Demo")
    print("="*40)

    solver = TemporalNeuralSolver(model_path)

    # Generate multiple trajectories
    trajectories = [generate_sample_trajectory(length=10) for _ in range(5)]
    print(f"📊 Generated {len(trajectories)} trajectories")

    # Batch prediction
    start_time = time.time()
    results = solver.predict_batch(trajectories, batch_size=32)
    total_time = time.time() - start_time

    print(f"✅ Batch Results:")
    print(f"   Total trajectories: {len(results)}")
    print(f"   Total time: {total_time*1000:.3f}ms")
    print(f"   Average latency per sample: {np.mean([r.latency_ms for r in results]):.3f}ms")
    print(f"   Throughput: {len(results)/total_time:.0f} predictions/second")

def demo_benchmark(model_path: str) -> None:
    """Demonstrate comprehensive benchmarking"""
    print("\n📊 Benchmark Demo")
    print("="*40)

    solver = TemporalNeuralSolver(model_path)

    # Run benchmark
    stats = solver.benchmark(num_samples=1000, return_raw_latencies=True)

    # Display results
    print(f"\n🏆 Benchmark Summary:")
    print(f"   Samples: {stats['num_samples']}")
    print(f"   Success rate: {stats['success_rate']:.1f}%")
    print(f"   Mean latency: {stats['latency_ms']['mean']:.3f}ms")
    print(f"   P99.9 latency: {stats['latency_ms']['p99_9']:.3f}ms")
    print(f"   Throughput: {stats['throughput_pps']:.0f} pps")

    print(f"\n🎯 Success Criteria:")
    for criterion, passed in stats['success_criteria'].items():
        status = "✅" if passed else "❌"
        print(f"   {criterion}: {status}")

    # Plot latency distribution
    if 'raw_latencies' in stats:
        try:
            plt.figure(figsize=(10, 6))

            latencies = stats['raw_latencies']

            plt.subplot(1, 2, 1)
            plt.hist(latencies, bins=50, alpha=0.7, edgecolor='black')
            plt.axvline(stats['latency_ms']['p99_9'], color='red', linestyle='--',
                       label=f'P99.9: {stats["latency_ms"]["p99_9"]:.3f}ms')
            plt.axvline(0.9, color='green', linestyle='--', label='Target: 0.9ms')
            plt.xlabel('Latency (ms)')
            plt.ylabel('Frequency')
            plt.title('Latency Distribution')
            plt.legend()
            plt.grid(True, alpha=0.3)

            plt.subplot(1, 2, 2)
            plt.plot(latencies[:100], alpha=0.7)  # First 100 samples
            plt.xlabel('Sample Number')
            plt.ylabel('Latency (ms)')
            plt.title('Latency Time Series')
            plt.grid(True, alpha=0.3)

            plt.suptitle('Benchmark Results - Latency Analysis', fontsize=14, fontweight='bold')
            plt.tight_layout()
            plt.show()

        except Exception as e:
            print(f"⚠️  Plotting failed: {e}")

def main():
    """Main demonstration function"""
    parser = argparse.ArgumentParser(
        description="Python Inference Example for Temporal Neural Solver"
    )
    parser.add_argument(
        "model_path",
        help="Path to ONNX model file"
    )
    parser.add_argument(
        "--demo",
        choices=["basic", "batch", "benchmark", "all"],
        default="all",
        help="Which demo to run"
    )
    parser.add_argument(
        "--no-plots",
        action="store_true",
        help="Disable matplotlib plots"
    )

    args = parser.parse_args()

    # Disable plotting if requested
    if args.no_plots:
        def dummy_plot(*args, **kwargs):
            pass
        global plot_trajectory_and_prediction
        plot_trajectory_and_prediction = dummy_plot

    print("🚀 Temporal Neural Solver - Python Inference Example")
    print("="*60)
    print(f"Model: {args.model_path}")
    print(f"Demo: {args.demo}")
    print()

    # Check if model exists
    if not Path(args.model_path).exists():
        print(f"❌ Model file not found: {args.model_path}")
        print("Please ensure the ONNX model file exists.")
        return

    try:
        # Run requested demos
        if args.demo in ["basic", "all"]:
            demo_basic_usage(args.model_path)

        if args.demo in ["batch", "all"]:
            demo_batch_processing(args.model_path)

        if args.demo in ["benchmark", "all"]:
            demo_benchmark(args.model_path)

        print("\n🎉 Demo complete!")
        print("\n💡 Next steps:")
        print("- Integrate the TemporalNeuralSolver class into your application")
        print("- Customize input preprocessing for your data")
        print("- Monitor latency in production environments")
        print("- Use batch processing for higher throughput")

    except Exception as e:
        print(f"❌ Demo failed: {e}")
        import traceback
        traceback.print_exc()

if __name__ == "__main__":
    main()