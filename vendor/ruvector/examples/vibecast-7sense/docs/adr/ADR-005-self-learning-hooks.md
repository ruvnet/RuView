# ADR-005: Self-Learning and Hooks Integration

## Status

Proposed

## Date

2026-01-15

## Context

7sense processes bioacoustic data through Perch 2.0 embeddings (1536-D vectors) stored in RuVector with HNSW indexing. To maximize the value of this acoustic geometry, we need a self-learning system that:

1. Continuously improves retrieval quality based on user feedback
2. Discovers and consolidates successful clustering configurations
3. Learns species-specific embedding characteristics over time
4. Prevents catastrophic forgetting when adapting to new domains (marine vs avian vs terrestrial)

RuVector includes a built-in GNN layer designed for index self-improvement, and the claude-flow framework provides a comprehensive hooks system with 27 hooks and 12 background workers that can orchestrate continuous learning pipelines.

## Decision

We will implement a four-stage learning loop architecture integrated with claude-flow hooks, utilizing SONA (Self-Optimizing Neural Architecture) patterns and EWC++ (Elastic Weight Consolidation) for continual learning without forgetting.

### Learning Loop Architecture

```
+-------------------+     +------------------+     +-------------------+     +---------------------+
|     RETRIEVE      | --> |      JUDGE       | --> |      DISTILL      | --> |    CONSOLIDATE      |
| (HNSW + Pattern)  |     | (Verdict System) |     | (LoRA Fine-tune)  |     | (EWC++ Integration) |
+-------------------+     +------------------+     +-------------------+     +---------------------+
         ^                                                                            |
         |                                                                            |
         +----------------------------------------------------------------------------+
                                    Continuous Feedback Loop
```

#### Stage 1: RETRIEVE

Fetch relevant patterns from the ReasoningBank using HNSW-indexed vector search:

```bash
# Search for similar bioacoustic analysis patterns
npx @claude-flow/cli@latest memory search \
  --query "whale song clustering high-frequency harmonics" \
  --namespace patterns \
  --limit 5 \
  --threshold 0.7

# Retrieve species-specific embedding characteristics
npx @claude-flow/cli@latest hooks intelligence pattern-search \
  --query "humpback whale vocalization" \
  --namespace species \
  --top-k 3
```

Performance characteristics:
- HNSW retrieval: 150x-12,500x faster than brute force
- Pattern matching: 761 decisions/sec
- Sub-millisecond adaptation via SONA

#### Stage 2: JUDGE

Evaluate retrieved patterns with a verdict system that scores relevance and success:

```typescript
interface BioacousticVerdict {
  pattern_id: string;
  task_type: 'clustering' | 'motif_discovery' | 'species_identification' | 'anomaly_detection';
  verdict: 'success' | 'partial' | 'failure';
  confidence: number;  // 0.0 - 1.0
  metrics: {
    silhouette_score?: number;      // For clustering
    retrieval_precision?: number;   // For search quality
    user_correction_rate?: number;  // For feedback integration
    snr_threshold_effectiveness?: number;
  };
  feedback_source: 'automatic' | 'user_correction' | 'expert_annotation';
}
```

Verdict aggregation rules:
- Success (confidence > 0.85): Promote pattern to long-term memory
- Partial (0.5 < confidence < 0.85): Mark for refinement
- Failure (confidence < 0.5): Demote or archive with failure context

#### Stage 3: DISTILL

Extract key learnings via LoRA (Low-Rank Adaptation) fine-tuning:

```bash
# Train neural patterns on successful bioacoustic analysis
npx @claude-flow/cli@latest hooks intelligence trajectory-start \
  --task "clustering whale songs by call type" \
  --agent "bioacoustic-analyzer"

# Record analysis steps
npx @claude-flow/cli@latest hooks intelligence trajectory-step \
  --trajectory-id "$TRAJ_ID" \
  --action "applied hierarchical clustering with ward linkage" \
  --result "silhouette score 0.78" \
  --quality 0.85

# Complete trajectory with success
npx @claude-flow/cli@latest hooks intelligence trajectory-end \
  --trajectory-id "$TRAJ_ID" \
  --success true \
  --feedback "user confirmed 23/25 clusters as valid call types"
```

