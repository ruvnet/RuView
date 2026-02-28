# Neuro-Symbolic Query Execution - Implementation Plan

## Overview

### Problem Statement

Current vector search in ruvector is purely neural (similarity-based): given a query vector, find the k most similar vectors by cosine/Euclidean distance. However, real-world queries often involve **logical constraints** that pure vector similarity cannot express:

**Examples of Unsupported Queries:**
- "Find vectors similar to X **AND** published after 2023 **AND** tagged as 'research'"
- "Find vectors similar to X **OR** similar to Y, **EXCLUDING** category 'spam'"
- "Find vectors where `metadata.price < 100` **AND** similarity > 0.8"
- "Find vectors in graph community C **AND** within 2 hops of node N"

**Current Limitations:**
- No support for boolean logic (AND, OR, NOT)
- Cannot filter by metadata attributes
- Cannot combine vector similarity with graph structure
- Forces post-processing filtering (inefficient)
- No way to express complex multi-modal queries

**Performance Impact:**
- Retrieving 10,000 vectors then filtering to 10 wastes 99.9% of computation
- No index acceleration for metadata predicates
- Cannot push down filters to HNSW search

### Proposed Solution

**Neuro-Symbolic Query Execution**: A hybrid query engine that combines neural vector similarity with symbolic logical constraints.

**Key Components:**

1. **Query Language**: Extend existing Cypher/SQL support with vector similarity operators
2. **Hybrid Scoring**: Combine vector similarity scores with predicate satisfaction
3. **Filter Pushdown**: Apply logical constraints during HNSW search (not after)
4. **Multi-Modal Indexing**: Index metadata attributes alongside vectors
5. **Constraint Propagation**: Use graph structure to prune search space

**Architecture:**
```
Query: "MATCH (v:Vector) WHERE vector_similarity(v.embedding, $query) > 0.8
        AND v.year >= 2023 AND v.category IN ['research', 'papers']
        RETURN v ORDER BY similarity DESC LIMIT 10"

      ↓ Parse & Optimize

Neural Component:        Symbolic Component:
vector_similarity > 0.8  year >= 2023 AND category IN [...]
      ↓                        ↓
  HNSW Search            Metadata Index
      ↓                        ↓
      └──────── Merge ─────────┘
               ↓
        Hybrid Scoring (α * neural + β * symbolic)
               ↓
        Top-K Results
```

### Expected Benefits

**Quantified Performance Improvements:**

| Query Type | Current (Post-Filter) | Neuro-Symbolic | Improvement |
|------------|----------------------|----------------|-------------|
| Similarity + 1 filter | 50ms (10K retrieved) | 5ms (100 retrieved) | **10x faster** |
| Similarity + 3 filters | 200ms (50K retrieved) | 8ms (200 retrieved) | **25x faster** |
| Complex boolean logic | Not supported | 15ms | **∞** (new capability) |
| Multi-modal query | Manual joins | 20ms | **50x faster** |

**Qualitative Benefits:**
- Express complex queries naturally (no manual post-processing)
- Efficient execution with filter pushdown
- Support for real-world use cases (e-commerce, research, RAG)
- Better accuracy through multi-modal fusion
- Graph-aware queries (community detection, path constraints)

## Technical Design

### Architecture Diagram (ASCII Art)

```
┌─────────────────────────────────────────────────────────────────┐
│              Neuro-Symbolic Query Execution Pipeline             │
└─────────────────────────────────────────────────────────────────┘

User Query (SQL/Cypher + Vector Similarity)
     │
     │  Example: "SELECT * FROM vectors
     │             WHERE cosine_similarity(embedding, $query) > 0.8
     │             AND category = 'research' AND year >= 2023
     │             ORDER BY similarity DESC LIMIT 10"
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Query Parser & AST Builder                                      │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Parse query into Abstract Syntax Tree (AST)               │ │
│  │  ┌──────────────────────────────────────────────────────┐  │ │
│  │  │ SELECT                                               │  │ │
│  │  │   WHERE                                              │  │ │
│  │  │     AND                                              │  │ │
│  │  │       ├─ cosine_similarity(emb, $q) > 0.8 [NEURAL]  │  │ │
│  │  │       ├─ category = 'research'        [SYMBOLIC]    │  │ │
│  │  │       └─ year >= 2023                 [SYMBOLIC]    │  │ │
│  │  │   ORDER BY similarity DESC                           │  │ │
│  │  │   LIMIT 10                                           │  │ │
│  │  └──────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Query Optimizer                                                 │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Analyze predicates and rewrite query for efficiency      │ │
│  │                                                             │ │
│  │  1. Predicate Pushdown:                                    │ │
│  │     Move filters into HNSW search (before candidate gen)   │ │
│  │                                                             │ │
│  │  2. Index Selection:                                       │ │
│  │     Choose best index for symbolic predicates              │ │
│  │     - category: inverted index                             │ │
│  │     - year: range index (B-tree)                           │ │
│  │                                                             │ │
│  │  3. Execution Strategy:                                    │ │
│  │     - If few categories: scan category index first         │ │
│  │     - If similarity selective: HNSW first, then filter     │ │
│  │     - If balanced: hybrid merge                            │ │
│  │                                                             │ │
│  │  4. Hybrid Scoring:                                        │ │
│  │     score = α * neural_sim + β * symbolic_score            │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Execution Plan                                                  │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  Step 1: HNSW Search (neural)                              │ │
│  │    - Target: similarity > 0.8                              │ │
│  │    - Candidate pool: ef=200                                │ │
│  │    - Early termination: collect ~100 candidates            │ │
│  │    - Filter during search: year >= 2023                    │ │
│  │    Output: {node_id, similarity} for ~100 candidates       │ │
│  │                                                             │ │
│  │  Step 2: Symbolic Filtering (metadata index)               │ │
│  │    - Lookup category index: category = 'research'          │ │
│  │    - Intersect with HNSW candidates                        │ │
│  │    Output: {node_id, similarity, metadata} for ~30 nodes   │ │
│  │                                                             │ │
│  │  Step 3: Hybrid Scoring                                    │ │
│  │    - Compute symbolic_score (e.g., recency bonus)          │ │
│  │    - Combined: 0.7 * similarity + 0.3 * symbolic_score     │ │
│  │    Output: {node_id, hybrid_score}                         │ │
│  │                                                             │ │
│  │  Step 4: Top-K Selection                                   │ │
│  │    - Sort by hybrid_score DESC                             │ │
│  │    - Return top 10                                         │ │
│  └────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
     │
     ▼
┌─────────────────────────────────────────────────────────────────┐
│  Result Set                                                      │
│  [{id: 42, similarity: 0.95, category: 'research', year: 2024}, │
│   {id: 137, similarity: 0.92, category: 'research', year: 2023},│
│   ...]                                                           │
└─────────────────────────────────────────────────────────────────┘


┌─────────────────────────────────────────────────────────────────┐
│              Indexing & Storage Architecture                     │
└─────────────────────────────────────────────────────────────────┘

Vector Data:
┌─────────────────────────────────────────────────────────────────┐
│  HNSW Index (vector similarity)                                  │
│  - Node ID → Embedding vector                                   │
│  - Graph structure for approximate NN search                    │
└─────────────────────────────────────────────────────────────────┘

Metadata Data:
┌─────────────────────────────────────────────────────────────────┐
│  Inverted Index (categorical attributes)                        │
│  - category → {node_ids}                                        │
│  - tag → {node_ids}                                             │
│  - author → {node_ids}                                          │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  B-Tree Index (range attributes)                                │
│  - year → sorted {node_ids}                                     │
│  - price → sorted {node_ids}                                    │
│  - timestamp → sorted {node_ids}                                │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  Roaring Bitmap Index (set operations)                          │
│  - Efficient AND/OR/NOT on node ID sets                         │
│  - Compressed storage for sparse sets                           │
└─────────────────────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────────────────────┐
│  Graph Index (structural constraints)                           │
│  - Community membership: community_id → {node_ids}              │
│  - k-hop neighborhoods: precomputed for common queries          │
│  - Path constraints: shortest path caches                       │
└─────────────────────────────────────────────────────────────────┘
```

