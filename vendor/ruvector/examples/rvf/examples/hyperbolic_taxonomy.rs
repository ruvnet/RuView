//! Exotic Capability: Hierarchy-Aware Search
//!
//! Demonstrates RVF for taxonomy node embeddings where parent-child
//! relationships are captured in embedding space (representing Poincare
//! embeddings). Hierarchically related nodes cluster closer together.
//!
//! Taxonomy structure:
//!   5 top-level categories, 4 children each, 3 grandchildren each = 85 nodes
//!
//! Features:
//!   - Taxonomy tree with hierarchy-aware embeddings
//!   - Metadata: node_name, depth, parent_name
//!   - Query for hierarchically related nodes
//!   - Filter by depth to get specific levels
//!
//! RVF segments used: VEC_SEG, MANIFEST_SEG
//!
//! Run: cargo run --example hyperbolic_taxonomy

use rvf_runtime::{
    FilterExpr, MetadataEntry, MetadataValue, QueryOptions, RvfOptions, RvfStore, SearchResult,
};
use rvf_runtime::filter::FilterValue;
use rvf_runtime::options::DistanceMetric;
use tempfile::TempDir;

/// Simple LCG-based pseudo-random vector generator for deterministic results.
fn random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut v = Vec::with_capacity(dim);
    let mut x = seed.wrapping_add(1);
    for _ in 0..dim {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
    }
    v
}

/// Create a child vector that is close to its parent in embedding space.
/// Adds a small perturbation scaled by depth to represent Poincare embedding
/// where children cluster near parents.
fn child_vector(parent: &[f32], child_seed: u64, perturbation_scale: f32) -> Vec<f32> {
    let dim = parent.len();
    let noise = random_vector(dim, child_seed);
    parent
        .iter()
        .zip(noise.iter())
        .map(|(p, n)| p + n * perturbation_scale)
        .collect()
}

/// A taxonomy node with its name, depth, parent name, and vector ID.
struct TaxonomyNode {
    id: u64,
    name: String,
    depth: u64,
    parent_name: String,
}