LoRA benefits for bioacoustics:
- 99% parameter reduction (critical for edge deployment on field sensors)
- 10-100x faster training than full fine-tuning
- Minimal memory footprint for continuous learning

#### Stage 4: CONSOLIDATE

Prevent catastrophic forgetting via EWC++ when learning new domains:

```bash
# Force SONA learning cycle with EWC++ consolidation
npx @claude-flow/cli@latest hooks intelligence learn \
  --consolidate true \
  --trajectory-ids "$WHALE_TRAJ,$BIRD_TRAJ,$INSECT_TRAJ"
```

EWC++ strategy for bioacoustics:
- Compute Fisher information matrix for critical embedding dimensions
- Penalize changes to weights important for existing species recognition
- Allow plasticity for new acoustic domains (marine -> avian -> terrestrial)

### Claude-Flow Hooks Integration

#### Pre-Task Hook: Route Bioacoustic Analysis Tasks

The `pre-task` hook routes incoming analysis requests to optimal processing paths:

```bash
# Before starting any bioacoustic analysis
npx @claude-flow/cli@latest hooks pre-task \
  --task-id "analysis-$(date +%s)" \
  --description "cluster humpback whale songs from Pacific Northwest dataset"
```

Routing decisions based on task characteristics:

| Task Type | Recommended Agent | Model Tier | Rationale |
|-----------|-------------------|------------|-----------|
| Simple retrieval | retrieval-agent | Haiku | Fast kNN lookup |
| Clustering | clustering-specialist | Sonnet | Algorithm selection |
| Motif discovery | sequence-analyzer | Sonnet | Temporal pattern analysis |
| Cross-species analysis | bioacoustic-expert | Opus | Complex reasoning |
| Anomaly detection | anomaly-detector | Haiku | Real-time processing |
| Embedding refinement | ml-specialist | Opus | Architecture decisions |

Pre-task also retrieves relevant patterns:

```bash
# Get routing recommendation with pattern retrieval
npx @claude-flow/cli@latest hooks route \
  --task "identify dialect variations in orca pod communications" \
  --context "Pacific Northwest, 2024 field recordings"
```

Output includes:
- Recommended agent type and model tier
- Top-3 similar successful patterns from memory
- Suggested HNSW parameters based on past success
- Estimated confidence and processing time

#### Post-Task Hook: Store Successful Patterns

After successful analysis, store the pattern for future retrieval:

```bash
# Record task completion
npx @claude-flow/cli@latest hooks post-task \
  --task-id "$TASK_ID" \
  --success true \
  --agent "clustering-specialist" \
  --quality 0.92

# Store the successful pattern
npx @claude-flow/cli@latest memory store \
  --namespace patterns \
  --key "whale-clustering-hierarchical-ward-2026-01" \
  --value '{
    "task_type": "clustering",
    "species_group": "cetacean",
    "algorithm": "hierarchical",
    "linkage": "ward",
    "distance_metric": "cosine",
    "min_cluster_size": 5,
    "silhouette_score": 0.78,
    "num_clusters_discovered": 23,
    "snr_threshold": 15,
    "embedding_preprocessing": "l2_normalize",
    "hnsw_params": {"ef_construction": 200, "M": 32}
  }'

# Train neural patterns on the success
npx @claude-flow/cli@latest hooks post-edit \
  --file "analysis-results.json" \
  --success true \
  --train-neural true
```

#### Pre-Edit Hook: Context for Embedding Refinement

Before modifying embedding configurations or HNSW parameters:

```bash
# Get context before editing embedding pipeline
npx @claude-flow/cli@latest hooks pre-edit \
  --file "src/embeddings/perch_config.rs" \
  --operation "refactor"
```

Returns:
- Related patterns that worked for similar configurations
- Agent recommendations for the edit type
- Risk assessment for the change
- Suggested validation tests

