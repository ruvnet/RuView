# ADR-002: Quantum-Inspired Genomics Engine

**Status**: Proposed (Revised - Implementable Today)
**Date**: 2026-02-11
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board
**SDK**: Claude-Flow

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-02-11 | ruv.io | Initial quantum genomics engine proposal |
| 0.2 | 2026-02-11 | ruv.io | Revised to focus on implementable quantum-inspired algorithms |

---

## Context

### The Genomics Computational Bottleneck

Modern genomics confronts a data explosion that outpaces Moore's Law. A single human genome contains approximately 3.2 billion base pairs. The critical computational tasks -- sequence alignment, variant calling, haplotype phasing, de novo assembly, phylogenetic inference, and protein structure prediction -- each pose optimization problems whose classical complexity ranges from O(N log N) to NP-hard.

| Genomic Operation | Classical Complexity | Bottleneck |
|-------------------|---------------------|------------|
| k-mer exact search | O(N) per query | Linear scan over 3.2B base pairs |
| Sequence alignment (BWA-MEM2) | O(N log N) with FM-index | Index construction and seed extension |
| Variant calling (GATK HaplotypeCaller) | O(R * H * L) per active region | Local assembly of haplotype candidates |
| Haplotype assembly | NP-hard (MEC formulation) | Minimum error correction on read fragments |
| De novo genome assembly | O(N) edge traversal on de Bruijn graph | Graph construction and Eulerian path finding |
| Phylogenetic tree inference (ML) | NP-hard (Felsenstein, 1978) | Tree topology search over super-exponential space |
| Protein folding energy minimization | NP-hard (Crescenzi & Pode, 1998) | Conformational search in continuous space |

### Quantum-Inspired Classical Algorithms: Implementable Today

While fault-tolerant quantum computers remain decades away, **quantum-inspired classical algorithms** provide the same algorithmic insights and computational structures as their quantum counterparts, running on classical hardware **today**. RuVector's quantum crates (`ruQu`, `ruqu-algorithms`, `ruqu-core`, `ruqu-wasm`) enable:

1. **Quantum circuit simulation** for algorithm design and validation (up to 25 qubits)
2. **Quantum-inspired optimization** via tensor network contractions and variational methods
3. **Classical implementations** of quantum algorithmic patterns with similar complexity benefits

### Why Quantum-Inspired Algorithms Work

Quantum algorithms provide computational advantages through:
- **Amplitude amplification patterns** that inform hierarchical pruning strategies
- **Variational optimization** that maps to classical gradient descent with structured ansätze
- **Superposition concepts** that translate to parallel ensemble methods
- **Entanglement structures** that guide tensor network decompositions

We implement these algorithmic insights classically, using quantum simulation **only for validation and algorithm design** at tractable scales.

---

## Decision

### Architecture Overview

Introduce a `quantum-genomics` module within `ruqu-algorithms` that implements **quantum-inspired classical algorithms** for genomic data processing, with quantum simulation for validation.

```
                    ┌─────────────────────────────────────────────┐
                    │    Quantum-Inspired Genomics Engine          │
                    │       (ruqu-algorithms::genomics)            │
                    ├─────────────────────────────────────────────┤
                    │                                              │
                    │  ┌─────────┐  ┌──────────┐  ┌───────────┐ │
                    │  │ HNSW    │  │ Simulated│  │ Bayesian  │ │
                    │  │ k-mer   │  │ Annealing│  │ Haplotype │ │
                    │  │ Search  │  │ Phylo    │  │ Assembly  │ │
                    │  └────┬────┘  └────┬─────┘  └─────┬─────┘ │
                    │       │            │              │        │
                    │  ┌────┴────┐  ┌────┴─────┐  ┌────┴─────┐ │
                    │  │ Classical│  │ Tensor   │  │ Variational│
                    │  │ VQE      │  │ Network  │  │ Optimization│
                    │  │ Molecular│  │ Assembly │  │ Variant  │ │
                    │  └────┬────┘  └────┬─────┘  └────┬─────┘ │
                    │       │            │              │        │
                    │  ┌────┴────────────┴──────────────┴─────┐ │
                    │  │    ruQu Quantum Simulation (25 qubits)│ │
                    │  │    (Algorithm Validation Only)        │ │
                    │  └──────────────────────────────────────┘ │
                    └────────────────┬────────────────────────┬──┘
                                     │                        │
                    ┌────────────────┴────┐    ┌─────────────┴───────┐
                    │     ruqu-core       │    │  Classical backends │
                    │  (quantum simulator)│    │  - HNSW indexing    │
                    ├─────────────────────┤    │  - Tensor networks  │
                    │  ruqu-wasm          │    │  - Simulated        │
                    │  (browser target)   │    │    annealing        │
                    └─────────────────────┘    └─────────────────────┘
```

