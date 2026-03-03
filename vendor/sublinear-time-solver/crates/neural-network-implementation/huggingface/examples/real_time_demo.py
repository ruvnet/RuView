#!/usr/bin/env python3
"""
Real-Time Inference Demo for Temporal Neural Solver

This script demonstrates real-time inference capabilities of the Temporal Neural Solver,
simulating time-critical applications like HFT, robotics, and autonomous systems.
"""

import time
import threading
import queue
import argparse
import json
from pathlib import Path
from typing import Dict, List, Optional, Callable
from dataclasses import dataclass, field
from collections import deque
import warnings
warnings.filterwarnings('ignore')

try:
    import numpy as np
    import onnxruntime as ort
    import matplotlib.pyplot as plt
    from matplotlib.animation import FuncAnimation
    import pandas as pd
except ImportError as e:
    print(f"❌ Missing dependencies: {e}")
    print("Install with: pip install numpy onnxruntime matplotlib pandas")
    exit(1)

@dataclass
class RealTimeEvent:
    """Real-time event with timestamp and data"""
    timestamp: float
    data: np.ndarray
    event_id: int
    metadata: Dict = field(default_factory=dict)

@dataclass
class PredictionEvent:
    """Prediction result event"""
    timestamp: float
    prediction: np.ndarray
    latency_ms: float
    event_id: int
    success: bool
    metadata: Dict = field(default_factory=dict)

class RealTimeDataGenerator:
    """Generates realistic real-time data streams"""

    def __init__(self, frequency_hz: float = 100.0, noise_level: float = 0.1):
        self.frequency_hz = frequency_hz
        self.noise_level = noise_level
        self.start_time = time.time()
        self.event_counter = 0

    def generate_market_data(self) -> RealTimeEvent:
        """Generate synthetic high-frequency trading data"""
        current_time = time.time()
        elapsed = current_time - self.start_time

        # Simulate price movements with trend and volatility
        base_price = 100.0
        trend = 0.001 * elapsed  # Slow upward trend
        volatility = 0.5 * np.sin(2 * np.pi * elapsed * 0.1)  # Cyclical volatility
        noise = np.random.normal(0, self.noise_level)

        # Create OHLCV-like data over small time windows
        price = base_price + trend + volatility + noise
        volume = 1000 + 500 * np.abs(noise)

        # Simulate order book features
        bid_ask_spread = 0.01 + 0.005 * np.abs(noise)
        market_depth = 10000 + 2000 * noise

        data = np.array([price, volume, bid_ask_spread, market_depth], dtype=np.float32)

        self.event_counter += 1
        return RealTimeEvent(
            timestamp=current_time,
            data=data,
            event_id=self.event_counter,
            metadata={"type": "market_data", "symbol": "DEMO/USD"}
        )

    def generate_sensor_data(self) -> RealTimeEvent:
        """Generate synthetic robotics/autonomous vehicle sensor data"""
        current_time = time.time()
        elapsed = current_time - self.start_time

        # Simulate vehicle motion in a circular path
        angular_freq = 0.5  # rad/s
        radius = 5.0

        # Position
        x = radius * np.cos(angular_freq * elapsed) + np.random.normal(0, self.noise_level * 0.1)
        y = radius * np.sin(angular_freq * elapsed) + np.random.normal(0, self.noise_level * 0.1)

        # Velocity
        vx = -radius * angular_freq * np.sin(angular_freq * elapsed) + np.random.normal(0, self.noise_level * 0.05)
        vy = radius * angular_freq * np.cos(angular_freq * elapsed) + np.random.normal(0, self.noise_level * 0.05)

        data = np.array([x, y, vx, vy], dtype=np.float32)

        self.event_counter += 1
        return RealTimeEvent(
            timestamp=current_time,
            data=data,
            event_id=self.event_counter,
            metadata={"type": "sensor_data", "vehicle_id": "demo_vehicle"}
        )

    def generate_iot_data(self) -> RealTimeEvent:
        """Generate synthetic IoT edge device data"""
        current_time = time.time()
        elapsed = current_time - self.start_time

        # Simulate environmental sensors with daily patterns
        time_of_day = (elapsed % 86400) / 86400  # Normalize to [0, 1] for daily cycle

        # Temperature with daily cycle
        base_temp = 20.0
        daily_variation = 10.0 * np.sin(2 * np.pi * time_of_day - np.pi/2)  # Peak at noon
        temp = base_temp + daily_variation + np.random.normal(0, self.noise_level)

        # Humidity (inverse correlation with temperature)
        humidity = 60.0 - 0.5 * daily_variation + np.random.normal(0, self.noise_level * 5)

        # Light level (solar pattern)
        light = max(0, 1000 * np.sin(np.pi * time_of_day)) + np.random.normal(0, self.noise_level * 10)

        # Motion detection (binary with some activity patterns)
        motion_prob = 0.1 + 0.2 * np.sin(4 * np.pi * time_of_day)  # More active during day
        motion = 1.0 if np.random.random() < motion_prob else 0.0

        data = np.array([temp, humidity, light, motion], dtype=np.float32)

        self.event_counter += 1
        return RealTimeEvent(
            timestamp=current_time,
            data=data,
            event_id=self.event_counter,
            metadata={"type": "iot_data", "device_id": "demo_sensor"}
        )

