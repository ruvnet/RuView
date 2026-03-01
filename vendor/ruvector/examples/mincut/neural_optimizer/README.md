# Neural Temporal Graph Optimization

This example demonstrates how to use simple neural networks to learn and predict optimal graph configurations over time for minimum cut problems.

## 🎯 What This Example Does

The neural optimizer learns from graph evolution history to predict which modifications will lead to better minimum cut values. It uses reinforcement learning principles to guide graph transformations.

## 🧠 Core Concepts

### 1. **Temporal Graph Optimization**

Graphs often evolve over time (social networks, infrastructure, etc.). The challenge is predicting how changes will affect properties like minimum cut:

```
Time t0: Graph A → mincut = 5.0
Time t1: Add edge (3,7) → mincut = 3.2  ✓ Better!
Time t2: Remove edge (1,4) → mincut = 8.1  ✗ Worse!
```

**Goal**: Learn which actions improve the mincut.

### 2. **Why Neural Networks?**

Graph optimization is **NP-hard** because:
- Combinatorially many possible modifications
- Non-linear relationship between structure and mincut
- Need to predict long-term effects

Neural networks can:
- **Learn patterns** from historical data
- **Generalize** to unseen graph configurations
- **Make fast predictions** without solving mincut repeatedly

### 3. **Reinforcement Learning Basics**

Our optimizer uses a simple RL approach:

```
State (S): Current graph features
  ├─ Node count
  ├─ Edge count
  ├─ Density
  └─ Average degree

Action (A): Graph modification
  ├─ Add random edge
  ├─ Remove random edge
  └─ Do nothing

Reward (R): Change in mincut quality
Policy (π): Neural network that chooses actions
Value (V): Neural network that predicts future mincut
```

**RL Loop**:
```
1. Observe current state S
2. Policy π predicts best action A
3. Apply action A to graph
4. Observe new mincut value R
5. Learn: Update π and V based on R
6. Repeat
```

### 4. **Simple Neural Network**

We implement a basic feedforward network **without external dependencies**:

```rust
Input Layer (4 features)
    ↓
Hidden Layer (8 neurons, ReLU activation)
    ↓
Output Layer (3 actions for policy, 1 value for predictor)
```

**Forward Pass**:
```
hidden = ReLU(input × W1 + b1)
output = hidden × W2 + b2
```

**Training**: Evolutionary strategy (mutation-based)
- Create population of networks with small random changes
- Evaluate fitness on training data
- Select best performer
- Repeat

## 🔍 How It Works

### Phase 1: Training Data Generation

```rust
// Generate random graphs and record their mincuts
for _ in 0..20 {
    let graph = generate_random_graph(10, 0.3);
    let mincut = calculate_mincut(&graph);
    optimizer.record_observation(&graph, mincut);
}
```

### Phase 2: Neural Network Training

```rust
// Train using evolutionary strategy
optimizer.train(generations: 50, population_size: 20);

// Each generation:
// 1. Create population by mutating current network
// 2. Evaluate fitness (prediction accuracy)
// 3. Select best network
```

### Phase 3: Optimization Loop

```rust
// Neural-guided optimization
for step in 0..30 {
    // 1. Extract features from current graph
    let features = extract_features(&graph);

    // 2. Policy network predicts best action
    let action = policy_network.forward(&features);

    // 3. Apply action (add/remove edge)
    apply_action(&mut graph, action);

    // 4. Calculate new mincut
    let mincut = calculate_mincut(&graph);

    // 5. Record for continuous learning
    optimizer.record_observation(&graph, mincut);
}
```

### Phase 4: Comparison

```rust
// Compare neural-guided vs random actions
Neural-Guided: Average mincut = 4.2
Random Baseline: Average mincut = 5.8
Improvement: 27.6%
```

## 🚀 Running the Example

```bash
# From the ruvector root directory
cargo run --example mincut_neural_optimizer --release -p ruvector-mincut

# Expected output:
# ╔════════════════════════════════════════════════════════════╗
# ║   Neural Temporal Graph Optimization Example              ║
# ║   Learning to Predict Optimal Graph Configurations        ║
# ╚════════════════════════════════════════════════════════════╝
#
# 📊 Initializing Neural Graph Optimizer
# 🔬 Generating Training Data
# 🧠 Training Neural Networks
# ⚖️  Comparing Optimization Strategies
# 📈 Results Comparison
# 🔮 Prediction vs Actual
```