### Core Data Structures (Rust)

```rust
// File: crates/ruvector-query/src/neuro_symbolic/mod.rs

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

/// Neuro-symbolic query execution engine
pub struct NeuroSymbolicEngine {
    /// HNSW index for vector similarity
    hnsw_index: Arc<HnswIndex>,

    /// Metadata indexes (inverted, B-tree, etc.)
    metadata_indexes: MetadataIndexes,

    /// Query optimizer
    optimizer: QueryOptimizer,

    /// Execution planner
    planner: ExecutionPlanner,

    /// Hybrid scoring configuration
    scoring_config: HybridScoringConfig,
}

/// Query representation (SQL/Cypher AST)
#[derive(Debug, Clone)]
pub struct Query {
    /// SELECT clause (which fields to return)
    pub select: Vec<String>,

    /// WHERE clause (predicates)
    pub where_clause: Option<Predicate>,

    /// ORDER BY clause
    pub order_by: Vec<OrderBy>,

    /// LIMIT clause
    pub limit: Option<usize>,

    /// OFFSET clause
    pub offset: Option<usize>,
}

/// Predicate tree (boolean logic)
#[derive(Debug, Clone)]
pub enum Predicate {
    /// Neural predicate: vector similarity
    VectorSimilarity {
        field: String,
        query_vector: Vec<f32>,
        operator: ComparisonOp,  // >, <, =
        threshold: f32,
        metric: SimilarityMetric,  // cosine, euclidean, dot
    },

    /// Symbolic predicate: metadata constraint
    Attribute {
        field: String,
        operator: ComparisonOp,
        value: Value,
    },

    /// Graph predicate: structural constraint
    Graph {
        constraint: GraphConstraint,
    },

    /// Boolean operators
    And(Box<Predicate>, Box<Predicate>),
    Or(Box<Predicate>, Box<Predicate>),
    Not(Box<Predicate>),
}

#[derive(Debug, Clone)]
pub enum GraphConstraint {
    /// Node in community
    InCommunity { community_id: u32 },

    /// Within k hops of node
    WithinKHops { source_node: u32, k: usize },

    /// On path between two nodes
    OnPath { source: u32, target: u32 },

    /// Has edge to node
    ConnectedTo { node_id: u32 },
}

#[derive(Debug, Clone, Copy)]
pub enum ComparisonOp {
    Eq,    // =
    Ne,    // !=
    Lt,    // <
    Le,    // <=
    Gt,    // >
    Ge,    // >=
    In,    // IN (...)
    Like,  // LIKE (string pattern)
}

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    List(Vec<Value>),
}

#[derive(Debug, Clone, Copy)]
pub enum SimilarityMetric {
    Cosine,
    Euclidean,
    DotProduct,
    L1,
}

/// Metadata indexing structures
pub struct MetadataIndexes {
    /// Inverted indexes for categorical fields
    inverted: HashMap<String, InvertedIndex>,

    /// B-tree indexes for range queries
    btree: HashMap<String, BTreeIndex>,

    /// Roaring bitmap for set operations
    bitmap_store: BitmapStore,

    /// Graph structural indexes
    graph_index: GraphStructureIndex,
}

/// Inverted index: field_value → {node_ids}
pub struct InvertedIndex {
    /// Map from value to posting list (node IDs)
    postings: HashMap<String, RoaringBitmap>,

    /// Statistics for query optimization
    stats: IndexStats,
}

/// B-tree index for range queries
pub struct BTreeIndex {
    /// Sorted map from value to node IDs
    tree: BTreeMap<OrderedValue, RoaringBitmap>,

    /// Statistics
    stats: IndexStats,
}

/// Roaring bitmap store for efficient set operations
pub struct BitmapStore {
    /// Node ID sets as compressed bitmaps
    bitmaps: HashMap<String, RoaringBitmap>,
}

/// Graph structure indexes
pub struct GraphStructureIndex {
    /// Community assignments
    communities: HashMap<u32, RoaringBitmap>,

    /// k-hop neighborhoods (precomputed)
    khop_cache: HashMap<(u32, usize), RoaringBitmap>,

    /// Shortest path cache
    path_cache: PathCache,
}

#[derive(Debug, Default)]
pub struct IndexStats {
    pub num_unique_values: usize,
    pub total_postings: usize,
    pub avg_posting_length: f64,
    pub selectivity: f64,  // fraction of nodes matching
}

/// Query execution plan
#[derive(Debug)]
pub struct ExecutionPlan {
    /// Ordered steps to execute
    pub steps: Vec<ExecutionStep>,

    /// Estimated cost
    pub estimated_cost: f64,

    /// Estimated result size
    pub estimated_results: usize,
}

#[derive(Debug)]
pub enum ExecutionStep {
    /// HNSW vector search
    VectorSearch {
        query_vector: Vec<f32>,
        similarity_threshold: f32,
        metric: SimilarityMetric,
        ef: usize,
        filters: Vec<InlineFilter>,  // Filters applied during search
    },

    /// Metadata index lookup
    IndexScan {
        index_name: String,
        predicate: Predicate,
    },

    /// Graph structure traversal
    GraphTraversal {
        constraint: GraphConstraint,
    },

    /// Set intersection (AND)
    Intersect {
        left: Box<ExecutionStep>,
        right: Box<ExecutionStep>,
    },

    /// Set union (OR)
    Union {
        left: Box<ExecutionStep>,
        right: Box<ExecutionStep>,
    },

    /// Set difference (NOT)
    Difference {
        left: Box<ExecutionStep>,
        right: Box<ExecutionStep>,
    },

    /// Hybrid scoring
    HybridScore {
        neural_scores: HashMap<u32, f32>,
        symbolic_scores: HashMap<u32, f32>,
        alpha: f32,  // neural weight
        beta: f32,   // symbolic weight
    },

    /// Top-K selection
    TopK {
        input: Box<ExecutionStep>,
        k: usize,
        order_by: Vec<OrderBy>,
    },
}

/// Filter applied during HNSW search (pushdown)
#[derive(Debug, Clone)]
pub struct InlineFilter {
    pub field: String,
    pub operator: ComparisonOp,
    pub value: Value,
}

/// Hybrid scoring configuration
#[derive(Debug, Clone)]
pub struct HybridScoringConfig {
    /// Weight for neural similarity score
    pub neural_weight: f32,

    /// Weight for symbolic score
    pub symbolic_weight: f32,

    /// Normalization method
    pub normalization: NormalizationMethod,
}

#[derive(Debug, Clone, Copy)]
pub enum NormalizationMethod {
    /// Min-max normalization [0, 1]
    MinMax,

    /// Z-score normalization
    ZScore,

    /// None (assume scores already normalized)
    None,
}

/// Query result
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    /// Matched node IDs
    pub node_ids: Vec<u32>,

    /// Neural similarity scores
    pub neural_scores: Vec<f32>,

    /// Symbolic scores (if applicable)
    pub symbolic_scores: Option<Vec<f32>>,

    /// Hybrid scores
    pub hybrid_scores: Vec<f32>,

    /// Metadata for each result
    pub metadata: Vec<HashMap<String, Value>>,

    /// Query execution statistics
    pub stats: QueryStats,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct QueryStats {
    /// Total execution time (milliseconds)
    pub total_time_ms: f64,

    /// Time breakdown by step
    pub step_times: Vec<(String, f64)>,

    /// Number of candidates evaluated
    pub candidates_evaluated: usize,

    /// Number of results returned
    pub results_returned: usize,

    /// Index usage
    pub indexes_used: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OrderBy {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Wrapper for ordered values in B-tree
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OrderedValue {
    Int(i64),
    Float(OrderedFloat<f64>),
    String(String),
}

use ordered_float::OrderedFloat;
use roaring::RoaringBitmap;
use std::collections::BTreeMap;
use std::sync::Arc;
```