#### Post-Edit Hook: Train Neural Patterns

After successful configuration changes:

```bash
# Record successful embedding refinement
npx @claude-flow/cli@latest hooks post-edit \
  --file "src/embeddings/perch_config.rs" \
  --success true \
  --agent "ml-specialist"

# Store the refinement as a pattern
npx @claude-flow/cli@latest hooks intelligence pattern-store \
  --pattern "HNSW ef_search=150 optimal for whale song retrieval" \
  --type "configuration" \
  --confidence 0.88 \
  --metadata '{"species": "cetacean", "corpus_size": 500000}'
```

### Memory Namespaces for Bioacoustics

#### Namespace: `patterns`

Stores successful clustering and analysis configurations:

```bash
# Store clustering pattern
npx @claude-flow/cli@latest memory store \
  --namespace patterns \
  --key "birdsong-dbscan-dawn-chorus" \
  --value '{
    "algorithm": "DBSCAN",
    "eps": 0.15,
    "min_samples": 3,
    "preprocessing": ["l2_normalize", "pca_128"],
    "context": "dawn_chorus",
    "success_rate": 0.91,
    "species_groups": ["passerine", "corvid"],
    "temporal_window": "04:00-07:00"
  }'

# Search for relevant patterns
npx @claude-flow/cli@latest memory search \
  --namespace patterns \
  --query "clustering algorithm for dense dawn chorus recordings"
```

Pattern schema:
```typescript
interface ClusteringPattern {
  algorithm: 'DBSCAN' | 'HDBSCAN' | 'hierarchical' | 'kmeans' | 'spectral';
  parameters: Record<string, number | string>;
  preprocessing: string[];
  context: string;
  success_rate: number;
  species_groups: string[];
  environmental_conditions?: {
    habitat?: string;
    time_of_day?: string;
    season?: string;
    weather?: string;
  };
  hnsw_tuning?: {
    ef_construction: number;
    ef_search: number;
    M: number;
  };
}
```

#### Namespace: `motifs`

Stores discovered sequence patterns and syntactic structures:

```bash
# Store discovered motif
npx @claude-flow/cli@latest memory store \
  --namespace motifs \
  --key "humpback-song-unit-sequence-A" \
  --value '{
    "species": "Megaptera novaeangliae",
    "pattern_type": "song_unit_sequence",
    "sequence": ["A1", "A2", "B1", "A1", "C1"],
    "transition_probabilities": {
      "A1->A2": 0.85,
      "A2->B1": 0.72,
      "B1->A1": 0.68,
      "A1->C1": 0.45
    },
    "typical_duration_ms": 45000,
    "occurrence_rate": 0.34,
    "recording_ids": ["rec_2024_001", "rec_2024_002"],
    "discovered_by": "sequence-analyzer",
    "confidence": 0.89
  }'

# Search for similar motifs
npx @claude-flow/cli@latest memory search \
  --namespace motifs \
  --query "humpback whale song phrase transitions"
```

Motif schema:
```typescript
interface SequenceMotif {
  species: string;
  pattern_type: 'song_unit_sequence' | 'call_response' | 'alarm_cascade' | 'contact_pattern';
  sequence: string[];
  transition_probabilities: Record<string, number>;
  typical_duration_ms: number;
  occurrence_rate: number;
  temporal_context?: {
    time_of_day?: string;
    season?: string;
    behavioral_context?: string;
  };
  recording_ids: string[];
  discovered_by: string;
  confidence: number;
  validation_status: 'automatic' | 'expert_verified' | 'disputed';
}
```

#### Namespace: `species`

Stores species-specific embedding characteristics:

