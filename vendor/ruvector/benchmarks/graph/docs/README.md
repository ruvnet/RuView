# RuVector Graph Database Benchmarks

Comprehensive benchmark suite for RuVector's graph database implementation, comparing performance with Neo4j baseline.

## Overview

This benchmark suite validates RuVector's performance claims:
- **10x+ faster** than Neo4j for graph traversals
- **100x+ faster** for simple node/edge lookups
- **Sub-linear scaling** with graph size

## Components

### 1. Rust Benchmarks (`graph_bench.rs`)

Located in `/home/user/ruvector/crates/ruvector-graph/benches/graph_bench.rs`

**Benchmark Categories:**

#### Node Operations
- `node_insertion_single` - Single node insertion (1, 10, 100, 1000 nodes)
- `node_insertion_batch` - Batch insertion (100, 1K, 10K nodes)
- `node_insertion_bulk` - Bulk insertion optimized path (10K, 100K, 1M nodes)

#### Edge Operations
- `edge_creation` - Edge creation benchmarks (100, 1K, 10K edges)

#### Query Operations
- `query_node_lookup` - Simple ID-based node lookup (100K nodes)
- `query_1hop_traversal` - 1-hop neighbor traversal (fan-out: 1, 10, 100)
- `query_2hop_traversal` - 2-hop BFS traversal
- `query_path_finding` - Shortest path algorithms
- `query_aggregation` - Aggregation queries (count, avg, etc.)

#### Concurrency
- `concurrent_operations` - Concurrent read/write (2, 4, 8, 16 threads)

#### Memory
- `memory_usage` - Memory tracking (10K, 100K, 1M nodes)

**Run Rust Benchmarks:**
```bash
cd /home/user/ruvector/crates/ruvector-graph
cargo bench --bench graph_bench

# Run specific benchmark
cargo bench --bench graph_bench -- node_insertion

# Save baseline
cargo bench --bench graph_bench -- --save-baseline my-baseline
```

### 2. TypeScript Test Scenarios (`graph-scenarios.ts`)

Defines high-level benchmark scenarios:

- **Social Network** (1M users, 10M friendships)
  - Friend recommendations
  - Mutual friends
  - Influencer detection

- **Knowledge Graph** (100K entities, 1M relationships)
  - Multi-hop reasoning
  - Path finding
  - Pattern matching

- **Temporal Graph** (500K events)
  - Time-range queries
  - State transitions
  - Event aggregation

- **Recommendation Engine**
  - Collaborative filtering
  - Item recommendations
  - Trending items

- **Fraud Detection**
  - Circular transfer detection
  - Network analysis
  - Risk scoring

### 3. Data Generator (`graph-data-generator.ts`)

Uses `@ruvector/agentic-synth` to generate realistic synthetic graph data.

**Features:**
- AI-powered realistic data generation
- Multiple graph topologies
- Scale-free networks (preferential attachment)
- Temporal event sequences

**Generate Datasets:**
```bash
cd /home/user/ruvector/benchmarks
npm run graph:generate
```

**Datasets Generated:**
- `social_network` - 1M nodes, 10M edges
- `knowledge_graph` - 100K entities, 1M relationships
- `temporal_events` - 500K events with transitions

### 4. Comparison Runner (`comparison-runner.ts`)

Runs benchmarks on both RuVector and Neo4j, compares results.

**Run Comparisons:**
```bash
# All scenarios
npm run graph:compare

# Specific scenario
npm run graph:compare:social
npm run graph:compare:knowledge
npm run graph:compare:temporal
```

**Comparison Metrics:**
- Execution time (ms)
- Throughput (ops/sec)
- Memory usage (MB)
- Latency percentiles (p50, p95, p99)
- Speedup calculation
- Pass/fail verdict

### 5. Results Reporter (`results-report.ts`)

Generates comprehensive HTML and Markdown reports.

**Generate Reports:**
```bash
npm run graph:report
```

**Output:**
- `benchmark-report.html` - Interactive HTML dashboard with charts
- `benchmark-report.md` - Markdown summary
- `benchmark-data.json` - Raw JSON data

## Quick Start

