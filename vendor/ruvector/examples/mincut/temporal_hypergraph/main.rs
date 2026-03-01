//! # Temporal Hypergraphs: Time-Varying Hyperedges with Causal Constraints
//!
//! This example implements temporal hypergraphs with:
//! - Phase 1: Core data structures (TemporalInterval, TemporalHyperedge, TimeSeries)
//! - Phase 2: Storage and indexing (temporal index, time-range queries)
//! - Phase 3: Causal constraint inference (spike-timing learning)
//! - Phase 4: Query language (temporal operators)
//! - Phase 5: MinCut integration (temporal snapshots, evolution tracking)
//!
//! Run: `cargo run --example temporal_hypergraph`

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

// ============================================================================
// PHASE 1: CORE DATA STRUCTURES
// ============================================================================

/// Temporal validity interval with Allen's algebra support
#[derive(Debug, Clone)]
struct TemporalInterval {
    /// Start time (milliseconds from epoch)
    start_ms: u64,
    /// End time (None = ongoing)
    end_ms: Option<u64>,
    /// Validity type
    validity: ValidityType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ValidityType {
    Exists,     // Hyperedge exists during interval
    Valid,      // Hyperedge is active
    Scheduled,  // Future scheduled
    Historical, // Past event
}

/// Allen's 13 interval relations
#[derive(Debug, Clone, Copy, PartialEq)]
enum AllenRelation {
    Before,         // X ends before Y starts
    Meets,          // X ends exactly when Y starts
    Overlaps,       // X starts before Y, ends during Y
    Starts,         // X starts with Y, ends before Y
    During,         // X is contained within Y
    Finishes,       // X starts after Y, ends with Y
    Equals,         // X and Y are identical
    FinishedBy,     // Inverse of Finishes
    Contains,       // Inverse of During
    StartedBy,      // Inverse of Starts
    OverlappedBy,   // Inverse of Overlaps
    MetBy,          // Inverse of Meets
    After,          // Inverse of Before
}

impl TemporalInterval {
    fn new(start_ms: u64, end_ms: Option<u64>) -> Self {
        Self {
            start_ms,
            end_ms,
            validity: ValidityType::Valid,
        }
    }

    fn contains(&self, t: u64) -> bool {
        t >= self.start_ms && self.end_ms.map(|e| t < e).unwrap_or(true)
    }

    fn overlaps(&self, other: &TemporalInterval) -> bool {
        let self_end = self.end_ms.unwrap_or(u64::MAX);
        let other_end = other.end_ms.unwrap_or(u64::MAX);
        self.start_ms < other_end && other.start_ms < self_end
    }

    fn duration_ms(&self) -> Option<u64> {
        self.end_ms.map(|e| e.saturating_sub(self.start_ms))
    }

    /// Compute Allen's interval relation
    fn allen_relation(&self, other: &TemporalInterval) -> AllenRelation {
        let s1 = self.start_ms;
        let e1 = self.end_ms.unwrap_or(u64::MAX);
        let s2 = other.start_ms;
        let e2 = other.end_ms.unwrap_or(u64::MAX);

        if e1 < s2 { AllenRelation::Before }
        else if e1 == s2 { AllenRelation::Meets }
        else if s1 < s2 && e1 > s2 && e1 < e2 { AllenRelation::Overlaps }
        else if s1 == s2 && e1 < e2 { AllenRelation::Starts }
        else if s1 > s2 && e1 < e2 { AllenRelation::During }
        else if s1 > s2 && e1 == e2 { AllenRelation::Finishes }
        else if s1 == s2 && e1 == e2 { AllenRelation::Equals }
        else if s1 == s2 && e1 > e2 { AllenRelation::StartedBy }
        else if s1 < s2 && e1 > e2 { AllenRelation::Contains }
        else if s1 > s2 && s1 < e2 && e1 > e2 { AllenRelation::OverlappedBy }
        else if s1 == e2 { AllenRelation::MetBy }
        else { AllenRelation::After }
    }
}

/// Time-varying property value
#[derive(Debug, Clone)]
struct TimeSeries {
    name: String,
    points: Vec<(u64, f64)>, // (timestamp_ms, value)
    interpolation: Interpolation,
}

#[derive(Debug, Clone, Copy)]
enum Interpolation {
    Step,   // Constant until next point
    Linear, // Linear interpolation
    None,   // Exact points only
}

impl TimeSeries {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            points: Vec::new(),
            interpolation: Interpolation::Step,
        }
    }

    fn add_point(&mut self, t: u64, value: f64) {
        self.points.push((t, value));
        self.points.sort_by_key(|(t, _)| *t);
    }

    fn value_at(&self, t: u64) -> Option<f64> {
        match self.interpolation {
            Interpolation::Step => {
                self.points.iter()
                    .rev()
                    .find(|(pt, _)| *pt <= t)
                    .map(|(_, v)| *v)
            }
            Interpolation::Linear => {
                let before = self.points.iter().rev().find(|(pt, _)| *pt <= t);
                let after = self.points.iter().find(|(pt, _)| *pt > t);

                match (before, after) {
                    (Some((t1, v1)), Some((t2, v2))) => {
                        let ratio = (t - t1) as f64 / (t2 - t1) as f64;
                        Some(v1 + ratio * (v2 - v1))
                    }
                    (Some((_, v)), None) => Some(*v),
                    (None, Some((_, v))) => Some(*v),
                    (None, None) => None,
                }
            }
            Interpolation::None => {
                self.points.iter()
                    .find(|(pt, _)| *pt == t)
                    .map(|(_, v)| *v)
            }
        }
    }
}