```bash
# Store species embedding profile
npx @claude-flow/cli@latest memory store \
  --namespace species \
  --key "orca-pacific-northwest-resident" \
  --value '{
    "species": "Orcinus orca",
    "population": "Southern Resident",
    "location": "Pacific Northwest",
    "embedding_characteristics": {
      "centroid_cluster_distance": 0.12,
      "intra_pod_variance": 0.08,
      "inter_pod_variance": 0.23,
      "frequency_range_hz": [500, 12000],
      "dominant_frequencies_hz": [2000, 5000, 8000]
    },
    "retrieval_optimization": {
      "optimal_k": 15,
      "distance_threshold": 0.25,
      "ef_search": 200
    },
    "known_call_types": 34,
    "dialect_markers": ["S01", "S02", "S03"],
    "last_updated": "2026-01-15"
  }'

# Search for species characteristics
npx @claude-flow/cli@latest memory search \
  --namespace species \
  --query "cetacean vocalization embedding characteristics Pacific"
```

Species schema:
```typescript
interface SpeciesEmbeddingProfile {
  species: string;
  population?: string;
  location?: string;
  embedding_characteristics: {
    centroid_cluster_distance: number;
    intra_population_variance: number;
    inter_population_variance: number;
    frequency_range_hz: [number, number];
    dominant_frequencies_hz: number[];
    embedding_norm_range?: [number, number];
  };
  retrieval_optimization: {
    optimal_k: number;
    distance_threshold: number;
    ef_search: number;
    ef_construction?: number;
  };
  known_call_types: number;
  dialect_markers?: string[];
  acoustic_niche?: {
    typical_snr_db: number;
    overlap_species: string[];
    distinguishing_features: string[];
  };
  last_updated: string;
}
```

### Background Workers Utilization

#### Worker: `optimize` - HNSW Parameter Tuning

Continuously optimizes HNSW parameters based on retrieval quality:

```bash
# Dispatch HNSW optimization worker
npx @claude-flow/cli@latest hooks worker dispatch \
  --trigger optimize \
  --context "bioacoustic-hnsw" \
  --priority high

# Check optimization status
npx @claude-flow/cli@latest hooks worker status
```

Optimization targets:
- `ef_construction`: Balance between index build time and recall
- `ef_search`: Balance between query latency and accuracy
- `M`: Balance between memory usage and graph connectivity

Automated tuning workflow:
1. Sample recent queries and their success rates
2. Run parameter sweep on subset
3. Evaluate recall@k and latency
4. Apply best parameters if improvement > 5%
5. Store successful configuration in `patterns` namespace

```typescript
interface HNSWOptimizationResult {
  previous_params: { ef_construction: number; ef_search: number; M: number };
  new_params: { ef_construction: number; ef_search: number; M: number };
  improvement: {
    recall_at_10: number;  // Percentage improvement
    latency_p99_ms: number;
    memory_mb: number;
  };
  evaluation_corpus_size: number;
  applied: boolean;
  timestamp: string;
}
```

#### Worker: `consolidate` - Memory Consolidation

Consolidates learned patterns and prevents memory fragmentation:

```bash
# Dispatch consolidation worker (low priority, runs during idle)
npx @claude-flow/cli@latest hooks worker dispatch \
  --trigger consolidate \
  --priority low \
  --background true
```

Consolidation operations:
1. Merge similar patterns within each namespace
2. Archive low-confidence or stale patterns
3. Update pattern embeddings for improved retrieval
4. Compute and cache centroid patterns for fast routing
5. Run EWC++ to protect critical learned weights

```bash
# Force SONA learning cycle with consolidation
npx @claude-flow/cli@latest hooks intelligence learn \
  --consolidate true
```

Consolidation schedule:
- Hourly: Merge patterns with >0.95 similarity
- Daily: Archive patterns not accessed in 30 days
- Weekly: Full EWC++ consolidation pass

#### Worker: `audit` - Data Quality Checks

Validates embedding quality and detects drift:

```bash
# Dispatch audit worker
npx @claude-flow/cli@latest hooks worker dispatch \
  --trigger audit \
  --context "embedding-quality" \
  --priority critical
```

Audit checks:
1. **Embedding health**: Detect NaN, infinity, or collapsed embeddings
2. **Distribution drift**: Compare embedding statistics over time
3. **Retrieval quality**: Sample-based precision/recall checks
4. **Label consistency**: Cross-reference with expert annotations
5. **Temporal coherence**: Verify sequence relationships

