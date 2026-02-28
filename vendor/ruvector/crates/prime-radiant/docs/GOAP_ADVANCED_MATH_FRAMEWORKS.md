# GOAP Implementation Plan: Advanced Mathematical Frameworks for Prime-Radiant

**Version**: 1.0.0
**Date**: 2026-01-22
**Author**: SPARC-GOAP Planning System
**Status**: Planning Phase

---

## Executive Summary

This document provides a comprehensive Goal-Oriented Action Plan (GOAP) for implementing 6 cutting-edge mathematical frameworks into the Prime-Radiant coherence engine. Each framework enhances the existing sheaf Laplacian architecture with advanced theoretical foundations.

### Current State Analysis

```rust
current_state = {
    sheaf_substrate: true,           // SheafGraph, SheafNode, SheafEdge, RestrictionMap
    spectral_analysis: "basic",      // Eigenvalue drift detection, basic Laplacian
    coherence_engine: true,          // Energy computation, residual calculation
    attention_system: true,          // Topology-gated, MoE, PDE diffusion
    mincut_isolation: true,          // Subpolynomial dynamic mincut
    hyperbolic_geometry: true,       // Poincare ball, depth-weighted energy
    governance_layer: true,          // Policy bundles, witness records
    wasm_support: "partial",         // Some crates have WASM bindings
    test_coverage: "~70%",
    sheaf_cohomology: false,
    category_theory: false,
    homotopy_type_theory: false,
    spectral_invariants: "basic",
    causal_abstraction: false,
    quantum_topology: false
}

goal_state = {
    sheaf_cohomology: true,          // H^0, H^1 computation, obstruction detection
    category_theory: true,           // Functorial retrieval, topos-theoretic belief
    homotopy_type_theory: true,      // HoTT embedding, proof assistant style
    spectral_invariants: "advanced", // Cheeger bounds, spectral collapse prediction
    causal_abstraction: true,        // Causal layers, structural causality
    quantum_topology: true,          // TQC encodings, spectral topology
    all_wasm_exports: true,
    test_coverage: ">85%",
    benchmarks_complete: true,
    adr_documented: true
}
```

---

## Framework 1: Sheaf Cohomology

### Goal State Definition

Compute cohomological obstructions for belief graphs, enabling detection of global consistency failures that local residuals miss.

### Mathematical Foundation

```text
Sheaf Cohomology on Graphs:
- H^0(X, F) = Global sections (consistent assignments)
- H^1(X, F) = Obstruction cocycles (inconsistency indicators)
- Coboundary operator: delta: C^0 -> C^1
- Cohomology energy: E_coh = ||H^1(X, F)||^2
```

### Module Architecture

```
crates/prime-radiant/src/cohomology/
├── mod.rs                    # Module root, public API
├── cochain.rs               # C^0, C^1 cochain spaces
├── coboundary.rs            # Coboundary operator implementation
├── cohomology_group.rs      # H^0, H^1 computation
├── obstruction.rs           # Obstruction detection and classification
├── sheaf_diffusion.rs       # Diffusion with cohomology indicators
├── neural_sheaf.rs          # Sheaf Neural Network layers
└── config.rs                # Configuration and parameters
```

### Key Data Structures

```rust
/// Cochain in degree k
pub struct Cochain<const K: usize> {
    /// Values indexed by k-simplices
    values: HashMap<SimplexId<K>, Vec<f32>>,
    /// Dimension of stalk
    stalk_dim: usize,
}

/// Cohomology class in H^k
pub struct CohomologyClass<const K: usize> {
    /// Representative cocycle
    representative: Cochain<K>,
    /// Betti number contribution
    betti_contribution: usize,
    /// Energy measure
    cohomology_energy: f32,
}

/// Obstruction indicator
pub struct Obstruction {
    /// Location (edge or higher simplex)
    location: SimplexId<1>,
    /// Obstruction class in H^1
    class: CohomologyClass<1>,
    /// Severity (0.0 to 1.0)
    severity: f32,
    /// Suggested repair strategy
    repair_hint: RepairStrategy,
}

/// Sheaf Neural Network layer
pub struct SheafNeuralLayer {
    /// Learnable restriction maps
    rho_weights: HashMap<EdgeId, Array2<f32>>,
    /// Laplacian diffusion operator
    laplacian: SheafLaplacian,
    /// Cohomology-aware attention
    attention: CohomologyAttention,
}
```

### Key Traits

```rust
/// Computes sheaf cohomology
pub trait SheafCohomology {
    type Sheaf;
    type Coefficient;

    /// Compute H^0 (global sections)
    fn h0(&self, sheaf: &Self::Sheaf) -> CohomologyGroup<0>;

    /// Compute H^1 (first cohomology)
    fn h1(&self, sheaf: &Self::Sheaf) -> CohomologyGroup<1>;

    /// Check if sheaf is globally consistent
    fn is_consistent(&self, sheaf: &Self::Sheaf) -> bool;

    /// Identify obstruction cocycles
    fn obstructions(&self, sheaf: &Self::Sheaf) -> Vec<Obstruction>;
}

/// Cohomology-informed diffusion
pub trait CohomologyDiffusion {
    /// Diffuse with cohomology-weighted Laplacian
    fn diffuse_with_cohomology(
        &self,
        state: &[f32],
        steps: usize,
        cohomology_weight: f32,
    ) -> Vec<f32>;
}
```

### Integration Points

| Existing Module | Integration Type | Description |
|-----------------|-----------------|-------------|
| `substrate::SheafGraph` | Extension | Add simplex enumeration methods |
| `coherence::CoherenceEngine` | Augment | Add H^1 energy to total energy |
| `attention::AttentionCoherence` | Augment | Cohomology-weighted attention |
| `learned_rho::LearnedRestrictionMap` | Extend | Train rho to minimize H^1 |

### WASM Export Strategy

```rust
#[wasm_bindgen]
pub struct WasmSheafCohomology {
    inner: SheafCohomology,
}

#[wasm_bindgen]
impl WasmSheafCohomology {
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<WasmSheafCohomology, JsValue>;

    pub fn compute_h0(&self, graph: &WasmSheafGraph) -> JsValue;
    pub fn compute_h1(&self, graph: &WasmSheafGraph) -> JsValue;
    pub fn detect_obstructions(&self, graph: &WasmSheafGraph) -> JsValue;
    pub fn cohomology_energy(&self, graph: &WasmSheafGraph) -> f32;
}
```

### Test Cases

