# Temporal Hypergraphs: Time-Varying Hyperedges with Causal Constraints

## Overview

Temporal Hypergraphs extend standard hyperedges with **time-varying validity**, **causal ordering constraints**, and **spike-timing based temporal reasoning**. This enables modeling of complex temporal relationships where multiple entities participate in events that have duration, ordering, and causal dependencies.

## Core Concept

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        TEMPORAL HYPERGRAPH                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  TIME ──────────────────────────────────────────────────────────────────►   │
│        t₀        t₁        t₂        t₃        t₄        t₅                │
│        │         │         │         │         │         │                 │
│  ┌─────┴─────────┴─────────┴─────────┴─────────┴─────────┴─────┐           │
│  │                    HYPEREDGE H₁                              │           │
│  │  ┌───────────────────────────────────────┐                   │           │
│  │  │ Nodes: {A, B, C}                      │                   │           │
│  │  │ Valid: [t₀, t₃)                       │                   │           │
│  │  │ Type: "MEETING"                       │                   │           │
│  │  └───────────────────────────────────────┘                   │           │
│  │        │                                                     │           │
│  │        │ CAUSES (Δt = t₂ - t₀)                              │           │
│  │        ▼                                                     │           │
│  │  ┌───────────────────────────────────────────────────┐       │           │
│  │  │ HYPEREDGE H₂                                      │       │           │
│  │  │ Nodes: {B, D, E}                                  │       │           │
│  │  │ Valid: [t₂, t₅)                                   │       │           │
│  │  │ Type: "PROJECT"                                   │       │           │
│  │  │ Constraint: AFTER(H₁)                             │       │           │
│  │  └───────────────────────────────────────────────────┘       │           │
│  └──────────────────────────────────────────────────────────────┘           │
│                                                                             │
│  CAUSAL GRAPH:  H₁ ───causes───► H₂ ───prevents───► H₃                     │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Data Structures

### 1. Temporal Hyperedge

```rust
// crates/ruvector-graph/src/temporal/hyperedge.rs

use chrono::{DateTime, Utc, Duration};

/// Temporal validity interval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalInterval {
    /// Start time (inclusive)
    pub start: DateTime<Utc>,
    /// End time (exclusive), None = ongoing
    pub end: Option<DateTime<Utc>>,
    /// Validity type
    pub validity: ValidityType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidityType {
    /// Hyperedge exists during interval
    Exists,
    /// Hyperedge is valid/active during interval
    Valid,
    /// Hyperedge is scheduled for interval
    Scheduled,
    /// Hyperedge was true during interval (historical)
    Historical,
}

impl TemporalInterval {
    /// Check if interval contains a point in time
    pub fn contains(&self, t: DateTime<Utc>) -> bool {
        t >= self.start && self.end.map(|e| t < e).unwrap_or(true)
    }

    /// Check if intervals overlap
    pub fn overlaps(&self, other: &TemporalInterval) -> bool {
        let self_end = self.end.unwrap_or(DateTime::<Utc>::MAX_UTC);
        let other_end = other.end.unwrap_or(DateTime::<Utc>::MAX_UTC);

        self.start < other_end && other.start < self_end
    }

    /// Allen's interval algebra relations
    pub fn relation(&self, other: &TemporalInterval) -> AllenRelation {
        // Implement all 13 Allen relations
        // before, meets, overlaps, starts, during, finishes, equals, etc.
        todo!()
    }
}

/// Hyperedge with temporal dimension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalHyperedge {
    /// Base hyperedge
    pub hyperedge: Hyperedge,

    /// Temporal validity intervals (can have multiple)
    pub intervals: Vec<TemporalInterval>,

    /// Causal constraints
    pub causal_constraints: Vec<CausalConstraint>,

    /// Temporal properties (can vary over time)
    pub temporal_properties: HashMap<String, TimeSeries>,

    /// Version history
    pub versions: Vec<HyperedgeVersion>,

    /// Spike-timing metadata (for SNN integration)
    pub spike_metadata: Option<SpikeMetadata>,
}

/// Causal constraint between hyperedges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalConstraint {
    /// Type of causal relationship
    pub constraint_type: CausalConstraintType,
    /// Target hyperedge ID
    pub target: HyperedgeId,
    /// Minimum time delay
    pub min_delay: Option<Duration>,
    /// Maximum time delay
    pub max_delay: Option<Duration>,
    /// Causal strength (learned from SNN)
    pub strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CausalConstraintType {
    /// This hyperedge must come AFTER target
    After,
    /// This hyperedge must come BEFORE target
    Before,
    /// This hyperedge CAUSES target
    Causes,
    /// This hyperedge PREVENTS target
    Prevents,
    /// This hyperedge must OVERLAP with target
    Overlaps,
    /// This hyperedge must be CONTAINED in target
    During,
    /// This hyperedge ENABLES target (necessary but not sufficient)
    Enables,
}
```