```typescript
interface AuditResult {
  check_type: 'embedding_health' | 'distribution_drift' | 'retrieval_quality' | 'label_consistency';
  status: 'pass' | 'warning' | 'fail';
  metrics: {
    nan_rate?: number;
    norm_variance?: number;
    drift_score?: number;
    precision_at_10?: number;
    consistency_rate?: number;
  };
  affected_recordings?: string[];
  recommended_action?: string;
  timestamp: string;
}
```

Automated responses:
- Warning: Log and notify, continue processing
- Fail: Pause ingestion, alert operators, revert to last known good state

### Transfer Learning from Related Projects

#### Project Transfer Protocol

Leverage patterns from related bioacoustic projects:

```bash
# Transfer patterns from a related whale research project
npx @claude-flow/cli@latest hooks transfer \
  --source-path "/projects/cetacean-acoustics" \
  --min-confidence 0.8 \
  --filter "species:cetacean"

# Transfer from IPFS-distributed pattern registry
npx @claude-flow/cli@latest hooks transfer store \
  --pattern-id "marine-mammal-clustering-v2"
```

Transfer eligibility criteria:
1. Source project confidence > 0.8
2. Domain overlap > 50% (based on species groups)
3. No conflicting patterns in target
4. Embedding model compatibility (same Perch version)

Transfer adaptation process:
1. Retrieve candidate patterns from source
2. Validate against target domain characteristics
3. Apply domain adaptation if needed (fine-tune on local data)
4. Integrate with reduced initial confidence (0.7x)
5. Gradually increase confidence based on local success

```bash
# Check transfer candidates
npx @claude-flow/cli@latest transfer store-search \
  --query "bioacoustic clustering" \
  --category "marine" \
  --min-rating 4.0 \
  --verified true
```

### Feedback Loops: User Corrections to Embedding Refinement

#### Correction Capture

```typescript
interface UserCorrection {
  correction_id: string;
  timestamp: string;
  user_id: string;
  expertise_level: 'novice' | 'intermediate' | 'expert' | 'domain_expert';
  correction_type: 'cluster_assignment' | 'species_label' | 'call_type' | 'sequence_boundary';
  original_prediction: {
    value: string;
    confidence: number;
    source: 'automatic' | 'pattern_match';
  };
  corrected_value: string;
  affected_segments: string[];
  context?: string;
}
```

#### Feedback Integration Pipeline

```bash
# Step 1: Log user correction
npx @claude-flow/cli@latest memory store \
  --namespace corrections \
  --key "correction-$(date +%s)-$USER" \
  --value '{
    "correction_type": "species_label",
    "original": {"value": "Megaptera novaeangliae", "confidence": 0.72},
    "corrected": "Balaenoptera musculus",
    "segment_ids": ["seg_001", "seg_002"],
    "user_expertise": "domain_expert"
  }'

# Step 2: Trigger learning from correction
npx @claude-flow/cli@latest hooks intelligence trajectory-start \
  --task "learn from species misclassification correction"

npx @claude-flow/cli@latest hooks intelligence trajectory-step \
  --trajectory-id "$TRAJ_ID" \
  --action "analyzed embedding distance between humpback and blue whale" \
  --result "found confounding frequency overlap in low-SNR segments" \
  --quality 0.7

npx @claude-flow/cli@latest hooks intelligence trajectory-end \
  --trajectory-id "$TRAJ_ID" \
  --success true \
  --feedback "updated SNR threshold from 10 to 15 dB for cetacean classification"

# Step 3: Update species namespace
npx @claude-flow/cli@latest memory store \
  --namespace species \
  --key "blue-whale-humpback-distinction" \
  --value '{
    "confusion_pair": ["Megaptera novaeangliae", "Balaenoptera musculus"],
    "distinguishing_features": ["frequency_range", "call_duration"],
    "recommended_snr_threshold": 15,
    "embedding_distance_threshold": 0.18
  }'
```