1. **Unit Tests**
   - `test_coboundary_squares_to_zero`: delta^2 = 0
   - `test_exact_sequence`: im(delta) subset of ker(delta)
   - `test_consistent_sheaf_h1_vanishes`: H^1 = 0 for consistent sheafs
   - `test_obstruction_detection`: Known obstructions are found

2. **Property Tests**
   - `prop_betti_numbers_stable`: Betti numbers unchanged under small perturbations
   - `prop_cohomology_energy_nonnegative`: E_coh >= 0

3. **Integration Tests**
   - `test_cohomology_with_mincut`: Obstructions correlate with cut edges
   - `test_sheaf_neural_convergence`: SNN training reduces H^1

### Benchmarks

| Benchmark | Target | Notes |
|-----------|--------|-------|
| `H^1 computation (1K nodes)` | <10ms | Sparse matrix ops |
| `Obstruction detection (1K nodes)` | <5ms | After H^1 cached |
| `SNN forward pass (1K nodes)` | <20ms | GPU optional |

### ADR Outline

**ADR-020: Sheaf Cohomology Integration**
- Status: Proposed
- Context: Need global consistency detection beyond local residuals
- Decision: Implement H^0/H^1 with coboundary operator
- Consequences: More accurate hallucination detection, higher compute

---

## Framework 2: Category Theory / Topos

### Goal State Definition

Implement functorial retrieval systems and topos-theoretic belief models with higher category coherence laws.

### Mathematical Foundation

```text
Category-Theoretic Coherence:
- Objects: Belief states, contexts
- Morphisms: Belief transformations
- Functors: Context-preserving mappings
- Natural transformations: Coherence laws
- Topos: Generalized logic over belief graphs
```

### Module Architecture

```
crates/prime-radiant/src/category/
├── mod.rs                    # Module root
├── category.rs               # Category trait and basic types
├── functor.rs                # Functor implementations
├── natural_transform.rs      # Natural transformations
├── monad.rs                  # Monad for belief composition
├── topos/
│   ├── mod.rs               # Topos submodule
│   ├── subobject.rs         # Subobject classifier
│   ├── internal_logic.rs    # Internal logic operations
│   └── sheaf_topos.rs       # Sheaf topos on coherence graph
├── retrieval.rs              # Functorial retrieval system
├── coherence_laws.rs         # Higher coherence laws (associativity, etc.)
└── config.rs
```

### Key Data Structures

```rust
/// A category of belief states
pub struct BeliefCategory {
    /// Object set (belief state types)
    objects: Vec<BeliefType>,
    /// Morphism set (transformations)
    morphisms: HashMap<(BeliefType, BeliefType), Vec<BeliefMorphism>>,
    /// Identity morphisms
    identities: HashMap<BeliefType, BeliefMorphism>,
}

/// Functor between categories
pub struct BeliefFunctor<C: Category, D: Category> {
    /// Object mapping
    object_map: HashMap<C::Object, D::Object>,
    /// Morphism mapping (preserves composition)
    morphism_map: HashMap<C::Morphism, D::Morphism>,
}

/// Natural transformation
pub struct NaturalTransformation<F: Functor, G: Functor> {
    /// Components: eta_X: F(X) -> G(X) for each object X
    components: HashMap<F::Source::Object, F::Target::Morphism>,
}

/// Topos over belief graph
pub struct BeliefTopos {
    /// Underlying category
    category: BeliefCategory,
    /// Subobject classifier (truth values)
    omega: SubobjectClassifier,
    /// Internal Heyting algebra for logic
    heyting: HeytingAlgebra,
    /// Sheaf condition enforcement
    sheaf_condition: SheafCondition,
}

/// Coherence law checker
pub struct CoherenceLaw {
    /// Law name (e.g., "associativity", "unit")
    name: String,
    /// Diagram that must commute
    diagram: CommutativeDiagram,
    /// Tolerance for approximate commutativity
    tolerance: f32,
}
```

### Key Traits

```rust
/// Category abstraction
pub trait Category {
    type Object: Clone + Eq + Hash;
    type Morphism: Clone;

    fn identity(&self, obj: &Self::Object) -> Self::Morphism;
    fn compose(&self, f: &Self::Morphism, g: &Self::Morphism) -> Option<Self::Morphism>;
    fn source(&self, f: &Self::Morphism) -> Self::Object;
    fn target(&self, f: &Self::Morphism) -> Self::Object;
}

/// Functor between categories
pub trait Functor {
    type Source: Category;
    type Target: Category;

    fn map_object(&self, obj: &<Self::Source as Category>::Object)
        -> <Self::Target as Category>::Object;
    fn map_morphism(&self, f: &<Self::Source as Category>::Morphism)
        -> <Self::Target as Category>::Morphism;
}

/// Topos operations
pub trait Topos: Category {
    type SubobjectClassifier;

    fn omega(&self) -> &Self::SubobjectClassifier;
    fn truth(&self) -> Self::Morphism;  // 1 -> Omega
    fn chi(&self, mono: &Self::Morphism) -> Self::Morphism;  // Characteristic morphism

    /// Internal logic
    fn internal_and(&self, a: &Self::Morphism, b: &Self::Morphism) -> Self::Morphism;
    fn internal_or(&self, a: &Self::Morphism, b: &Self::Morphism) -> Self::Morphism;
    fn internal_implies(&self, a: &Self::Morphism, b: &Self::Morphism) -> Self::Morphism;
}

/// Functorial retrieval
pub trait FunctorialRetrieval {
    type Query;
    type Result;
    type Context;

    /// Retrieve with functor-preserved context
    fn retrieve(&self, query: Self::Query, context: Self::Context) -> Vec<Self::Result>;

    /// Verify naturality (consistency across context changes)
    fn verify_naturality(&self, transform: &NaturalTransformation) -> bool;
}
```

### Integration Points

| Existing Module | Integration Type | Description |
|-----------------|-----------------|-------------|
| `substrate::RestrictionMap` | Morphism | Rho maps as category morphisms |
| `coherence::CoherenceEngine` | Functor | Engine as functor from graphs to energies |
| `governance::PolicyBundle` | Topos | Policies as internal logic formulas |

### WASM Export Strategy

```rust
#[wasm_bindgen]
pub struct WasmBeliefTopos {
    inner: BeliefTopos,
}

#[wasm_bindgen]
impl WasmBeliefTopos {
    pub fn internal_entailment(&self, premise: JsValue, conclusion: JsValue) -> bool;
    pub fn check_coherence_law(&self, law_name: &str) -> f32;  // Returns violation magnitude
    pub fn functorial_retrieve(&self, query: JsValue, context: JsValue) -> JsValue;
}
```