### 2. Time Series Properties

```rust
/// Time-varying property value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    /// Property name
    pub name: String,
    /// Time-value pairs
    pub points: Vec<(DateTime<Utc>, PropertyValue)>,
    /// Interpolation method
    pub interpolation: Interpolation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Interpolation {
    /// Step function (constant until next point)
    Step,
    /// Linear interpolation
    Linear,
    /// No interpolation (only exact points)
    None,
}

impl TimeSeries {
    /// Get value at specific time
    pub fn value_at(&self, t: DateTime<Utc>) -> Option<PropertyValue> {
        match self.interpolation {
            Interpolation::Step => {
                self.points.iter()
                    .rev()
                    .find(|(pt, _)| *pt <= t)
                    .map(|(_, v)| v.clone())
            }
            Interpolation::Linear => {
                // Find surrounding points and interpolate
                let before = self.points.iter().rev().find(|(pt, _)| *pt <= t);
                let after = self.points.iter().find(|(pt, _)| *pt > t);

                match (before, after) {
                    (Some((t1, v1)), Some((t2, v2))) => {
                        // Linear interpolation
                        self.interpolate_linear(*t1, v1, *t2, v2, t)
                    }
                    (Some((_, v)), None) => Some(v.clone()),
                    (None, Some((_, v))) => Some(v.clone()),
                    (None, None) => None,
                }
            }
            Interpolation::None => {
                self.points.iter()
                    .find(|(pt, _)| *pt == t)
                    .map(|(_, v)| v.clone())
            }
        }
    }
}
```

### 3. Spike-Timing Integration

```rust
/// Spike metadata for temporal hyperedges
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpikeMetadata {
    /// Neuron ID representing this hyperedge
    pub neuron_id: usize,
    /// Spike times when hyperedge was activated
    pub spike_times: Vec<f64>,
    /// Learned causal weights to other hyperedges
    pub causal_weights: HashMap<HyperedgeId, f64>,
}

/// SNN-based temporal hypergraph analyzer
pub struct TemporalHypergraphSNN {
    /// Causal discovery SNN
    causal_snn: CausalDiscoverySNN,
    /// Mapping: hyperedge → neuron
    hyperedge_neurons: HashMap<HyperedgeId, usize>,
    /// Reverse mapping: neuron → hyperedge
    neuron_hyperedges: HashMap<usize, HyperedgeId>,
}

impl TemporalHypergraphSNN {
    /// Convert hyperedge event to spike
    pub fn hyperedge_to_spike(&self, hyperedge: &TemporalHyperedge, event_time: f64) -> Spike {
        let neuron_id = self.hyperedge_neurons
            .get(&hyperedge.hyperedge.id)
            .copied()
            .unwrap_or(0);

        Spike {
            neuron_id,
            time: event_time,
        }
    }

    /// Learn causal relationships from spike timing
    pub fn learn_causality(&mut self, spikes: &[Spike]) {
        for spike in spikes {
            let event = GraphEvent {
                event_type: GraphEventType::HyperedgeActivation,
                vertex: None,
                edge: None,
                data: spike.neuron_id as f64,
            };
            self.causal_snn.observe_event(event, spike.time);
        }
    }

    /// Extract learned causal graph between hyperedges
    pub fn extract_hyperedge_causality(&self) -> HashMap<(HyperedgeId, HyperedgeId), f64> {
        let causal_graph = self.causal_snn.extract_causal_graph();
        let mut result = HashMap::new();

        for edge in causal_graph.edges() {
            if let (Some(src_he), Some(tgt_he)) = (
                self.neuron_hyperedges.get(&edge.source),
                self.neuron_hyperedges.get(&edge.target),
            ) {
                result.insert((src_he.clone(), tgt_he.clone()), edge.strength);
            }
        }

        result
    }
}
```

