# Prime-Radiant Domain Model

## Overview

Prime-Radiant is a mathematical framework for AI interpretability, built on rigorous foundations from algebraic topology, category theory, and quantum mechanics. This document describes the domain model using Domain-Driven Design (DDD) principles.

---

## Bounded Contexts

### 1. Cohomology Context

**Purpose**: Analyze topological structure of representations and detect coherence failures.

#### Aggregates

**Sheaf** (Aggregate Root)
- Contains: Presheaf, Sections, RestrictionMaps
- Invariants: Gluing axioms, locality conditions
- Behavior: Compute cohomology, detect obstructions

**ChainComplex**
- Contains: ChainGroups, BoundaryMaps
- Invariants: d^2 = 0 (boundary of boundary is zero)
- Behavior: Compute homology groups

#### Value Objects

- `Section`: Data over an open set
- `RestrictionMap`: Linear map between stalks
- `BettiNumbers`: Topological invariants
- `PersistenceDiagram`: Multi-scale topology

#### Domain Events

- `CoherenceViolationDetected`: When H^1 is non-trivial
- `TopologyChanged`: When underlying graph structure changes
- `SectionUpdated`: When local data is modified

---

### 2. Category Context

**Purpose**: Model compositional structure and preserve mathematical properties.

#### Aggregates

**Category** (Aggregate Root)
- Contains: Objects, Morphisms
- Invariants: Identity, associativity
- Behavior: Compose morphisms, verify laws

**Topos** (Aggregate Root)
- Contains: Category, SubobjectClassifier, Products, Exponentials
- Invariants: Finite limits, exponentials exist
- Behavior: Internal logic, subobject classification

#### Entities

- `Object`: An element of the category
- `Morphism`: A transformation between objects
- `Functor`: Structure-preserving map between categories
- `NaturalTransformation`: Morphism between functors

#### Value Objects

- `MorphismId`: Unique identifier
- `ObjectId`: Unique identifier
- `CompositionResult`: Result of morphism composition

#### Domain Events

- `MorphismAdded`: New morphism in category
- `FunctorApplied`: Functor maps between categories
- `CoherenceVerified`: Axioms confirmed

---

### 3. HoTT Context (Homotopy Type Theory)

**Purpose**: Provide type-theoretic foundations for proofs and equivalences.

#### Aggregates

**TypeUniverse** (Aggregate Root)
- Contains: Types, Terms, Judgments
- Invariants: Type formation rules
- Behavior: Type checking, univalence

**Path** (Entity)
- Properties: Start, End, Homotopy
- Invariants: Endpoints match types
- Behavior: Concatenation, inversion, transport

#### Value Objects

- `Type`: A type in the universe
- `Term`: An element of a type
- `Equivalence`: Bidirectional map with proofs
- `IdentityType`: The type of paths between terms

#### Domain Services

- `PathInduction`: J-eliminator for paths
- `Transport`: Move values along paths
- `Univalence`: Equivalence = Identity

---

### 4. Spectral Context

**Purpose**: Analyze eigenvalue structure and spectral invariants.

#### Aggregates

**SpectralDecomposition** (Aggregate Root)
- Contains: Eigenvalues, Eigenvectors
- Invariants: Orthogonality, completeness
- Behavior: Compute spectrum, effective dimension

#### Value Objects

- `Eigenspace`: Subspace for eigenvalue
- `SpectralGap`: Distance between eigenvalues
- `SpectralFingerprint`: Comparison signature
- `ConditionNumber`: Numerical stability measure

#### Domain Services

- `LanczosIteration`: Efficient eigenvalue computation
- `CheegerAnalysis`: Spectral gap and graph cuts

---

### 5. Causal Context

**Purpose**: Implement causal abstraction for mechanistic interpretability.

#### Aggregates

**CausalModel** (Aggregate Root)
- Contains: Variables, Edges, StructuralEquations
- Invariants: DAG structure (no cycles)
- Behavior: Intervention, counterfactual reasoning

**CausalAbstraction** (Aggregate Root)
- Contains: LowModel, HighModel, VariableMapping
- Invariants: Interventional consistency
- Behavior: Verify abstraction, compute IIA

#### Entities

- `Variable`: A node in the causal graph
- `Intervention`: An action on a variable
- `Circuit`: Minimal subnetwork for behavior

#### Value Objects

- `StructuralEquation`: Functional relationship
- `InterventionResult`: Outcome of intervention
- `AlignmentScore`: How well mechanisms match

#### Domain Events