### Test Cases

1. **Unit Tests**
   - `test_identity_law`: id o f = f = f o id
   - `test_associativity`: (f o g) o h = f o (g o h)
   - `test_functor_preserves_composition`: F(g o f) = F(g) o F(f)
   - `test_naturality_square_commutes`

2. **Property Tests**
   - `prop_topos_has_terminal_object`
   - `prop_subobject_classifier_unique`

### Benchmarks

| Benchmark | Target | Notes |
|-----------|--------|-------|
| `Functor application (1K objects)` | <5ms | |
| `Naturality check (100 morphisms)` | <10ms | |
| `Internal logic query` | <1ms | |

### ADR Outline

**ADR-021: Category-Theoretic Belief Models**
- Status: Proposed
- Context: Need compositional semantics for belief transformations
- Decision: Implement topos-theoretic framework
- Consequences: Enables formal verification, steeper learning curve

---

## Framework 3: Homotopy Type Theory

### Goal State Definition

Embed HoTT formal system for proof-carrying coherence verification with Coq/Agda-style type checking.

### Mathematical Foundation

```text
Homotopy Type Theory:
- Types as spaces
- Terms as points
- Equality types as paths: Id(A, x, y)
- Path induction (J-eliminator)
- Univalence: (A ≃ B) ≃ (A = B)
- Higher inductive types for coherence
```

### Module Architecture

```
crates/prime-radiant/src/hott/
├── mod.rs                    # Module root
├── types/
│   ├── mod.rs
│   ├── universe.rs           # Type universe hierarchy
│   ├── identity.rs           # Identity types (paths)
│   ├── sigma.rs              # Dependent sum types
│   ├── pi.rs                 # Dependent product types
│   └── higher_inductive.rs   # HITs for coherence graphs
├── paths/
│   ├── mod.rs
│   ├── path.rs               # Path type implementation
│   ├── composition.rs        # Path composition
│   ├── inverse.rs            # Path inversion
│   └── homotopy.rs           # Homotopies between paths
├── univalence/
│   ├── mod.rs
│   ├── equivalence.rs        # Type equivalences
│   ├── transport.rs          # Transport along paths
│   └── ua.rs                 # Univalence axiom
├── proofs/
│   ├── mod.rs
│   ├── proof_term.rs         # Proof term representation
│   ├── type_checker.rs       # Bidirectional type checking
│   ├── normalization.rs      # Beta/eta normalization
│   └── coherence_proof.rs    # Proofs of coherence properties
├── embedding.rs              # Embed coherence in HoTT
└── config.rs
```

### Key Data Structures

```rust
/// HoTT Universe level
pub type Level = u32;

/// HoTT Type
#[derive(Clone, Debug)]
pub enum HoTTType {
    /// Universe at level i
    Universe(Level),
    /// Identity type Id_A(x, y)
    Identity { ty: Box<HoTTType>, left: Term, right: Term },
    /// Dependent sum Σ(x:A).B(x)
    Sigma { base: Box<HoTTType>, fiber: Box<Closure> },
    /// Dependent product Π(x:A).B(x)
    Pi { domain: Box<HoTTType>, codomain: Box<Closure> },
    /// Higher inductive type
    HIT(HigherInductiveType),
    /// Base types
    Unit, Empty, Bool, Nat,
}

/// HoTT Term (proof term)
#[derive(Clone, Debug)]
pub enum Term {
    /// Variable
    Var(usize),
    /// Lambda abstraction
    Lambda { ty: HoTTType, body: Box<Term> },
    /// Application
    App { func: Box<Term>, arg: Box<Term> },
    /// Pair (for Sigma types)
    Pair { fst: Box<Term>, snd: Box<Term> },
    /// Reflexivity proof: refl : Id_A(x, x)
    Refl,
    /// J-eliminator for identity
    J { motive: Box<Term>, refl_case: Box<Term>, path: Box<Term> },
    /// Transport along path
    Transport { path: Box<Term>, point: Box<Term> },
    /// Constructor for HIT
    HITConstructor { hit: HigherInductiveType, idx: usize, args: Vec<Term> },
}

/// Higher Inductive Type for coherence graphs
#[derive(Clone, Debug)]
pub struct CoherenceHIT {
    /// Point constructors (nodes)
    points: Vec<PointConstructor>,
    /// Path constructors (edges -> paths)
    paths: Vec<PathConstructor>,
    /// Higher path constructors (coherences)
    higher_paths: Vec<HigherPathConstructor>,
}

/// Proof of coherence property
pub struct CoherenceProof {
    /// Statement being proved
    statement: HoTTType,
    /// Proof term
    proof: Term,
    /// Normalized form
    normal_form: Option<Term>,
    /// Type-checking trace
    derivation: TypeDerivation,
}
```

### Key Traits

```rust
/// Type checking
pub trait TypeChecker {
    type Error;

    /// Check term has given type
    fn check(&self, ctx: &Context, term: &Term, ty: &HoTTType) -> Result<(), Self::Error>;

    /// Infer type of term
    fn infer(&self, ctx: &Context, term: &Term) -> Result<HoTTType, Self::Error>;

    /// Check type well-formedness
    fn check_type(&self, ctx: &Context, ty: &HoTTType) -> Result<Level, Self::Error>;
}

/// Path operations
pub trait PathOps {
    /// Compose paths: p o q
    fn compose(&self, p: &Term, q: &Term) -> Term;

    /// Invert path: p^{-1}
    fn invert(&self, p: &Term) -> Term;

    /// Transport along path
    fn transport(&self, path: &Term, point: &Term) -> Term;

    /// Apply function to path
    fn ap(&self, f: &Term, p: &Term) -> Term;
}

/// Coherence embedding
pub trait CoherenceEmbedding {
    /// Embed sheaf graph as HIT
    fn embed_graph(&self, graph: &SheafGraph) -> CoherenceHIT;

    /// Embed edge constraint as path type
    fn embed_constraint(&self, edge: &SheafEdge) -> HoTTType;

    /// Construct coherence proof
    fn prove_coherence(&self, graph: &SheafGraph) -> Result<CoherenceProof, ProofError>;
}
```

### Integration Points

| Existing Module | Integration Type | Description |
|-----------------|-----------------|-------------|
| `substrate::SheafGraph` | Embed | Graph as HIT type |
| `coherence::CoherenceEnergy` | Proof | Energy bounds as theorems |
| `governance::WitnessRecord` | Proof term | Witnesses as proof terms |

### WASM Export Strategy

