# Embedding Crystallization

## Overview

### Problem Statement
Most vector databases require pre-defined hierarchical structures or manual clustering. This creates several problems:
1. **Static hierarchies**: Cannot adapt to changing data distributions
2. **Manual tuning**: Requires expert knowledge to choose hierarchy depth and branching
3. **Poor adaptation**: Hierarchy may not match natural data clusters
4. **Rigid structure**: Cannot reorganize as data evolves

### Proposed Solution
Automatically form hierarchical structure from flat embeddings through a physics-inspired crystallization process:
1. **Nucleation**: Identify dense clusters as crystal "seeds"
2. **Growth**: Expand crystals outward from nuclei
3. **Competition**: Crystals compete for boundary regions
4. **Equilibrium**: Self-organizing hierarchy emerges

Like physical crystals growing from a supersaturated solution, embedding crystals grow from dense regions in embedding space.

### Expected Benefits
- **Automatic hierarchy**: No manual structure design needed
- **Adaptive organization**: Hierarchy evolves with data
- **Natural clusters**: Respects inherent data structure
- **Multi-scale representation**: From coarse (crystal) to fine (individual points)
- **20-40% faster search**: Hierarchical pruning reduces search space

### Novelty Claim
First application of crystal growth dynamics to vector database organization. Unlike:
- **K-means clustering**: Fixed K, no hierarchy
- **Hierarchical clustering**: Bottom-up, computationally expensive
- **LSH**: Random projections, no semantic structure

Embedding Crystallization uses physics-inspired dynamics to discover natural hierarchical organization.

## Technical Design

### Architecture Diagram
```
┌────────────────────────────────────────────────────────────────────┐
│                    Embedding Crystallization                        │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │              Phase 1: Nucleation Detection                  │   │
│  │                                                             │   │
│  │   Flat Embedding Space                                     │   │
│  │   ┌──────────────────────────────────────────┐            │   │
│  │   │  ●  ●●●  ●     ●   ●●●●●   ●             │            │   │
│  │   │     ● ●        ●    ● ●    ●             │            │   │
│  │   │  ●  ●●●  ●     ●   ●●●●●   ●             │            │   │
│  │   │                ●                         │            │   │
│  │   │  ●●●●●                  ●●●              │            │   │
│  │   │   ● ●        ●           ●●  ●           │            │   │
│  │   │  ●●●●●                  ●●●              │            │   │
│  │   │         ▲           ▲         ▲          │            │   │
│  │   └─────────│───────────│─────────│──────────┘            │   │
│  │          Nucleus 1   Nucleus 2  Nucleus 3                 │   │
│  │          (ρ > ρ_crit)                                      │   │
│  └────────────────────────────────────────────────────────────┘   │
│                            │                                        │
│                            ▼                                        │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │              Phase 2: Crystal Growth                        │   │
│  │                                                             │   │
│  │   Iteration 0:        Iteration 5:       Iteration 10:     │   │
│  │   ┌──────┐            ┌──────┐           ┌──────┐         │   │
│  │   │  ◎   │            │ ╔══╗ │           │╔════╗│         │   │
│  │   │      │            │ ║  ║ │           │║    ║│         │   │
│  │   │  ◎   │    ───▶    │ ╚══╝ │   ───▶    │╚════╝│         │   │
│  │   │      │            │      │           │      │         │   │
│  │   │  ◎   │            │ ╔══╗ │           │╔════╗│         │   │
│  │   └──────┘            │ ║  ║ │           │║    ║│         │   │
│  │                        │ ╚══╝ │           │╚════╝│         │   │
│  │   ◎ = Nucleus         └──────┘           └──────┘         │   │
│  │   ═ = Crystal         Growth rate: v = -∇E                │   │
│  └────────────────────────────────────────────────────────────┘   │
│                            │                                        │
│                            ▼                                        │
│  ┌────────────────────────────────────────────────────────────┐   │
│  │           Phase 3: Hierarchical Organization                │   │
│  │                                                             │   │
│  │                    Root (Global)                           │   │
│  │                         │                                   │   │
│  │            ┌────────────┼────────────┐                     │   │
│  │            │            │            │                     │   │
│  │       Crystal 1    Crystal 2    Crystal 3                 │   │
│  │        (Topic 1)   (Topic 2)   (Topic 3)                  │   │
│  │            │            │            │                     │   │
│  │       ┌────┴────┐  ┌───┴───┐   ┌────┴────┐              │   │
│  │       │         │  │       │   │         │              │   │
│  │    SubCrystal SubCrystal  ...  ...      ...             │   │
│  │    (Subtopic) (Subtopic)                                 │   │
│  │       │         │                                         │   │
│  │    ● ● ●     ● ● ●  ← Individual embeddings              │   │
│  │                                                            │   │
│  └────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────────────────────────────────────┘
```