/// Causal constraint between hyperedges
#[derive(Debug, Clone)]
struct CausalConstraint {
    constraint_type: CausalConstraintType,
    target_id: usize,
    min_delay_ms: Option<u64>,
    max_delay_ms: Option<u64>,
    strength: f64, // Learned from observations
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CausalConstraintType {
    After,    // Must come after target
    Before,   // Must come before target
    Causes,   // Causes target to occur
    Prevents, // Prevents target
    Enables,  // Necessary but not sufficient
    Overlaps, // Must overlap with target
}

/// Hyperedge with temporal dimension
#[derive(Debug, Clone)]
struct TemporalHyperedge {
    id: usize,
    name: String,
    nodes: Vec<u64>,
    hyperedge_type: String,
    intervals: Vec<TemporalInterval>,
    causal_constraints: Vec<CausalConstraint>,
    properties: HashMap<String, TimeSeries>,
    confidence: f64,
}

impl TemporalHyperedge {
    fn new(id: usize, name: &str, nodes: Vec<u64>, he_type: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
            nodes,
            hyperedge_type: he_type.to_string(),
            intervals: Vec::new(),
            causal_constraints: Vec::new(),
            properties: HashMap::new(),
            confidence: 1.0,
        }
    }

    fn add_interval(&mut self, start: u64, end: Option<u64>) {
        self.intervals.push(TemporalInterval::new(start, end));
    }

    fn is_valid_at(&self, t: u64) -> bool {
        self.intervals.iter().any(|i| i.contains(t))
    }

    fn add_property(&mut self, name: &str, t: u64, value: f64) {
        self.properties
            .entry(name.to_string())
            .or_insert_with(|| TimeSeries::new(name))
            .add_point(t, value);
    }
}

// ============================================================================
// PHASE 2: STORAGE AND INDEXING
// ============================================================================

/// Temporal index for efficient time-range queries
struct TemporalIndex {
    /// Hyperedges sorted by start time
    by_start: Vec<(u64, usize)>, // (start_ms, hyperedge_id)
    /// Hyperedges sorted by end time
    by_end: Vec<(u64, usize)>,
}

impl TemporalIndex {
    fn new() -> Self {
        Self {
            by_start: Vec::new(),
            by_end: Vec::new(),
        }
    }

    fn add(&mut self, he_id: usize, interval: &TemporalInterval) {
        self.by_start.push((interval.start_ms, he_id));
        if let Some(end) = interval.end_ms {
            self.by_end.push((end, he_id));
        }
        self.by_start.sort_by_key(|(t, _)| *t);
        self.by_end.sort_by_key(|(t, _)| *t);
    }

    /// Find all hyperedges valid at time t
    fn query_at(&self, t: u64) -> Vec<usize> {
        // Started before or at t
        let started: HashSet<_> = self.by_start.iter()
            .filter(|(start, _)| *start <= t)
            .map(|(_, id)| *id)
            .collect();

        // Ended after t (or not ended)
        let ended: HashSet<_> = self.by_end.iter()
            .filter(|(end, _)| *end <= t)
            .map(|(_, id)| *id)
            .collect();

        started.difference(&ended).copied().collect()
    }