```rust
#[wasm_bindgen]
pub struct WasmHoTTChecker {
    inner: HoTTTypeChecker,
}

#[wasm_bindgen]
impl WasmHoTTChecker {
    pub fn check_coherence(&self, graph: &WasmSheafGraph) -> JsValue;  // Returns proof or error
    pub fn verify_proof(&self, proof_json: &str) -> bool;
    pub fn normalize(&self, term_json: &str) -> String;
}
```

### Test Cases

1. **Unit Tests**
   - `test_refl_type_checks`: refl : Id_A(x, x)
   - `test_j_eliminator`: J computes correctly on refl
   - `test_transport_along_refl`: transport(refl, x) = x
   - `test_path_composition_associative`

2. **Property Tests**
   - `prop_type_checking_decidable`
   - `prop_normalization_terminates`
   - `prop_proofs_verify`

### Benchmarks

| Benchmark | Target | Notes |
|-----------|--------|-------|
| `Type checking (small proof)` | <1ms | |
| `Proof normalization` | <10ms | |
| `Coherence proof construction (100 nodes)` | <100ms | |

### ADR Outline

**ADR-022: Homotopy Type Theory Integration**
- Status: Proposed
- Context: Need formal verification of coherence properties
- Decision: Implement HoTT core with proof terms
- Consequences: Enables proof export to Coq/Agda, significant complexity

---

## Framework 4: Spectral Invariants (Advanced)

### Goal State Definition

Extend current spectral analysis with Cheeger bounds, second eigenvalue cut prediction, and spectral collapse predictors.

### Mathematical Foundation

```text
Advanced Spectral Theory:
- Cheeger inequality: h(G) >= λ_2 / 2  (h = conductance)
- Second eigenvalue: λ_2 (algebraic connectivity)
- Spectral gap: λ_2 - λ_1 (stability indicator)
- Higher eigenvalue ratios: predict structural changes
- Spectral collapse: λ_i -> λ_j as graph degenerates
```

### Module Architecture

```
crates/prime-radiant/src/spectral/
├── mod.rs                    # Module root (extends coherence/spectral.rs)
├── cheeger.rs                # Cheeger bounds and conductance
├── eigenvalue/
│   ├── mod.rs
│   ├── second.rs             # λ_2 analysis and prediction
│   ├── higher.rs             # Higher eigenvalue analysis
│   ├── gap.rs                # Spectral gap tracking
│   └── collapse.rs           # Spectral collapse detection
├── cut_prediction.rs         # Predict cuts from eigenvalues
├── stability.rs              # Stability analysis
├── laplacian/
│   ├── mod.rs
│   ├── normalized.rs         # Normalized Laplacian
│   ├── sheaf_laplacian.rs    # Full sheaf Laplacian matrix
│   └── sparse.rs             # Sparse matrix operations
└── config.rs
```

### Key Data Structures

```rust
/// Cheeger analysis result
pub struct CheegerAnalysis {
    /// Cheeger constant (conductance lower bound)
    cheeger_constant: f32,
    /// Lower bound from spectral gap: λ_2 / 2
    spectral_lower_bound: f32,
    /// Upper bound: √(2 * λ_2)
    spectral_upper_bound: f32,
    /// Tightness of bound
    bound_tightness: f32,
    /// Suggested cut set (if Cheeger is low)
    suggested_cut: Option<Vec<EdgeId>>,
}

/// Second eigenvalue analysis
pub struct SecondEigenvalueAnalysis {
    /// λ_2 value
    lambda_2: f64,
    /// Corresponding eigenvector (Fiedler vector)
    fiedler_vector: Vec<f64>,
    /// Predicted cut (from Fiedler vector sign)
    predicted_cut: CutPartition,
    /// Cut quality score
    cut_quality: f32,
    /// Time trend of λ_2
    lambda_2_trend: TrendDirection,
}

/// Spectral collapse indicator
pub struct SpectralCollapse {
    /// Collapsing eigenvalue pairs
    collapsing_pairs: Vec<(usize, usize)>,
    /// Collapse velocity (rate of approach)
    collapse_velocity: f64,
    /// Predicted time to collapse
    time_to_collapse: Option<Duration>,
    /// Severity level
    severity: CollapseSeverity,
    /// Structural interpretation
    interpretation: String,
}

/// Full spectral signature
pub struct SpectralSignature {
    /// Eigenvalue spectrum (sorted)
    spectrum: Vec<f64>,
    /// Spectral density
    density: SpectralDensity,
    /// Cheeger bound
    cheeger: CheegerAnalysis,
    /// Key eigenvalue analyses
    key_eigenvalues: KeyEigenvalueSet,
    /// Collapse indicators
    collapse_indicators: Vec<SpectralCollapse>,
}
```

### Key Traits

```rust
/// Cheeger bound computation
pub trait CheegerBound {
    /// Compute Cheeger constant approximation
    fn cheeger_constant(&self, graph: &SheafGraph) -> f32;

    /// Compute spectral bounds
    fn spectral_bounds(&self, lambda_2: f64) -> (f64, f64);

    /// Find approximate Cheeger cut
    fn find_cheeger_cut(&self, graph: &SheafGraph) -> Option<CutPartition>;
}

/// Second eigenvalue analysis
pub trait SecondEigenvalue {
    /// Compute λ_2 efficiently
    fn compute_lambda_2(&self, laplacian: &SheafLaplacian) -> f64;

    /// Compute Fiedler vector
    fn fiedler_vector(&self, laplacian: &SheafLaplacian) -> Vec<f64>;

    /// Predict optimal cut from Fiedler
    fn predict_cut(&self, fiedler: &[f64]) -> CutPartition;
}

/// Spectral collapse detection
pub trait CollapseDetector {
    /// Detect eigenvalue collapse
    fn detect_collapse(&self, history: &EigenvalueHistory) -> Vec<SpectralCollapse>;

    /// Predict future collapse
    fn predict_collapse(&self, current: &[f64], velocity: &[f64]) -> Option<SpectralCollapse>;

    /// Interpret collapse structurally
    fn interpret(&self, collapse: &SpectralCollapse) -> String;
}
```

### Integration Points

| Existing Module | Integration Type | Description |
|-----------------|-----------------|-------------|
| `coherence::spectral` | Extend | Add advanced analysis |
| `mincut::IncoherenceIsolator` | Use | Cheeger cut -> mincut |
| `attention::AttentionCoherence` | Inform | Spectral weights |

### WASM Export Strategy