### Core Data Structures

```rust
/// Crystal structure (hierarchical cluster)
#[derive(Clone, Debug)]
pub struct Crystal {
    /// Unique crystal identifier
    pub id: CrystalId,

    /// Centroid (center of mass)
    pub centroid: Vec<f32>,

    /// Radius (effective size)
    pub radius: f32,

    /// Member nodes
    pub members: Vec<NodeId>,

    /// Parent crystal (if not root)
    pub parent: Option<CrystalId>,

    /// Child crystals (subclusters)
    pub children: Vec<CrystalId>,

    /// Hierarchy level (0 = root)
    pub level: usize,

    /// Density at nucleation
    pub density: f32,

    /// Growth rate
    pub growth_rate: f32,

    /// Energy (stability measure)
    pub energy: f32,

    /// Metadata
    pub metadata: CrystalMetadata,
}

/// Crystal metadata
#[derive(Clone, Debug)]
pub struct CrystalMetadata {
    /// Formation timestamp
    pub formed_at: SystemTime,

    /// Number of growth iterations
    pub growth_iterations: usize,

    /// Stability score (0-1)
    pub stability: f32,

    /// Semantic label (if available)
    pub label: Option<String>,
}

/// Nucleation site (seed for crystal)
#[derive(Clone, Debug)]
pub struct NucleationSite {
    /// Center point
    pub center: Vec<f32>,

    /// Local density
    pub density: f32,

    /// Seed nodes
    pub seeds: Vec<NodeId>,

    /// Critical radius
    pub critical_radius: f32,
}

/// Crystallization configuration
#[derive(Clone, Debug)]
pub struct CrystallizationConfig {
    /// Density threshold for nucleation
    pub nucleation_threshold: f32,  // default: 0.7

    /// Minimum nodes for nucleation
    pub min_nucleus_size: usize,  // default: 10

    /// Growth rate parameter
    pub growth_rate: f32,  // default: 0.1

    /// Maximum hierarchy depth
    pub max_depth: usize,  // default: 5

    /// Energy function
    pub energy_function: EnergyFunction,

    /// Growth stopping criterion
    pub stopping_criterion: StoppingCriterion,

    /// Allow crystal merging
    pub allow_merging: bool,
}

/// Energy function for crystal stability
#[derive(Clone, Debug)]
pub enum EnergyFunction {
    /// Within-cluster variance
    Variance,

    /// Silhouette score
    Silhouette,

    /// Density-based
    Density,

    /// Custom function
    Custom(fn(&Crystal, &[Vec<f32>]) -> f32),
}

/// Stopping criterion for growth
#[derive(Clone, Debug)]
pub enum StoppingCriterion {
    /// Maximum iterations
    MaxIterations(usize),

    /// Energy convergence
    EnergyConvergence { threshold: f32 },

    /// No more boundary nodes
    NoBoundary,

    /// Combined criteria
    Combined(Vec<StoppingCriterion>),
}

/// Crystallization state
pub struct CrystallizationState {
    /// All crystals (hierarchical)
    crystals: Vec<Crystal>,

    /// Node to crystal assignment
    node_assignments: Vec<CrystalId>,

    /// Hierarchy tree
    hierarchy: CrystalTree,

    /// Configuration
    config: CrystallizationConfig,

    /// Growth history (for analysis)
    growth_history: Vec<GrowthSnapshot>,
}

/// Crystal hierarchy tree
#[derive(Clone, Debug)]
pub struct CrystalTree {
    /// Root crystal (entire dataset)
    root: CrystalId,

    /// Tree structure
    nodes: HashMap<CrystalId, CrystalTreeNode>,

    /// Fast level-based lookup
    levels: Vec<Vec<CrystalId>>,
}

#[derive(Clone, Debug)]
pub struct CrystalTreeNode {
    pub crystal_id: CrystalId,
    pub parent: Option<CrystalId>,
    pub children: Vec<CrystalId>,
    pub level: usize,
}

/// Snapshot of growth process
#[derive(Clone, Debug)]
pub struct GrowthSnapshot {
    pub iteration: usize,
    pub num_crystals: usize,
    pub total_energy: f32,
    pub avg_crystal_size: f32,
    pub timestamp: SystemTime,
}
```

### Key Algorithms