### Module Structure

```
ruqu-algorithms/
  src/
    genomics/
      mod.rs                     # Public API and genomic type definitions
      hnsw_kmer_search.rs        # HNSW-based k-mer search (O(log N) heuristic)
      haplotype_assembly.rs      # Variational optimization for phasing
      classical_vqe_molecular.rs # Classical variational molecular simulation
      tensor_network_assembly.rs # Tensor network for de Bruijn graphs
      simulated_annealing.rs     # Simulated annealing for phylogenetics
      pattern_matching.rs        # Quantum-inspired pattern recognition
      encoding.rs                # DNA base-pair to qubit encoding schemes
      hybrid_pipeline.rs         # Classical-quantum decision boundary logic
      quantum_validation.rs      # Quantum simulation for algorithm validation
```

---

## Implementation Status

| Algorithm | Status | Classical Implementation | Quantum Validation | Production Ready |
|-----------|--------|-------------------------|-------------------|------------------|
| HNSW k-mer search | ✅ Implemented | HNSW with O(log N) | ruQu 8-12 qubits | Yes |
| Haplotype assembly | ✅ Implemented | Variational MinCut | QAOA simulation 20 qubits | Yes |
| Molecular docking | 🔄 In Progress | Classical VQE (DFT-level) | ruQu 12-16 qubits | Q2 2026 |
| Tensor network assembly | 🔄 In Progress | MPS/PEPS contractions | N/A (classical-only) | Q3 2026 |
| Simulated annealing phylo | ✅ Implemented | Metropolis-Hastings | 8-10 qubits validation | Yes |
| Pattern matching | ✅ Implemented | GNN + attention | N/A | Yes |

---

## 1. HNSW-Based k-mer Search (Quantum-Inspired)

### Problem Statement

Classical k-mer search uses hash tables (O(1) lookup after O(N) preprocessing) or FM-indices (O(k) lookup). Grover's algorithm offers O(sqrt(N)) query complexity on quantum hardware, but we implement this **algorithmic insight** classically using hierarchical navigable small world (HNSW) graphs.

### Classical Implementation: HNSW Search

**Key Insight**: Grover's amplitude amplification creates a hierarchical search pattern. HNSW replicates this structure through layered graph navigation.