    /// Find hyperedges valid during interval
    fn query_during(&self, start: u64, end: u64) -> Vec<usize> {
        let mut result = HashSet::new();
        for t in (start..=end).step_by(100) { // Sample every 100ms
            for id in self.query_at(t) {
                result.insert(id);
            }
        }
        result.into_iter().collect()
    }
}

/// Main temporal hypergraph storage
struct TemporalHypergraphDB {
    hyperedges: HashMap<usize, TemporalHyperedge>,
    temporal_index: TemporalIndex,
    next_id: usize,
    causal_graph: HashMap<(usize, usize), f64>, // (cause, effect) -> strength
}

impl TemporalHypergraphDB {
    fn new() -> Self {
        Self {
            hyperedges: HashMap::new(),
            temporal_index: TemporalIndex::new(),
            next_id: 0,
            causal_graph: HashMap::new(),
        }
    }

    fn add_hyperedge(&mut self, mut he: TemporalHyperedge) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        he.id = id;

        for interval in &he.intervals {
            self.temporal_index.add(id, interval);
        }

        self.hyperedges.insert(id, he);
        id
    }

    fn get(&self, id: usize) -> Option<&TemporalHyperedge> {
        self.hyperedges.get(&id)
    }

    fn query_at_time(&self, t: u64) -> Vec<&TemporalHyperedge> {
        self.temporal_index.query_at(t)
            .iter()
            .filter_map(|id| self.hyperedges.get(id))
            .collect()
    }

    fn query_by_type(&self, he_type: &str, t: u64) -> Vec<&TemporalHyperedge> {
        self.query_at_time(t)
            .into_iter()
            .filter(|he| he.hyperedge_type == he_type)
            .collect()
    }

    /// Learn causal relationship from observed sequence
    fn learn_causality(&mut self, cause_id: usize, effect_id: usize, delay_ms: u64) {
        let key = (cause_id, effect_id);
        let current = self.causal_graph.get(&key).copied().unwrap_or(0.0);

        // STDP-like learning: closer in time = stronger causality
        let time_factor = 1.0 / (1.0 + delay_ms as f64 / 100.0);
        let new_strength = current + 0.1 * time_factor;

        self.causal_graph.insert(key, new_strength.min(1.0));
    }

    fn get_causal_strength(&self, cause_id: usize, effect_id: usize) -> f64 {
        self.causal_graph.get(&(cause_id, effect_id)).copied().unwrap_or(0.0)
    }
}

// ============================================================================
// PHASE 3: CAUSAL CONSTRAINT INFERENCE
// ============================================================================

/// Spike metadata for causal learning
#[derive(Debug, Clone)]
struct SpikeEvent {
    hyperedge_id: usize,
    time_ms: u64,
    spike_type: SpikeType,
}

#[derive(Debug, Clone, Copy)]
enum SpikeType {
    Activation,   // Hyperedge became active
    Deactivation, // Hyperedge became inactive
    Update,       // Property changed
}

/// SNN-based causal learner
struct CausalLearner {
    spike_history: VecDeque<SpikeEvent>,
    learning_window_ms: u64,
    min_strength_threshold: f64,
}

impl CausalLearner {
    fn new() -> Self {
        Self {
            spike_history: VecDeque::new(),
            learning_window_ms: 500,
            min_strength_threshold: 0.1,
        }
    }

