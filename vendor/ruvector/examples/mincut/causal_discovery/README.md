# Temporal Causal Discovery in Networks

This example demonstrates **causal inference** in dynamic graph networks — discovering which events *cause* other events, not just correlate with them.

## 🎯 What This Example Does

1. **Tracks Network Events**: Records timestamped events (edge cuts, mincut changes, partitions)
2. **Discovers Causality**: Identifies patterns like "Edge cut → MinCut drop (within 100ms)"
3. **Builds Causal Graph**: Shows relationships between event types
4. **Predicts Future Events**: Uses learned patterns to forecast what happens next
5. **Analyzes Latency**: Measures delays between causes and effects

## 🧠 Core Concepts

### Correlation vs Causation

**Correlation** means two things happen together:
- Ice cream sales and drownings both increase in summer
- They're correlated but neither *causes* the other

**Causation** means one thing *makes* another happen:
- Cutting a critical edge *causes* the minimum cut to change
- Temporal ordering matters: causes precede effects

### Granger Causality

Named after economist Clive Granger (Nobel Prize 2003), this concept defines causality based on *predictive power*:

> **Event X "Granger-causes" Y if:**
> 1. X occurs before Y (temporal precedence)
> 2. Past values of X improve prediction of Y
> 3. This relationship is statistically significant

**Example in our network:**
```
EdgeCut(1,3) ──[30ms]──> MinCutChange
                ↓
   "Cutting edge (1,3) causes mincut to drop 30ms later"
```

**How we detect it:**
- Track all events with precise timestamps
- For each effect, look backwards in time for potential causes
- Count how often pattern repeats
- Measure consistency of delay
- Calculate confidence score

### Temporal Window

We use a **causality window** (default: 200ms) to limit how far back we search:

```
[------- 200ms window -------]
    ↑                      ↑
 Cause                  Effect
```

- Events within window: potential causal relationship
- Events outside window: too distant to be direct cause
- Adjustable based on your system's dynamics

## 🔍 How It Works

### 1. Event Recording

Every network operation records an event:

```rust
enum NetworkEvent {
    EdgeCut(from, to, timestamp),
    MinCutChange(new_value, timestamp),
    PartitionChange(set_a, set_b, timestamp),
    NodeIsolation(node_id, timestamp),
}
```

### 2. Causality Detection

For each event, we look backwards to find causes:

```
Time:     T=0ms    T=30ms    T=60ms    T=90ms
Event:    EdgeCut  ------->  MinCut    ------->  Partition
          (1,3)              drops                 changes

Analysis:
- EdgeCut ──[30ms]──> MinCutChange (cause-effect found!)
- MinCutChange ──[30ms]──> PartitionChange (another pattern!)
```

### 3. Confidence Calculation

Confidence score combines:
- **Occurrence frequency**: How often effect follows cause
- **Timing consistency**: How stable the delay is

```rust
confidence = 0.7 * (occurrences / total_effects)
           + 0.3 * (1 / timing_variance)
```

Higher confidence = more reliable causal relationship.

### 4. Prediction

Based on recent events, predict what happens next:

```
Recent events: EdgeCut(2,4)
Known pattern: EdgeCut ──[40ms]──> PartitionChange (80% confidence)
Prediction:    PartitionChange expected in ~40ms
```

## 📊 Output Explained

### Event Timeline
```
T+    0ms: MinCutChange - MinCut=9.00
T+   50ms: EdgeCut - Edge(1, 3)
T+   80ms: MinCutChange - MinCut=7.00
```
Shows chronological event sequence with timestamps.

### Causal Graph
```
EdgeCut ──[35ms]──> MinCutChange (confidence: 85%, n=3)
  └─ Delay range: 30ms - 45ms

EdgeCut ──[50ms]──> NodeIsolation (confidence: 62%, n=2)
  └─ Delay range: 45ms - 55ms
```

Reads as: "EdgeCut causes MinCutChange after 35ms on average, observed 3 times with 85% confidence"

### Predictions
```
1. PartitionChange in ~40ms (confidence: 75%)
2. MinCutChange in ~35ms (confidence: 68%)
```

Based on current events, what's likely to happen next.

## 🚀 Running the Example

```bash
cd /home/user/ruvector
cargo run --example mincut_causal_discovery
```

Or with optimizations:
```bash
cargo run --release --example mincut_causal_discovery
```

## 🎓 Practical Applications

### 1. **Network Failure Prediction**
- Learn: "When switch X fails, router Y fails within 500ms"
- Predict: Switch X just failed → proactively reroute traffic from Y

### 2. **Distributed System Debugging**
- Track: Service timeouts, database locks, cache misses
- Discover: "Cache miss → DB lock → timeout cascade"
- Fix: Optimize cache hit rate to prevent cascades

### 3. **Performance Optimization**
- Identify: Which operations cause bottlenecks?
- Example: "Large query → memory spike → GC pause → latency spike"
- Optimize: Cache large queries to break causal chain

### 4. **Anomaly Detection**
- Learn normal causal patterns
- Alert when unusual pattern appears
- Example: "MinCut changed but no edge was cut!" (security breach?)

### 5. **Capacity Planning**
- Predict: "Current load increase → server failure in 2 hours"
- Action: Scale proactively before failure

## 🔧 Customization

### Adjust Causality Window
```rust
let mut analyzer = CausalNetworkAnalyzer::new();
analyzer.causality_window = Duration::from_millis(500); // Longer window
```

### Change Confidence Threshold
```rust
analyzer.confidence_threshold = 0.5; // Require 50% confidence (stricter)
```

### Track Custom Events
```rust
enum NetworkEvent {
    // Add your own event types
    CustomEvent(String, Instant),
    // ...existing types...
}
```

## 📚 Further Reading

1. **Granger Causality**:
   - Original paper: Granger, C.W.J. (1969). "Investigating Causal Relations by Econometric Models"
   - Applied to time series forecasting

2. **Causal Inference**:
   - Pearl, J. (2009). "Causality: Models, Reasoning, and Inference"
   - Gold standard for causal reasoning

3. **Network Dynamics**:
   - Barabási, A.L. "Network Science" (free online)
   - Chapter on temporal networks

4. **Practical Systems**:
   - Google's "Borgmon" and causal analysis for datacenter monitoring
   - Netflix's chaos engineering and failure causality

## ⚠️ Limitations

1. **Correlation ≠ Causation**: Our algorithm detects temporal correlation. True causation requires domain knowledge.

2. **Confounding Variables**: A third event C might cause both A and B, making them appear causally related.

3. **Feedback Loops**: A causes B causes A (circular). Our simple model doesn't handle these well.

4. **Statistical Significance**: Small sample sizes may show spurious patterns. Need sufficient data.

## 🎯 Key Takeaways

- ✅ **Temporal ordering** is crucial: causes precede effects
- ✅ **Consistency** matters: reliable patterns have stable delays
- ✅ **Prediction** is the test: if knowing X helps predict Y, X may cause Y
- ✅ **Context** is king: domain knowledge validates statistical findings
- ⚠️ **Correlation ≠ Causation**: always verify with experiments

---

**Pro tip**: Use this with the incremental minimum cut example to track how the cut evolves over time and predict critical changes before they happen!