```rust
// Pseudocode for embedding crystallization

/// Main crystallization algorithm
fn crystallize(
    embeddings: &[Vec<f32>],
    config: CrystallizationConfig
) -> CrystallizationState {
    // Phase 1: Detect nucleation sites
    let nucleation_sites = detect_nucleation_sites(
        embeddings,
        config.nucleation_threshold,
        config.min_nucleus_size
    );

    // Phase 2: Initialize crystals from nuclei
    let mut crystals = Vec::new();
    for (i, site) in nucleation_sites.iter().enumerate() {
        crystals.push(Crystal {
            id: i,
            centroid: site.center.clone(),
            radius: site.critical_radius,
            members: site.seeds.clone(),
            parent: None,
            children: Vec::new(),
            level: 0,
            density: site.density,
            growth_rate: config.growth_rate,
            energy: compute_energy(site.seeds, embeddings, &config),
            metadata: CrystalMetadata::new(),
        });
    }

    // Phase 3: Grow crystals
    let mut node_assignments = vec![None; embeddings.len()];
    for crystal in &crystals {
        for &member in &crystal.members {
            node_assignments[member] = Some(crystal.id);
        }
    }

    let mut iteration = 0;
    loop {
        let mut changed = false;

        // Find boundary nodes (unassigned or contestable)
        let boundary_nodes = find_boundary_nodes(
            embeddings,
            &node_assignments,
            &crystals
        );

        if boundary_nodes.is_empty() {
            break;
        }

        // Assign boundary nodes to nearest growing crystal
        for node_id in boundary_nodes {
            let (best_crystal, energy_change) = find_best_crystal(
                node_id,
                embeddings,
                &crystals,
                &config
            );

            // Only add if energy decreases (stability)
            if energy_change < 0.0 {
                crystals[best_crystal].members.push(node_id);
                node_assignments[node_id] = Some(best_crystal);
                changed = true;
            }
        }

        // Update crystal properties
        for crystal in &mut crystals {
            update_centroid(crystal, embeddings);
            update_radius(crystal, embeddings);
            crystal.energy = compute_energy(&crystal.members, embeddings, &config);
        }

        iteration += 1;

        if !changed || should_stop(&config.stopping_criterion, iteration, &crystals) {
            break;
        }
    }

    // Phase 4: Build hierarchy (recursive crystallization)
    let hierarchy = build_hierarchy(&mut crystals, embeddings, &config);

    CrystallizationState {
        crystals,
        node_assignments,
        hierarchy,
        config,
        growth_history: Vec::new(),
    }
}

/// Detect nucleation sites using density estimation
fn detect_nucleation_sites(
    embeddings: &[Vec<f32>],
    threshold: f32,
    min_size: usize
) -> Vec<NucleationSite> {
    let mut sites = Vec::new();

    // Build density field using KDE
    let density_field = estimate_density(embeddings);

    // Find local maxima above threshold
    for (i, &density) in density_field.iter().enumerate() {
        if density < threshold {
            continue;
        }

        // Check if local maximum
        let neighbors = find_neighbors(i, embeddings, radius=1.0);
        let is_maximum = neighbors.iter().all(|&j| {
            density_field[j] <= density
        });

        if !is_maximum {
            continue;
        }

        // Collect seed nodes within critical radius
        let critical_radius = estimate_critical_radius(density);
        let seeds: Vec<NodeId> = embeddings.iter()
            .enumerate()
            .filter(|(j, emb)| {
                let dist = euclidean_distance(&embeddings[i], emb);
                dist <= critical_radius
            })
            .map(|(j, _)| j)
            .collect();

        if seeds.len() >= min_size {
            sites.push(NucleationSite {
                center: embeddings[i].clone(),
                density,
                seeds,
                critical_radius,
            });
        }
    }

    // Remove overlapping sites (keep higher density)
    sites = remove_overlapping_sites(sites);

    sites
}

/// Estimate density using Kernel Density Estimation
fn estimate_density(embeddings: &[Vec<f32>]) -> Vec<f32> {
    let n = embeddings.len();
    let mut density = vec![0.0; n];

    // Adaptive bandwidth (Scott's rule)
    let bandwidth = estimate_bandwidth(embeddings);

    for i in 0..n {
        for j in 0..n {
            let dist = euclidean_distance(&embeddings[i], &embeddings[j]);
            density[i] += gaussian_kernel(dist, bandwidth);
        }
        density[i] /= n as f32;
    }

    density
}

/// Find best crystal for boundary node
fn find_best_crystal(
    node_id: NodeId,
    embeddings: &[Vec<f32>],
    crystals: &[Crystal],
    config: &CrystallizationConfig
) -> (CrystalId, f32) {
    let embedding = &embeddings[node_id];

    let mut best_crystal = 0;
    let mut best_energy_change = f32::MAX;

    for (i, crystal) in crystals.iter().enumerate() {
        // Distance to crystal centroid
        let dist = euclidean_distance(embedding, &crystal.centroid);

        // Only consider if within growth radius
        if dist > crystal.radius + config.growth_rate {
            continue;
        }

        // Compute energy change if node joins this crystal
        let mut temp_members = crystal.members.clone();
        temp_members.push(node_id);

        let new_energy = compute_energy(&temp_members, embeddings, config);
        let energy_change = new_energy - crystal.energy;

        if energy_change < best_energy_change {
            best_energy_change = energy_change;
            best_crystal = i;
        }
    }

    (best_crystal, best_energy_change)
}

/// Build hierarchical structure via recursive crystallization
fn build_hierarchy(
    crystals: &mut Vec<Crystal>,
    embeddings: &[Vec<f32>],
    config: &CrystallizationConfig
) -> CrystalTree {
    let mut tree = CrystalTree::new();

    // Start with level 0 (base crystals)
    for crystal in crystals.iter_mut() {
        crystal.level = 0;
        tree.add_node(crystal.id, None, 0);
    }

    // Recursively create parent levels
    for level in 0..config.max_depth {
        let current_level_crystals: Vec<_> = crystals.iter()
            .filter(|c| c.level == level)
            .map(|c| c.id)
            .collect();

        if current_level_crystals.len() <= 1 {
            break;  // Only one cluster, stop
        }

        // Treat crystals as embeddings (their centroids)
        let crystal_centroids: Vec<_> = current_level_crystals.iter()
            .map(|&id| crystals[id].centroid.clone())
            .collect();

        // Recursively crystallize at higher level
        let parent_config = CrystallizationConfig {
            nucleation_threshold: config.nucleation_threshold * 0.8,  // Relax threshold
            ..config.clone()
        };

        let parent_sites = detect_nucleation_sites(
            &crystal_centroids,
            parent_config.nucleation_threshold,
            2  // At least 2 child crystals
        );

        // Create parent crystals
        for (i, site) in parent_sites.iter().enumerate() {
            let parent_id = crystals.len();

            // Children are crystals in this parent's region
            let children: Vec<CrystalId> = site.seeds.iter()
                .map(|&seed_idx| current_level_crystals[seed_idx])
                .collect();

            // Collect all members from children
            let mut all_members = Vec::new();
            for &child_id in &children {
                all_members.extend(&crystals[child_id].members);
            }

            let parent = Crystal {
                id: parent_id,
                centroid: site.center.clone(),
                radius: site.critical_radius,
                members: all_members,
                parent: None,
                children: children.clone(),
                level: level + 1,
                density: site.density,
                growth_rate: config.growth_rate,
                energy: 0.0,  // Computed later
                metadata: CrystalMetadata::new(),
            };

            crystals.push(parent);
            tree.add_node(parent_id, None, level + 1);

            // Update children's parent pointers
            for &child_id in &children {
                crystals[child_id].parent = Some(parent_id);
                tree.set_parent(child_id, parent_id);
            }
        }
    }

    tree
}
```