### Key Algorithms (Pseudocode)

#### 1. Query Execution Algorithm

```python
function execute_neuro_symbolic_query(query: Query, engine: NeuroSymbolicEngine) -> QueryResult:
    """
    Execute neuro-symbolic query with hybrid scoring.

    Main algorithm: parse → optimize → plan → execute → score → return
    """
    start_time = now()

    # Step 1: Parse query into AST (already done, query is AST)
    # Step 2: Optimize query (predicate pushdown, index selection)
    optimized_query = engine.optimizer.optimize(query)

    # Step 3: Generate execution plan
    plan = engine.planner.create_plan(optimized_query)

    # Step 4: Execute plan steps
    result_set = execute_plan(plan, engine)

    # Step 5: Hybrid scoring
    if has_both_neural_and_symbolic(plan):
        result_set = apply_hybrid_scoring(
            result_set,
            engine.scoring_config
        )

    # Step 6: Apply ORDER BY and LIMIT
    result_set = sort_and_limit(
        result_set,
        query.order_by,
        query.limit,
        query.offset
    )

    # Step 7: Fetch metadata for results
    metadata = fetch_metadata(result_set.node_ids, query.select)

    execution_time = now() - start_time

    return QueryResult(
        node_ids=result_set.node_ids,
        neural_scores=result_set.neural_scores,
        symbolic_scores=result_set.symbolic_scores,
        hybrid_scores=result_set.hybrid_scores,
        metadata=metadata,
        stats=QueryStats(
            total_time_ms=execution_time,
            candidates_evaluated=result_set.candidates_evaluated,
            results_returned=len(result_set.node_ids),
            indexes_used=plan.indexes_used
        )
    )


function execute_plan(plan: ExecutionPlan, engine: NeuroSymbolicEngine) -> IntermediateResult:
    """
    Recursively execute plan steps.
    """
    results = None

    for step in plan.steps:
        match step:
            case VectorSearch:
                # HNSW search with optional filters
                results = execute_vector_search(step, engine.hnsw_index)

            case IndexScan:
                # Lookup in metadata index
                results = execute_index_scan(step, engine.metadata_indexes)

            case GraphTraversal:
                # Graph structure query
                results = execute_graph_traversal(step, engine.metadata_indexes.graph_index)

            case Intersect:
                # AND: set intersection
                left = execute_plan_step(step.left, engine)
                right = execute_plan_step(step.right, engine)
                results = intersect_results(left, right)

            case Union:
                # OR: set union
                left = execute_plan_step(step.left, engine)
                right = execute_plan_step(step.right, engine)
                results = union_results(left, right)

            case Difference:
                # NOT: set difference
                left = execute_plan_step(step.left, engine)
                right = execute_plan_step(step.right, engine)
                results = difference_results(left, right)

            case HybridScore:
                # Compute hybrid scores
                results = compute_hybrid_scores(
                    step.neural_scores,
                    step.symbolic_scores,
                    step.alpha,
                    step.beta
                )

            case TopK:
                # Select top-k results
                input_results = execute_plan_step(step.input, engine)
                results = select_top_k(input_results, step.k, step.order_by)

    return results


function execute_vector_search(step: VectorSearch, hnsw: HnswIndex) -> IntermediateResult:
    """
    HNSW search with filter pushdown.

    Key optimization: Apply symbolic filters during HNSW traversal
    to avoid generating candidates that will be filtered out anyway.
    """
    query_vector = step.query_vector
    similarity_threshold = step.similarity_threshold
    ef = step.ef
    inline_filters = step.filters

    # HNSW search with inline filtering
    candidates = []
    visited = set()

    # Start from entry point
    current_node = hnsw.entry_point
    layer = hnsw.max_layer

    while layer >= 0:
        # Greedy search at this layer
        while True:
            neighbors = hnsw.get_neighbors(current_node, layer)
            best_neighbor = None
            best_distance = float('inf')

            for neighbor in neighbors:
                if neighbor in visited:
                    continue

                # Apply inline filters BEFORE computing distance
                if not passes_inline_filters(neighbor, inline_filters, hnsw.metadata):
                    continue  # Skip this neighbor entirely

                # Compute distance only for filtered candidates
                distance = compute_distance(query_vector, hnsw.get_vector(neighbor))
                similarity = distance_to_similarity(distance, step.metric)

                if similarity >= similarity_threshold:
                    candidates.append((neighbor, similarity))

                if distance < best_distance:
                    best_distance = distance
                    best_neighbor = neighbor

                visited.add(neighbor)

            if best_neighbor is None:
                break  # No improvement

            current_node = best_neighbor

        layer -= 1

    # Sort candidates by similarity
    candidates.sort(key=lambda x: x[1], reverse=True)

    return IntermediateResult(
        node_ids=[node_id for node_id, _ in candidates],
        neural_scores=[score for _, score in candidates],
        candidates_evaluated=len(visited)
    )


function passes_inline_filters(node_id: u32, filters: List[InlineFilter], metadata: MetadataStore) -> bool:
    """
    Check if node passes all inline filters.

    This avoids computing distance for nodes that fail metadata constraints.
    """
    for filter in filters:
        node_value = metadata.get(node_id, filter.field)
        if not evaluate_predicate(node_value, filter.operator, filter.value):
            return False  # Failed a filter

    return True  # Passed all filters


function execute_index_scan(step: IndexScan, indexes: MetadataIndexes) -> IntermediateResult:
    """
    Scan metadata index to get matching node IDs.
    """
    index_name = step.index_name
    predicate = step.predicate

    match predicate:
        case Attribute(field, operator, value):
            if operator == ComparisonOp.Eq:
                # Exact match: use inverted index
                posting_list = indexes.inverted[field].lookup(value)
                return IntermediateResult(
                    node_ids=posting_list.to_vec(),
                    symbolic_scores=[1.0] * len(posting_list)  # Binary: matches or not
                )

            elif operator in [ComparisonOp.Lt, ComparisonOp.Le, ComparisonOp.Gt, ComparisonOp.Ge]:
                # Range query: use B-tree index
                matching_nodes = indexes.btree[field].range_query(operator, value)
                return IntermediateResult(
                    node_ids=matching_nodes.to_vec(),
                    symbolic_scores=[1.0] * len(matching_nodes)
                )

            elif operator == ComparisonOp.In:
                # IN query: union of inverted index lookups
                all_nodes = RoaringBitmap()
                for v in value.list:
                    posting_list = indexes.inverted[field].lookup(v)
                    all_nodes |= posting_list  # Union

                return IntermediateResult(
                    node_ids=all_nodes.to_vec(),
                    symbolic_scores=[1.0] * len(all_nodes)
                )


function execute_graph_traversal(step: GraphTraversal, graph_index: GraphStructureIndex) -> IntermediateResult:
    """
    Execute graph structural constraint.
    """
    match step.constraint:
        case InCommunity(community_id):
            # Lookup precomputed community membership
            node_ids = graph_index.communities.get(community_id)
            return IntermediateResult(
                node_ids=node_ids.to_vec(),
                symbolic_scores=[1.0] * len(node_ids)
            )

        case WithinKHops(source_node, k):
            # Lookup precomputed k-hop neighborhood
            key = (source_node, k)
            if key in graph_index.khop_cache:
                node_ids = graph_index.khop_cache[key]
            else:
                # Compute on-the-fly via BFS
                node_ids = compute_khop_neighbors(source_node, k, graph_index.graph)

            return IntermediateResult(
                node_ids=node_ids.to_vec(),
                symbolic_scores=[1.0 / (1 + distance)] for distance in range(len(node_ids))
            )

        case OnPath(source, target):
            # Check path cache
            path_nodes = graph_index.path_cache.get_path(source, target)
            return IntermediateResult(
                node_ids=path_nodes,
                symbolic_scores=[1.0] * len(path_nodes)
            )


function intersect_results(left: IntermediateResult, right: IntermediateResult) -> IntermediateResult:
    """
    Set intersection (AND): keep nodes in both sets.

    Use Roaring Bitmap for efficient intersection.
    """
    left_bitmap = RoaringBitmap.from_sorted(left.node_ids)
    right_bitmap = RoaringBitmap.from_sorted(right.node_ids)

    intersection = left_bitmap & right_bitmap  # Bitmap AND

    # Combine scores (average for simplicity)
    node_ids = intersection.to_vec()
    combined_scores = []
    for node_id in node_ids:
        left_score = left.get_score(node_id)
        right_score = right.get_score(node_id)
        combined_scores.append((left_score + right_score) / 2.0)

    return IntermediateResult(
        node_ids=node_ids,
        scores=combined_scores
    )


function apply_hybrid_scoring(result_set, config: HybridScoringConfig) -> IntermediateResult:
    """
    Combine neural and symbolic scores.

    Formula: hybrid_score = α * normalize(neural) + β * normalize(symbolic)
    """
    neural_scores = result_set.neural_scores
    symbolic_scores = result_set.symbolic_scores

    # Normalize scores to [0, 1]
    if config.normalization == NormalizationMethod.MinMax:
        neural_norm = min_max_normalize(neural_scores)
        symbolic_norm = min_max_normalize(symbolic_scores)
    elif config.normalization == NormalizationMethod.ZScore:
        neural_norm = z_score_normalize(neural_scores)
        symbolic_norm = z_score_normalize(symbolic_scores)
    else:
        neural_norm = neural_scores
        symbolic_norm = symbolic_scores

    # Combine with weights
    alpha = config.neural_weight
    beta = config.symbolic_weight
    hybrid_scores = [
        alpha * n + beta * s
        for n, s in zip(neural_norm, symbolic_norm)
    ]

    result_set.hybrid_scores = hybrid_scores
    return result_set
```