### 1. Generate Test Data
```bash
cd /home/user/ruvector/benchmarks
npm run graph:generate
```

### 2. Run Rust Benchmarks
```bash
npm run graph:bench
```

### 3. Run Comparison Tests
```bash
npm run graph:compare
```

### 4. Generate Report
```bash
npm run graph:report
```

### 5. View Results
```bash
npm run dashboard
# Open http://localhost:8000/results/graph/benchmark-report.html
```

## Complete Workflow

Run all benchmarks end-to-end:
```bash
npm run graph:all
```

This will:
1. Generate synthetic datasets using agentic-synth
2. Run Rust criterion benchmarks
3. Compare with Neo4j baseline
4. Generate HTML/Markdown reports

## Performance Targets

### ✅ Target: 10x Faster Traversals
- 1-hop traversal: >10x speedup
- 2-hop traversal: >10x speedup
- Multi-hop reasoning: >10x speedup

### ✅ Target: 100x Faster Lookups
- Node by ID: >100x speedup
- Edge lookup: >100x speedup
- Property access: >100x speedup

### ✅ Target: Sub-linear Scaling
- Performance remains consistent as graph grows
- Memory usage scales efficiently
- Query time independent of total graph size

## Dataset Specifications

### Social Network
```typescript
{
  nodes: 1_000_000,
  edges: 10_000_000,
  labels: ['Person', 'Post', 'Comment', 'Group'],
  avgDegree: 10,
  topology: 'scale-free' // Preferential attachment
}
```

### Knowledge Graph
```typescript
{
  nodes: 100_000,
  edges: 1_000_000,
  labels: ['Person', 'Organization', 'Location', 'Event', 'Concept'],
  avgDegree: 10,
  topology: 'semantic-network'
}
```

### Temporal Events
```typescript
{
  nodes: 500_000,
  edges: 2_000_000,
  labels: ['Event', 'State', 'Entity'],
  timeRange: '365 days',
  topology: 'temporal-causal'
}
```

## Agentic-Synth Integration

The benchmark suite uses `@ruvector/agentic-synth` for intelligent synthetic data generation:

```typescript
import { AgenticSynth } from '@ruvector/agentic-synth';

const synth = new AgenticSynth({
  provider: 'gemini',
  model: 'gemini-2.0-flash-exp'
});

// Generate realistic user profiles
const users = await synth.generateStructured({
  type: 'json',
  count: 10000,
  schema: {
    name: 'string',
    age: 'number',
    location: 'string',
    interests: 'array<string>'
  },
  prompt: 'Generate diverse social media user profiles...'
});
```

## Results Directory Structure

```
benchmarks/
├── data/
│   └── graph/
│       ├── social_network_nodes.json
│       ├── social_network_edges.json
│       ├── knowledge_graph_nodes.json
│       └── temporal_events_nodes.json
├── results/
│   └── graph/
│       ├── social_network_comparison.json
│       ├── benchmark-report.html
│       ├── benchmark-report.md
│       └── benchmark-data.json
└── graph/
    ├── graph-scenarios.ts
    ├── graph-data-generator.ts
    ├── comparison-runner.ts
    └── results-report.ts
```

## CI/CD Integration

Add to GitHub Actions:
```yaml
- name: Run Graph Benchmarks
  run: |
    cd benchmarks
    npm install
    npm run graph:all

- name: Upload Results
  uses: actions/upload-artifact@v3
  with:
    name: graph-benchmarks
    path: benchmarks/results/graph/
```

## Troubleshooting

### Neo4j Not Available
If Neo4j is not installed, the comparison runner will use baseline metrics from previous runs or estimates.

### Memory Issues
For large datasets (>1M nodes), increase Node.js heap:
```bash
NODE_OPTIONS="--max-old-space-size=8192" npm run graph:generate
```

### Criterion Baseline
Reset benchmark baselines:
```bash
cd crates/ruvector-graph
cargo bench --bench graph_bench -- --save-baseline new-baseline
```

## Contributing

When adding new benchmarks:
1. Add Rust benchmark to `graph_bench.rs`
2. Create corresponding TypeScript scenario
3. Update data generator if needed
4. Document expected performance targets
5. Update this README

## License

MIT - See LICENSE file