#### Feedback Weight by Expertise

| Expertise Level | Weight | Trigger Threshold | Immediate Action |
|-----------------|--------|-------------------|------------------|
| Domain Expert | 1.0 | 1 correction | Update pattern |
| Expert | 0.8 | 2 corrections | Update pattern |
| Intermediate | 0.5 | 5 corrections | Flag for review |
| Novice | 0.2 | 10 corrections | Queue for expert |

#### Continuous Refinement Loop

```
User Correction
      |
      v
+------------------+
| Correction Store |  (namespace: corrections)
+------------------+
      |
      v
+------------------+
| Pattern Analysis |  (identify affected patterns)
+------------------+
      |
      v
+------------------+
| Verdict Update   |  (reduce confidence of failed patterns)
+------------------+
      |
      v
+------------------+
| SONA Learning    |  (trajectory-based fine-tuning)
+------------------+
      |
      v
+------------------+
| EWC++ Consolidate|  (protect other learned patterns)
+------------------+
      |
      v
+------------------+
| Pattern Update   |  (store refined pattern)
+------------------+
      |
      v
Improved Retrieval
```

### Implementation Checklist

#### Phase 1: Core Infrastructure (Week 1-2)

- [ ] Set up memory namespaces (`patterns`, `motifs`, `species`, `corrections`)
- [ ] Implement pre-task hook for bioacoustic task routing
- [ ] Implement post-task hook for pattern storage
- [ ] Configure HNSW parameters for 1536-D Perch embeddings
- [ ] Set up audit worker for embedding health checks

#### Phase 2: Learning Integration (Week 3-4)

- [ ] Implement trajectory tracking for analysis workflows
- [ ] Configure LoRA fine-tuning for embedding refinement
- [ ] Set up EWC++ consolidation schedule
- [ ] Implement feedback capture from user interface
- [ ] Configure optimize worker for HNSW tuning

#### Phase 3: Advanced Features (Week 5-6)

- [ ] Implement motif discovery and storage
- [ ] Set up species-specific embedding profiles
- [ ] Configure transfer learning from related projects
- [ ] Implement expertise-weighted feedback integration
- [ ] Set up consolidate worker for memory optimization

#### Phase 4: Monitoring and Refinement (Ongoing)

- [ ] Dashboard for learning metrics
- [ ] Alerting for quality degradation
- [ ] A/B testing for pattern effectiveness
- [ ] Regular audit of learned patterns

## Consequences

### Positive

1. **Continuous Improvement**: System gets better with every analysis task
2. **Domain Adaptation**: EWC++ allows learning new species without forgetting existing knowledge
3. **Expert Knowledge Capture**: User corrections are systematically integrated
4. **Efficient Processing**: Pattern reuse reduces computation for common tasks
5. **Transparent Learning**: Trajectory tracking provides explainability
6. **Cross-Project Synergy**: Transfer learning leverages community knowledge

### Negative

1. **Complexity**: Multiple interacting systems require careful orchestration
2. **Storage Growth**: Pattern storage will grow over time (mitigated by consolidation)
3. **Cold Start**: Initial deployments lack learned patterns (mitigated by transfer)
4. **Feedback Dependency**: Quality depends on user correction quality

### Neutral

1. **Operational Overhead**: Background workers require monitoring
2. **Parameter Tuning**: Initial HNSW parameters need manual optimization
3. **Expertise Requirements**: Domain experts needed for high-quality feedback

## References

1. RuVector GNN Architecture: https://github.com/ruvnet/ruvector
2. SONA Pattern Documentation: claude-flow v3 hooks system
3. EWC++ Paper: "Overcoming catastrophic forgetting in neural networks"
4. Perch 2.0 Embeddings: https://arxiv.org/abs/2508.04665
5. HNSW Algorithm: "Efficient and robust approximate nearest neighbor search"
6. LoRA Fine-tuning: "LoRA: Low-Rank Adaptation of Large Language Models"
