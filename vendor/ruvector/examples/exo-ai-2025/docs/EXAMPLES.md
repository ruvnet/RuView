# EXO-AI 2025 - Usage Examples

This guide provides practical examples for using the EXO-AI 2025 cognitive substrate.

## Table of Contents

1. [Basic Pattern Storage](#basic-pattern-storage)
2. [Hypergraph Query Examples](#hypergraph-query-examples)
3. [Temporal Memory Examples](#temporal-memory-examples)
4. [Federation Examples](#federation-examples)
5. [WASM Examples](#wasm-examples)
6. [Node.js Examples](#nodejs-examples)
7. [Advanced Scenarios](#advanced-scenarios)

---

## Basic Pattern Storage

### Creating and Storing Patterns

```rust
use exo_manifold::{ManifoldEngine, ManifoldConfig};
use exo_core::{Pattern, PatternId, Metadata, SubstrateTime};
use burn::backend::NdArray;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize manifold engine
    let config = ManifoldConfig {
        dimension: 384,
        max_descent_steps: 100,
        learning_rate: 0.01,
        convergence_threshold: 1e-4,
        hidden_layers: 3,
        hidden_dim: 256,
        omega_0: 30.0,
    };

    let device = Default::default();
    let mut engine = ManifoldEngine::<NdArray>::new(config, device);

    // Create a pattern
    let pattern = Pattern {
        id: PatternId::new(),
        embedding: vec![0.1, 0.2, 0.3, /* ... 384 dimensions */],
        metadata: Metadata::default(),
        timestamp: SubstrateTime::now(),
        antecedents: vec![],
        salience: 0.95,
    };

    // Deform manifold (continuous storage)
    let delta = engine.deform(pattern, 0.95)?;

    println!("Manifold deformed with salience: {}", 0.95);

    Ok(())
}
```

### Querying Similar Patterns

```rust
use exo_manifold::ManifoldEngine;

fn query_similar(
    engine: &ManifoldEngine<NdArray>,
    query_embedding: Vec<f32>,
    k: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    // Retrieve via gradient descent
    let results = engine.retrieve(&query_embedding, k)?;

    println!("Found {} similar patterns:", results.len());
    for (i, result) in results.iter().enumerate() {
        println!(
            "  {}. Score: {:.4}, Distance: {:.4}",
            i + 1,
            result.score,
            result.distance
        );
    }

    Ok(())
}
```

### Strategic Forgetting

```rust
use exo_manifold::ManifoldEngine;

fn forget_low_salience(
    engine: &mut ManifoldEngine<NdArray>,
) -> Result<(), Box<dyn std::error::Error>> {
    let salience_threshold = 0.1;  // Forget patterns < 0.1 salience
    let decay_rate = 0.95;          // 95% decay

    let pruned_count = engine.forget(salience_threshold, decay_rate)?;

    println!("Pruned {} low-salience patterns", pruned_count);

    Ok(())
}
```

---

## Hypergraph Query Examples

### Creating Higher-Order Relations

```rust
use exo_hypergraph::{HypergraphSubstrate, HypergraphConfig};
use exo_core::{EntityId, Relation, RelationType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = HypergraphConfig {
        enable_sheaf: true,
        max_dimension: 3,
        epsilon: 1e-6,
    };

    let mut hypergraph = HypergraphSubstrate::new(config);

    // Create entities
    let alice = EntityId::new();
    let bob = EntityId::new();
    let charlie = EntityId::new();
    let project = EntityId::new();

    hypergraph.add_entity(alice, serde_json::json!({"name": "Alice"}));
    hypergraph.add_entity(bob, serde_json::json!({"name": "Bob"}));
    hypergraph.add_entity(charlie, serde_json::json!({"name": "Charlie"}));
    hypergraph.add_entity(project, serde_json::json!({"name": "EXO-AI"}));

    // Create 4-way hyperedge (team collaboration)
    let relation = Relation {
        relation_type: RelationType::new("team_collaboration"),
        properties: serde_json::json!({
            "role": "development",
            "weight": 0.9,
            "start_date": "2025-01-01"
        }),
    };

    let hyperedge_id = hypergraph.create_hyperedge(
        &[alice, bob, charlie, project],
        &relation,
    )?;

    println!("Created hyperedge: {}", hyperedge_id);

    Ok(())
}
```

### Persistent Homology

```rust
use exo_hypergraph::HypergraphSubstrate;

fn analyze_topology(
    hypergraph: &HypergraphSubstrate,
) -> Result<(), Box<dyn std::error::Error>> {
    // Compute persistent homology in dimension 1 (loops)
    let dimension = 1;
    let epsilon_range = (0.0, 1.0);

    let diagram = hypergraph.persistent_homology(dimension, epsilon_range);

    println!("Persistence Diagram (dimension {}):", dimension);
    for (birth, death) in diagram.pairs {
        let persistence = death - birth;
        println!("  Feature: birth={:.4}, death={:.4}, persistence={:.4}",
                 birth, death, persistence);
    }

    Ok(())
}
```

### Betti Numbers

```rust
use exo_hypergraph::HypergraphSubstrate;

fn compute_betti_numbers(
    hypergraph: &HypergraphSubstrate,
) -> Result<(), Box<dyn std::error::Error>> {
    let max_dim = 3;
    let betti = hypergraph.betti_numbers(max_dim);

    println!("Betti Numbers:");
    println!("  β₀ (connected components): {}", betti[0]);
    println!("  β₁ (1D holes/loops): {}", betti[1]);
    println!("  β₂ (2D voids): {}", betti[2]);
    println!("  β₃ (3D cavities): {}", betti[3]);

    Ok(())
}
```

### Sheaf Consistency

```rust
use exo_hypergraph::HypergraphSubstrate;
use exo_core::SectionId;

fn check_consistency(
    hypergraph: &HypergraphSubstrate,
    sections: &[SectionId],
) -> Result<(), Box<dyn std::error::Error>> {
    let result = hypergraph.check_sheaf_consistency(sections);

    match result {
        exo_core::SheafConsistencyResult::Consistent => {
            println!("✓ Sheaf is consistent");
        }
        exo_core::SheafConsistencyResult::Inconsistent(violations) => {
            println!("✗ Sheaf inconsistencies detected:");
            for violation in violations {
                println!("  - {}", violation);
            }
        }
        exo_core::SheafConsistencyResult::NotConfigured => {
            println!("! Sheaf checking not enabled");
        }
    }

    Ok(())
}
```

---

## Temporal Memory Examples

### Causal Pattern Storage

```rust
use exo_temporal::{TemporalMemory, TemporalConfig};
use exo_core::{Pattern, PatternId, Metadata};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let memory = TemporalMemory::new(TemporalConfig::default());

    // Store initial pattern
    let p1 = Pattern {
        id: PatternId::new(),
        embedding: vec![1.0, 0.0, 0.0],
        metadata: Metadata::default(),
        timestamp: exo_core::SubstrateTime::now(),
        antecedents: vec![],
        salience: 0.9,
    };
    let id1 = p1.id;
    memory.store(p1, &[])?;

    // Store dependent pattern (causal chain)
    let p2 = Pattern {
        id: PatternId::new(),
        embedding: vec![0.9, 0.1, 0.0],
        metadata: Metadata::default(),
        timestamp: exo_core::SubstrateTime::now(),
        antecedents: vec![id1],  // Caused by p1
        salience: 0.85,
    };
    let id2 = p2.id;
    memory.store(p2, &[id1])?;

    // Third generation
    let p3 = Pattern {
        id: PatternId::new(),
        embedding: vec![0.8, 0.2, 0.0],
        metadata: Metadata::default(),
        timestamp: exo_core::SubstrateTime::now(),
        antecedents: vec![id2],
        salience: 0.8,
    };
    memory.store(p3, &[id2])?;

    println!("Created causal chain: p1 → p2 → p3");

    Ok(())
}
```

### Causal Queries

```rust
use exo_temporal::{TemporalMemory, CausalConeType};
use exo_core::{Query, SubstrateTime};

fn causal_query_example(
    memory: &TemporalMemory,
    origin_id: exo_core::PatternId,
) -> Result<(), Box<dyn std::error::Error>> {
    let query = Query::from_embedding(vec![1.0, 0.0, 0.0])
        .with_origin(origin_id);

    // Query past light-cone
    let past_results = memory.causal_query(
        &query,
        SubstrateTime::now(),
        CausalConeType::Past,
    );

    println!("Past causally-related patterns:");
    for result in past_results {
        println!(
            "  Pattern: {}, Similarity: {:.3}, Causal distance: {:?}, Combined score: {:.3}",
            result.pattern.id,
            result.similarity,
            result.causal_distance,
            result.combined_score
        );
    }

    // Query future light-cone
    let future_results = memory.causal_query(
        &query,
        SubstrateTime::now(),
        CausalConeType::Future,
    );

    println!("\nFuture causally-related patterns: {}", future_results.len());

    Ok(())
}
```

### Memory Consolidation

```rust
use exo_temporal::TemporalMemory;

fn consolidation_example(
    memory: &TemporalMemory,
) -> Result<(), Box<dyn std::error::Error>> {
    // Trigger manual consolidation
    let result = memory.consolidate();

    println!("Consolidation Results:");
    println!("  Patterns promoted to long-term: {}", result.promoted_count);
    println!("  Patterns discarded (low salience): {}", result.discarded_count);
    println!("  Average salience of promoted: {:.3}", result.avg_salience);

    // Get memory statistics
    let stats = memory.stats();
    println!("\nMemory Statistics:");
    println!("  Short-term: {} patterns", stats.short_term.pattern_count);
    println!("  Long-term: {} patterns", stats.long_term.pattern_count);
    println!("  Causal graph: {} nodes, {} edges",
             stats.causal_graph.node_count,
             stats.causal_graph.edge_count);

    Ok(())
}
```

### Anticipatory Pre-fetching

```rust
use exo_temporal::{TemporalMemory, AnticipationHint, TemporalPhase};

fn prefetch_example(
    memory: &TemporalMemory,
    recent_patterns: Vec<exo_core::PatternId>,
) -> Result<(), Box<dyn std::error::Error>> {
    let hints = vec![
        AnticipationHint::Sequential {
            last_k_patterns: recent_patterns,
        },
        AnticipationHint::Temporal {
            current_phase: TemporalPhase::WorkingHours,
        },
    ];

    // Pre-fetch predicted patterns
    memory.anticipate(&hints);

    println!("Pre-fetch cache warmed based on anticipation hints");

    // Later query may hit cache
    let query = Query::from_embedding(vec![1.0, 0.0, 0.0]);
    if let Some(cached_results) = memory.check_cache(&query) {
        println!("✓ Cache hit! Got {} results without search", cached_results.len());
    } else {
        println!("✗ Cache miss, performing search");
    }

    Ok(())
}
```

---

## Federation Examples

### Joining a Federation

```rust
use exo_federation::{FederatedMesh, PeerAddress};
use exo_core::SubstrateInstance;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create local substrate
    let local_substrate = SubstrateInstance::new(
        exo_core::SubstrateConfig::default()
    )?;

    // Create federated mesh
    let mut mesh = FederatedMesh::new(local_substrate)?;

    println!("Local peer ID: {}", mesh.local_id.0);

    // Connect to federation peer
    let peer = PeerAddress::new(
        "peer.example.com".to_string(),
        9000,
        vec![/* peer's public key */],
    );

    let token = mesh.join_federation(&peer).await?;

    println!("✓ Joined federation");
    println!("  Peer ID: {}", token.peer_id);
    println!("  Capabilities: {:?}", token.capabilities);

    Ok(())
}
```

### Federated Query

```rust
use exo_federation::{FederatedMesh, FederationScope};

async fn federated_query_example(
    mesh: &FederatedMesh,
) -> Result<(), Box<dyn std::error::Error>> {
    let query_data = b"search query".to_vec();

    // Local query only
    let local_results = mesh.federated_query(
        query_data.clone(),
        FederationScope::Local,
    ).await?;

    println!("Local results: {}", local_results.len());

    // Direct peers
    let direct_results = mesh.federated_query(
        query_data.clone(),
        FederationScope::Direct,
    ).await?;

    println!("Direct peer results: {}", direct_results.len());

    // Global (multi-hop with onion routing)
    let global_results = mesh.federated_query(
        query_data,
        FederationScope::Global { max_hops: 3 },
    ).await?;

    println!("Global federation results: {}", global_results.len());

    // Process results
    for result in global_results {
        println!(
            "  Source: {}, Score: {:.3}",
            result.source.0,
            result.score
        );
    }

    Ok(())
}
```

### Byzantine Consensus

```rust
use exo_federation::{FederatedMesh, StateUpdate};

async fn consensus_example(
    mesh: &FederatedMesh,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create state update
    let update = StateUpdate {
        update_id: "update-001".to_string(),
        data: b"new state data".to_vec(),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    };

    // Byzantine fault-tolerant commit
    // Requires 3f+1 peers where f = ⌊(N-1)/3⌋
    let proof = mesh.byzantine_commit(update).await?;

    println!("✓ Byzantine consensus achieved");
    println!("  Signatures: {}", proof.signatures.len());
    println!("  Fault tolerance: f = {}", proof.fault_tolerance);

    Ok(())
}
```

---

## WASM Examples

### Browser-based Cognitive Substrate

```javascript
// index.html
<!DOCTYPE html>
<html>
<head>
    <title>EXO-AI WASM Demo</title>
</head>
<body>
    <h1>EXO-AI Cognitive Substrate (WASM)</h1>
    <button id="store">Store Pattern</button>
    <button id="query">Query Similar</button>
    <div id="output"></div>

    <script type="module">
        import init, { ExoSubstrate, Pattern } from './pkg/exo_wasm.js';

        async function main() {
            // Initialize WASM module
            await init();

            // Create substrate
            const substrate = new ExoSubstrate({
                dimensions: 384,
                distance_metric: "cosine",
                use_hnsw: true,
                enable_temporal: true,
                enable_causal: true
            });

            console.log('EXO substrate initialized');

            // Store pattern button
            document.getElementById('store').onclick = () => {
                const embedding = new Float32Array(384);
                for (let i = 0; i < 384; i++) {
                    embedding[i] = Math.random();
                }

                const pattern = new Pattern(
                    embedding,
                    { text: "Example pattern", timestamp: Date.now() },
                    []  // no antecedents
                );

                const id = substrate.store(pattern);
                document.getElementById('output').innerHTML +=
                    `<p>Stored pattern: ${id}</p>`;
            };

            // Query button
            document.getElementById('query').onclick = async () => {
                const queryEmbedding = new Float32Array(384);
                for (let i = 0; i < 384; i++) {
                    queryEmbedding[i] = Math.random();
                }

                const results = await substrate.query(queryEmbedding, 5);

                let html = '<h3>Query Results:</h3><ul>';
                results.forEach((r, i) => {
                    html += `<li>Result ${i+1}: Score ${r.score.toFixed(4)}</li>`;
                });
                html += '</ul>';

                document.getElementById('output').innerHTML = html;
            };

            // Show stats
            const stats = substrate.stats();
            console.log('Stats:', stats);
        }

        main();
    </script>
</body>
</html>
```

---

## Node.js Examples

### Basic Node.js Usage

```typescript
// example.ts
import { ExoSubstrateNode } from 'exo-node';

async function main() {
    // Create substrate
    const substrate = new ExoSubstrateNode({
        dimensions: 384,
        storagePath: './substrate.db',
        enableHypergraph: true,
        enableTemporal: true
    });

    // Store patterns
    const patterns = [];
    for (let i = 0; i < 100; i++) {
        const embedding = new Float32Array(384);
        for (let j = 0; j < 384; j++) {
            embedding[j] = Math.random();
        }

        const id = await substrate.store({
            embedding,
            metadata: {
                text: `Document ${i}`,
                category: i % 3 === 0 ? 'A' : i % 3 === 1 ? 'B' : 'C'
            },
            antecedents: []
        });

        patterns.push(id);
    }

    console.log(`Stored ${patterns.length} patterns`);

    // Query
    const queryEmbedding = new Float32Array(384);
    for (let i = 0; i < 384; i++) {
        queryEmbedding[i] = Math.random();
    }

    const results = await substrate.search(queryEmbedding, 10);

    console.log('Top 10 Results:');
    results.forEach((r, i) => {
        console.log(`  ${i+1}. ID: ${r.id}, Score: ${r.score.toFixed(4)}`);
    });

    // Hypergraph query
    const hypergraphResult = await substrate.hypergraphQuery(
        JSON.stringify({
            type: 'BettiNumbers',
            maxDimension: 2
        })
    );

    console.log('Hypergraph result:', hypergraphResult);

    // Stats
    const stats = await substrate.stats();
    console.log('Substrate stats:', stats);
}

main().catch(console.error);
```

---

## Advanced Scenarios

### Multi-Modal Pattern Storage

```rust
use exo_manifold::ManifoldEngine;
use exo_core::{Pattern, Metadata, MetadataValue};

fn multi_modal_example() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = create_engine();

    // Text pattern
    let text_pattern = Pattern {
        id: PatternId::new(),
        embedding: embed_text("The quick brown fox"),
        metadata: {
            let mut m = Metadata::default();
            m.fields.insert(
                "modality".to_string(),
                MetadataValue::String("text".to_string())
            );
            m.fields.insert(
                "content".to_string(),
                MetadataValue::String("The quick brown fox".to_string())
            );
            m
        },
        timestamp: SubstrateTime::now(),
        antecedents: vec![],
        salience: 0.9,
    };

    // Image pattern
    let image_pattern = Pattern {
        id: PatternId::new(),
        embedding: embed_image("path/to/fox.jpg"),
        metadata: {
            let mut m = Metadata::default();
            m.fields.insert(
                "modality".to_string(),
                MetadataValue::String("image".to_string())
            );
            m.fields.insert(
                "path".to_string(),
                MetadataValue::String("path/to/fox.jpg".to_string())
            );
            m
        },
        timestamp: SubstrateTime::now(),
        antecedents: vec![text_pattern.id],  // Causal link
        salience: 0.85,
    };

    engine.deform(text_pattern, 0.9)?;
    engine.deform(image_pattern, 0.85)?;

    Ok(())
}
```

### Hierarchical Pattern Retrieval

```rust
use exo_temporal::TemporalMemory;

fn hierarchical_retrieval() -> Result<(), Box<dyn std::error::Error>> {
    let memory = TemporalMemory::default();

    // Store hierarchical patterns
    let root = store_pattern(&memory, "root concept", vec![])?;
    let child1 = store_pattern(&memory, "child 1", vec![root])?;
    let child2 = store_pattern(&memory, "child 2", vec![root])?;
    let grandchild = store_pattern(&memory, "grandchild", vec![child1])?;

    // Query with causal constraints
    let query = Query::from_embedding(embed_text("root concept"))
        .with_origin(root);

    let descendants = memory.causal_query(
        &query,
        SubstrateTime::now(),
        CausalConeType::Future,  // Get all descendants
    );

    println!("Found {} descendants of root", descendants.len());

    Ok(())
}
```

---

## See Also

- [API Documentation](./API.md) - Complete API reference
- [Test Strategy](./TEST_STRATEGY.md) - Testing approach
- [Integration Guide](./INTEGRATION_TEST_GUIDE.md) - Integration testing

---

**Questions?** Open an issue at https://github.com/ruvnet/ruvector/issues