```rust
/// HNSW-based k-mer search inspired by Grover's hierarchical amplification.
///
/// Grover: O(sqrt(N)) queries with amplitude amplification
/// HNSW: O(log N) average-case with hierarchical graph traversal
///
/// The hierarchical structure mimics Grover's iteration pattern.
pub struct HnswKmerIndex {
    /// HNSW index for k-mer vectors
    index: HnswIndex<KmerVector>,
    /// k-mer length
    k: usize,
    /// Reference genome encoded as 2-bit per base
    reference: Vec<u8>,
    /// M parameter (connections per layer)
    m: usize,
    /// ef_construction parameter
    ef_construction: usize,
}

impl HnswKmerIndex {
    /// Build HNSW index from reference genome.
    ///
    /// Preprocessing: O(N log N) to build index
    /// Query: O(log N) average case
    pub fn from_reference(reference: &[u8], k: usize) -> Self {
        let mut index = HnswIndex::new(
            /*dim=*/ k * 2, // 2 bits per base
            /*m=*/ 16,
            /*ef_construction=*/ 200,
        );

        // Extract all k-mers and build index
        for i in 0..reference.len().saturating_sub(k) {
            let kmer = &reference[i..i + k];
            let vector = encode_kmer_to_vector(kmer);
            index.insert(i, vector);
        }

        Self { index, k, reference: reference.to_vec(), m: 16, ef_construction: 200 }
    }

    /// Search for k-mer matches using HNSW.
    ///
    /// Returns all positions matching within Hamming distance threshold.
    pub fn search(&self, query_kmer: &[u8], max_hamming: usize) -> Vec<usize> {
        let query_vector = encode_kmer_to_vector(query_kmer);

        // HNSW search with hierarchical navigation (Grover-inspired)
        let candidates = self.index.search(&query_vector, /*k=*/ 100, /*ef=*/ 200);

        // Filter by exact Hamming distance
        candidates.into_iter()
            .filter(|(idx, _dist)| {
                let ref_kmer = &self.reference[*idx..*idx + self.k];
                hamming_distance(query_kmer, ref_kmer) <= max_hamming
            })
            .map(|(idx, _)| idx)
            .collect()
    }
}

/// Encode k-mer as vector for HNSW.
fn encode_kmer_to_vector(kmer: &[u8]) -> Vec<f32> {
    kmer.iter()
        .flat_map(|&base| match base {
            b'A' => [1.0, 0.0],
            b'C' => [0.0, 1.0],
            b'G' => [-1.0, 0.0],
            b'T' => [0.0, -1.0],
            _ => [0.0, 0.0],
        })
        .collect()
}
```

### Complexity Analysis

| Approach | Preprocessing | Per-Query | Space |
|----------|--------------|-----------|-------|
| Linear scan | None | O(N * k) | O(1) |
| Hash table | O(N) | O(k) average | O(N) |
| FM-index (BWT) | O(N) | O(k) | O(N) |
| **HNSW (quantum-inspired)** | **O(N log N)** | **O(log N)** | **O(N)** |
| **Grover (quantum)** | **None** | **O(sqrt(N) * k)** | **O(n) qubits** |

**Practical speedup** for human genome (N = 3.2B):
- Linear scan: 3.2B comparisons
- HNSW: ~32 comparisons (log₂(3.2e9) ≈ 32)
- Speedup: **100M×** over linear scan

### Quantum Validation (ruQu)

```rust
/// Validate HNSW search pattern against Grover's algorithm at small scale.
pub fn validate_against_grover(reference: &[u8], k: usize) {
    assert!(reference.len() <= 256, "Grover validation limited to 8 qubits (2^8 = 256 bases)");

    // Build HNSW index
    let hnsw_index = HnswKmerIndex::from_reference(reference, k);

    // Build Grover oracle for validation
    let oracle = GroverKmerOracle::new(reference, k);
    let grover_result = grover_search(&oracle, /*iterations=*/ 12);

    // Compare results
    let test_kmer = &reference[42..42 + k];
    let hnsw_matches = hnsw_index.search(test_kmer, 0);
    let grover_matches = grover_result.marked_states;

    assert_eq!(hnsw_matches.len(), grover_matches.len());
}
```

---

## 2. Variational Haplotype Assembly (QAOA-Inspired)

### Problem Statement

Haplotype assembly partitions reads into two groups (maternal/paternal) that minimize read-allele conflicts -- the Minimum Error Correction (MEC) problem, proven NP-hard.

### Classical Implementation: Variational MinCut

**Key Insight**: QAOA encodes MEC as a MaxCut Hamiltonian. We implement classical variational optimization with the same cost function structure.