class RealTimePredictor:
    """Real-time inference engine with sub-millisecond latency"""

    def __init__(self, model_path: str, sequence_length: int = 10):
        self.model_path = Path(model_path)
        self.sequence_length = sequence_length

        # Configure ONNX Runtime for minimal latency
        self.session_options = ort.SessionOptions()
        self.session_options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
        self.session_options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        self.session_options.intra_op_num_threads = 1

        # Load model
        if self.model_path.exists():
            self.session = ort.InferenceSession(
                str(self.model_path),
                sess_options=self.session_options,
                providers=['CPUExecutionProvider']
            )
            self.input_name = self.session.get_inputs()[0].name
            print(f"✅ Model loaded: {self.model_path}")
        else:
            print(f"⚠️  Model not found: {self.model_path}, using synthetic predictor")
            self.session = None

        # Sliding window buffer for sequence data
        self.data_buffer = deque(maxlen=self.sequence_length)

        # Performance tracking
        self.prediction_count = 0
        self.total_latency = 0.0
        self.max_latency = 0.0
        self.latency_history = deque(maxlen=1000)

    def add_data_point(self, data: np.ndarray) -> bool:
        """Add data point to sliding window buffer"""
        self.data_buffer.append(data)
        return len(self.data_buffer) == self.sequence_length

    def predict(self, event: RealTimeEvent) -> PredictionEvent:
        """Run real-time prediction on event data"""
        # Add to buffer
        buffer_ready = self.add_data_point(event.data)

        if not buffer_ready:
            # Not enough data yet - return dummy prediction
            return PredictionEvent(
                timestamp=time.time(),
                prediction=np.zeros(4, dtype=np.float32),
                latency_ms=0.0,
                event_id=event.event_id,
                success=False,
                metadata={"error": "insufficient_data"}
            )

        # Prepare input sequence
        sequence = np.array(list(self.data_buffer), dtype=np.float32)
        sequence = sequence.reshape(1, self.sequence_length, -1)

        # Run inference with timing
        start_time = time.perf_counter()

        try:
            if self.session is not None:
                # Real model inference
                outputs = self.session.run(None, {self.input_name: sequence})
                prediction = outputs[0][0]
            else:
                # Synthetic prediction for demo
                time.sleep(0.0008)  # Simulate 0.8ms processing time
                prediction = sequence[0, -1] + np.random.normal(0, 0.01, 4)

            end_time = time.perf_counter()
            latency_ms = (end_time - start_time) * 1000

            # Update statistics
            self.prediction_count += 1
            self.total_latency += latency_ms
            self.max_latency = max(self.max_latency, latency_ms)
            self.latency_history.append(latency_ms)

            return PredictionEvent(
                timestamp=end_time,
                prediction=prediction,
                latency_ms=latency_ms,
                event_id=event.event_id,
                success=True,
                metadata={
                    "avg_latency_ms": self.total_latency / self.prediction_count,
                    "max_latency_ms": self.max_latency
                }
            )

        except Exception as e:
            end_time = time.perf_counter()
            latency_ms = (end_time - start_time) * 1000

            return PredictionEvent(
                timestamp=end_time,
                prediction=np.zeros(4, dtype=np.float32),
                latency_ms=latency_ms,
                event_id=event.event_id,
                success=False,
                metadata={"error": str(e)}
            )

    def get_statistics(self) -> Dict:
        """Get current performance statistics"""
        if self.prediction_count == 0:
            return {"predictions": 0}

        latencies = list(self.latency_history)
        return {
            "predictions": self.prediction_count,
            "avg_latency_ms": self.total_latency / self.prediction_count,
            "max_latency_ms": self.max_latency,
            "current_p99_ms": np.percentile(latencies, 99) if latencies else 0,
            "current_p99_9_ms": np.percentile(latencies, 99.9) if latencies else 0,
            "sub_millisecond_rate": (np.array(latencies) < 1.0).mean() * 100 if latencies else 0,
        }

