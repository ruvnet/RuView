#!/usr/bin/env python3
"""
ONNX Performance Benchmark Script for Temporal Neural Solver

This script validates the performance of exported ONNX models to ensure
they meet the sub-millisecond latency requirements.
"""

import argparse
import time
import json
import statistics
from pathlib import Path
from typing import Dict, List, Tuple, Optional
import warnings
warnings.filterwarnings('ignore')

try:
    import numpy as np
    import onnxruntime as ort
    import matplotlib.pyplot as plt
    import seaborn as sns
    from scipy import stats
    import pandas as pd
except ImportError as e:
    print(f"❌ Missing dependencies: {e}")
    print("Install with: pip install numpy onnxruntime matplotlib seaborn scipy pandas")
    exit(1)

class ONNXBenchmarker:
    """Comprehensive ONNX model performance benchmarker"""

    def __init__(self, model_path: str, optimize: bool = True):
        self.model_path = Path(model_path)
        self.model_name = self.model_path.stem

        # Configure ONNX Runtime for optimal performance
        self.session_options = ort.SessionOptions()
        if optimize:
            self.session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
            self.session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
            self.session_options.intra_op_num_threads = 1  # Single thread for latency

        # Load model
        try:
            self.session = ort.InferenceSession(
                str(self.model_path),
                sess_options=self.session_options,
                providers=['CPUExecutionProvider']
            )
            print(f"✅ Loaded model: {self.model_path}")
        except Exception as e:
            print(f"❌ Failed to load model {self.model_path}: {e}")
            raise

        # Get model info
        self.input_info = self.session.get_inputs()[0]
        self.output_info = self.session.get_outputs()[0]

        print(f"📊 Model Info:")
        print(f"   Input: {self.input_info.name} {self.input_info.shape}")
        print(f"   Output: {self.output_info.name} {self.output_info.shape}")

    def generate_test_data(self, batch_size: int = 1, sequence_length: int = 10,
                          feature_dim: int = 4) -> np.ndarray:
        """Generate realistic test data"""
        # Create realistic time series data
        data = []
        for b in range(batch_size):
            trajectory = []
            # Generate sinusoidal trajectory with noise
            for i in range(sequence_length):
                t = i / sequence_length
                x = np.sin(2 * np.pi * t) + np.random.normal(0, 0.1)
                y = np.cos(2 * np.pi * t) + np.random.normal(0, 0.1)
                vx = 2 * np.pi * np.cos(2 * np.pi * t) + np.random.normal(0, 0.05)
                vy = -2 * np.pi * np.sin(2 * np.pi * t) + np.random.normal(0, 0.05)
                trajectory.append([x, y, vx, vy])
            data.append(trajectory)

        return np.array(data, dtype=np.float32)

    def warmup(self, num_runs: int = 100) -> None:
        """Warmup the model for stable benchmarking"""
        print(f"🔥 Warming up model ({num_runs} runs)...")

        test_data = self.generate_test_data()
        input_dict = {self.input_info.name: test_data}

        for _ in range(num_runs):
            _ = self.session.run(None, input_dict)

        print("✅ Warmup complete")

    def benchmark_latency(self, num_samples: int = 10000, batch_size: int = 1) -> Dict:
        """Comprehensive latency benchmark"""
        print(f"⏱️  Running latency benchmark ({num_samples} samples, batch_size={batch_size})...")

        # Generate test data
        test_data = self.generate_test_data(batch_size)
        input_dict = {self.input_info.name: test_data}

        # Collect latency measurements
        latencies = []
        errors = 0

        for i in range(num_samples):
            if i % 1000 == 0 and i > 0:
                print(f"   Progress: {i}/{num_samples}")

            try:
                start_time = time.perf_counter()
                outputs = self.session.run(None, input_dict)
                end_time = time.perf_counter()

                latency_ms = (end_time - start_time) * 1000
                latencies.append(latency_ms)

                # Validate output shape
                if outputs[0].shape != (batch_size, 4):
                    errors += 1

            except Exception as e:
                errors += 1
                print(f"   Error in run {i}: {e}")

        latencies = np.array(latencies)

        # Calculate comprehensive statistics
        results = {
            'num_samples': len(latencies),
            'batch_size': batch_size,
            'errors': errors,
            'success_rate': (len(latencies) / num_samples) * 100,
            'latency_stats': {
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
            'raw_latencies': latencies.tolist()
        }

        # Success criteria check
        results['success_criteria'] = {
            'p99_9_under_0_9ms': results['latency_stats']['p99_9'] < 0.9,
            'success_rate_over_99_percent': results['success_rate'] > 99.0,
            'p99_9_latency_ms': results['latency_stats']['p99_9']
        }

        print(f"✅ Latency benchmark complete:")
        print(f"   Mean: {results['latency_stats']['mean']:.3f}ms")
        print(f"   P99.9: {results['latency_stats']['p99_9']:.3f}ms")
        print(f"   Success rate: {results['success_rate']:.1f}%")
        print(f"   Sub-ms target: {'✅' if results['success_criteria']['p99_9_under_0_9ms'] else '❌'}")

        return results

    def benchmark_throughput(self, duration_seconds: int = 30) -> Dict:
        """Throughput benchmark"""
        print(f"🚀 Running throughput benchmark ({duration_seconds}s)...")

        test_data = self.generate_test_data(1)
        input_dict = {self.input_info.name: test_data}

        start_time = time.perf_counter()
        end_time = start_time + duration_seconds

        predictions = 0
        latencies = []

        while time.perf_counter() < end_time:
            iter_start = time.perf_counter()
            _ = self.session.run(None, input_dict)
            iter_end = time.perf_counter()

            predictions += 1
            latencies.append((iter_end - iter_start) * 1000)

        total_time = time.perf_counter() - start_time
        throughput = predictions / total_time

        results = {
            'duration_seconds': total_time,
            'total_predictions': predictions,
            'throughput_pps': throughput,
            'avg_latency_ms': np.mean(latencies),
            'latency_std_ms': np.std(latencies)
        }

        print(f"✅ Throughput: {throughput:.0f} predictions/second")
        print(f"   Average latency: {results['avg_latency_ms']:.3f}ms")

        return results

    def benchmark_batch_sizes(self, batch_sizes: List[int] = None) -> Dict:
        """Benchmark different batch sizes"""
        if batch_sizes is None:
            batch_sizes = [1, 2, 4, 8, 16, 32]

        print(f"📊 Benchmarking batch sizes: {batch_sizes}")

        results = {}

        for batch_size in batch_sizes:
            print(f"\n🔄 Testing batch size {batch_size}...")

            # Generate data for this batch size
            test_data = self.generate_test_data(batch_size)
            input_dict = {self.input_info.name: test_data}

            # Run a smaller benchmark for each batch size
            latencies = []
            num_runs = max(100, 1000 // batch_size)  # Fewer runs for larger batches

            for _ in range(num_runs):
                start_time = time.perf_counter()
                _ = self.session.run(None, input_dict)
                end_time = time.perf_counter()

                latency_ms = (end_time - start_time) * 1000
                latencies.append(latency_ms)

            latencies = np.array(latencies)

            # Calculate per-sample latency
            per_sample_latency = latencies / batch_size
            throughput = batch_size / (np.mean(latencies) / 1000)

            results[batch_size] = {
                'batch_latency_ms': {
                    'mean': float(np.mean(latencies)),
                    'p99': float(np.percentile(latencies, 99)),
                    'p99_9': float(np.percentile(latencies, 99.9))
                },
                'per_sample_latency_ms': {
                    'mean': float(np.mean(per_sample_latency)),
                    'p99': float(np.percentile(per_sample_latency, 99)),
                    'p99_9': float(np.percentile(per_sample_latency, 99.9))
                },
                'throughput_pps': throughput
            }

            print(f"   Batch latency P99.9: {results[batch_size]['batch_latency_ms']['p99_9']:.3f}ms")
            print(f"   Per-sample latency P99.9: {results[batch_size]['per_sample_latency_ms']['p99_9']:.3f}ms")
            print(f"   Throughput: {throughput:.0f} predictions/second")

        return results

    def memory_benchmark(self) -> Dict:
        """Basic memory usage benchmark"""
        import psutil
        import os

        print("💾 Running memory benchmark...")

        process = psutil.Process(os.getpid())

        # Baseline memory
        baseline_memory = process.memory_info().rss / 1024 / 1024  # MB

        # Load model and run inference
        test_data = self.generate_test_data(1)
        input_dict = {self.input_info.name: test_data}

        # Run inference
        _ = self.session.run(None, input_dict)

        # Peak memory during inference
        peak_memory = process.memory_info().rss / 1024 / 1024  # MB

        # Run multiple inferences to check for memory leaks
        for _ in range(100):
            _ = self.session.run(None, input_dict)

        final_memory = process.memory_info().rss / 1024 / 1024  # MB

        results = {
            'baseline_memory_mb': baseline_memory,
            'peak_memory_mb': peak_memory,
            'final_memory_mb': final_memory,
            'memory_usage_mb': peak_memory - baseline_memory,
            'memory_leak_mb': final_memory - peak_memory
        }

        print(f"✅ Memory usage: {results['memory_usage_mb']:.1f}MB")
        print(f"   Memory leak check: {results['memory_leak_mb']:.1f}MB")

        return results

    def create_report(self, results: Dict, output_path: Optional[str] = None) -> None:
        """Create comprehensive benchmark report"""
        if output_path is None:
            output_path = f"{self.model_name}_benchmark_report.json"

        # Add metadata
        results['metadata'] = {
            'model_name': self.model_name,
            'model_path': str(self.model_path),
            'benchmark_timestamp': time.strftime('%Y-%m-%d %H:%M:%S UTC'),
            'onnxruntime_version': ort.__version__,
            'numpy_version': np.__version__
        }

        # Save JSON report
        with open(output_path, 'w') as f:
            json.dump(results, f, indent=2)

        print(f"📄 Report saved: {output_path}")

        # Create visualizations if matplotlib is available
        self.create_visualizations(results)

    def create_visualizations(self, results: Dict) -> None:
        """Create benchmark visualizations"""
        try:
            plt.style.use('seaborn-v0_8')
            fig, axes = plt.subplots(2, 2, figsize=(15, 10))
            fig.suptitle(f'ONNX Benchmark Results: {self.model_name}', fontsize=16, fontweight='bold')

            # 1. Latency distribution
            if 'latency_benchmark' in results:
                latencies = results['latency_benchmark']['raw_latencies'][:1000]  # First 1000 for plotting
                axes[0, 0].hist(latencies, bins=50, alpha=0.7, edgecolor='black')
                axes[0, 0].axvline(results['latency_benchmark']['latency_stats']['p99_9'],
                                  color='red', linestyle='--', label='P99.9')
                axes[0, 0].axvline(0.9, color='green', linestyle='--', label='Target (0.9ms)')
                axes[0, 0].set_xlabel('Latency (ms)')
                axes[0, 0].set_ylabel('Frequency')
                axes[0, 0].set_title('Latency Distribution')
                axes[0, 0].legend()
                axes[0, 0].grid(True, alpha=0.3)

            # 2. Batch size comparison
            if 'batch_benchmark' in results:
                batch_sizes = list(results['batch_benchmark'].keys())
                batch_sizes = [int(bs) for bs in batch_sizes]
                per_sample_p99_9 = [results['batch_benchmark'][str(bs)]['per_sample_latency_ms']['p99_9']
                                   for bs in batch_sizes]

                axes[0, 1].plot(batch_sizes, per_sample_p99_9, 'o-', linewidth=2, markersize=8)
                axes[0, 1].axhline(0.9, color='red', linestyle='--', label='Target (0.9ms)')
                axes[0, 1].set_xlabel('Batch Size')
                axes[0, 1].set_ylabel('Per-Sample P99.9 Latency (ms)')
                axes[0, 1].set_title('Latency vs Batch Size')
                axes[0, 1].legend()
                axes[0, 1].grid(True, alpha=0.3)

            # 3. Throughput
            if 'throughput_benchmark' in results:
                throughput = results['throughput_benchmark']['throughput_pps']
                axes[1, 0].bar(['Throughput'], [throughput], color='skyblue', edgecolor='black')
                axes[1, 0].set_ylabel('Predictions/Second')
                axes[1, 0].set_title('Model Throughput')
                axes[1, 0].grid(True, alpha=0.3)

                # Add text annotation
                axes[1, 0].text(0, throughput + throughput*0.05, f'{throughput:.0f} pps',
                               ha='center', va='bottom', fontweight='bold')

            # 4. Success criteria summary
            axes[1, 1].axis('off')
            if 'latency_benchmark' in results:
                criteria_text = "🎯 Success Criteria:\n\n"
                p99_9 = results['latency_benchmark']['latency_stats']['p99_9']
                success_rate = results['latency_benchmark']['success_rate']

                criteria_text += f"✅ P99.9 < 0.9ms: {p99_9:.3f}ms\n" if p99_9 < 0.9 else f"❌ P99.9 < 0.9ms: {p99_9:.3f}ms\n"
                criteria_text += f"✅ Success rate: {success_rate:.1f}%\n" if success_rate > 99 else f"❌ Success rate: {success_rate:.1f}%\n"

                if 'memory_benchmark' in results:
                    memory_mb = results['memory_benchmark']['memory_usage_mb']
                    criteria_text += f"ℹ️  Memory usage: {memory_mb:.1f}MB\n"

                if 'throughput_benchmark' in results:
                    throughput = results['throughput_benchmark']['throughput_pps']
                    criteria_text += f"ℹ️  Throughput: {throughput:.0f} pps\n"

                axes[1, 1].text(0.1, 0.8, criteria_text, fontsize=12, verticalalignment='top',
                               bbox=dict(boxstyle="round,pad=0.5", facecolor="lightgray", alpha=0.8))

            plt.tight_layout()

            # Save plot
            plot_filename = f"{self.model_name}_benchmark_plots.png"
            plt.savefig(plot_filename, dpi=300, bbox_inches='tight')
            plt.show()

            print(f"📊 Plots saved: {plot_filename}")

        except Exception as e:
            print(f"⚠️  Could not create visualizations: {e}")

def compare_models(model_paths: List[str]) -> None:
    """Compare multiple ONNX models"""
    print("🔄 Comparing multiple models...")

    all_results = {}

    for model_path in model_paths:
        print(f"\n{'='*50}")
        print(f"Benchmarking: {model_path}")
        print('='*50)

        try:
            benchmarker = ONNXBenchmarker(model_path)
            benchmarker.warmup(50)  # Reduced warmup for comparison

            # Quick benchmark
            latency_results = benchmarker.benchmark_latency(1000)  # Reduced samples
            throughput_results = benchmarker.benchmark_throughput(10)  # Reduced duration

            all_results[Path(model_path).stem] = {
                'latency': latency_results,
                'throughput': throughput_results
            }

        except Exception as e:
            print(f"❌ Failed to benchmark {model_path}: {e}")

    # Create comparison report
    if len(all_results) > 1:
        print(f"\n🏆 MODEL COMPARISON SUMMARY")
        print("="*60)

        print(f"{'Model':<20} {'P99.9 (ms)':<12} {'Throughput (pps)':<15} {'Sub-ms':<8}")
        print("-"*60)

        for model_name, results in all_results.items():
            p99_9 = results['latency']['latency_stats']['p99_9']
            throughput = results['throughput']['throughput_pps']
            sub_ms = "✅" if p99_9 < 1.0 else "❌"

            print(f"{model_name:<20} {p99_9:<12.3f} {throughput:<15.0f} {sub_ms:<8}")

        # Save comparison
        with open('model_comparison.json', 'w') as f:
            json.dump(all_results, f, indent=2)
        print(f"\n📄 Comparison saved: model_comparison.json")

def main():
    """Main entry point"""
    parser = argparse.ArgumentParser(description="ONNX Performance Benchmark for Temporal Neural Solver")
    parser.add_argument("model_path", help="Path to ONNX model file")
    parser.add_argument("--samples", type=int, default=10000, help="Number of latency samples")
    parser.add_argument("--throughput-duration", type=int, default=30, help="Throughput test duration (seconds)")
    parser.add_argument("--batch-sizes", nargs='+', type=int, default=[1, 2, 4, 8, 16], help="Batch sizes to test")
    parser.add_argument("--no-optimize", action="store_true", help="Disable ONNX optimizations")
    parser.add_argument("--compare", nargs='+', help="Compare multiple models")
    parser.add_argument("--output", help="Output report filename")
    parser.add_argument("--quick", action="store_true", help="Run quick benchmark (fewer samples)")

    args = parser.parse_args()

    if args.compare:
        compare_models(args.compare)
        return

    # Adjust parameters for quick mode
    if args.quick:
        args.samples = 1000
        args.throughput_duration = 10
        args.batch_sizes = [1, 4, 16]

    print("🚀 ONNX Performance Benchmark")
    print("="*50)
    print(f"Model: {args.model_path}")
    print(f"Samples: {args.samples}")
    print(f"Throughput duration: {args.throughput_duration}s")
    print(f"Batch sizes: {args.batch_sizes}")
    print()

    # Create benchmarker
    benchmarker = ONNXBenchmarker(args.model_path, optimize=not args.no_optimize)

    # Run warmup
    benchmarker.warmup()

    # Collect all results
    all_results = {}

    # 1. Latency benchmark
    all_results['latency_benchmark'] = benchmarker.benchmark_latency(args.samples)

    # 2. Throughput benchmark
    all_results['throughput_benchmark'] = benchmarker.benchmark_throughput(args.throughput_duration)

    # 3. Batch size benchmark
    all_results['batch_benchmark'] = benchmarker.benchmark_batch_sizes(args.batch_sizes)

    # 4. Memory benchmark
    all_results['memory_benchmark'] = benchmarker.memory_benchmark()

    # Create comprehensive report
    benchmarker.create_report(all_results, args.output)

    print("\n🎉 Benchmark complete!")
    print("\nKey Results:")
    print(f"   P99.9 Latency: {all_results['latency_benchmark']['latency_stats']['p99_9']:.3f}ms")
    print(f"   Throughput: {all_results['throughput_benchmark']['throughput_pps']:.0f} predictions/second")
    print(f"   Memory usage: {all_results['memory_benchmark']['memory_usage_mb']:.1f}MB")

    # Final success check
    p99_9_success = all_results['latency_benchmark']['latency_stats']['p99_9'] < 0.9
    print(f"\n🎯 Sub-millisecond target: {'✅ ACHIEVED' if p99_9_success else '❌ NOT MET'}")

if __name__ == "__main__":
    main()