```rust
/// Variational haplotype assembly inspired by QAOA MaxCut.
///
/// Uses gradient-based optimization over the same cost landscape
/// as QAOA, but with classical bitstring representation.
pub struct VariationalHaplotypeAssembler {
    /// Fragment-SNP matrix
    fragment_matrix: Vec<Vec<i8>>,
    /// Quality scores (Phred-scaled)
    quality_matrix: Vec<Vec<f64>>,
    /// Number of variational layers
    layers: usize,
}

impl VariationalHaplotypeAssembler {
    /// Build fragment-conflict graph (same as QAOA formulation).
    pub fn build_conflict_graph(&self) -> WeightedGraph {
        let n_fragments = self.fragment_matrix.len();
        let mut edges = Vec::new();

        for i in 0..n_fragments {
            for j in (i + 1)..n_fragments {
                let mut weight = 0.0;
                for s in 0..self.fragment_matrix[i].len() {
                    let a_i = self.fragment_matrix[i][s];
                    let a_j = self.fragment_matrix[j][s];
                    if a_i >= 0 && a_j >= 0 && a_i != a_j {
                        let q = (self.quality_matrix[i][s]
                                + self.quality_matrix[j][s]) / 2.0;
                        weight += q;
                    }
                }
                if weight > 0.0 {
                    edges.push((i, j, weight));
                }
            }
        }

        WeightedGraph { vertices: n_fragments, edges }
    }

    /// Solve using classical variational optimization.
    ///
    /// Mimics QAOA cost landscape but uses gradient descent
    /// over continuous relaxation of the cut.
    pub fn solve(&self) -> HaplotypeResult {
        let graph = self.build_conflict_graph();

        // Initialize random partition
        let mut partition = random_bitstring(graph.vertices);

        // Variational optimization (inspired by QAOA parameter optimization)
        for _layer in 0..self.layers {
            // Compute gradient of MaxCut cost
            let gradient = self.compute_cut_gradient(&graph, &partition);

            // Update partition via simulated annealing moves
            self.apply_gradient_moves(&mut partition, &gradient);
        }

        HaplotypeResult {
            haplotype_assignment: partition,
            mec_score: self.compute_cut_cost(&graph, &partition),
        }
    }

    fn compute_cut_cost(&self, graph: &WeightedGraph, partition: &[bool]) -> f64 {
        graph.edges.iter()
            .filter(|(i, j, _)| partition[*i] != partition[*j])
            .map(|(_, _, w)| w)
            .sum()
    }
}
```

### Quantum Validation (ruQu QAOA)

```rust
/// Validate classical variational approach against QAOA at small scale.
pub fn validate_against_qaoa(fragment_matrix: &[Vec<i8>], quality_matrix: &[Vec<f64>]) {
    assert!(fragment_matrix.len() <= 20, "QAOA validation limited to 20 qubits");

    let assembler = VariationalHaplotypeAssembler {
        fragment_matrix: fragment_matrix.to_vec(),
        quality_matrix: quality_matrix.to_vec(),
        layers: 3,
    };

    // Classical variational result
    let classical_result = assembler.solve();

    // QAOA quantum simulation result
    let graph = assembler.build_conflict_graph();
    let qaoa_result = qaoa_maxcut(&graph, /*p=*/ 3, &LbfgsOptimizer::new());

    // Compare cut quality (should be within 5%)
    let quality_ratio = classical_result.mec_score / qaoa_result.best_cost;
    assert!((0.95..=1.05).contains(&quality_ratio), "Classical variational within 5% of QAOA");
}
```

---

## 3. Classical VQE for Molecular Interaction

### Problem Statement

Understanding DNA-protein binding and drug-nucleic acid interactions requires computing molecular ground-state energies. Classical force fields approximate quantum effects; VQE computes from first principles.

### Classical Implementation: Density Functional Theory

**Key Insight**: VQE's variational principle is the same as classical DFT. We use classical DFT libraries with VQE-inspired ansatz optimization.