- `InterventionApplied`: Variable was modified
- `CircuitDiscovered`: Minimal mechanism found
- `AbstractionViolation`: Models disagree under intervention

---

### 6. Quantum Context

**Purpose**: Apply quantum-inspired methods to representation analysis.

#### Aggregates

**QuantumState** (Aggregate Root)
- Contains: Amplitudes
- Invariants: Normalization
- Behavior: Measure, evolve, entangle

**DensityMatrix** (Aggregate Root)
- Contains: Matrix elements
- Invariants: Positive semi-definite, trace 1
- Behavior: Entropy, purity, partial trace

#### Value Objects

- `Entanglement`: Correlation measure
- `TopologicalInvariant`: Robust property
- `BerryPhase`: Geometric phase

#### Domain Services

- `EntanglementAnalysis`: Compute entanglement measures
- `TDAService`: Topological data analysis

---

## Cross-Cutting Concerns

### Error Handling

All contexts use a unified error type hierarchy:

```rust
pub enum PrimeRadiantError {
    Cohomology(CohomologyError),
    Category(CategoryError),
    HoTT(HoTTError),
    Spectral(SpectralError),
    Causal(CausalError),
    Quantum(QuantumError),
}
```

### Numerical Precision

- Default epsilon: 1e-10
- Configurable per computation
- Automatic condition number checking

### Serialization

All value objects and aggregates implement:
- `serde::Serialize` and `serde::Deserialize`
- Custom formats for mathematical objects

---

## Context Map

```
┌─────────────────────────────────────────────────────────────────┐
│                     Prime-Radiant Core                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐       │
│  │ Cohomology  │────▶│  Category   │────▶│    HoTT     │       │
│  │   Context   │     │   Context   │     │   Context   │       │
│  └─────────────┘     └─────────────┘     └─────────────┘       │
│         │                   │                   │               │
│         │                   │                   │               │
│         ▼                   ▼                   ▼               │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐       │
│  │  Spectral   │────▶│   Causal    │────▶│   Quantum   │       │
│  │   Context   │     │   Context   │     │   Context   │       │
│  └─────────────┘     └─────────────┘     └─────────────┘       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘

Relationships:
─────────────
Cohomology ──[U]──▶ Category  : Sheaves are presheaves + gluing (Upstream/Downstream)
Category   ──[U]──▶ HoTT      : Categories model type theory
Spectral   ──[S]──▶ Cohomology: Laplacian eigenvalues for cohomology (Shared Kernel)
Causal     ──[C]──▶ Category  : Causal abstraction as functors (Conformist)
Quantum    ──[P]──▶ Category  : Quantum channels as morphisms (Partnership)
```

---

## Ubiquitous Language

| Term | Definition |
|------|------------|
| **Sheaf** | Assignment of data to open sets satisfying gluing axioms |
| **Cohomology** | Measure of obstruction to extending local sections globally |
| **Morphism** | Structure-preserving map between objects |
| **Functor** | Structure-preserving map between categories |
| **Path** | Continuous map from interval, proof of equality in HoTT |
| **Equivalence** | Bidirectional map with inverse proofs |
| **Spectral Gap** | Difference between consecutive eigenvalues |
| **Intervention** | Fixing a variable to a value (do-operator) |
| **Entanglement** | Non-local correlation in quantum states |
| **Betti Number** | Dimension of homology group |

---

## Implementation Guidelines

### Aggregate Design

1. Keep aggregates small and focused
2. Use value objects for immutable data
3. Enforce invariants in aggregate root
4. Emit domain events for state changes

### Repository Pattern

Each aggregate root has a repository:

```rust
pub trait SheafRepository {
    fn find_by_id(&self, id: SheafId) -> Option<Sheaf>;
    fn save(&mut self, sheaf: Sheaf) -> Result<(), Error>;
    fn find_by_topology(&self, graph: &Graph) -> Vec<Sheaf>;
}
```

### Factory Pattern

Complex aggregates use factories:

```rust
pub struct SheafFactory {
    pub fn from_neural_network(network: &NeuralNetwork) -> Sheaf;
    pub fn from_knowledge_graph(kg: &KnowledgeGraph) -> Sheaf;
}
```

### Domain Services

Cross-aggregate operations use services:

```rust
pub struct CoherenceService {
    pub fn check_global_consistency(sheaf: &Sheaf) -> CoherenceReport;
    pub fn optimize_sections(sheaf: &mut Sheaf) -> OptimizationResult;
}
```