```rust
#[wasm_bindgen]
pub struct WasmSpectralAnalysis {
    inner: SpectralAnalyzer,
}

#[wasm_bindgen]
impl WasmSpectralAnalysis {
    pub fn cheeger_bounds(&self, graph: &WasmSheafGraph) -> JsValue;
    pub fn predict_cut(&self, graph: &WasmSheafGraph) -> JsValue;
    pub fn detect_collapse(&self) -> JsValue;
    pub fn spectral_signature(&self, graph: &WasmSheafGraph) -> JsValue;
}
```

### Test Cases

1. **Unit Tests**
   - `test_cheeger_inequality_holds`: h >= λ_2/2
   - `test_fiedler_vector_orthogonal_to_constant`: <v_2, 1> = 0
   - `test_collapse_detection_accuracy`

2. **Property Tests**
   - `prop_eigenvalues_nonnegative`
   - `prop_spectral_gap_positive_for_connected`
   - `prop_fiedler_cut_valid`

### Benchmarks

| Benchmark | Target | Notes |
|-----------|--------|-------|
| `λ_2 computation (1K nodes)` | <50ms | Use iterative methods |
| `Full spectrum (1K nodes)` | <500ms | |
| `Cheeger cut (1K nodes)` | <20ms | |

### ADR Outline

**ADR-023: Advanced Spectral Invariants**
- Status: Proposed
- Context: Current spectral analysis lacks predictive power
- Decision: Add Cheeger bounds, collapse detection
- Consequences: Better cut prediction, more accurate drift warning

---

## Framework 5: Causal Abstraction Networks

### Goal State Definition

Implement causal abstraction layers with structural causality enforcement for belief propagation.

### Mathematical Foundation

```text
Causal Abstraction:
- Structural Causal Models (SCM)
- Interventions: do(X = x)
- Causal graphs: DAGs with edge semantics
- Abstraction: High-level -> Low-level mapping
- Causal consistency: Interventions commute with abstraction
```

### Module Architecture

```
crates/prime-radiant/src/causal/
├── mod.rs                    # Module root
├── scm/
│   ├── mod.rs                # Structural Causal Model
│   ├── variable.rs           # Causal variables
│   ├── mechanism.rs          # Causal mechanisms (functions)
│   └── intervention.rs       # Do-calculus operations
├── dag/
│   ├── mod.rs                # Causal DAG
│   ├── builder.rs            # DAG construction
│   ├── validity.rs           # Acyclicity checking
│   └── paths.rs              # Causal paths (d-separation)
├── abstraction/
│   ├── mod.rs                # Causal abstraction
│   ├── layer.rs              # Abstraction layer
│   ├── mapping.rs            # High-low mapping
│   ├── consistency.rs        # Consistency checking
│   └── constructive.rs       # Constructive abstraction
├── enforcement.rs            # Causality enforcement
├── propagation.rs            # Belief propagation with causality
└── config.rs
```

### Key Data Structures

```rust
/// Causal variable
pub struct CausalVariable {
    /// Variable identifier
    id: VariableId,
    /// Variable name
    name: String,
    /// Domain type
    domain: VariableDomain,
    /// Current value (if observed/intervened)
    value: Option<Value>,
    /// Is this variable intervened?
    intervened: bool,
}

/// Structural Causal Model
pub struct StructuralCausalModel {
    /// Variables in the model
    variables: HashMap<VariableId, CausalVariable>,
    /// Causal DAG
    dag: CausalDAG,
    /// Mechanisms: parent values -> child value
    mechanisms: HashMap<VariableId, Mechanism>,
    /// Exogenous noise terms
    noise: HashMap<VariableId, NoiseDistribution>,
}

/// Causal abstraction layer
pub struct AbstractionLayer {
    /// Source model (low-level)
    source: StructuralCausalModel,
    /// Target model (high-level)
    target: StructuralCausalModel,
    /// Variable mapping: high -> Vec<low>
    variable_mapping: HashMap<VariableId, Vec<VariableId>>,
    /// Intervention mapping: maps interventions
    intervention_mapping: InterventionMapping,
}

/// Causal coherence constraint
pub struct CausalConstraint {
    /// Nodes that must respect causal order
    causal_nodes: Vec<NodeId>,
    /// Required causal edges
    required_edges: Vec<(NodeId, NodeId)>,
    /// Forbidden edges (would create cycles)
    forbidden_edges: Vec<(NodeId, NodeId)>,
    /// Enforcement strength
    strength: f32,
}
```

### Key Traits

```rust
/// Structural Causal Model operations
pub trait SCMOps {
    /// Perform intervention do(X = x)
    fn intervene(&mut self, var: VariableId, value: Value);

    /// Compute causal effect P(Y | do(X = x))
    fn causal_effect(&self, target: VariableId, intervention: &[(VariableId, Value)]) -> Distribution;

    /// Check d-separation
    fn d_separated(&self, x: VariableId, y: VariableId, z: &[VariableId]) -> bool;

    /// Find causal ancestors
    fn ancestors(&self, var: VariableId) -> Vec<VariableId>;
}

/// Causal abstraction
pub trait CausalAbstraction {
    /// Check abstraction consistency
    fn is_consistent(&self) -> bool;

    /// Lift intervention to high level
    fn lift_intervention(&self, low_intervention: Intervention) -> Option<Intervention>;

    /// Project high-level state to low level
    fn project(&self, high_state: &State) -> State;

    /// Compute abstraction error
    fn abstraction_error(&self) -> f64;
}

/// Causal enforcement for coherence
pub trait CausalEnforcement {
    /// Add causal constraints to graph
    fn add_causal_constraint(&mut self, constraint: CausalConstraint);

    /// Check if edge respects causality
    fn is_causally_valid(&self, source: NodeId, target: NodeId) -> bool;

    /// Compute causal energy (violation of causal constraints)
    fn causal_energy(&self, graph: &SheafGraph) -> f32;

    /// Suggest causal repairs
    fn suggest_repairs(&self, graph: &SheafGraph) -> Vec<CausalRepair>;
}
```

### Integration Points

| Existing Module | Integration Type | Description |
|-----------------|-----------------|-------------|
| `substrate::SheafGraph` | Augment | Add causal edge semantics |
| `coherence::CoherenceEngine` | Add term | Causal energy in total |
| `governance::PolicyBundle` | Extend | Causal policy constraints |
| `ruvllm_integration` | Gate | Causal validity for LLM outputs |

### WASM Export Strategy