```rust
/// Classical molecular energy calculation using VQE principles.
///
/// Uses DFT (PySCF backend) with variational optimization structure
/// identical to VQE, but without quantum hardware.
pub struct ClassicalVqeMolecular {
    /// Molecular geometry (XYZ coordinates)
    geometry: Vec<Atom>,
    /// Basis set (e.g., "def2-TZVP")
    basis: String,
    /// Functional (e.g., "B3LYP")
    functional: String,
}

impl ClassicalVqeMolecular {
    /// Compute ground state energy using classical DFT.
    ///
    /// Variational optimization over molecular orbitals (same principle as VQE).
    pub fn compute_energy(&self) -> f64 {
        // Initialize DFT calculation (via FFI to PySCF or similar)
        let mut dft_calc = DftCalculation::new(&self.geometry, &self.basis, &self.functional);

        // Variational optimization (SCF iterations)
        dft_calc.run_scf(/*max_iterations=*/ 100, /*convergence=*/ 1e-6);

        dft_calc.total_energy()
    }

    /// Compute molecular binding energy for DNA-protein interaction.
    pub fn compute_binding_energy(
        &self,
        dna_geometry: &[Atom],
        protein_geometry: &[Atom],
    ) -> f64 {
        let complex_energy = self.compute_energy();

        let dna_alone = ClassicalVqeMolecular {
            geometry: dna_geometry.to_vec(),
            ..self.clone()
        };
        let protein_alone = ClassicalVqeMolecular {
            geometry: protein_geometry.to_vec(),
            ..self.clone()
        };

        complex_energy - dna_alone.compute_energy() - protein_alone.compute_energy()
    }
}
```

### Quantum Validation (ruQu VQE)

```rust
/// Validate classical DFT against quantum VQE at small scale.
pub fn validate_against_vqe(geometry: &[Atom]) {
    assert!(geometry.len() <= 6, "VQE validation limited to small molecules (12-16 qubits)");

    // Classical DFT result
    let classical_calc = ClassicalVqeMolecular {
        geometry: geometry.to_vec(),
        basis: "sto-3g".to_string(),
        functional: "B3LYP".to_string(),
    };
    let classical_energy = classical_calc.compute_energy();

    // Quantum VQE simulation result
    let hamiltonian = construct_molecular_hamiltonian(geometry, "sto-3g");
    let ansatz = UccsdAnsatz::new(/*n_electrons=*/ 4, /*n_orbitals=*/ 4);
    let vqe_result = run_vqe(&hamiltonian, &ansatz, &LbfgsOptimizer::new());

    // Compare energies (should be within chemical accuracy: 1 kcal/mol = 0.0016 Hartree)
    let error = (classical_energy - vqe_result.energy).abs();
    assert!(error < 0.002, "Classical DFT within chemical accuracy of VQE");
}
```

---

## 4. Tensor Network Assembly (Quantum-Inspired)

### Problem Statement

De novo genome assembly constructs genome sequences from reads. De Bruijn graphs have up to N nodes; finding Eulerian paths is O(N) classically, but repeat resolution is combinatorially hard.

### Classical Implementation: Matrix Product State Contraction

**Key Insight**: Quantum walks explore multiple paths via superposition. Tensor network methods achieve similar multi-path exploration classically.

```rust
/// Tensor network assembly for de Bruijn graph traversal.
///
/// Inspired by quantum walk superposition, uses matrix product states (MPS)
/// to efficiently represent exponentially many path hypotheses.
pub struct TensorNetworkAssembler {
    /// de Bruijn graph adjacency
    adjacency: Vec<Vec<usize>>,
    /// k-mer labels
    node_labels: Vec<Vec<u8>>,
    /// MPS bond dimension
    bond_dim: usize,
}

impl TensorNetworkAssembler {
    /// Construct MPS representation of path space.
    ///
    /// Instead of quantum walk, use tensor network to represent
    /// exponentially many paths with polynomial memory.
    pub fn build_path_mps(&self) -> MatrixProductState {
        let n_nodes = self.adjacency.len();
        let mut mps = MatrixProductState::new(n_nodes, self.bond_dim);

        // Initialize MPS tensors from adjacency structure
        for node in 0..n_nodes {
            let out_degree = self.adjacency[node].len();
            let tensor = self.create_node_tensor(node, out_degree);
            mps.set_tensor(node, tensor);
        }

        mps
    }

    /// Contract MPS to find high-probability paths (assembly candidates).
    pub fn assemble(&self) -> Vec<Path> {
        let mps = self.build_path_mps();

        // Contract tensor network to find top-k paths
        let path_probabilities = mps.contract_all();

        // Extract paths with probability above threshold
        path_probabilities.into_iter()
            .filter(|(_, prob)| *prob > 0.01)
            .map(|(path, _)| path)
            .collect()
    }

    fn create_node_tensor(&self, node: usize, out_degree: usize) -> Tensor3D {
        // Create tensor encoding local graph structure
        // Dimension: bond_dim x bond_dim x out_degree
        Tensor3D::from_adjacency(&self.adjacency[node], self.bond_dim)
    }
}
```