## Temporal Query Language

### 4. Extended Cypher for Temporal Hypergraphs

```rust
/// Temporal Cypher query extensions
pub enum TemporalCypherExtension {
    /// Query at specific time point
    /// MATCH (n)-[r]->(m) AT TIME '2024-01-15T10:00:00Z'
    AtTime(DateTime<Utc>),

    /// Query during interval
    /// MATCH (n)-[r]->(m) DURING ['2024-01-01', '2024-02-01']
    During(TemporalInterval),

    /// Query with temporal ordering
    /// MATCH (h1:Hyperedge) BEFORE (h2:Hyperedge)
    Before(HyperedgePattern, HyperedgePattern),

    /// Query causal relationships
    /// MATCH (h1:Hyperedge) CAUSES (h2:Hyperedge)
    Causes(HyperedgePattern, HyperedgePattern),

    /// Query temporal evolution
    /// MATCH (n) EVOLVES FROM '2024-01-01' TO '2024-12-31'
    Evolves { start: DateTime<Utc>, end: DateTime<Utc> },

    /// Query with Allen relations
    /// MATCH (h1) OVERLAPS (h2)
    AllenRelation(AllenRelation, HyperedgePattern, HyperedgePattern),
}

/// Temporal query executor
pub struct TemporalCypherExecutor {
    graph: TemporalHypergraphDB,
    snn: TemporalHypergraphSNN,
}

impl TemporalCypherExecutor {
    /// Execute temporal Cypher query
    pub fn execute(&self, query: &str) -> Result<TemporalQueryResult> {
        let parsed = self.parse_temporal_cypher(query)?;

        match parsed.temporal_extension {
            Some(TemporalCypherExtension::AtTime(t)) => {
                self.execute_at_time(&parsed.base_query, t)
            }
            Some(TemporalCypherExtension::Causes(src, tgt)) => {
                self.execute_causal_query(&src, &tgt)
            }
            Some(TemporalCypherExtension::Evolves { start, end }) => {
                self.execute_evolution_query(&parsed.base_query, start, end)
            }
            _ => self.execute_base_query(&parsed.base_query),
        }
    }

    /// Execute causal query using SNN
    fn execute_causal_query(
        &self,
        source: &HyperedgePattern,
        target: &HyperedgePattern,
    ) -> Result<TemporalQueryResult> {
        // Find matching hyperedges
        let source_hyperedges = self.graph.match_pattern(source)?;
        let target_hyperedges = self.graph.match_pattern(target)?;

        // Query causal relationships from SNN
        let causality = self.snn.extract_hyperedge_causality();

        let mut results = Vec::new();
        for src in &source_hyperedges {
            for tgt in &target_hyperedges {
                if let Some(&strength) = causality.get(&(src.id.clone(), tgt.id.clone())) {
                    if strength > 0.1 {
                        results.push(CausalMatch {
                            source: src.clone(),
                            target: tgt.clone(),
                            strength,
                            delay: self.estimate_causal_delay(src, tgt),
                        });
                    }
                }
            }
        }

        Ok(TemporalQueryResult::Causal(results))
    }
}
```

