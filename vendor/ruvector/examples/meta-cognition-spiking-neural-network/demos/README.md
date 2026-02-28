# AgentDB Comprehensive Demonstrations

This directory contains a comprehensive exploration of AgentDB's capabilities, showcasing the full power of the 2.0.0-alpha.2.11 release.

## 🎯 What's Included

### 1. Vector Search (`vector-search/`)
**File**: `semantic-search.js`

Demonstrates AgentDB's blazing-fast vector search capabilities using RuVector:
- **150x faster** than cloud-based alternatives
- Sub-millisecond query latency
- Semantic similarity search
- Filtered search by metadata
- Performance benchmarking

**Key Features**:
- HNSW indexing
- Cosine similarity
- Real-time search
- Native Rust performance

**Run it**:
```bash
node demos/vector-search/semantic-search.js
```

### 2. Attention Mechanisms (`attention/`)
**File**: `all-mechanisms.js`

Showcases all 5 attention mechanisms included in AgentDB:

1. **Multi-Head Attention** - Standard transformer attention with parallel heads
2. **Flash Attention** - Memory-efficient block-wise computation
3. **Linear Attention** - O(N) complexity using kernel approximations
4. **Hyperbolic Attention** - Poincaré ball model for hierarchical data
5. **MoE Attention** - Mixture of Experts with dynamic routing

**Key Features**:
- All 5 mechanisms demonstrated
- Performance comparisons
- Use case explanations
- Expert routing visualization

**Run it**:
```bash
node demos/attention/all-mechanisms.js
```

### 3. Self-Discovery System (`self-discovery/`)
**File**: `cognitive-explorer.js`

A cognitive system that autonomously explores its own capabilities:

**What It Does**:
- Discovers and catalogs its own abilities
- Stores discoveries in semantic memory
- Reflects on performance patterns
- Builds hierarchical knowledge graphs
- Generates insights from experience

**Cognitive Patterns Demonstrated**:
- Self-awareness through performance monitoring
- Pattern recognition across discoveries
- Hierarchical knowledge organization
- Continuous learning and improvement
- Meta-cognition (thinking about thinking)

**Key Features**:
- Autonomous exploration
- Semantic memory storage
- Knowledge graph construction
- Performance analysis
- Insight generation

**Run it**:
```bash
node demos/self-discovery/cognitive-explorer.js
```

## 🚀 Quick Start

### Run All Demonstrations
```bash
# Make the runner executable
chmod +x demos/run-all.js

# Run all demos in sequence
node demos/run-all.js
```

This will execute all demonstrations automatically, showing you the full capabilities of AgentDB.

### Run Individual Demos
```bash
# Vector search only
node demos/vector-search/semantic-search.js

# Attention mechanisms only
node demos/attention/all-mechanisms.js

# Self-discovery system only
node demos/self-discovery/cognitive-explorer.js
```

## 📊 Expected Output

### Vector Search Demo
```
🔎 AgentDB Vector Search Demonstration
======================================================================

📚 Creating Vector Database...

✅ Vector database created with 128 dimensions
📊 Using RuVector (Rust backend) - 150x faster than cloud alternatives

📝 Indexing documents...
  ✓ Indexed: Introduction to Neural Networks (AI)
  ...

🔍 Running Semantic Search Queries...

📊 Performance Statistics:
  Average Search Latency: 0.234ms
  Queries per Second: 4273
```

### Attention Mechanisms Demo
```
🧠 AgentDB Attention Mechanisms Demonstration
======================================================================

🔵 1. DOT PRODUCT ATTENTION (Basic)
✅ Initialized Dot Product Attention
⚡ Computation time: 1.234ms

🔵 2. MULTI-HEAD ATTENTION (Standard Transformer)
✅ Initialized with 4 attention heads
⚡ Computation time: 2.456ms
...
```

### Self-Discovery Demo
```
🧠 AgentDB Self-Discovery System
======================================================================

🚀 Beginning Self-Discovery Process...

🔍 Exploring: Vector Search
✅ Discovery recorded: Vector Search
   Duration: 1.234ms
   Category: Core Systems

🤔 SELF-REFLECTION: Analyzing Discoveries
📊 Total Discoveries: 6
✅ Successful: 6

💡 Generated Insights:
   1. Average capability execution: 2.145ms
   2. Fastest category: Core Systems (1.234ms avg)
   ...
```