```rust
#[wasm_bindgen]
pub struct WasmCausalModel {
    inner: StructuralCausalModel,
}

#[wasm_bindgen]
impl WasmCausalModel {
    pub fn intervene(&mut self, var_id: u64, value: JsValue);
    pub fn causal_effect(&self, target: u64, interventions: JsValue) -> JsValue;
    pub fn is_d_separated(&self, x: u64, y: u64, z: JsValue) -> bool;
}

#[wasm_bindgen]
pub struct WasmCausalEnforcement {
    inner: CausalEnforcer,
}

#[wasm_bindgen]
impl WasmCausalEnforcement {
    pub fn causal_energy(&self, graph: &WasmSheafGraph) -> f32;
    pub fn check_validity(&self, source: u64, target: u64) -> bool;
}
```

### Test Cases

1. **Unit Tests**
   - `test_intervention_blocks_parents`
   - `test_d_separation_correct`
   - `test_abstraction_consistency`
   - `test_causal_energy_zero_for_valid_graph`

2. **Property Tests**
   - `prop_dag_acyclic`
   - `prop_intervention_idempotent`
   - `prop_abstraction_commutes_with_intervention`

### Benchmarks

| Benchmark | Target | Notes |
|-----------|--------|-------|
| `Intervention (100 vars)` | <1ms | |
| `D-separation check` | <0.1ms | |
| `Causal energy (1K nodes)` | <10ms | |

### ADR Outline

**ADR-024: Causal Abstraction Networks**
- Status: Proposed
- Context: Coherence should respect causal structure
- Decision: Implement SCM with abstraction layers
- Consequences: More interpretable coherence, can explain failures

---

## Framework 6: Quantum/Algebraic Topology

### Goal State Definition

Implement topological quantum encodings and spectral topology invariants for robust coherence detection.

### Mathematical Foundation

```text
Algebraic Topology for Coherence:
- Simplicial complexes from belief graphs
- Persistent homology: H_k across filtrations
- Betti numbers: β_0 (components), β_1 (loops), β_2 (voids)
- Topological Data Analysis (TDA)
- Quantum topology: topological quantum codes

Quantum Aspects:
- Anyonic braiding for coherence locks
- Topological protection from noise
- Quantum error correction via topology
```

### Module Architecture

```
crates/prime-radiant/src/topology/
├── mod.rs                    # Module root
├── simplicial/
│   ├── mod.rs
│   ├── simplex.rs            # Simplices (0, 1, 2, ...)
│   ├── complex.rs            # Simplicial complex
│   ├── filtration.rs         # Filtered complex
│   └── boundary.rs           # Boundary operator
├── homology/
│   ├── mod.rs
│   ├── chain.rs              # Chain groups
│   ├── cycle.rs              # Cycle and boundary groups
│   ├── betti.rs              # Betti number computation
│   └── persistent.rs         # Persistent homology
├── tda/
│   ├── mod.rs                # Topological Data Analysis
│   ├── rips.rs               # Vietoris-Rips complex
│   ├── alpha.rs              # Alpha complex
│   ├── mapper.rs             # Mapper algorithm
│   └── persistence_diagram.rs # Persistence diagrams/barcodes
├── quantum/
│   ├── mod.rs
│   ├── toric_code.rs         # Toric code encoding
│   ├── surface_code.rs       # Surface code
│   ├── anyon.rs              # Anyonic systems
│   ├── braiding.rs           # Braiding operations
│   └── topological_qec.rs    # Topological QEC
├── invariants.rs             # Spectral topology invariants
├── encoding.rs               # Topology -> coherence encoding
└── config.rs
```

### Key Data Structures

```rust
/// k-simplex (vertex set)
pub struct Simplex<const K: usize> {
    /// Vertices (sorted)
    vertices: [VertexId; K + 1],
    /// Optional weight/filtration value
    filtration_value: Option<f64>,
}

/// Simplicial complex
pub struct SimplicialComplex {
    /// Simplices by dimension
    simplices: Vec<HashSet<SimplexId>>,
    /// Maximum dimension
    max_dim: usize,
    /// Filtration (if filtered)
    filtration: Option<Filtration>,
}

/// Persistent homology result
pub struct PersistentHomology {
    /// Persistence diagram
    diagram: PersistenceDiagram,
    /// Betti numbers at each filtration level
    betti_curve: Vec<Vec<usize>>,
    /// Persistent Betti numbers
    persistent_betti: Vec<usize>,
    /// Topological features (birth, death pairs)
    features: Vec<TopologicalFeature>,
}

/// Persistence diagram
pub struct PersistenceDiagram {
    /// (birth, death) pairs for each dimension
    pairs: Vec<Vec<(f64, f64)>>,  // pairs[dim] = [(birth, death), ...]
    /// Essential features (never die)
    essential: Vec<Vec<f64>>,     // essential[dim] = [birth, ...]
}

/// Toric code state
pub struct ToricCodeState {
    /// Lattice dimensions
    dimensions: (usize, usize),
    /// Qubit states on edges
    edge_qubits: HashMap<EdgeId, QubitState>,
    /// Syndrome measurements
    syndromes: SyndromeMeasurements,
    /// Logical qubit state
    logical_state: LogicalQubitState,
}

/// Anyonic coherence lock
pub struct AnyonLock {
    /// Anyons in the system
    anyons: Vec<Anyon>,
    /// Braiding history
    braiding_history: Vec<BraidingOperation>,
    /// Topological charge
    total_charge: TopologicalCharge,
    /// Lock strength (from braiding complexity)
    lock_strength: f64,
}
```

### Key Traits

```rust
/// Simplicial complex operations
pub trait SimplicialOps {
    /// Compute boundary operator
    fn boundary(&self, simplex: &Simplex) -> Chain;

    /// Compute homology groups
    fn homology(&self, dim: usize) -> HomologyGroup;

    /// Compute Betti numbers
    fn betti_numbers(&self) -> Vec<usize>;

    /// Build from graph
    fn from_graph(graph: &SheafGraph, dim: usize) -> Self;
}

/// Persistent homology
pub trait PersistentHomologyOps {
    /// Compute persistent homology
    fn compute(&self, filtration: &Filtration) -> PersistentHomology;

    /// Bottleneck distance between diagrams
    fn bottleneck_distance(&self, other: &PersistenceDiagram) -> f64;

    /// Wasserstein distance
    fn wasserstein_distance(&self, other: &PersistenceDiagram, p: f64) -> f64;

    /// Persistent Betti numbers
    fn persistent_betti(&self, birth: f64, death: f64) -> Vec<usize>;
}

/// Quantum topology operations
pub trait QuantumTopology {
    /// Encode coherence in topological code
    fn encode(&self, coherence: &CoherenceEnergy) -> ToricCodeState;

    /// Decode from topological state
    fn decode(&self, state: &ToricCodeState) -> CoherenceEnergy;

    /// Detect and correct errors
    fn error_correct(&self, state: &mut ToricCodeState) -> CorrectionResult;

    /// Compute topological protection factor
    fn protection_factor(&self, state: &ToricCodeState) -> f64;
}

/// Anyonic locks for coherence
pub trait AnyonicLock {
    /// Create lock from coherence state
    fn create_lock(&self, coherence: &CoherenceEnergy) -> AnyonLock;

    /// Verify lock integrity
    fn verify_lock(&self, lock: &AnyonLock) -> bool;

    /// Strengthen lock via braiding
    fn strengthen(&mut self, lock: &mut AnyonLock, operations: &[BraidingOperation]);
}
```