### API Design

```rust
/// Public API for Embedding Crystallization
pub trait EmbeddingCrystallization {
    /// Crystallize flat embeddings into hierarchy
    fn crystallize(
        embeddings: &[Vec<f32>],
        config: CrystallizationConfig,
    ) -> Result<CrystallizationState, CrystalError>;

    /// Search using crystal hierarchy
    fn search(
        &self,
        query: &[f32],
        k: usize,
        options: CrystalSearchOptions,
    ) -> Result<Vec<SearchResult>, CrystalError>;

    /// Add new embeddings (incremental crystallization)
    fn add_embeddings(
        &mut self,
        new_embeddings: &[Vec<f32>],
    ) -> Result<(), CrystalError>;

    /// Get crystal by ID
    fn get_crystal(&self, id: CrystalId) -> Option<&Crystal>;

    /// Get crystals at level
    fn get_level(&self, level: usize) -> Vec<&Crystal>;

    /// Find crystal containing node
    fn find_crystal(&self, node_id: NodeId) -> Option<CrystalId>;

    /// Traverse hierarchy
    fn traverse(&self, strategy: TraversalStrategy) -> CrystalIterator;

    /// Export hierarchy for visualization
    fn export_hierarchy(&self) -> HierarchyExport;

    /// Get crystallization statistics
    fn statistics(&self) -> CrystalStatistics;

    /// Recrystallize (rebuild hierarchy)
    fn recrystallize(&mut self) -> Result<(), CrystalError>;
}

/// Search options for crystallization
#[derive(Clone, Debug)]
pub struct CrystalSearchOptions {
    /// Start search at level
    pub start_level: usize,

    /// Use hierarchical pruning
    pub enable_pruning: bool,

    /// Pruning threshold (discard crystals with similarity < threshold)
    pub pruning_threshold: f32,

    /// Maximum crystals to explore
    pub max_crystals: usize,
}

/// Traversal strategies
#[derive(Clone, Debug)]
pub enum TraversalStrategy {
    /// Breadth-first (level by level)
    BreadthFirst,

    /// Depth-first (branch by branch)
    DepthFirst,

    /// Largest crystals first
    SizeOrder,

    /// Highest density first
    DensityOrder,
}

/// Hierarchy statistics
#[derive(Clone, Debug)]
pub struct CrystalStatistics {
    pub total_crystals: usize,
    pub depth: usize,
    pub avg_branching_factor: f32,
    pub avg_crystal_size: f32,
    pub density_distribution: Vec<f32>,
    pub energy_distribution: Vec<f32>,
}

/// Hierarchy export for visualization
#[derive(Clone, Debug, Serialize)]
pub struct HierarchyExport {
    pub crystals: Vec<CrystalExport>,
    pub edges: Vec<HierarchyEdge>,
    pub statistics: CrystalStatistics,
}

#[derive(Clone, Debug, Serialize)]
pub struct CrystalExport {
    pub id: CrystalId,
    pub level: usize,
    pub size: usize,
    pub centroid: Vec<f32>,
    pub radius: f32,
    pub label: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct HierarchyEdge {
    pub parent: CrystalId,
    pub child: CrystalId,
}
```