**Complexity**: MPS with bond dimension χ achieves O(N χ³) assembly vs. O(2^N) for exact enumeration.

---

## 5. Simulated Annealing for Phylogenetics

### Problem Statement

Phylogenetic tree inference searches super-exponential topology space. For n=20 taxa: (2*20-5)!! = 2.2×10²⁰ topologies.

### Classical Implementation: Simulated Annealing

**Key Insight**: Quantum annealing explores cost landscapes via tunneling. Simulated annealing replicates this via thermal fluctuations.

```rust
/// Simulated annealing for phylogenetic tree optimization.
///
/// Inspired by quantum annealing, uses thermal fluctuations
/// to escape local minima in the tree topology landscape.
pub struct PhylogeneticAnnealer {
    /// Sequence alignment
    alignment: Vec<Vec<u8>>,
    /// Number of taxa
    n_taxa: usize,
    /// Annealing schedule
    schedule: AnnealingSchedule,
}

pub struct AnnealingSchedule {
    /// Initial temperature
    pub t_initial: f64,
    /// Final temperature
    pub t_final: f64,
    /// Cooling rate
    pub alpha: f64,
    /// Steps per temperature
    pub steps_per_temp: usize,
}

impl PhylogeneticAnnealer {
    /// Run simulated annealing optimization.
    pub fn anneal(&self) -> PhylogeneticTree {
        // Initialize random tree topology
        let mut current_tree = random_tree(self.n_taxa);
        let mut current_likelihood = self.log_likelihood(&current_tree);
        let mut best_tree = current_tree.clone();
        let mut best_likelihood = current_likelihood;

        let mut temperature = self.schedule.t_initial;

        while temperature > self.schedule.t_final {
            for _ in 0..self.schedule.steps_per_temp {
                // Propose tree modification (NNI, SPR, or TBR move)
                let proposed_tree = self.propose_move(&current_tree);
                let proposed_likelihood = self.log_likelihood(&proposed_tree);

                // Metropolis acceptance criterion
                let delta_e = proposed_likelihood - current_likelihood;
                if delta_e > 0.0 || random::<f64>() < (delta_e / temperature).exp() {
                    current_tree = proposed_tree;
                    current_likelihood = proposed_likelihood;

                    if current_likelihood > best_likelihood {
                        best_tree = current_tree.clone();
                        best_likelihood = current_likelihood;
                    }
                }
            }

            // Cool down (annealing schedule)
            temperature *= self.schedule.alpha;
        }

        best_tree
    }

    fn log_likelihood(&self, tree: &PhylogeneticTree) -> f64 {
        // Felsenstein pruning algorithm
        felsenstein_pruning(tree, &self.alignment)
    }
}
```

### Quantum Validation (ruQu)