#### 2. Query Optimization

```python
function optimize_query(query: Query, optimizer: QueryOptimizer) -> Query:
    """
    Optimize query execution plan.

    Key optimizations:
    1. Predicate pushdown (filters into HNSW search)
    2. Index selection (choose best index for each predicate)
    3. Join reordering (cheapest predicates first)
    4. Early termination (stop when enough candidates found)
    """
    # Extract predicates from WHERE clause
    predicates = extract_predicates(query.where_clause)

    # Classify predicates
    neural_preds = [p for p in predicates if is_neural_predicate(p)]
    symbolic_preds = [p for p in predicates if is_symbolic_predicate(p)]
    graph_preds = [p for p in predicates if is_graph_predicate(p)]

    # Estimate selectivity for each predicate
    selectivities = {}
    for pred in predicates:
        selectivities[pred] = estimate_selectivity(pred, optimizer.stats)

    # Predicate pushdown: which filters can be applied during HNSW search?
    inline_filters = []
    post_filters = []

    for pred in symbolic_preds:
        if can_pushdown(pred):
            inline_filters.append(pred)
        else:
            post_filters.append(pred)

    # Index selection: choose best index for each symbolic predicate
    index_plan = {}
    for pred in symbolic_preds:
        best_index = choose_best_index(pred, optimizer.indexes, selectivities[pred])
        index_plan[pred] = best_index

    # Reorder predicates: most selective first
    ordered_predicates = sorted(predicates, key=lambda p: selectivities[p])

    # Build optimized execution plan
    optimized_query = rewrite_query(
        query,
        inline_filters=inline_filters,
        post_filters=post_filters,
        index_plan=index_plan,
        predicate_order=ordered_predicates
    )

    return optimized_query


function estimate_selectivity(predicate, stats) -> float:
    """
    Estimate fraction of nodes matching predicate.

    Uses index statistics (histograms, cardinality, etc.)
    """
    match predicate:
        case VectorSimilarity(threshold):
            # Estimate based on similarity distribution
            return estimate_similarity_selectivity(threshold, stats.similarity_histogram)

        case Attribute(field, operator, value):
            # Estimate based on attribute distribution
            if operator == ComparisonOp.Eq:
                return 1.0 / stats.cardinality[field]  # Uniform assumption
            elif operator in [Lt, Le, Gt, Ge]:
                return estimate_range_selectivity(field, operator, value, stats)
            elif operator == In:
                return len(value.list) / stats.cardinality[field]

        case Graph(constraint):
            # Estimate based on graph structure
            match constraint:
                case InCommunity(id):
                    return stats.community_sizes[id] / stats.total_nodes
                case WithinKHops(node, k):
                    return estimate_khop_size(node, k, stats) / stats.total_nodes


function can_pushdown(predicate) -> bool:
    """
    Check if predicate can be pushed into HNSW search.

    Only simple equality/range predicates on indexed fields can be pushed down.
    """
    match predicate:
        case Attribute(field, operator, value):
            # Can pushdown if operator is simple and field is indexed
            return operator in [Eq, Lt, Le, Gt, Ge, In] and is_indexed(field)

        case _:
            return False  # Complex predicates handled post-search
```