    fn record_spike(&mut self, event: SpikeEvent) {
        self.spike_history.push_back(event);

        // Prune old spikes
        while let Some(front) = self.spike_history.front() {
            if let Some(back) = self.spike_history.back() {
                if back.time_ms.saturating_sub(front.time_ms) > self.learning_window_ms * 10 {
                    self.spike_history.pop_front();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Infer causal relationships from spike timing
    fn infer_causality(&self, db: &mut TemporalHypergraphDB) -> Vec<(usize, usize, f64)> {
        let mut inferred = Vec::new();
        let spikes: Vec<_> = self.spike_history.iter().collect();

        for i in 0..spikes.len() {
            for j in (i + 1)..spikes.len() {
                let cause = &spikes[i];
                let effect = &spikes[j];

                let delay = effect.time_ms.saturating_sub(cause.time_ms);

                if delay > 0 && delay < self.learning_window_ms {
                    db.learn_causality(cause.hyperedge_id, effect.hyperedge_id, delay);

                    let strength = db.get_causal_strength(cause.hyperedge_id, effect.hyperedge_id);
                    if strength >= self.min_strength_threshold {
                        inferred.push((cause.hyperedge_id, effect.hyperedge_id, strength));
                    }
                }
            }
        }

        inferred
    }
}

// ============================================================================
// PHASE 4: QUERY LANGUAGE
// ============================================================================

/// Temporal query types
#[derive(Debug, Clone)]
enum TemporalQuery {
    /// Get hyperedges at specific time
    AtTime(u64),
    /// Get hyperedges during interval
    During(u64, u64),
    /// Find causal relationships
    Causes(String, String), // (cause_type, effect_type)
    /// Find evolution of hyperedge
    Evolution(usize, u64, u64),
    /// Allen relation query
    AllenQuery(AllenRelation, usize),
}

/// Query result
#[derive(Debug)]
enum QueryResult {
    Hyperedges(Vec<usize>),
    CausalPairs(Vec<(usize, usize, f64)>),
    Evolution(Vec<(u64, f64)>), // (time, mincut_value)
}

/// Query executor
struct QueryExecutor<'a> {
    db: &'a TemporalHypergraphDB,
}

impl<'a> QueryExecutor<'a> {
    fn new(db: &'a TemporalHypergraphDB) -> Self {
        Self { db }
    }

    fn execute(&self, query: TemporalQuery) -> QueryResult {
        match query {
            TemporalQuery::AtTime(t) => {
                let ids: Vec<_> = self.db.query_at_time(t)
                    .iter()
                    .map(|he| he.id)
                    .collect();
                QueryResult::Hyperedges(ids)
            }
            TemporalQuery::During(start, end) => {
                let ids = self.db.temporal_index.query_during(start, end);
                QueryResult::Hyperedges(ids)
            }
            TemporalQuery::Causes(cause_type, effect_type) => {
                let mut pairs = Vec::new();
                for ((cause_id, effect_id), &strength) in &self.db.causal_graph {
                    if let (Some(cause), Some(effect)) =
                        (self.db.get(*cause_id), self.db.get(*effect_id)) {
                        if cause.hyperedge_type == cause_type &&
                           effect.hyperedge_type == effect_type &&
                           strength > 0.1 {
                            pairs.push((*cause_id, *effect_id, strength));
                        }
                    }
                }
                pairs.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
                QueryResult::CausalPairs(pairs)
            }
            TemporalQuery::Evolution(he_id, start, end) => {
                // Track property evolution
                let mut evolution = Vec::new();
                if let Some(he) = self.db.get(he_id) {
                    if let Some(series) = he.properties.get("confidence") {
                        for t in (start..=end).step_by(100) {
                            if let Some(v) = series.value_at(t) {
                                evolution.push((t, v));
                            }
                        }
                    }
                }
                QueryResult::Evolution(evolution)
            }
            TemporalQuery::AllenQuery(relation, he_id) => {
                let mut matches = Vec::new();
                if let Some(target) = self.db.get(he_id) {
                    for (_, he) in &self.db.hyperedges {
                        if he.id == he_id { continue; }
                        for t_int in &target.intervals {
                            for h_int in &he.intervals {
                                if h_int.allen_relation(t_int) == relation {
                                    matches.push(he.id);
                                    break;
                                }
                            }
                        }
                    }
                }
                QueryResult::Hyperedges(matches)
            }
        }
    }
}

// ============================================================================
// PHASE 5: MINCUT INTEGRATION
// ============================================================================

/// Simple graph for MinCut computation
struct SimpleGraph {
    vertices: HashSet<u64>,
    edges: HashMap<(u64, u64), f64>,
}

impl SimpleGraph {
    fn new() -> Self {
        Self {
            vertices: HashSet::new(),
            edges: HashMap::new(),
        }
    }

    fn add_edge(&mut self, u: u64, v: u64, weight: f64) {
        self.vertices.insert(u);
        self.vertices.insert(v);
        let key = if u < v { (u, v) } else { (v, u) };
        *self.edges.entry(key).or_insert(0.0) += weight;
    }

    fn weighted_degree(&self, v: u64) -> f64 {
        self.edges.iter()
            .filter(|((a, b), _)| *a == v || *b == v)
            .map(|(_, w)| *w)
            .sum()
    }

    fn approx_mincut(&self) -> f64 {
        self.vertices.iter()
            .map(|&v| self.weighted_degree(v))
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0)
    }
}

/// Temporal MinCut analyzer
struct TemporalMinCut<'a> {
    db: &'a TemporalHypergraphDB,
}

impl<'a> TemporalMinCut<'a> {
    fn new(db: &'a TemporalHypergraphDB) -> Self {
        Self { db }
    }