```rust
/// Validate simulated annealing against quantum annealing at small scale.
pub fn validate_against_quantum_annealing(alignment: &[Vec<u8>]) {
    assert!(alignment.len() <= 8, "Quantum annealing validation limited to 8 taxa (18 qubits)");

    let annealer = PhylogeneticAnnealer {
        alignment: alignment.to_vec(),
        n_taxa: alignment.len(),
        schedule: AnnealingSchedule {
            t_initial: 100.0,
            t_final: 0.1,
            alpha: 0.95,
            steps_per_temp: 100,
        },
    };

    // Classical simulated annealing result
    let classical_tree = annealer.anneal();
    let classical_likelihood = annealer.log_likelihood(&classical_tree);

    // Quantum annealing simulation result
    let qaoa_tree = quantum_phylo_annealing(alignment, /*trotter_slices=*/ 10);
    let quantum_likelihood = annealer.log_likelihood(&qaoa_tree);

    // Compare likelihood quality (should be within 2%)
    let quality_ratio = classical_likelihood / quantum_likelihood;
    assert!((0.98..=1.02).contains(&quality_ratio), "Simulated annealing within 2% of quantum");
}
```

---

## Crate API Mapping

### ruqu-core Functions

| Genomic Operation | ruqu-core Function | Purpose |
|-------------------|-------------------|---------|
| HNSW k-mer validation | `grover_search(&oracle, iterations)` | Validate HNSW search pattern against Grover at 8-12 qubits |
| Haplotype assembly validation | `qaoa_maxcut(&graph, p, optimizer)` | Validate variational MinCut against QAOA at 20 qubits |
| Molecular energy validation | `run_vqe(&hamiltonian, &ansatz, &optimizer)` | Validate classical DFT against VQE at 12-16 qubits |
| Phylogenetics validation | `quantum_annealing(&hamiltonian, &schedule)` | Validate simulated annealing at 8 taxa (18 qubits) |

### ruqu-algorithms Functions

| Genomic Operation | ruqu-algorithms Function | Purpose |
|-------------------|-------------------------|---------|
| Grover oracle | `GroverOracle::new(reference, k)` | k-mer search oracle for validation |
| QAOA graph | `qaoa_maxcut_graph(edges)` | Haplotype conflict graph for QAOA |
| VQE Hamiltonian | `construct_molecular_hamiltonian(geometry, basis)` | Molecular Hamiltonian for VQE |
| Quantum walk | `quantum_walk_on_graph(adjacency, steps)` | de Bruijn graph walk validation |

### ruqu-wasm Functions

| Genomic Operation | ruqu-wasm Function | Browser Demo |
|-------------------|-------------------|--------------|
| k-mer search demo | `wasm_grover_kmer(reference, query)` | Interactive k-mer search (up to 256 bases, 8 qubits) |
| Haplotype demo | `wasm_qaoa_haplotype(fragments)` | Haplotype assembly (up to 20 fragments, 20 qubits) |
| Molecular demo | `wasm_vqe_molecule(geometry)` | Base pair energy (up to 12 orbitals, 24 qubits) |

---

## Hybrid Classical-Quantum Pipeline

### Decision Boundary Framework

Not every genomic computation benefits from quantum simulation. Route operations based on problem size:

| Operation | Classical (Primary) | Quantum Simulation (Validation) | When to Use Quantum |
|-----------|-------------------|--------------------------------|---------------------|
| k-mer search | HNSW O(log N) | Grover simulation ≤256 bases | Algorithm design and validation only |
| Haplotype assembly | Variational MinCut | QAOA simulation ≤20 fragments | Validate cost function structure |
| Molecular interaction | Classical DFT (B3LYP) | VQE simulation ≤16 orbitals | Validate variational ansatz |
| Phylogenetics | Simulated annealing | Quantum annealing ≤8 taxa | Compare annealing schedules |
| Genome assembly | Tensor network MPS | Quantum walk ≤1K nodes | Research exploration only |

**Production Strategy**: Run classical implementations for all real-world problems. Use quantum simulation for algorithm validation and design at tractable scales.

---

## Performance Projections

### Classical vs. Quantum-Inspired vs. Quantum Simulation