### API Design (Function Signatures)

```rust
// File: crates/ruvector-query/src/neuro_symbolic/mod.rs

impl NeuroSymbolicEngine {
    /// Create a new neuro-symbolic query engine
    pub fn new(
        hnsw_index: Arc<HnswIndex>,
        metadata_path: impl AsRef<Path>,
    ) -> Result<Self, QueryError>;

    /// Execute a query (SQL or Cypher syntax)
    pub fn execute_query(
        &self,
        query: &str,
    ) -> Result<QueryResult, QueryError>;

    /// Execute a parsed query (AST)
    pub fn execute_parsed_query(
        &self,
        query: Query,
    ) -> Result<QueryResult, QueryError>;

    /// Add metadata index for a field
    pub fn create_index(
        &mut self,
        field: &str,
        index_type: IndexType,
    ) -> Result<(), QueryError>;

    /// Update hybrid scoring configuration
    pub fn set_scoring_config(&mut self, config: HybridScoringConfig);

    /// Get query execution statistics
    pub fn stats(&self) -> QueryEngineStats;
}

#[derive(Debug, Clone, Copy)]
pub enum IndexType {
    Inverted,  // Categorical fields
    BTree,     // Range queries
    Bitmap,    // Set operations
}

impl Query {
    /// Parse SQL query string into AST
    pub fn parse_sql(query: &str) -> Result<Self, ParseError>;

    /// Parse Cypher query string into AST
    pub fn parse_cypher(query: &str) -> Result<Self, ParseError>;

    /// Validate query syntax and semantics
    pub fn validate(&self) -> Result<(), ValidationError>;
}

impl Predicate {
    /// Evaluate predicate on a node
    pub fn evaluate(
        &self,
        node_id: u32,
        vector_store: &VectorStore,
        metadata_store: &MetadataStore,
    ) -> bool;

    /// Extract referenced fields
    pub fn referenced_fields(&self) -> Vec<String>;

    /// Check if predicate is neural (vector similarity)
    pub fn is_neural(&self) -> bool;

    /// Check if predicate is symbolic (metadata)
    pub fn is_symbolic(&self) -> bool;

    /// Check if predicate is graph-structural
    pub fn is_graph_structural(&self) -> bool;
}

impl MetadataIndexes {
    /// Create indexes from metadata file
    pub fn from_metadata(path: impl AsRef<Path>) -> Result<Self, IndexError>;

    /// Add inverted index for field
    pub fn add_inverted_index(
        &mut self,
        field: &str,
        values: HashMap<String, Vec<u32>>,
    ) -> Result<(), IndexError>;

    /// Add B-tree index for field
    pub fn add_btree_index(
        &mut self,
        field: &str,
        values: Vec<(OrderedValue, u32)>,
    ) -> Result<(), IndexError>;

    /// Query inverted index
    pub fn query_inverted(
        &self,
        field: &str,
        value: &str,
    ) -> Option<&RoaringBitmap>;

    /// Query B-tree index (range)
    pub fn query_btree_range(
        &self,
        field: &str,
        operator: ComparisonOp,
        value: OrderedValue,
    ) -> Option<RoaringBitmap>;

    /// Intersect bitmaps (AND operation)
    pub fn intersect(&self, bitmaps: &[RoaringBitmap]) -> RoaringBitmap;

    /// Union bitmaps (OR operation)
    pub fn union(&self, bitmaps: &[RoaringBitmap]) -> RoaringBitmap;

    /// Difference bitmaps (NOT operation)
    pub fn difference(&self, left: &RoaringBitmap, right: &RoaringBitmap) -> RoaringBitmap;
}

#[derive(Debug, Default)]
pub struct QueryEngineStats {
    pub total_queries: u64,
    pub avg_query_time_ms: f64,
    pub cache_hit_rate: f64,
    pub avg_candidates_evaluated: f64,
}
```