**Note**: This example uses a simplified mincut approximation for demonstration purposes. In production, you would use the full `DynamicMinCut` algorithm from the `ruvector-mincut` crate. The approximation is based on graph statistics (minimum degree × average edge weight) to keep the example focused on neural optimization concepts without computational overhead.

## 📊 Key Components

### 1. **NeuralNetwork**

Simple feedforward network with:
- Linear transformations (matrix multiplication)
- ReLU activation
- Gradient-free optimization (evolutionary)

```rust
struct NeuralNetwork {
    weights_hidden: Vec<Vec<f64>>,
    bias_hidden: Vec<f64>,
    weights_output: Vec<Vec<f64>>,
    bias_output: Vec<f64>,
}
```

### 2. **NeuralGraphOptimizer**

Main optimizer combining:
- **Policy Network**: Decides which action to take
- **Value Network**: Predicts future mincut value
- **Training History**: Stores (state, mincut) pairs

```rust
struct NeuralGraphOptimizer {
    policy_network: NeuralNetwork,
    value_network: NeuralNetwork,
    history: Vec<(Vec<f64>, f64)>,
}
```

### 3. **Feature Extraction**

Converts graph to feature vector:

```rust
fn extract_features(graph: &Graph) -> Vec<f64> {
    vec![
        normalized_node_count,
        normalized_edge_count,
        graph_density,
        normalized_avg_degree,
    ]
}
```

## 🎓 Educational Insights

### Why This Matters

1. **Predictive Power**: Learn from past to predict future
2. **Computational Efficiency**: Fast predictions vs repeated mincut calculations
3. **Adaptive Strategy**: Improves with more data
4. **Transferable Knowledge**: Patterns learned generalize

### When to Use Neural Optimization

✅ **Good for**:
- Dynamic graphs that evolve over time
- Repeated optimization on similar graphs
- Need for fast approximate solutions
- Learning from historical patterns

❌ **Not ideal for**:
- One-time optimization (use exact algorithms)
- Very small graphs (overhead not worth it)
- Guaranteed optimal solutions required

### Limitations of This Simple Approach

1. **Linear Model**: Real problems may need deeper networks
2. **Gradient-Free Training**: Slower than gradient descent
3. **Feature Engineering**: Hand-crafted features may miss patterns
4. **Small Training Set**: More data = better predictions

### Extensions

**Easy Improvements**:
- Add more graph features (clustering coefficient, centrality)
- Larger networks (more layers, neurons)
- Better training (gradient descent with backpropagation)
- Experience replay (store and reuse good/bad examples)

**Advanced Extensions**:
- Graph Neural Networks (GNNs) for structure learning
- Deep Q-Learning with temporal difference
- Multi-agent optimization (parallel learners)
- Transfer learning across graph families

## 🔗 Related Examples

- `basic_mincut.rs` - Simple minimum cut calculation
- `comparative_algorithms.rs` - Compare different algorithms
- `real_world_networks.rs` - Apply to real network data

## 📚 Further Reading

### Reinforcement Learning
- **Sutton & Barto**: "Reinforcement Learning: An Introduction"
- **Policy Gradient Methods**: Learn action selection directly
- **Value Function Approximation**: Neural networks for RL

### Graph Optimization
- **Combinatorial Optimization**: NP-hard problems
- **Graph Neural Networks**: Deep learning on graphs
- **Temporal Networks**: Time-evolving graph analysis

### Minimum Cut Applications
- Network reliability
- Image segmentation
- Community detection
- Circuit design

## 💡 Key Takeaways

1. **Neural networks learn patterns** that guide graph optimization
2. **Simple linear models** can be effective for basic tasks
3. **Reinforcement learning** naturally fits sequential decision making
4. **Training on history** enables future prediction
5. **Evolutionary strategies** work without gradient computation

---

**Remember**: This is a pedagogical example showing concepts. Production systems would use more sophisticated techniques (deep learning libraries, gradient descent, GNNs), but the core ideas remain the same!