class RealTimeSimulator:
    """Real-time inference simulation orchestrator"""

    def __init__(
        self,
        model_path: str,
        scenario: str = "market",
        frequency_hz: float = 100.0,
        duration_seconds: float = 60.0
    ):
        self.scenario = scenario
        self.frequency_hz = frequency_hz
        self.duration_seconds = duration_seconds

        # Initialize components
        self.data_generator = RealTimeDataGenerator(frequency_hz)
        self.predictor = RealTimePredictor(model_path)

        # Event queues
        self.input_queue = queue.Queue(maxsize=1000)
        self.output_queue = queue.Queue(maxsize=1000)

        # Monitoring
        self.events_processed = 0
        self.simulation_start_time = None
        self.running = False

        # Results storage
        self.results = {
            "events": [],
            "predictions": [],
            "statistics": {}
        }

    def data_producer_thread(self):
        """Producer thread generating real-time data"""
        interval = 1.0 / self.frequency_hz

        while self.running:
            start_time = time.time()

            # Generate data based on scenario
            if self.scenario == "market":
                event = self.data_generator.generate_market_data()
            elif self.scenario == "robotics":
                event = self.data_generator.generate_sensor_data()
            elif self.scenario == "iot":
                event = self.data_generator.generate_iot_data()
            else:
                event = self.data_generator.generate_sensor_data()

            try:
                self.input_queue.put(event, timeout=0.001)
            except queue.Full:
                print("⚠️  Input queue full, dropping event")

            # Maintain frequency
            elapsed = time.time() - start_time
            sleep_time = max(0, interval - elapsed)
            if sleep_time > 0:
                time.sleep(sleep_time)

    def inference_consumer_thread(self):
        """Consumer thread running inference"""
        while self.running:
            try:
                event = self.input_queue.get(timeout=0.1)
                prediction = self.predictor.predict(event)
                self.output_queue.put(prediction)
                self.events_processed += 1

                # Check for latency violations
                if prediction.success and prediction.latency_ms > 1.0:
                    print(f"⚠️  Latency violation: {prediction.latency_ms:.3f}ms")

            except queue.Empty:
                continue

    def monitoring_thread(self):
        """Monitoring thread for real-time statistics"""
        while self.running:
            time.sleep(1.0)  # Update every second

            stats = self.predictor.get_statistics()
            elapsed = time.time() - self.simulation_start_time

            print(f"📊 [{elapsed:6.1f}s] Events: {self.events_processed:5d} | "
                  f"Avg: {stats.get('avg_latency_ms', 0):.3f}ms | "
                  f"P99.9: {stats.get('current_p99_9_ms', 0):.3f}ms | "
                  f"Sub-ms: {stats.get('sub_millisecond_rate', 0):.1f}%")

    def run_simulation(self):
        """Run the complete real-time simulation"""
        print(f"🚀 Starting real-time simulation")
        print(f"   Scenario: {self.scenario}")
        print(f"   Frequency: {self.frequency_hz} Hz")
        print(f"   Duration: {self.duration_seconds}s")
        print(f"   Expected events: {int(self.frequency_hz * self.duration_seconds)}")
        print()

        self.simulation_start_time = time.time()
        self.running = True

        # Start threads
        producer = threading.Thread(target=self.data_producer_thread, daemon=True)
        consumer = threading.Thread(target=self.inference_consumer_thread, daemon=True)
        monitor = threading.Thread(target=self.monitoring_thread, daemon=True)

        producer.start()
        consumer.start()
        monitor.start()

        # Run for specified duration
        time.sleep(self.duration_seconds)

        # Stop simulation
        self.running = False
        print("\n🛑 Stopping simulation...")

        # Wait for threads to finish
        producer.join(timeout=1.0)
        consumer.join(timeout=1.0)
        monitor.join(timeout=1.0)

        # Collect final results
        final_stats = self.predictor.get_statistics()
        self.results["statistics"] = final_stats

        print("\n✅ Simulation complete!")
        return self.results

    def print_summary(self):
        """Print simulation summary"""
        stats = self.results["statistics"]

        print("\n📋 REAL-TIME SIMULATION SUMMARY")
        print("=" * 50)
        print(f"Scenario: {self.scenario}")
        print(f"Events processed: {self.events_processed}")
        print(f"Total predictions: {stats.get('predictions', 0)}")
        print(f"Processing rate: {stats.get('predictions', 0) / self.duration_seconds:.1f} pps")

        print(f"\n⏱️  Latency Performance:")
        print(f"   Average: {stats.get('avg_latency_ms', 0):.3f}ms")
        print(f"   Maximum: {stats.get('max_latency_ms', 0):.3f}ms")
        print(f"   P99: {stats.get('current_p99_ms', 0):.3f}ms")
        print(f"   P99.9: {stats.get('current_p99_9_ms', 0):.3f}ms")

        print(f"\n🎯 Success Criteria:")
        sub_ms_rate = stats.get('sub_millisecond_rate', 0)
        p99_9 = stats.get('current_p99_9_ms', 0)

        print(f"   Sub-millisecond rate: {sub_ms_rate:.1f}% {'✅' if sub_ms_rate > 95 else '❌'}")
        print(f"   P99.9 < 1.0ms: {p99_9:.3f}ms {'✅' if p99_9 < 1.0 else '❌'}")
        print(f"   P99.9 < 0.9ms: {p99_9:.3f}ms {'✅' if p99_9 < 0.9 else '❌'}")

        # Application-specific metrics
        if self.scenario == "market":
            print(f"\n💰 HFT Application:")
            print(f"   Decision latency: {p99_9:.3f}ms")
            print(f"   Market opportunity: {'✅ Captured' if p99_9 < 0.5 else '⚠️  Marginal' if p99_9 < 1.0 else '❌ Missed'}")

        elif self.scenario == "robotics":
            print(f"\n🤖 Robotics Application:")
            print(f"   Control loop latency: {p99_9:.3f}ms")
            print(f"   Real-time control: {'✅ Achieved' if p99_9 < 1.0 else '❌ Failed'}")

        elif self.scenario == "iot":
            print(f"\n📱 IoT Edge Application:")
            print(f"   Edge inference: {p99_9:.3f}ms")
            print(f"   Battery efficient: {'✅ Yes' if stats.get('avg_latency_ms', 0) < 0.5 else '⚠️  Moderate'}")