## Integration Points

### Affected Crates/Modules

1. **`ruvector-query`** (New Crate)
   - New module: `src/neuro_symbolic/mod.rs` - Core engine
   - New module: `src/neuro_symbolic/parser.rs` - SQL/Cypher parser
   - New module: `src/neuro_symbolic/optimizer.rs` - Query optimizer
   - New module: `src/neuro_symbolic/planner.rs` - Execution planner
   - New module: `src/neuro_symbolic/indexes.rs` - Metadata indexing

2. **`ruvector-core`** (Integration)
   - Modified: `src/index/hnsw.rs` - Add filter callback support
   - Modified: `src/vector_store.rs` - Expose metadata API

3. **`ruvector-api`** (Exposure)
   - Modified: `src/query.rs` - Add neuro-symbolic query endpoint
   - New: `src/query/sql.rs` - SQL query interface
   - New: `src/query/cypher.rs` - Cypher query interface

4. **`ruvector-bindings`** (Language Bindings)
   - Modified: `python/src/lib.rs` - Expose query API
   - Modified: `nodejs/src/lib.rs` - Expose query API

### New Modules to Create

```
crates/ruvector-query/   # New crate
├── src/
│   ├── neuro_symbolic/
│   │   ├── mod.rs              # Core engine
│   │   ├── parser.rs           # Query parsing
│   │   ├── optimizer.rs        # Query optimization
│   │   ├── planner.rs          # Execution planning
│   │   ├── executor.rs         # Query execution
│   │   ├── indexes.rs          # Metadata indexing
│   │   ├── scoring.rs          # Hybrid scoring
│   │   └── stats.rs            # Statistics collection
│   └── lib.rs

examples/
├── neuro_symbolic_queries/
│   ├── sql_examples.rs         # SQL query examples
│   ├── cypher_examples.rs      # Cypher query examples
│   ├── hybrid_scoring.rs       # Hybrid scoring examples
│   └── README.md
```

### Dependencies on Other Features

**Depends On:**
- **HNSW Index**: Core vector search functionality
- **Existing Cypher Support**: Extend existing graph query support

**Synergies With:**
- **GNN-Guided Routing (Feature 1)**: Can use GNN for smarter query execution
- **Incremental Learning (Feature 2)**: Real-time index updates support streaming queries

**External Dependencies:**
- `sqlparser` - SQL parsing
- `cypher-parser` - Cypher parsing (if not already present)
- `roaring` - Roaring Bitmap for efficient set operations
- `serde` - Query serialization

## Regression Prevention

### What Existing Functionality Could Break

1. **Pure Vector Search Performance**
   - Risk: Adding metadata lookups slows down simple vector queries
   - Impact: Regression in baseline HNSW performance

2. **Memory Usage**
   - Risk: Metadata indexes consume excessive RAM
   - Impact: OOM on large datasets

3. **Query Correctness**
   - Risk: Filter pushdown logic has bugs, returns wrong results
   - Impact: Incorrect search results

4. **Cypher Compatibility**
   - Risk: Extending Cypher syntax breaks existing queries
   - Impact: Breaking change for existing users

### Test Cases to Prevent Regressions

