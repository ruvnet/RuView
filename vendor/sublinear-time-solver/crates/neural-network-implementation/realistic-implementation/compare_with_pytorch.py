#!/usr/bin/env python3
"""
Compare our Rust implementation with established PyTorch models
This provides ground truth for realistic performance expectations
"""

import time
import numpy as np
import torch
import torch.nn as nn
from typing import List, Tuple
import json

class SimpleGRU(nn.Module):
    """Small GRU network matching the paper's specification"""
    def __init__(self, input_size=128, hidden_size=32, output_size=4):
        super().__init__()
        self.gru = nn.GRU(input_size, hidden_size, batch_first=True)
        self.fc = nn.Linear(hidden_size, output_size)

    def forward(self, x):
        # x shape: (batch, input_size) -> need to add seq dimension
        x = x.unsqueeze(1)  # (batch, 1, input_size)
        out, _ = self.gru(x)
        # Take the single timestep output
        out = self.fc(out.squeeze(1))
        return out

class SimpleTCN(nn.Module):
    """Temporal Convolutional Network for comparison"""
    def __init__(self, input_size=128, hidden_size=32, output_size=4):
        super().__init__()
        self.conv1 = nn.Conv1d(1, hidden_size, kernel_size=3, padding=1)
        self.conv2 = nn.Conv1d(hidden_size, hidden_size, kernel_size=3, padding=1)
        self.fc = nn.Linear(hidden_size * input_size, output_size)
        self.relu = nn.ReLU()

    def forward(self, x):
        # x shape: (batch, input_size)
        x = x.unsqueeze(1)  # Add channel dimension
        x = self.relu(self.conv1(x))
        x = self.relu(self.conv2(x))
        x = x.flatten(1)
        return self.fc(x)

class SimpleFeedforward(nn.Module):
    """Simple 2-layer network like our Rust implementation"""
    def __init__(self, input_size=128, hidden_size=32, output_size=4):
        super().__init__()
        self.fc1 = nn.Linear(input_size, hidden_size)
        self.fc2 = nn.Linear(hidden_size, output_size)
        self.relu = nn.ReLU()

    def forward(self, x):
        x = self.relu(self.fc1(x))
        return self.fc2(x)

def benchmark_model(model: nn.Module, input_size: int, iterations: int = 1000) -> dict:
    """Benchmark a PyTorch model with realistic timing"""
    model.eval()

    # Warmup
    with torch.no_grad():
        dummy_input = torch.randn(1, input_size)
        for _ in range(100):
            _ = model(dummy_input)

    # Actual benchmark
    timings = []
    with torch.no_grad():
        for _ in range(iterations):
            input_tensor = torch.randn(1, input_size)

            start = time.perf_counter()
            output = model(input_tensor)
            # Force computation to complete
            _ = output.cpu().numpy()
            end = time.perf_counter()

            timings.append((end - start) * 1000)  # Convert to milliseconds

    timings.sort()
    return {
        "p50": timings[len(timings) // 2],
        "p90": timings[int(len(timings) * 0.9)],
        "p99": timings[int(len(timings) * 0.99)],
        "p999": timings[int(len(timings) * 0.999)],
        "average": np.mean(timings),
        "std": np.std(timings),
        "min": min(timings),
        "max": max(timings)
    }

def main():
    print("=" * 60)
    print("PyTorch Baseline Performance Comparison")
    print("=" * 60)
    print("\nTesting on CPU with models matching paper specification:")
    print("- Input size: 128")
    print("- Hidden size: 32")
    print("- Output size: 4")
    print("- Iterations: 1000")
    print()

    results = {}

    # Test each model type
    models = {
        "Feedforward (2-layer)": SimpleFeedforward(),
        "GRU (1-layer)": SimpleGRU(),
        "TCN (2-layer)": SimpleTCN()
    }

    for name, model in models.items():
        print(f"Benchmarking {name}...")
        stats = benchmark_model(model, input_size=128)
        results[name] = stats

        print(f"  P50:   {stats['p50']:.3f}ms")
        print(f"  P90:   {stats['p90']:.3f}ms")
        print(f"  P99:   {stats['p99']:.3f}ms")
        print(f"  P99.9: {stats['p999']:.3f}ms")
        print(f"  Avg:   {stats['average']:.3f}ms ± {stats['std']:.3f}ms")
        print(f"  Range: {stats['min']:.3f}ms - {stats['max']:.3f}ms")
        print()

    print("=" * 60)
    print("Analysis:")
    print("=" * 60)
    print()

    # Check if <0.9ms is realistic
    min_p999 = min(results[m]['p999'] for m in results)
    print(f"Best P99.9 latency achieved: {min_p999:.3f}ms")

    if min_p999 < 0.9:
        print("✅ Sub-0.9ms P99.9 latency ACHIEVED!")
    else:
        print(f"❌ Sub-0.9ms P99.9 latency NOT achieved")
        print(f"   Gap to target: {min_p999 - 0.9:.3f}ms")

    print()
    print("Realistic expectations for CPU inference:")
    print("- Simple feedforward: 0.5-5ms typically")
    print("- Small GRU: 2-10ms typically")
    print("- Small TCN: 1-8ms typically")
    print()
    print("Sub-millisecond (<1ms) is extremely challenging on CPU")
    print("Sub-0.9ms specifically would require:")
    print("- Highly optimized C/Rust implementation")
    print("- Quantization (INT8 or lower)")
    print("- Model pruning/distillation")
    print("- Hardware acceleration (GPU/TPU/NPU)")

    # Save results
    with open('pytorch_baseline_results.json', 'w') as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to pytorch_baseline_results.json")

if __name__ == "__main__":
    # Check PyTorch version
    print(f"PyTorch version: {torch.__version__}")
    print(f"Using device: CPU")
    print()

    main()