## Integration Points

### Affected Crates/Modules

1. **`crates/ruvector-core/src/hnsw/`**
   - Add hierarchical layer based on crystals
   - Integrate crystal-aware search

2. **`crates/ruvector-gnn/src/hierarchy/`**
   - Create hierarchy management module
   - Integrate with existing GNN layers

### New Modules to Create

1. **`crates/ruvector-gnn/src/crystallization/`**
   - `nucleation.rs` - Nucleation site detection
   - `growth.rs` - Crystal growth algorithms
   - `hierarchy.rs` - Hierarchy construction
   - `search.rs` - Crystal-aware search
   - `energy.rs` - Energy functions
   - `visualization.rs` - Hierarchy visualization

## Regression Prevention

### Test Cases

```rust
#[test]
fn test_hierarchy_coverage() {
    let state = crystallize_test_data();

    // Every node should belong to exactly one crystal at level 0
    for node_id in 0..embeddings.len() {
        let crystal_id = state.find_crystal(node_id).unwrap();
        let crystal = state.get_crystal(crystal_id).unwrap();
        assert_eq!(crystal.level, 0);
    }
}

#[test]
fn test_hierarchy_containment() {
    let state = crystallize_test_data();

    // Parent crystals must contain all child members
    for crystal in &state.crystals {
        if let Some(parent_id) = crystal.parent {
            let parent = state.get_crystal(parent_id).unwrap();
            for &member in &crystal.members {
                assert!(parent.members.contains(&member));
            }
        }
    }
}
```

## Implementation Phases

### Phase 1: Research Validation (2 weeks)
- Implement nucleation detection
- Test crystal growth on synthetic data
- Measure hierarchy quality
- **Deliverable**: Research report

### Phase 2: Core Implementation (3 weeks)
- Full crystallization algorithm
- Hierarchy construction
- Energy functions
- **Deliverable**: Working crystallization

### Phase 3: Integration (2 weeks)
- HNSW integration
- Search optimization
- API bindings
- **Deliverable**: Integrated feature

### Phase 4: Optimization (2 weeks)
- Incremental updates
- Performance tuning
- Visualization tools
- **Deliverable**: Production-ready

## Success Metrics

| Metric | Target |
|--------|--------|
| Search speedup | >30% |
| Hierarchy depth | 3-5 levels |
| Coverage | 100% nodes |
| Energy reduction | >40% vs. random |

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Poor nucleation | Adaptive thresholds, multiple strategies |
| Unstable growth | Energy-based stopping, regularization |
| Deep hierarchies | Max depth limit, pruning |
| High computation | Approximate methods, caching |