## 🎓 What You'll Learn

### About AgentDB
1. **Performance**: See the 150x speedup in action
2. **Attention Mechanisms**: Understand when to use each mechanism
3. **Cognitive Patterns**: Learn how AI systems can be self-aware
4. **Vector Search**: Master semantic similarity search
5. **Memory Systems**: Store and retrieve semantic memories

### About AI Architecture
1. **Attention is Key**: Different problems need different attention mechanisms
2. **Hyperbolic Geometry**: Natural representation of hierarchies
3. **Self-Reflection**: AI systems can monitor and improve themselves
4. **Knowledge Graphs**: Organize information hierarchically
5. **Semantic Memory**: Store meaning, not just data

## 🛠️ Technical Details

### Dependencies
All demonstrations use:
- `agentdb@2.0.0-alpha.2.11` - Main package
- `ruvector@0.1.26` - Vector search
- `@ruvector/attention@0.1.1` - Attention mechanisms

### Generated Files
The demonstrations create several database files:
- `demos/vector-search/semantic-db.bin` - Vector search index
- `demos/self-discovery/memory.bin` - Cognitive memory storage

These files persist across runs, so subsequent executions will be faster.

### System Requirements
- Node.js 16+
- ~100MB RAM per demo
- ~10MB disk space for generated files

## 📚 Code Examples

### Using Vector Search
```javascript
const { VectorDB } = require('ruvector');

const db = new VectorDB({
  dimensions: 128,
  maxElements: 1000
});

const vector = new Float32Array(128).fill(0.1);
await db.insert({ id: 'doc1', vector, metadata: { title: 'Example' } });

const results = await db.search(vector, 5);
```

### Using Attention Mechanisms
```javascript
const { MultiHeadAttention, HyperbolicAttention } = require('@ruvector/attention');

// Multi-head for general tasks
const mha = new MultiHeadAttention(64, 4);
const output = mha.compute(query, keys, values);

// Hyperbolic for hierarchies
const hyp = new HyperbolicAttention(64, -1.0);
const hierOutput = hyp.compute(query, keys, values);
```

## 🎯 Use Cases

### Vector Search
- Semantic document search
- RAG (Retrieval-Augmented Generation)
- Recommendation systems
- Duplicate detection
- Content clustering

### Attention Mechanisms
- **Multi-Head**: General transformers, language models
- **Flash**: Long sequences, production systems
- **Linear**: Real-time processing, streaming data
- **Hyperbolic**: Knowledge graphs, taxonomies, org charts
- **MoE**: Multi-task learning, domain-specific routing

### Self-Discovery
- AI agent introspection
- Autonomous capability mapping
- Performance monitoring
- Knowledge base construction
- Continuous learning systems

## 🔬 Advanced Topics

### Performance Optimization
- Vector dimension tuning
- Batch processing
- Index configuration
- Memory management

### Integration Patterns
- RAG pipelines
- Agent memory systems
- Semantic caching
- Knowledge graphs

### Research Applications
- Cognitive architectures
- Meta-learning
- Self-improving systems
- Emergent behaviors

## 📖 Further Reading

### Official Documentation
- [AgentDB README](../node_modules/agentdb/README.md)
- [RuVector Documentation](../node_modules/ruvector/README.md)
- [Attention Mechanisms Guide](../node_modules/@ruvector/attention/README.md)

### Related Demos
- [AgentDB Examples](../node_modules/agentdb/examples/)
- [Browser Examples](../node_modules/agentdb/examples/browser/)

## 🤝 Contributing

These demonstrations are designed to be:
- **Educational**: Learn by example
- **Extensible**: Build on these patterns
- **Practical**: Production-ready code

Feel free to:
- Modify and extend these demos
- Create new demonstrations
- Share your discoveries
- Build applications using these patterns

## 📝 License

These demonstrations follow the same license as AgentDB (MIT OR Apache-2.0).

---

## 🎉 Credits

**Package**: agentdb@2.0.0-alpha.2.11
**Session**: AgentDB Exploration & Self-Discovery
**Date**: December 2, 2025

Built with ❤️ using AgentDB's cognitive capabilities.

---

**Happy Exploring! 🚀**