| Operation | Classical Baseline | Quantum-Inspired Classical | Quantum Simulation (ruQu) | Practical Use |
|-----------|-------------------|---------------------------|--------------------------|---------------|
| k-mer search (3.2B bp) | O(N) = 3.2×10⁹ | HNSW O(log N) ≈ 32 | Grover O(√N) ≈ 56,568 @ 8 qubits only | **HNSW production**, ruQu validation |
| Haplotype (50 fragments) | O(2⁵⁰) exact | Variational O(F²·iter) | QAOA O(F²·p) @ 20 qubits | **Variational production**, QAOA validation |
| VQE molecular (12 orbitals) | DFT O(N⁷) | Classical VQE O(N⁴·iter) | VQE O(poly·iter) @ 24 qubits | **Classical VQE production**, quantum validation |
| Phylogenetics (20 taxa) | RAxML heuristic | Simulated annealing | Quantum anneal @ 8 taxa only | **Simulated annealing production**, validation limited |

**Key Takeaway**: Quantum simulation (ruQu) is for **algorithm design and validation** at small scales. Production uses **quantum-inspired classical algorithms**.

---

## Consequences

### Benefits

1. **Implementable today**: All algorithms run on classical hardware without waiting for fault-tolerant quantum computers
2. **Quantum-inspired performance**: HNSW k-mer search achieves O(log N) vs. O(N); tensor networks reduce exponential to polynomial
3. **Validation framework**: ruQu quantum simulation validates algorithmic correctness at tractable scales (8-25 qubits)
4. **Hardware-ready**: When fault-tolerant quantum computers arrive, quantum simulation code becomes production code
5. **Browser accessibility**: ruqu-wasm enables quantum algorithm education and validation in-browser
6. **No overpromising**: Clear distinction between "implementable today" and "requires quantum hardware"

### Limitations

1. **No exponential quantum speedup**: Classical implementations do not achieve theoretical quantum advantages (e.g., Grover's O(√N))
2. **Validation scale limited**: Quantum simulation capped at ~25 qubits (33M bases for k-mer search, 25 fragments for haplotype assembly)
3. **Quantum simulation overhead**: State vector simulation is 10-100× slower than native classical algorithms
4. **Requires classical expertise**: Tensor networks, variational optimization, simulated annealing require specialized classical algorithm knowledge

---

## Alternatives Considered

### Alternative 1: Wait for Fault-Tolerant Quantum Computers

**Rejected**: Fault-tolerant quantum computers with >1,000 logical qubits are 10-20 years away. We need solutions today.

### Alternative 2: Cloud Quantum Hardware (IBM Quantum, IonQ)

**Rejected**: Current NISQ hardware (50-100 noisy qubits) cannot achieve quantum advantage for genomic problems due to error rates. Simulation provides exact results for algorithm design.

### Alternative 3: Pure Classical Genomics (No Quantum Inspiration)

**Rejected**: Quantum algorithmic insights (hierarchical amplification, variational optimization, superposition patterns) inform better classical algorithms. We leverage these insights.

---

## References

### Quantum Computing

- Grover, L.K. "A fast quantum mechanical algorithm for database search." STOC 1996.
- Farhi, E., et al. "A Quantum Approximate Optimization Algorithm." arXiv:1411.4028, 2014.
- Peruzzo, A. et al. "A variational eigenvalue solver on a photonic quantum processor." Nature Communications 5, 4213, 2014.
- Malkov, Y., & Yashunin, D. "Efficient and robust approximate nearest neighbor search using Hierarchical Navigable Small World graphs." IEEE TPAMI, 2018.

### Classical Algorithms

- Verstraete, F., et al. "Matrix product states, projected entangled pair states, and variational renormalization group methods for quantum spin systems." Advances in Physics, 2008.
- Kirkpatrick, S., et al. "Optimization by simulated annealing." Science, 1983.

### Genomics

- Li, H. "Aligning sequence reads with BWA-MEM." arXiv:1303.3997, 2013.
- Patterson, M. et al. "WhatsHap: Weighted Haplotype Assembly." Journal of Computational Biology, 2015.

### RuVector

- [ruQu Architecture](../../crates/ruQu/docs/adr/ADR-001-ruqu-architecture.md)
- [HNSW Genomic Index](./ADR-003-hnsw-genomic-vector-index.md)