```rust
// File: crates/ruvector-query/tests/neuro_symbolic_regression_tests.rs

#[test]
fn test_pure_vector_search_unchanged() {
    // Simple vector queries should have zero overhead
    let engine = setup_test_engine();

    // Baseline: pure HNSW search (no filters)
    let query_baseline = "SELECT * FROM vectors ORDER BY similarity(embedding, $query) DESC LIMIT 10";

    let start = Instant::now();
    let results = engine.execute_query(query_baseline).unwrap();
    let time_with_engine = start.elapsed();

    // Direct HNSW (without query engine)
    let start = Instant::now();
    let results_direct = engine.hnsw_index.search(&query_vector, 10).unwrap();
    let time_direct = start.elapsed();

    // Query engine should add <5% overhead
    let overhead = (time_with_engine.as_secs_f64() / time_direct.as_secs_f64()) - 1.0;
    assert!(overhead < 0.05, "Overhead: {:.2}%, expected <5%", overhead * 100.0);

    // Results should be identical
    assert_eq!(results.node_ids, results_direct.node_ids);
}

#[test]
fn test_filter_correctness() {
    // Filtered queries must return correct subset
    let engine = setup_test_engine_with_metadata();

    let query = "SELECT * FROM vectors
                 WHERE similarity(embedding, $query) > 0.8
                 AND category = 'research'
                 AND year >= 2023
                 LIMIT 10";

    let results = engine.execute_query(query).unwrap();

    // Verify each result matches ALL predicates
    for node_id in &results.node_ids {
        let similarity = compute_similarity(&query_vector, engine.get_vector(*node_id));
        assert!(similarity > 0.8, "Node {} similarity: {}, expected >0.8", node_id, similarity);

        let category = engine.get_metadata(*node_id, "category");
        assert_eq!(category, "research", "Node {} category: {}, expected 'research'", node_id, category);

        let year = engine.get_metadata(*node_id, "year").parse::<i32>().unwrap();
        assert!(year >= 2023, "Node {} year: {}, expected >=2023", node_id, year);
    }
}

#[test]
fn test_filter_pushdown_performance() {
    // Pushdown filters should be much faster than post-filtering
    let engine = setup_test_engine_with_metadata();

    // With pushdown (optimized)
    let query_pushdown = "SELECT * FROM vectors
                          WHERE similarity(embedding, $query) > 0.8
                          AND category = 'research'
                          LIMIT 10";

    let start = Instant::now();
    let results_pushdown = engine.execute_query(query_pushdown).unwrap();
    let time_pushdown = start.elapsed();

    // Without pushdown (post-filter, manual implementation)
    let all_results = engine.hnsw_index.search(&query_vector, 10000).unwrap();
    let start = Instant::now();
    let results_post: Vec<_> = all_results.into_iter()
        .filter(|r| r.similarity > 0.8)
        .filter(|r| engine.get_metadata(r.node_id, "category") == "research")
        .take(10)
        .collect();
    let time_post = start.elapsed();

    // Pushdown should be ≥5x faster
    let speedup = time_post.as_secs_f64() / time_pushdown.as_secs_f64();
    assert!(speedup >= 5.0, "Speedup: {:.1}x, expected ≥5x", speedup);

    // Results should be identical
    assert_eq!(results_pushdown.node_ids.len(), results_post.len());
}

#[test]
fn test_hybrid_scoring_correctness() {
    // Hybrid scores should combine neural and symbolic correctly
    let engine = setup_test_engine();
    engine.set_scoring_config(HybridScoringConfig {
        neural_weight: 0.7,
        symbolic_weight: 0.3,
        normalization: NormalizationMethod::MinMax,
    });

    let query = "SELECT * FROM vectors
                 WHERE similarity(embedding, $query) > 0.5
                 AND year >= 2020
                 ORDER BY hybrid_score DESC
                 LIMIT 10";

    let results = engine.execute_query(query).unwrap();

    // Verify hybrid score formula
    for i in 0..results.node_ids.len() {
        let neural = results.neural_scores[i];
        let symbolic = results.symbolic_scores.as_ref().unwrap()[i];

        // Normalize (min-max)
        let neural_norm = (neural - 0.5) / (1.0 - 0.5);  // Assuming min=0.5, max=1.0
        let symbolic_norm = (symbolic - 0.0) / (1.0 - 0.0);  // Assuming min=0.0, max=1.0

        let expected_hybrid = 0.7 * neural_norm + 0.3 * symbolic_norm;
        let actual_hybrid = results.hybrid_scores[i];

        assert!((expected_hybrid - actual_hybrid).abs() < 1e-5,
            "Hybrid score mismatch: expected {}, got {}", expected_hybrid, actual_hybrid);
    }
}

#[test]
fn test_boolean_logic_correctness() {
    // AND/OR/NOT operations must be correct
    let engine = setup_test_engine();

    // Test AND
    let query_and = "SELECT * FROM vectors
                     WHERE category = 'A' AND tag = 'X'";
    let results_and = engine.execute_query(query_and).unwrap();

    for node_id in &results_and.node_ids {
        assert_eq!(engine.get_metadata(*node_id, "category"), "A");
        assert_eq!(engine.get_metadata(*node_id, "tag"), "X");
    }

    // Test OR
    let query_or = "SELECT * FROM vectors
                    WHERE category = 'A' OR category = 'B'";
    let results_or = engine.execute_query(query_or).unwrap();

    for node_id in &results_or.node_ids {
        let category = engine.get_metadata(*node_id, "category");
        assert!(category == "A" || category == "B");
    }

    // Test NOT
    let query_not = "SELECT * FROM vectors
                     WHERE category = 'A' AND NOT tag = 'X'";
    let results_not = engine.execute_query(query_not).unwrap();

    for node_id in &results_not.node_ids {
        assert_eq!(engine.get_metadata(*node_id, "category"), "A");
        assert_ne!(engine.get_metadata(*node_id, "tag"), "X");
    }
}
```

### Backward Compatibility Strategy

1. **Opt-In Feature**
   - Neuro-symbolic queries are opt-in (require explicit SQL/Cypher syntax)
   - Existing vector search API unchanged

2. **Graceful Degradation**
   - If metadata indexes not available, fallback to post-filtering
   - Log warning but do not crash

3. **Configuration**
   ```yaml
   query:
     neuro_symbolic:
       enabled: true  # Default: true
       metadata_indexes: true  # Default: true
       hybrid_scoring: true  # Default: true
   ```

4. **API Versioning**
   - New endpoints for neuro-symbolic queries (`/query/sql`, `/query/cypher`)
   - Existing endpoints (`/search`) unchanged

## Implementation Phases

### Phase 1: Core Infrastructure (Week 1-2)

**Goal**: Query parsing and basic execution

**Tasks**:
1. Implement SQL/Cypher parser
2. Build AST representation
3. Implement basic query executor (no optimization)
4. Unit tests for parsing and execution

**Deliverables**:
- `neuro_symbolic/parser.rs`
- `neuro_symbolic/executor.rs`
- Passing unit tests

**Success Criteria**:
- Can parse and execute simple queries (vector similarity only)
- Correct results (matches HNSW baseline)

### Phase 2: Metadata Indexing (Week 2-3)

**Goal**: Support symbolic predicates

**Tasks**:
1. Implement inverted index for categorical fields
2. Implement B-tree index for range queries
3. Integrate Roaring Bitmap for set operations
4. Test index correctness and performance

**Deliverables**:
- `neuro_symbolic/indexes.rs`
- Index creation and query APIs
- Benchmark report

**Success Criteria**:
- Indexes correctly return matching nodes
- Index queries <10ms for typical workloads
- Memory overhead <20% of vector data

### Phase 3: Filter Pushdown (Week 3-4)

**Goal**: Optimize query execution

**Tasks**:
1. Implement filter pushdown into HNSW search
2. Modify HNSW to support filter callbacks
3. Benchmark speedup vs post-filtering
4. Test correctness of pushdown logic

**Deliverables**:
- Modified `hnsw.rs` with filter support
- `neuro_symbolic/optimizer.rs`
- Performance benchmarks