### 5. Example Queries

```cypher
-- Find all meetings that caused project formations
MATCH (meeting:Hyperedge {type: 'MEETING'})
      CAUSES
      (project:Hyperedge {type: 'PROJECT'})
WHERE meeting.duration > 60
RETURN meeting, project, causal_strength, causal_delay

-- Find hyperedges valid during Q1 2024
MATCH (h:Hyperedge)
DURING ['2024-01-01', '2024-03-31']
RETURN h.type, count(*) AS count

-- Trace temporal evolution of a relationship
MATCH (team:Hyperedge {id: 'team-alpha'})
EVOLVES FROM '2024-01-01' TO '2024-12-31'
RETURN timestamp, team.members, team.confidence

-- Find overlapping events
MATCH (h1:Hyperedge {type: 'MEETING'})
      OVERLAPS
      (h2:Hyperedge {type: 'MEETING'})
WHERE h1 <> h2
RETURN h1, h2, overlap_duration

-- Counterfactual query
MATCH (cause:Hyperedge)
      CAUSES
      (effect:Hyperedge {outcome: 'failure'})
COUNTERFACTUAL NOT cause
PREDICT effect.outcome
```

## MinCut Integration

### 6. Temporal MinCut

```rust
/// MinCut on temporal hypergraphs
pub struct TemporalMinCut {
    /// Static MinCut analyzer
    static_analyzer: RuVectorGraphAnalyzer,
    /// Time window for snapshot
    window: Duration,
}

impl TemporalMinCut {
    /// Compute MinCut at specific time
    pub fn mincut_at(&self, t: DateTime<Utc>) -> u64 {
        // Build snapshot graph at time t
        let snapshot = self.build_snapshot(t);

        let mut analyzer = RuVectorGraphAnalyzer::new(snapshot);
        analyzer.min_cut()
    }

    /// Compute MinCut evolution over time
    pub fn mincut_evolution(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        step: Duration,
    ) -> Vec<(DateTime<Utc>, u64)> {
        let mut results = Vec::new();
        let mut t = start;

        while t <= end {
            results.push((t, self.mincut_at(t)));
            t = t + step;
        }

        results
    }

    /// Find time of minimum connectivity
    pub fn find_vulnerability_window(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Option<TemporalInterval> {
        let evolution = self.mincut_evolution(start, end, Duration::hours(1));

        // Find minimum mincut value
        let min_idx = evolution.iter()
            .enumerate()
            .min_by_key(|(_, (_, v))| *v)?
            .0;

        // Expand window around minimum
        let window_start = evolution.get(min_idx.saturating_sub(1))
            .map(|(t, _)| *t)
            .unwrap_or(start);
        let window_end = evolution.get(min_idx + 1)
            .map(|(t, _)| *t);

        Some(TemporalInterval {
            start: window_start,
            end: window_end,
            validity: ValidityType::Valid,
        })
    }

    /// Build graph snapshot at time t
    fn build_snapshot(&self, t: DateTime<Utc>) -> Arc<DynamicGraph> {
        let graph = Arc::new(DynamicGraph::new());

        // Add only hyperedges valid at time t
        for hyperedge in self.get_valid_hyperedges(t) {
            // Convert hyperedge to clique in graph
            for i in 0..hyperedge.hyperedge.nodes.len() {
                for j in (i + 1)..hyperedge.hyperedge.nodes.len() {
                    let u = self.node_to_vertex(&hyperedge.hyperedge.nodes[i]);
                    let v = self.node_to_vertex(&hyperedge.hyperedge.nodes[j]);
                    let _ = graph.insert_edge(u, v, hyperedge.hyperedge.confidence as f64);
                }
            }
        }

        graph
    }
}
```

### 7. Causal MinCut