    /// Build graph snapshot at specific time
    fn build_snapshot(&self, t: u64) -> SimpleGraph {
        let mut graph = SimpleGraph::new();

        for he in self.db.query_at_time(t) {
            // Convert hyperedge to clique
            for i in 0..he.nodes.len() {
                for j in (i + 1)..he.nodes.len() {
                    graph.add_edge(he.nodes[i], he.nodes[j], he.confidence);
                }
            }
        }

        graph
    }

    /// Compute MinCut at specific time
    fn mincut_at(&self, t: u64) -> f64 {
        let graph = self.build_snapshot(t);
        graph.approx_mincut()
    }

    /// Compute MinCut evolution over time
    fn mincut_evolution(&self, start: u64, end: u64, step: u64) -> Vec<(u64, f64)> {
        let mut results = Vec::new();
        let mut t = start;

        while t <= end {
            results.push((t, self.mincut_at(t)));
            t += step;
        }

        results
    }

    /// Find vulnerability window (lowest MinCut)
    fn find_vulnerability_window(&self, start: u64, end: u64) -> Option<(u64, f64)> {
        let evolution = self.mincut_evolution(start, end, 100);
        evolution.into_iter()
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
    }
}

/// Causal MinCut - find minimum intervention to prevent outcome
struct CausalMinCut<'a> {
    db: &'a TemporalHypergraphDB,
}

impl<'a> CausalMinCut<'a> {
    fn new(db: &'a TemporalHypergraphDB) -> Self {
        Self { db }
    }

    /// Find minimum set of hyperedges to prevent target
    fn minimum_intervention(&self, target_id: usize) -> Vec<usize> {
        // Find all causes of target
        let mut causes: Vec<(usize, f64)> = self.db.causal_graph.iter()
            .filter(|((_, effect), _)| *effect == target_id)
            .map(|((cause, _), &strength)| (*cause, strength))
            .collect();

        // Sort by causal strength (highest first)
        causes.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Return hyperedges that, if removed, would break causal chain
        causes.into_iter()
            .take(3) // Top 3 causes
            .map(|(id, _)| id)
            .collect()
    }

    /// Find critical causal paths to outcome
    fn critical_paths(&self, target_id: usize, max_depth: usize) -> Vec<Vec<usize>> {
        let mut paths = Vec::new();
        self.trace_paths(target_id, &mut Vec::new(), &mut paths, max_depth);
        paths
    }

    fn trace_paths(&self, current: usize, path: &mut Vec<usize>,
                   all_paths: &mut Vec<Vec<usize>>, depth: usize) {
        if depth == 0 {
            return;
        }

        // Find all causes of current
        let causes: Vec<usize> = self.db.causal_graph.iter()
            .filter(|((_, effect), strength)| *effect == current && **strength > 0.2)
            .map(|((cause, _), _)| *cause)
            .collect();

        if causes.is_empty() {
            // End of path
            if !path.is_empty() {
                let mut full_path = path.clone();
                full_path.push(current);
                all_paths.push(full_path);
            }
        } else {
            for cause in causes {
                path.push(cause);
                self.trace_paths(current, path, all_paths, depth - 1);
                path.pop();
            }
        }
    }
}