### Integration Points

| Existing Module | Integration Type | Description |
|-----------------|-----------------|-------------|
| `substrate::SheafGraph` | Build | Graph -> simplicial complex |
| `coherence::EnergyHistory` | Filtration | Energy levels as filtration |
| `spectral::SpectralAnalysis` | Combine | Spectral + topological invariants |
| `distributed::DistributedCoherence` | Encode | Topological encoding for distribution |

### WASM Export Strategy

```rust
#[wasm_bindgen]
pub struct WasmTopology {
    inner: TopologyEngine,
}

#[wasm_bindgen]
impl WasmTopology {
    pub fn betti_numbers(&self, graph: &WasmSheafGraph) -> JsValue;
    pub fn persistent_homology(&self, graph: &WasmSheafGraph, max_dim: usize) -> JsValue;
    pub fn persistence_diagram(&self, graph: &WasmSheafGraph) -> JsValue;
}

#[wasm_bindgen]
pub struct WasmQuantumTopology {
    inner: QuantumTopologyEngine,
}

#[wasm_bindgen]
impl WasmQuantumTopology {
    pub fn encode_coherence(&self, energy: f32) -> JsValue;
    pub fn topological_protection(&self) -> f64;
}
```

### Test Cases

1. **Unit Tests**
   - `test_boundary_squares_to_zero`: d^2 = 0
   - `test_euler_characteristic`: sum (-1)^k * beta_k = chi
   - `test_toric_code_detects_errors`
   - `test_braiding_preserves_charge`

2. **Property Tests**
   - `prop_betti_numbers_stable_under_homotopy`
   - `prop_persistence_diagram_valid`
   - `prop_topological_protection_positive`

### Benchmarks

| Benchmark | Target | Notes |
|-----------|--------|-------|
| `Betti numbers (1K nodes)` | <50ms | Use sparse matrix |
| `Persistent homology (1K nodes)` | <200ms | |
| `Toric code encode` | <10ms | |
| `Error correction` | <5ms | |

### ADR Outline

**ADR-025: Quantum/Algebraic Topology**
- Status: Proposed
- Context: Need robust topological invariants and noise protection
- Decision: Implement TDA + quantum topology
- Consequences: Topologically protected coherence, significant compute

---

## Implementation Order and Dependencies

### Dependency Graph

```
                    ┌─────────────────────────────────────┐
                    │                                     │
                    ▼                                     │
┌──────────────┐   ┌──────────────┐   ┌──────────────┐   │
│ 4. Spectral  │◄──│ 1. Sheaf     │──►│ 6. Quantum/  │───┘
│  Invariants  │   │  Cohomology  │   │   Topology   │
└──────┬───────┘   └──────┬───────┘   └──────────────┘
       │                  │                    ▲
       │                  │                    │
       │                  ▼                    │
       │           ┌──────────────┐           │
       └──────────►│ 2. Category/ │───────────┘
                   │    Topos     │
                   └──────┬───────┘
                          │
                          ▼
                   ┌──────────────┐
                   │ 3. Homotopy  │
                   │    Type      │
                   │    Theory    │
                   └──────────────┘
                          ▲
                          │
                   ┌──────────────┐
                   │ 5. Causal    │
                   │  Abstraction │
                   └──────────────┘
```

### Implementation Phases

#### Phase 1: Foundation (Weeks 1-3)
1. **Spectral Invariants (Advanced)** - Extends existing `spectral.rs`
   - Cheeger bounds
   - λ_2 cut prediction
   - Collapse detection

2. **Sheaf Cohomology** - New module
   - Cochain complexes
   - Coboundary operator
   - H^0, H^1 computation

#### Phase 2: Core Theory (Weeks 4-6)
3. **Category Theory/Topos** - New module
   - Category primitives
   - Functor implementations
   - Topos basics

4. **Quantum/Algebraic Topology** - New module
   - Simplicial complex
   - Persistent homology
   - TDA core

#### Phase 3: Advanced Theory (Weeks 7-9)
5. **Homotopy Type Theory** - New module
   - Type system
   - Path types
   - Type checker

6. **Causal Abstraction** - New module
   - SCM implementation
   - Abstraction layers
   - Enforcement

#### Phase 4: Integration (Weeks 10-12)
- Cross-module integration
- WASM exports
- Comprehensive benchmarks
- Documentation and ADRs

### Milestones

| Milestone | Week | Deliverables |
|-----------|------|--------------|
| M1: Spectral + Cohomology Core | 3 | Cheeger, H^1, tests |
| M2: Category + Topology Core | 6 | Topos, TDA, tests |
| M3: HoTT + Causal Core | 9 | Type checker, SCM, tests |
| M4: Full Integration | 12 | WASM, benches, ADRs |

### Feature Flags

Add to `Cargo.toml`:

```toml
[features]
# New feature flags
cohomology = ["nalgebra"]
category = []
hott = []
spectral-advanced = ["nalgebra", "spectral"]
causal = ["petgraph"]
quantum-topology = ["nalgebra"]

# Combined features
advanced-math = [
    "cohomology",
    "category",
    "hott",
    "spectral-advanced",
    "causal",
    "quantum-topology"
]
```

---

## Success Metrics

### Per-Framework Metrics

| Framework | Key Metric | Target |
|-----------|-----------|--------|
| Sheaf Cohomology | H^1 detects obstructions | >95% accuracy |
| Category Theory | Functor composition correct | 100% |
| HoTT | Proof verification | 100% sound |
| Spectral Advanced | Cut prediction | >80% accuracy |
| Causal Abstraction | Abstraction consistency | 100% |
| Quantum Topology | Error correction | >99% |

### Overall Metrics

| Metric | Target |
|--------|--------|
| Test coverage | >85% |
| Benchmark regressions | 0 |
| WASM bundle size increase | <500KB |
| Documentation coverage | 100% public API |

---