def create_live_visualization(simulator: RealTimeSimulator):
    """Create live visualization of real-time inference"""
    try:
        fig, axes = plt.subplots(2, 2, figsize=(12, 8))
        fig.suptitle(f'Real-Time Inference Monitor - {simulator.scenario.title()}', fontsize=14)

        # Data storage for plotting
        times = deque(maxlen=200)
        latencies = deque(maxlen=200)
        predictions = deque(maxlen=200)
        throughput = deque(maxlen=200)

        def update_plots(frame):
            if not simulator.running:
                return

            current_time = time.time() - simulator.simulation_start_time
            stats = simulator.predictor.get_statistics()

            # Update data
            times.append(current_time)
            latencies.append(stats.get('current_p99_9_ms', 0))
            throughput.append(stats.get('predictions', 0) / max(current_time, 1))

            # Get recent prediction if available
            try:
                prediction = simulator.output_queue.get_nowait()
                predictions.append(np.mean(prediction.prediction))
            except queue.Empty:
                if predictions:
                    predictions.append(predictions[-1])
                else:
                    predictions.append(0)

            # Clear and redraw plots
            for ax in axes.flat:
                ax.clear()

            # Plot 1: Latency over time
            if times and latencies:
                axes[0, 0].plot(list(times), list(latencies), 'b-', alpha=0.7)
                axes[0, 0].axhline(y=1.0, color='r', linestyle='--', label='1ms target')
                axes[0, 0].axhline(y=0.9, color='g', linestyle='--', label='0.9ms target')
                axes[0, 0].set_ylabel('P99.9 Latency (ms)')
                axes[0, 0].set_title('Latency Performance')
                axes[0, 0].legend()
                axes[0, 0].grid(True, alpha=0.3)

            # Plot 2: Throughput
            if times and throughput:
                axes[0, 1].plot(list(times), list(throughput), 'g-', alpha=0.7)
                axes[0, 1].set_ylabel('Predictions/Second')
                axes[0, 1].set_title('Throughput')
                axes[0, 1].grid(True, alpha=0.3)

            # Plot 3: Prediction values
            if times and predictions:
                axes[1, 0].plot(list(times), list(predictions), 'orange', alpha=0.7)
                axes[1, 0].set_ylabel('Prediction Value')
                axes[1, 0].set_title('Prediction Trend')
                axes[1, 0].grid(True, alpha=0.3)

            # Plot 4: Statistics summary
            axes[1, 1].axis('off')
            if stats:
                stats_text = f"""Real-Time Statistics:

Events: {simulator.events_processed:,}
Predictions: {stats.get('predictions', 0):,}
Avg Latency: {stats.get('avg_latency_ms', 0):.3f}ms
Max Latency: {stats.get('max_latency_ms', 0):.3f}ms
P99.9 Latency: {stats.get('current_p99_9_ms', 0):.3f}ms
Sub-ms Rate: {stats.get('sub_millisecond_rate', 0):.1f}%

Status: {'🟢 REAL-TIME' if stats.get('current_p99_9_ms', 0) < 1.0 else '🟡 MARGINAL' if stats.get('current_p99_9_ms', 0) < 2.0 else '🔴 DELAYED'}"""

                axes[1, 1].text(0.1, 0.9, stats_text, fontsize=10, verticalalignment='top',
                               fontfamily='monospace')

            plt.tight_layout()

        # Create animation
        ani = FuncAnimation(fig, update_plots, interval=100, cache_frame_data=False)
        return ani

    except Exception as e:
        print(f"⚠️  Visualization failed: {e}")
        return None