// ============================================================================
// MAIN: DEMO ALL PHASES
// ============================================================================

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║  TEMPORAL HYPERGRAPHS: Time-Varying Causal Networks        ║");
    println!("║  Implementing All 5 Phases from Research Spec              ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    let start = Instant::now();

    // ========== PHASE 1: Core Data Structures ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📦 PHASE 1: Core Data Structures");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let mut db = TemporalHypergraphDB::new();

    // Create temporal hyperedges representing meetings and projects
    let mut meeting1 = TemporalHyperedge::new(0, "Team Alpha Meeting", vec![1, 2, 3], "MEETING");
    meeting1.add_interval(0, Some(100));
    meeting1.add_property("attendees", 0, 3.0);
    meeting1.add_property("confidence", 50, 0.9);
    let m1_id = db.add_hyperedge(meeting1);

    let mut meeting2 = TemporalHyperedge::new(0, "Team Beta Meeting", vec![2, 4, 5], "MEETING");
    meeting2.add_interval(80, Some(180));
    meeting2.add_property("confidence", 100, 0.85);
    let m2_id = db.add_hyperedge(meeting2);

    let mut project1 = TemporalHyperedge::new(0, "Project X Launch", vec![1, 2, 4, 5], "PROJECT");
    project1.add_interval(150, Some(500));
    project1.add_property("progress", 150, 0.0);
    project1.add_property("progress", 300, 0.5);
    project1.add_property("progress", 450, 0.9);
    let p1_id = db.add_hyperedge(project1);

    let mut decision1 = TemporalHyperedge::new(0, "Budget Approval", vec![1, 3], "DECISION");
    decision1.add_interval(120, Some(130));
    let d1_id = db.add_hyperedge(decision1);

    let mut failure1 = TemporalHyperedge::new(0, "System Failure", vec![4, 5, 6], "FAILURE");
    failure1.add_interval(400, Some(420));
    let f1_id = db.add_hyperedge(failure1);

    println!("Created {} temporal hyperedges", db.hyperedges.len());

    // Demo Allen's interval algebra
    if let (Some(m1), Some(m2)) = (db.get(m1_id), db.get(m2_id)) {
        let relation = m1.intervals[0].allen_relation(&m2.intervals[0]);
        println!("Allen relation: Meeting1 {:?} Meeting2", relation);
    }

    // Demo TimeSeries
    if let Some(p1) = db.get(p1_id) {
        if let Some(progress) = p1.properties.get("progress") {
            println!("Project progress at t=250: {:?}", progress.value_at(250));
            println!("Project progress at t=400: {:?}", progress.value_at(400));
        }
    }

    println!("\n✅ Phase 1 complete: TemporalInterval, TemporalHyperedge, TimeSeries\n");

    // ========== PHASE 2: Storage and Indexing ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📂 PHASE 2: Storage and Indexing");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Query at specific time
    let t = 100;
    let active = db.query_at_time(t);
    println!("Hyperedges active at t={}: {:?}", t,
             active.iter().map(|h| &h.name).collect::<Vec<_>>());

    // Query by type
    let meetings = db.query_by_type("MEETING", 90);
    println!("Meetings at t=90: {:?}",
             meetings.iter().map(|h| &h.name).collect::<Vec<_>>());

    // Query during interval
    let during = db.temporal_index.query_during(100, 200);
    println!("Hyperedges during [100, 200]: {} found", during.len());

    println!("\n✅ Phase 2 complete: TemporalIndex, time-range queries\n");

    // ========== PHASE 3: Causal Inference ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🧠 PHASE 3: Causal Constraint Inference (Spike-Timing Learning)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let mut learner = CausalLearner::new();

    // Simulate spike events (hyperedge activations)
    let events = vec![
        SpikeEvent { hyperedge_id: m1_id, time_ms: 10, spike_type: SpikeType::Activation },
        SpikeEvent { hyperedge_id: m2_id, time_ms: 90, spike_type: SpikeType::Activation },
        SpikeEvent { hyperedge_id: d1_id, time_ms: 125, spike_type: SpikeType::Activation },
        SpikeEvent { hyperedge_id: p1_id, time_ms: 160, spike_type: SpikeType::Activation },
        SpikeEvent { hyperedge_id: f1_id, time_ms: 410, spike_type: SpikeType::Activation },
    ];

    println!("Recording {} spike events...", events.len());
    for event in events {
        learner.record_spike(event);
    }

    let inferred = learner.infer_causality(&mut db);
    println!("\nInferred causal relationships:");
    for (cause, effect, strength) in &inferred {
        if let (Some(c), Some(e)) = (db.get(*cause), db.get(*effect)) {
            println!("  {} → {} (strength: {:.2})", c.name, e.name, strength);
        }
    }

    println!("\n✅ Phase 3 complete: STDP-like causal learning from spike timing\n");

    // ========== PHASE 4: Query Language ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔍 PHASE 4: Temporal Query Language");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let executor = QueryExecutor::new(&db);

    // Query: AT TIME
    println!("Query: MATCH (h:Hyperedge) AT TIME 150");
    if let QueryResult::Hyperedges(ids) = executor.execute(TemporalQuery::AtTime(150)) {
        for id in ids {
            if let Some(he) = db.get(id) {
                println!("  → {}: {}", he.hyperedge_type, he.name);
            }
        }
    }

    // Query: DURING interval
    println!("\nQuery: MATCH (h:Hyperedge) DURING [100, 200]");
    if let QueryResult::Hyperedges(ids) = executor.execute(TemporalQuery::During(100, 200)) {
        println!("  → {} hyperedges active during interval", ids.len());
    }

    // Query: CAUSES
    println!("\nQuery: MATCH (m:MEETING) CAUSES (p:PROJECT)");
    if let QueryResult::CausalPairs(pairs) = executor.execute(
        TemporalQuery::Causes("MEETING".to_string(), "PROJECT".to_string())
    ) {
        for (cause, effect, strength) in pairs {
            if let (Some(c), Some(e)) = (db.get(cause), db.get(effect)) {
                println!("  → {} CAUSES {} (strength: {:.2})", c.name, e.name, strength);
            }
        }
    }

    // Query: Allen relation
    println!("\nQuery: MATCH (h) OVERLAPS (meeting1)");
    if let QueryResult::Hyperedges(ids) = executor.execute(
        TemporalQuery::AllenQuery(AllenRelation::Overlaps, m1_id)
    ) {
        for id in ids {
            if let Some(he) = db.get(id) {
                println!("  → {}", he.name);
            }
        }
    }

    println!("\n✅ Phase 4 complete: AT TIME, DURING, CAUSES, Allen queries\n");

    // ========== PHASE 5: MinCut Integration ==========
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("📊 PHASE 5: MinCut Integration");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let temporal_mincut = TemporalMinCut::new(&db);

    // MinCut at specific time
    println!("MinCut Snapshots:");
    for t in [50, 100, 150, 200, 300, 400].iter() {
        let mc = temporal_mincut.mincut_at(*t);
        let active = db.query_at_time(*t).len();
        println!("  t={:3}: MinCut = {:.2} ({} active hyperedges)", t, mc, active);
    }

    // MinCut evolution
    println!("\nMinCut Evolution [0, 500]:");
    let evolution = temporal_mincut.mincut_evolution(0, 500, 100);
    for (t, mc) in &evolution {
        let bar = "█".repeat((mc * 10.0) as usize);
        println!("  t={:3}: {:5.2} {}", t, mc, bar);
    }

    // Find vulnerability window
    if let Some((t, mc)) = temporal_mincut.find_vulnerability_window(0, 500) {
        println!("\n⚠️  Vulnerability window: t={} (MinCut={:.2})", t, mc);
    }

    // Causal MinCut
    let causal_mincut = CausalMinCut::new(&db);

    println!("\nCausal Analysis:");
    let intervention = causal_mincut.minimum_intervention(f1_id);
    if !intervention.is_empty() {
        println!("To prevent '{}', intervene on:", db.get(f1_id).map(|h| h.name.as_str()).unwrap_or("?"));
        for id in intervention {
            if let Some(he) = db.get(id) {
                let strength = db.get_causal_strength(id, f1_id);
                println!("  → {} (causal strength: {:.2})", he.name, strength);
            }
        }
    }

    println!("\n✅ Phase 5 complete: Temporal MinCut, evolution, vulnerability detection\n");

    // ========== SUMMARY ==========
    let elapsed = start.elapsed();
    println!("═══════════════════════════════════════════════════════════════");
    println!("                    IMPLEMENTATION SUMMARY                      ");
    println!("═══════════════════════════════════════════════════════════════");
    println!("  Phase 1: ✅ TemporalInterval, TemporalHyperedge, TimeSeries");
    println!("  Phase 2: ✅ TemporalIndex, time-range queries");
    println!("  Phase 3: ✅ Spike-timing causal learning (STDP-like)");
    println!("  Phase 4: ✅ Temporal query language (AT TIME, CAUSES, etc.)");
    println!("  Phase 5: ✅ Temporal MinCut, evolution, causal intervention");
    println!("───────────────────────────────────────────────────────────────");
    println!("  Total hyperedges: {}", db.hyperedges.len());
    println!("  Causal relations: {}", db.causal_graph.len());
    println!("  Execution time:   {:?}", elapsed);
    println!("═══════════════════════════════════════════════════════════════");
}