## Risk Assessment

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| HoTT complexity too high | Medium | High | Start with core subset, iterative expansion |
| Performance degradation | Medium | Medium | Lazy evaluation, feature flags |
| WASM size bloat | Low | Medium | Tree shaking, separate WASM crates |
| Integration conflicts | Low | High | Comprehensive integration tests |

### Mathematical Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| Incorrect cohomology | Low | High | Property tests, reference implementation comparison |
| Unsound type checker | Low | Critical | Formal verification of core rules |
| Wrong spectral bounds | Low | Medium | Compare with known graph families |

---

## References

1. Hansen, J., & Ghrist, R. (2019). "Toward a spectral theory of cellular sheaves."
2. Bodnar, C., et al. (2022). "Sheaf Neural Networks."
3. Univalent Foundations Program. (2013). "Homotopy Type Theory."
4. Cheeger, J. (1970). "A lower bound for the smallest eigenvalue of the Laplacian."
5. Pearl, J. (2009). "Causality: Models, Reasoning, and Inference."
6. Kitaev, A. (2003). "Fault-tolerant quantum computation by anyons."
7. Carlsson, G. (2009). "Topology and data."

---

## Appendix A: File Creation Summary

### New Files to Create

```
crates/prime-radiant/src/
├── cohomology/
│   ├── mod.rs
│   ├── cochain.rs
│   ├── coboundary.rs
│   ├── cohomology_group.rs
│   ├── obstruction.rs
│   ├── sheaf_diffusion.rs
│   ├── neural_sheaf.rs
│   └── config.rs
├── category/
│   ├── mod.rs
│   ├── category.rs
│   ├── functor.rs
│   ├── natural_transform.rs
│   ├── monad.rs
│   ├── topos/
│   │   ├── mod.rs
│   │   ├── subobject.rs
│   │   ├── internal_logic.rs
│   │   └── sheaf_topos.rs
│   ├── retrieval.rs
│   ├── coherence_laws.rs
│   └── config.rs
├── hott/
│   ├── mod.rs
│   ├── types/
│   │   ├── mod.rs
│   │   ├── universe.rs
│   │   ├── identity.rs
│   │   ├── sigma.rs
│   │   ├── pi.rs
│   │   └── higher_inductive.rs
│   ├── paths/
│   │   ├── mod.rs
│   │   ├── path.rs
│   │   ├── composition.rs
│   │   ├── inverse.rs
│   │   └── homotopy.rs
│   ├── univalence/
│   │   ├── mod.rs
│   │   ├── equivalence.rs
│   │   ├── transport.rs
│   │   └── ua.rs
│   ├── proofs/
│   │   ├── mod.rs
│   │   ├── proof_term.rs
│   │   ├── type_checker.rs
│   │   ├── normalization.rs
│   │   └── coherence_proof.rs
│   ├── embedding.rs
│   └── config.rs
├── spectral/                   # New advanced module
│   ├── mod.rs
│   ├── cheeger.rs
│   ├── eigenvalue/
│   │   ├── mod.rs
│   │   ├── second.rs
│   │   ├── higher.rs
│   │   ├── gap.rs
│   │   └── collapse.rs
│   ├── cut_prediction.rs
│   ├── stability.rs
│   ├── laplacian/
│   │   ├── mod.rs
│   │   ├── normalized.rs
│   │   ├── sheaf_laplacian.rs
│   │   └── sparse.rs
│   └── config.rs
├── causal/
│   ├── mod.rs
│   ├── scm/
│   │   ├── mod.rs
│   │   ├── variable.rs
│   │   ├── mechanism.rs
│   │   └── intervention.rs
│   ├── dag/
│   │   ├── mod.rs
│   │   ├── builder.rs
│   │   ├── validity.rs
│   │   └── paths.rs
│   ├── abstraction/
│   │   ├── mod.rs
│   │   ├── layer.rs
│   │   ├── mapping.rs
│   │   ├── consistency.rs
│   │   └── constructive.rs
│   ├── enforcement.rs
│   ├── propagation.rs
│   └── config.rs
├── topology/
│   ├── mod.rs
│   ├── simplicial/
│   │   ├── mod.rs
│   │   ├── simplex.rs
│   │   ├── complex.rs
│   │   ├── filtration.rs
│   │   └── boundary.rs
│   ├── homology/
│   │   ├── mod.rs
│   │   ├── chain.rs
│   │   ├── cycle.rs
│   │   ├── betti.rs
│   │   └── persistent.rs
│   ├── tda/
│   │   ├── mod.rs
│   │   ├── rips.rs
│   │   ├── alpha.rs
│   │   ├── mapper.rs
│   │   └── persistence_diagram.rs
│   ├── quantum/
│   │   ├── mod.rs
│   │   ├── toric_code.rs
│   │   ├── surface_code.rs
│   │   ├── anyon.rs
│   │   ├── braiding.rs
│   │   └── topological_qec.rs
│   ├── invariants.rs
│   ├── encoding.rs
│   └── config.rs
└── docs/
    ├── ADR-020-sheaf-cohomology.md
    ├── ADR-021-category-theory.md
    ├── ADR-022-homotopy-type-theory.md
    ├── ADR-023-spectral-invariants.md
    ├── ADR-024-causal-abstraction.md
    └── ADR-025-quantum-topology.md
```

### New Test Files

```
crates/prime-radiant/tests/
├── cohomology_tests.rs
├── category_tests.rs
├── hott_tests.rs
├── spectral_advanced_tests.rs
├── causal_tests.rs
└── topology_tests.rs
```

### New Benchmark Files

```
crates/prime-radiant/benches/
├── cohomology_bench.rs
├── category_bench.rs
├── hott_bench.rs
├── spectral_advanced_bench.rs
├── causal_bench.rs
└── topology_bench.rs
```

---

## Appendix B: Cargo.toml Additions

```toml
# Add to [dependencies]
# For cohomology and advanced spectral
nalgebra-sparse = { version = "0.10", optional = true }

# For TDA (optional external crate integration)
# Note: Consider implementing from scratch for WASM compatibility

# Add to [features]
cohomology = ["nalgebra", "nalgebra-sparse"]
category = []
hott = []
spectral-advanced = ["nalgebra", "nalgebra-sparse", "spectral"]
causal = ["petgraph"]
quantum-topology = ["nalgebra"]

advanced-math = [
    "cohomology",
    "category",
    "hott",
    "spectral-advanced",
    "causal",
    "quantum-topology"
]

# Update full feature
full = [
    # ... existing features ...
    "advanced-math",
]
```

---

*End of GOAP Implementation Plan*
