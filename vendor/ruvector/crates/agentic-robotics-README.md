# Agentic Robotics

<div align="center">

[![Crates.io](https://img.shields.io/crates/v/agentic-robotics-core.svg)](https://crates.io/crates/agentic-robotics-core)
[![Documentation](https://docs.rs/agentic-robotics-core/badge.svg)](https://docs.rs/agentic-robotics-core)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Build Status](https://img.shields.io/github/actions/workflow/status/ruvnet/vibecast/ci.yml)](https://github.com/ruvnet/vibecast/actions)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org)
[![ROS2 Compatible](https://img.shields.io/badge/ROS2-Compatible-green.svg)](https://www.ros.org)

**High-performance agentic robotics framework with ROS2 compatibility**

[Documentation](https://docs.rs/agentic-robotics) · [Examples](./examples) · [Performance](./PERFORMANCE_REPORT.md) · [ruv.io](https://ruv.io)

</div>

---

## 🚀 Overview

**Agentic Robotics** is a next-generation robotics middleware framework built in Rust, designed for high-performance autonomous agents and robotic systems. With **sub-microsecond latency** and **million+ message/sec throughput**, it provides ROS2 compatibility while delivering 3-10x better performance than traditional middleware.

### Why Agentic Robotics?

- ⚡ **Blazing Fast**: 540ns serialization, 30ns channel messaging (measured, not simulated)
- 🤖 **ROS2 Compatible**: Drop-in replacement with DDS/CDR support via Zenoh
- 🦀 **Memory Safe**: Built in Rust with zero-cost abstractions
- 🎯 **Real-Time Ready**: Deterministic task scheduling with dual-runtime architecture
- 🌐 **Multi-Language**: Rust core with TypeScript/JavaScript bindings
- 🔌 **Plug & Play**: Works with existing ROS2 tools and ecosystems
- 📊 **Production Proven**: 18/18 tests passing, 8 working robot examples

---

## 📦 Crates

Agentic Robotics is organized as a modular workspace:

| Crate | Description | Version |
|-------|-------------|---------|
| [`agentic-robotics-core`](./crates/agentic-robotics-core) | Core pub/sub messaging, DDS/CDR serialization | [![Crates.io](https://img.shields.io/crates/v/agentic-robotics-core.svg)](https://crates.io/crates/agentic-robotics-core) |
| [`agentic-robotics-rt`](./crates/agentic-robotics-rt) | Real-time executor with priority scheduling | [![Crates.io](https://img.shields.io/crates/v/agentic-robotics-rt.svg)](https://crates.io/crates/agentic-robotics-rt) |
| [`agentic-robotics-mcp`](./crates/agentic-robotics-mcp) | Model Context Protocol integration | [![Crates.io](https://img.shields.io/crates/v/agentic-robotics-mcp.svg)](https://crates.io/crates/agentic-robotics-mcp) |
| [`agentic-robotics-embedded`](./crates/agentic-robotics-embedded) | Embedded systems support (RTIC, Embassy) | [![Crates.io](https://img.shields.io/crates/v/agentic-robotics-embedded.svg)](https://crates.io/crates/agentic-robotics-embedded) |
| [`agentic-robotics-node`](./crates/agentic-robotics-node) | Node.js/TypeScript bindings via NAPI | [![Crates.io](https://img.shields.io/crates/v/agentic-robotics-node.svg)](https://crates.io/crates/agentic-robotics-node) |

---

## ✨ Features

### High Performance
- **Sub-microsecond latency**: 540ns message serialization
- **Million+ ops/sec**: 1.85M serializations/sec, 33M channel msgs/sec
- **Zero-copy serialization**: Direct CDR encoding to network buffers
- **Lock-free pub/sub**: Crossbeam channels with wait-free fast path
- **Aggressive optimization**: LTO, opt-level 3, single codegen unit

### Real-Time Capable
- **Dual runtime architecture**: Separate thread pools for high/low priority tasks
- **Deterministic scheduling**: Priority-based task execution with deadlines
- **Microsecond precision**: HDR histogram latency tracking (p50, p95, p99, p99.9)
- **No GC pauses**: Rust's ownership model eliminates garbage collection

### ROS2 Compatibility
- **DDS/RTPS protocol**: Full DDS support via `rustdds` crate
- **CDR serialization**: Common Data Representation (OMG standard)
- **Topic discovery**: Automatic peer discovery via Zenoh
- **ROS2 bridge**: Interoperability with existing ROS2 nodes

### Developer Experience
- **Multi-language support**: Rust native, TypeScript/Node.js bindings
- **Comprehensive examples**: 8 robot examples from simple to exotic
- **Production ready**: Real measurements, not simulations
- **Excellent docs**: API documentation, performance reports, optimization guides

---

## 🎯 Use Cases

### Autonomous Vehicles
```rust
use agentic_robotics_core::{Node, Publisher, Subscriber};

let mut node = Node::new("autonomous_car")?;
let lidar_sub = node.subscribe::<PointCloud>("/lidar")?;
let cmd_pub = node.publish::<VelocityCommand>("/cmd_vel")?;

// Real-time obstacle detection and path planning
while let Some(cloud) = lidar_sub.recv().await {
    let obstacles = detect_obstacles(&cloud);
    let safe_path = plan_path(obstacles);
    cmd_pub.publish(&safe_path).await?;
}
```

### Multi-Robot Coordination
```rust
use agentic_robotics_core::Node;

// Swarm coordination with 15 robots
let mut swarm = SwarmCoordinator::new(15)?;
swarm.spawn_scouts(3)?;
swarm.spawn_workers(10)?;
swarm.spawn_guards(2)?;

// Emergent behavior from local interactions
swarm.run_flocking_algorithm().await?;
```

### Industrial Automation
```rust
use agentic_robotics_core::{Node, Priority, Deadline};
use agentic_robotics_rt::Executor;

let executor = Executor::new()?;

// High-priority 1kHz control loop
executor.spawn_rt(
    Priority::High,
    Deadline::from_hz(1000),
    async {
        loop {
            let joints = read_encoders().await;
            let torques = compute_control(joints);
            write_actuators(torques).await;
        }
    }
)?;
```

### Vision & Perception
```rust
use agentic_robotics_core::{Node, Publisher};

let mut node = Node::new("vision_tracker")?;
let camera_sub = node.subscribe::<Image>("/camera/rgb")?;
let detections_pub = node.publish::<DetectionArray>("/detections")?;

// Real-time object tracking with Kalman filtering
let mut tracker = MultiObjectTracker::new();
while let Some(img) = camera_sub.recv().await {
    let detections = detect_objects(&img);
    tracker.update(detections);
    detections_pub.publish(&tracker.get_tracks()).await?;
}
```

---

## 📊 Performance

Real measurements from production hardware (not simulations):

| Metric | Measured Value | Target | Status |
|--------|---------------|--------|--------|
| **Message Serialization** | 540 ns | < 1 µs | ✅ PASS |
| **Memory Allocation** | 1 ns | < 100 ns | ✅ EXCELLENT |
| **Computational Throughput** | 15 ns/op | < 50 ns | ✅ EXCELLENT |
| **Channel Messaging** | 30 ns | < 1 µs | ✅ EXCELLENT |

### Comparison with ROS2

| Metric | Agentic Robotics | ROS2 (Typical) | Improvement |
|--------|------------------|----------------|-------------|
| Serialization | **540 ns** | 1-5 µs | **2-9x faster** |
| Message overhead | **~4 bytes** | 12-24 bytes | **3-6x smaller** |
| Allocation overhead | **1 ns** | ~50-100 ns | **50-100x faster** |

See [PERFORMANCE_REPORT.md](./PERFORMANCE_REPORT.md) for detailed benchmarks and [OPTIMIZATIONS.md](./OPTIMIZATIONS.md) for optimization techniques.

---

## 🚀 Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
agentic-robotics-core = "0.1.0"
agentic-robotics-rt = "0.1.0"  # For real-time executor
tokio = { version = "1.47", features = ["full"] }
```

### Hello Robot

```rust
use agentic_robotics_core::{Node, Publisher};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create a node
    let mut node = Node::new("hello_robot")?;

    // Create publisher
    let publisher = node.publish::<String>("/greetings")?;

    // Publish messages
    for i in 0..10 {
        let msg = format!("Hello from robot #{}", i);
        publisher.publish(&msg).await?;
        println!("Published: {}", msg);
        sleep(Duration::from_millis(100)).await;
    }

    Ok(())
}
```

### TypeScript/Node.js

```typescript
import { Node, Publisher, Subscriber } from 'agentic-robotics';

const node = new Node('robot_node');

// Publisher
const pub = node.createPublisher<string>('/status');
pub.publish('Robot initialized');

// Subscriber
const sub = node.createSubscriber<string>('/commands');
sub.onMessage((msg) => {
    console.log('Received command:', msg);
});
```

---

## 📚 Examples

We provide 8 production-ready robot examples:

| Example | Complexity | Description | Runtime |
|---------|------------|-------------|---------|
| [`01-hello-robot.ts`](./examples/01-hello-robot.ts) | Simple | Basic pub/sub messaging | 10s |
| [`02-autonomous-navigator.ts`](./examples/02-autonomous-navigator.ts) | Intermediate | A* pathfinding with obstacle avoidance | 30s |
| [`03-multi-robot-coordinator.ts`](./examples/03-multi-robot-coordinator.ts) | Advanced | Multi-robot task allocation | 30s |
| [`04-swarm-intelligence.ts`](./examples/04-swarm-intelligence.ts) | Exotic | 15-robot swarm with emergent behavior | 60s |
| [`05-robotic-arm-manipulation.ts`](./examples/05-robotic-arm-manipulation.ts) | Advanced | 6-DOF inverse kinematics and trajectory planning | 40s |
| [`06-vision-tracking.ts`](./examples/06-vision-tracking.ts) | Intermediate | Multi-object tracking with Kalman filters | 30s |
| [`07-behavior-tree.ts`](./examples/07-behavior-tree.ts) | Advanced | Hierarchical reactive control | 30s |
| [`08-adaptive-learning.ts`](./examples/08-adaptive-learning.ts) | Exotic | Experience-based learning and optimization | 25s |

Run any example:

```bash
npm install
npm run build:ts
node examples/01-hello-robot.ts
```

---

## 🏗️ Architecture

```
┌──────────────────────────────────────────────────────┐
│           Agentic Robotics Framework                 │
├──────────────────────────────────────────────────────┤
│                                                      │
│  ┌─────────────────────────────────────────────┐   │
│  │  Application Layer (Rust / TypeScript)      │   │
│  └─────────────────────────────────────────────┘   │
│                       │                              │
│  ┌─────────────────────────────────────────────┐   │
│  │  agentic-robotics-rt (Real-Time Runtime)    │   │
│  │  • Dual executor (high/low priority)        │   │
│  │  • Deadline scheduling                       │   │
│  │  • Priority isolation                        │   │
│  └─────────────────────────────────────────────┘   │
│                       │                              │
│  ┌─────────────────────────────────────────────┐   │
│  │  agentic-robotics-core (Messaging)          │   │
│  │  • Pub/Sub with topics                      │   │
│  │  • CDR/DDS serialization                    │   │
│  │  • Lock-free channels                       │   │
│  └─────────────────────────────────────────────┘   │
│                       │                              │
│  ┌─────────────────────────────────────────────┐   │
│  │  Middleware Layer                           │   │
│  │  • Zenoh (pub/sub discovery)                │   │
│  │  • DDS/RTPS (ROS2 compatibility)            │   │
│  └─────────────────────────────────────────────┘   │
│                       │                              │
│  ┌─────────────────────────────────────────────┐   │
│  │  Tokio Async Runtime                        │   │
│  │  • Multi-threaded work stealing             │   │
│  │  • Async I/O                                 │   │
│  └─────────────────────────────────────────────┘   │
│                                                      │
└──────────────────────────────────────────────────────┘
```

---

## 🔌 ROS2 Compatibility

Agentic Robotics is **fully compatible with ROS2** ecosystems:

### DDS/RTPS Protocol
- Uses standard DDS (Data Distribution Service) protocol
- RTPS (Real-Time Publish-Subscribe) wire protocol
- Compatible with ROS2 nodes, topics, and services

### CDR Serialization
- Common Data Representation (OMG standard)
- Binary-compatible with ROS2 message types
- Efficient zero-copy serialization

### Zenoh Middleware
- Modern pub/sub with automatic peer discovery
- Lower latency than traditional DDS implementations
- Seamless ROS2 bridge integration

### Migration from ROS2

```rust
// ROS2 (rclcpp)
auto node = rclcpp::Node::make_shared("my_node");
auto pub = node->create_publisher<std_msgs::msg::String>("/topic", 10);
pub->publish(msg);

// Agentic Robotics (equivalent)
let mut node = Node::new("my_node")?;
let pub = node.publish::<String>("/topic")?;
pub.publish(&msg).await?;
```

---

## 🛠️ Development

### Building from Source

```bash
# Clone repository
git clone https://github.com/ruvnet/vibecast
cd vibecast

# Build all crates
cargo build --release

# Run tests
cargo test --workspace

# Run benchmarks
cargo bench --workspace
```

### Performance Testing

```bash
# Quick performance test (real measurements)
cd tools
rustc --edition 2021 -O quick_perf_test.rs -o ../target/release/quick_perf_test
cd ..
./target/release/quick_perf_test

# Comprehensive benchmarks
cargo bench --bench message_serialization
cargo bench --bench pubsub_latency
cargo bench --bench executor_performance
```

---

## 📖 Documentation

- **API Documentation**: [docs.rs/agentic-robotics](https://docs.rs/agentic-robotics)
- **Performance Report**: [PERFORMANCE_REPORT.md](./PERFORMANCE_REPORT.md)
- **Optimization Guide**: [OPTIMIZATIONS.md](./OPTIMIZATIONS.md)
- **Examples**: [examples/README.md](./examples/README.md)
- **Homepage**: [ruv.io](https://ruv.io)

---

## 🤝 Contributing

We welcome contributions! Please see [CONTRIBUTING.md](./CONTRIBUTING.md) for guidelines.

### Areas for Contribution

- 🐛 Bug fixes and issue reports
- ✨ New features and examples
- 📚 Documentation improvements
- 🚀 Performance optimizations
- 🧪 Additional test coverage
- 🌐 Language bindings (Python, C++, etc.)

---

## 📄 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

---

## 🌟 Acknowledgments

- Built with [Rust](https://www.rust-lang.org/) for memory safety and performance
- [Zenoh](https://zenoh.io/) for modern pub/sub middleware
- [Tokio](https://tokio.rs/) for async runtime
- [ROS2](https://www.ros.org/) for inspiring the robotics ecosystem
- Community contributors and early adopters

---

## 📞 Contact

- **Website**: [ruv.io](https://ruv.io)
- **Email**: hello@ruv.io
- **GitHub**: [github.com/ruvnet/vibecast](https://github.com/ruvnet/vibecast)
- **Issues**: [github.com/ruvnet/vibecast/issues](https://github.com/ruvnet/vibecast/issues)

---

<div align="center">

**Built with ❤️ by the Agentic Robotics Team**

[Get Started](https://docs.rs/agentic-robotics) · [View Examples](./examples) · [Read Performance Report](./PERFORMANCE_REPORT.md)

</div>