```rust
/// MinCut on the causal graph between hyperedges
pub struct CausalMinCut {
    snn: TemporalHypergraphSNN,
}

impl CausalMinCut {
    /// Find minimum intervention to prevent an outcome
    pub fn minimum_intervention(
        &self,
        source_hyperedges: &[HyperedgeId],
        target_outcome: &HyperedgeId,
    ) -> Vec<HyperedgeId> {
        // Build causal graph
        let causality = self.snn.extract_hyperedge_causality();
        let causal_graph = self.build_causal_graph(&causality);

        // Compute MinCut between sources and target
        let mut analyzer = RuVectorGraphAnalyzer::new(causal_graph);
        let (side_a, side_b) = analyzer.partition().unwrap();

        // Find hyperedges in the cut
        let cut_hyperedges = self.find_cut_hyperedges(&side_a, &side_b);

        cut_hyperedges
    }

    /// Find critical causal paths
    pub fn critical_causal_paths(
        &self,
        outcome: &HyperedgeId,
    ) -> Vec<CausalPath> {
        let causality = self.snn.extract_hyperedge_causality();

        // Trace back all causal paths to outcome
        let mut paths = Vec::new();
        self.trace_causal_paths(outcome, &causality, &mut Vec::new(), &mut paths);

        // Rank by causal strength
        paths.sort_by(|a, b| b.total_strength.partial_cmp(&a.total_strength).unwrap());

        paths
    }
}
```

## Implementation Phases

### Phase 1: Core Data Structures
- [ ] Implement `TemporalInterval` with Allen's algebra
- [ ] Create `TemporalHyperedge` structure
- [ ] Add `TimeSeries` for temporal properties
- [ ] Implement version history

### Phase 2: Storage and Indexing
- [ ] Create temporal index (B-tree on intervals)
- [ ] Implement efficient time-range queries
- [ ] Add versioned storage backend
- [ ] Create snapshot generation

### Phase 3: SNN Integration
- [ ] Map hyperedges to neurons
- [ ] Implement spike-timing causal learning
- [ ] Create causal constraint inference
- [ ] Add strength estimation

### Phase 4: Query Language
- [ ] Extend Cypher parser for temporal operators
- [ ] Implement AT TIME queries
- [ ] Add CAUSES/PREVENTS queries
- [ ] Create evolution queries

### Phase 5: MinCut Integration
- [ ] Implement temporal MinCut snapshots
- [ ] Add MinCut evolution tracking
- [ ] Create causal MinCut for interventions
- [ ] Build vulnerability detection

## Key Metrics

| Metric | Target | Description |
|--------|--------|-------------|
| Temporal Query Latency | < 50ms | Time-range query performance |
| Snapshot Generation | < 10ms | Build graph at point in time |
| Causal Inference | < 100ms | Learn causality from spikes |
| MinCut Evolution | < 1s | Compute MinCut over time range |

## Novel Research Contributions

1. **Spike-Timing Hyperedge Causality**: Using STDP to learn causal relationships between N-ary events
2. **Temporal MinCut**: Extending subpolynomial MinCut to time-varying graphs
3. **Causal Intervention Planning**: Using MinCut to find minimum changes to prevent outcomes
4. **Allen Algebra + Causality**: Combining temporal logic with causal constraints

## Use Cases

### 1. Event Sequence Analysis
Model complex events (meetings, projects, decisions) as temporal hyperedges and discover causal chains.

### 2. Temporal Knowledge Graphs
Represent facts that change over time with automatic causality detection.

### 3. Process Mining
Analyze business processes as temporal hypergraphs with causal constraints.

### 4. Predictive Maintenance
Model system states as hyperedges and predict failures through causal analysis.

### 5. Social Network Dynamics
Track group formations/dissolutions over time with causal explanations.

## References

- Allen, J. (1983). *Maintaining Knowledge about Temporal Intervals*
- Maier, D. (2010). *Semantics of Temporal Queries*
- Pearl, J. (2009). *Causality: Models, Reasoning, and Inference*
- Sporns, O. (2010). *Networks of the Brain*