**Success Criteria**:
- ≥5x speedup for filtered queries
- Zero correctness regressions
- Works with complex boolean logic (AND/OR/NOT)

### Phase 4: Hybrid Scoring (Week 4-5)

**Goal**: Combine neural and symbolic scores

**Tasks**:
1. Implement hybrid scoring algorithm
2. Add score normalization methods
3. Tune weights (α, β) for best results
4. Test on real-world datasets

**Deliverables**:
- `neuro_symbolic/scoring.rs`
- Hybrid scoring benchmarks
- Configuration guide

**Success Criteria**:
- Hybrid queries improve relevance metrics (NDCG, MRR)
- Configurable weights work as expected
- Performance <20ms for typical queries

### Phase 5: Production Hardening (Week 5-6)

**Goal**: Production-ready feature

**Tasks**:
1. Add comprehensive error handling
2. Write documentation and examples
3. Stress testing (large datasets, complex queries)
4. Integration with existing Cypher support

**Deliverables**:
- Full error handling
- User documentation
- Example queries
- Regression test suite

**Success Criteria**:
- Zero crashes in stress tests
- Documentation complete
- Ready for alpha release

## Success Metrics

### Performance Benchmarks

**Primary Metrics** (Must Achieve):

| Query Type | Baseline (Post-Filter) | Neuro-Symbolic | Target Improvement |
|------------|------------------------|----------------|--------------------|
| Similarity + 1 filter | 50ms | 5ms | **10x faster** |
| Similarity + 3 filters | 200ms | 8ms | **25x faster** |
| Complex boolean (AND/OR/NOT) | N/A (manual) | 15ms | **New capability** |
| Multi-modal (vector + graph) | 500ms (manual joins) | 20ms | **25x faster** |

**Secondary Metrics**:

| Metric | Target |
|--------|--------|
| Index memory overhead | <20% of vector data |
| Query parsing time | <1ms |
| Hybrid scoring overhead | <2ms |
| Concurrent query throughput | Same as baseline |

### Accuracy Metrics

**Relevance Improvement** (on benchmark datasets):
- NDCG@10: +15% (hybrid scoring vs pure vector)
- MRR (Mean Reciprocal Rank): +20%
- Precision@10: +10%

**Correctness**:
- 100% of filtered results match all predicates
- Zero false positives or false negatives

### Memory/Latency Targets

**Memory**:
- Inverted indexes: <100MB per 1M nodes (categorical fields)
- B-tree indexes: <50MB per 1M nodes (range fields)
- Total overhead: <20% of vector index size

**Latency**:
- Simple query (1 filter): <10ms
- Complex query (3+ filters): <20ms
- Hybrid scoring: <5ms overhead
- P99 latency: <50ms

**Throughput**:
- Concurrent queries: Same as baseline HNSW
- No lock contention on indexes

## Risks and Mitigations

### Technical Risks

**Risk 1: Query Parser Complexity**

*Probability: Medium | Impact: Medium*

**Description**: SQL/Cypher parsing is complex, could have bugs or performance issues.

**Mitigation**:
- Use established parsing libraries (`sqlparser`, `cypher-parser`)
- Extensive test suite with edge cases
- Validate AST before execution
- Provide query validation tool

**Contingency**: Start with simple query subset, expand incrementally.

---

**Risk 2: Index Memory Overhead**

*Probability: High | Impact: Medium*

**Description**: Metadata indexes could consume excessive memory on large datasets.

**Mitigation**:
- Use compressed indexes (Roaring Bitmap for sparse sets)
- Make indexing optional (user chooses which fields to index)
- Monitor memory usage in tests
- Provide index size estimation tool

**Contingency**: Support external indexes (e.g., SQLite) for low-memory environments.

---

**Risk 3: Filter Pushdown Bugs**

*Probability: Medium | Impact: Critical*

**Description**: Incorrect filter logic could return wrong results.

**Mitigation**:
- Extensive correctness testing (ground truth validation)
- Compare pushdown results vs post-filtering
- Add assertion checks in debug builds
- Fuzzing for edge cases

**Contingency**: Add "safe mode" that validates results against post-filtering.

---

**Risk 4: Hybrid Scoring Tuning Difficulty**

*Probability: High | Impact: Low*

**Description**: Users may struggle to tune α/β weights for hybrid scoring.

**Mitigation**:
- Provide automatic weight tuning (based on query logs)
- Document recommended defaults for common use cases
- Add visualization tools for score distributions
- Support A/B testing framework

**Contingency**: Default to pure neural scoring (α=1, β=0) if user unsure.

---

**Risk 5: Cypher Integration Conflicts**

*Probability: Low | Impact: Medium*

**Description**: Extending Cypher syntax could conflict with existing graph queries.

**Mitigation**:
- Careful syntax design (use reserved keywords)
- Version Cypher extensions separately
- Extensive compatibility testing
- Document syntax differences

**Contingency**: Use separate query language (e.g., extended SQL only) if conflicts arise.

---

### Summary Risk Matrix

| Risk | Probability | Impact | Mitigation Priority |
|------|-------------|--------|---------------------|
| Query parser complexity | Medium | Medium | Medium |
| Index memory overhead | High | Medium | **HIGH** |
| Filter pushdown bugs | Medium | Critical | **CRITICAL** |
| Hybrid scoring tuning | High | Low | LOW |
| Cypher integration conflicts | Low | Medium | Medium |

---

## Next Steps

1. **Prototype Phase 1**: Build SQL parser and basic executor (1 week)
2. **Validate Queries**: Test on simple queries, measure correctness (2 days)
3. **Add Metadata Indexes**: Implement inverted + B-tree indexes (1 week)
4. **Benchmark Performance**: Measure speedup vs post-filtering (3 days)
5. **Iterate**: Optimize based on profiling (ongoing)

**Key Decision Points**:
- After Phase 1: Is query parsing fast enough? (<1ms target)
- After Phase 3: Does filter pushdown work correctly? (Zero regressions)
- After Phase 4: Does hybrid scoring improve relevance? (+10% NDCG required)

**Go/No-Go Criteria**:
- ✅ 5x+ speedup on filtered queries
- ✅ Zero correctness regressions
- ✅ Memory overhead <20%
- ✅ Improved relevance metrics