def main():
    """Main entry point for real-time demo"""
    parser = argparse.ArgumentParser(description="Real-Time Inference Demo")
    parser.add_argument("--model", default="system_b.onnx", help="Path to ONNX model")
    parser.add_argument("--scenario", choices=["market", "robotics", "iot"], default="market",
                       help="Application scenario")
    parser.add_argument("--frequency", type=float, default=100.0, help="Data frequency (Hz)")
    parser.add_argument("--duration", type=float, default=30.0, help="Simulation duration (seconds)")
    parser.add_argument("--visualize", action="store_true", help="Show live visualization")
    parser.add_argument("--save-results", help="Save results to JSON file")

    args = parser.parse_args()

    print("⚡ Temporal Neural Solver - Real-Time Inference Demo")
    print("=" * 60)

    # Create simulator
    simulator = RealTimeSimulator(
        model_path=args.model,
        scenario=args.scenario,
        frequency_hz=args.frequency,
        duration_seconds=args.duration
    )

    # Setup visualization if requested
    animation = None
    if args.visualize:
        try:
            animation = create_live_visualization(simulator)
            # Start visualization in background
            plt.ion()
            plt.show()
        except Exception as e:
            print(f"⚠️  Visualization setup failed: {e}")

    # Run simulation
    try:
        results = simulator.run_simulation()
        simulator.print_summary()

        # Save results if requested
        if args.save_results:
            with open(args.save_results, 'w') as f:
                json.dump(results, f, indent=2, default=str)
            print(f"\n📄 Results saved: {args.save_results}")

        # Keep visualization alive
        if animation and args.visualize:
            print("\n🖥️  Close the plot window to exit...")
            plt.ioff()
            plt.show()

    except KeyboardInterrupt:
        print("\n⚠️  Simulation interrupted by user")
        simulator.running = False

    except Exception as e:
        print(f"\n❌ Simulation failed: {e}")
        import traceback
        traceback.print_exc()

    print("\n🎉 Real-time demo complete!")

if __name__ == "__main__":
    main()