fn main() {
    println!("=== Hyperbolic Taxonomy: Hierarchy-Aware Search ===\n");

    let dim = 64;

    // ====================================================================
    // 1. Build taxonomy tree
    // ====================================================================
    println!("--- 1. Build Taxonomy Tree ---");

    let top_categories = ["Science", "Technology", "Arts", "Medicine", "Law"];

    let mut nodes: Vec<TaxonomyNode> = Vec::new();
    let mut vectors_list: Vec<Vec<f32>> = Vec::new();
    let mut next_id: u64 = 0;

    // Level 0: top-level categories
    let mut top_vectors = Vec::new();
    for cat in &top_categories {
        let vec = random_vector(dim, next_id * 1000 + 42);
        top_vectors.push(vec.clone());
        vectors_list.push(vec);
        nodes.push(TaxonomyNode {
            id: next_id,
            name: cat.to_string(),
            depth: 0,
            parent_name: "ROOT".to_string(),
        });
        next_id += 1;
    }

    // Level 1: 4 children per top-level
    let child_suffixes = ["Theory", "Applied", "History", "Modern"];
    let mut level1_vectors = Vec::new();
    let mut level1_parents = Vec::new();

    for (cat_idx, cat) in top_categories.iter().enumerate() {
        for (child_idx, suffix) in child_suffixes.iter().enumerate() {
            let parent_vec = &top_vectors[cat_idx];
            let seed = next_id * 1000 + 100 + child_idx as u64;
            let vec = child_vector(parent_vec, seed, 0.15);
            level1_vectors.push(vec.clone());
            level1_parents.push(cat_idx);
            vectors_list.push(vec);
            nodes.push(TaxonomyNode {
                id: next_id,
                name: format!("{}_{}", cat, suffix),
                depth: 1,
                parent_name: cat.to_string(),
            });
            next_id += 1;
        }
    }

    // Level 2: 3 grandchildren per level-1 node
    let grandchild_suffixes = ["Intro", "Advanced", "Research"];

    for l1_idx in 0..level1_vectors.len() {
        let parent_name = nodes[5 + l1_idx].name.clone(); // offset by 5 top-level nodes
        let parent_vec = &level1_vectors[l1_idx];

        for (gc_idx, suffix) in grandchild_suffixes.iter().enumerate() {
            let seed = next_id * 1000 + 200 + gc_idx as u64;
            let vec = child_vector(parent_vec, seed, 0.08);
            vectors_list.push(vec);
            nodes.push(TaxonomyNode {
                id: next_id,
                name: format!("{}_{}", parent_name, suffix),
                depth: 2,
                parent_name: parent_name.clone(),
            });
            next_id += 1;
        }
    }

    let total_nodes = nodes.len();
    let level0_count = 5;
    let level1_count = 5 * 4;
    let level2_count = 5 * 4 * 3;

    println!("  Taxonomy structure:");
    println!("    Level 0 (roots):         {} categories", level0_count);
    println!("    Level 1 (children):      {} subcategories", level1_count);
    println!("    Level 2 (grandchildren): {} leaf nodes", level2_count);
    println!("    Total nodes:             {}", total_nodes);
    assert_eq!(total_nodes, level0_count + level1_count + level2_count);

    // ====================================================================
    // 2. Create store and ingest
    // ====================================================================
    println!("\n--- 2. Create Taxonomy Store ---");

    let tmp_dir = TempDir::new().expect("failed to create temp dir");
    let store_path = tmp_dir.path().join("taxonomy.rvf");

    let options = RvfOptions {
        dimension: dim as u16,
        metric: DistanceMetric::L2,
        ..Default::default()
    };

    let mut store = RvfStore::create(&store_path, options).expect("failed to create store");

    let vec_refs: Vec<&[f32]> = vectors_list.iter().map(|v| v.as_slice()).collect();
    let ids: Vec<u64> = nodes.iter().map(|n| n.id).collect();

    // Metadata: node_name (0), depth (1), parent_name (2)
    let mut metadata = Vec::with_capacity(total_nodes * 3);
    for node in &nodes {
        metadata.push(MetadataEntry {
            field_id: 0,
            value: MetadataValue::String(node.name.clone()),
        });
        metadata.push(MetadataEntry {
            field_id: 1,
            value: MetadataValue::U64(node.depth),
        });
        metadata.push(MetadataEntry {
            field_id: 2,
            value: MetadataValue::String(node.parent_name.clone()),
        });
    }

    let ingest = store
        .ingest_batch(&vec_refs, &ids, Some(&metadata))
        .expect("ingest failed");
    println!("  Ingested {} taxonomy nodes ({} dims)", ingest.accepted, dim);

    // ====================================================================
    // 3. Query for nodes similar to a root concept
    // ====================================================================
    println!("\n--- 3. Hierarchy-Aware Query ---");

    // Use "Science" (ID 0) as query vector
    let science_vec = &vectors_list[0];
    let k = 15;

    let results = store
        .query(science_vec, k, &QueryOptions::default())
        .expect("query failed");

    println!("  Query: nodes similar to 'Science' (top-{}):", k);
    print_taxonomy_results(&results, &nodes);

    // Check that Science itself is nearest (distance ~0)
    assert_eq!(results[0].id, 0, "Science should be nearest to itself");
    println!("\n  Science's children should dominate top results.");

    // Count how many results share "Science" lineage
    let science_related = results.iter().filter(|r| {
        let node = &nodes[r.id as usize];
        node.name.starts_with("Science") || node.parent_name == "Science"
    }).count();
    println!("  Science-related in top-{}: {} nodes", k, science_related);

    // ====================================================================
    // 4. Filter by depth: roots only
    // ====================================================================
    println!("\n--- 4. Filter by Depth (Roots Only) ---");

    let filter_roots = FilterExpr::Eq(1, FilterValue::U64(0));
    let opts_roots = QueryOptions {
        filter: Some(filter_roots),
        ..Default::default()
    };
    let results_roots = store
        .query(science_vec, 5, &opts_roots)
        .expect("filtered query failed");

    println!("  Root nodes (depth=0):");
    print_taxonomy_results(&results_roots, &nodes);

    for r in &results_roots {
        assert_eq!(nodes[r.id as usize].depth, 0);
    }
    println!("  All results verified: depth == 0.");

    // ====================================================================
    // 5. Filter by depth: leaf nodes only
    // ====================================================================
    println!("\n--- 5. Filter by Depth (Leaves Only) ---");

    let filter_leaves = FilterExpr::Eq(1, FilterValue::U64(2));
    let opts_leaves = QueryOptions {
        filter: Some(filter_leaves),
        ..Default::default()
    };
    let results_leaves = store
        .query(science_vec, 10, &opts_leaves)
        .expect("filtered query failed");

    println!("  Leaf nodes (depth=2) nearest to 'Science':");
    print_taxonomy_results(&results_leaves, &nodes);

    for r in &results_leaves {
        assert_eq!(nodes[r.id as usize].depth, 2);
    }
    println!("  All results verified: depth == 2.");

    // ====================================================================
    // 6. Cross-branch query
    // ====================================================================
    println!("\n--- 6. Cross-Branch Query ---");

    // Query with a "Technology" vector to see cross-branch similarities
    let tech_vec = &vectors_list[1]; // Technology is ID 1
    let results_tech = store
        .query(tech_vec, 10, &QueryOptions::default())
        .expect("query failed");

    println!("  Nodes nearest to 'Technology':");
    print_taxonomy_results(&results_tech, &nodes);

    // ====================================================================
    // 7. Print taxonomy tree
    // ====================================================================
    println!("\n--- 7. Taxonomy Tree ---");

    for cat_idx in 0..5 {
        let cat = &nodes[cat_idx];
        println!("  {} (ID={}, depth={})", cat.name, cat.id, cat.depth);
        for child_idx in 0..4 {
            let l1_idx = 5 + cat_idx * 4 + child_idx;
            let child = &nodes[l1_idx];
            println!("    +-- {} (ID={})", child.name, child.id);
            for gc_idx in 0..3 {
                let l2_idx = 25 + (cat_idx * 4 + child_idx) * 3 + gc_idx;
                let grandchild = &nodes[l2_idx];
                println!("        +-- {} (ID={})", grandchild.name, grandchild.id);
            }
        }
    }

    // ====================================================================
    // Summary
    // ====================================================================
    println!("\n=== Hyperbolic Taxonomy Summary ===\n");
    println!("  Total nodes:         {}", total_nodes);
    println!("  Embedding dims:      {}", dim);
    println!("  Root query results:  {}", results_roots.len());
    println!("  Leaf query results:  {}", results_leaves.len());
    println!("  Science-related:     {} in top-{}", science_related, k);

    store.close().expect("failed to close store");
    println!("\nDone.");
}

fn print_taxonomy_results(results: &[SearchResult], nodes: &[TaxonomyNode]) {
    println!(
        "    {:>4}  {:>12}  {:>5}  {:>30}  {:>20}",
        "ID", "Distance", "Depth", "Name", "Parent"
    );
    println!(
        "    {:->4}  {:->12}  {:->5}  {:->30}  {:->20}",
        "", "", "", "", ""
    );
    for r in results {
        let node = &nodes[r.id as usize];
        println!(
            "    {:>4}  {:>12.6}  {:>5}  {:>30}  {:>20}",
            r.id, r.distance, node.depth, node.name, node.parent_name
        );
    }
}
