# ADR-002: Category Theory and Topos-Theoretic Belief Models

**Status**: Accepted
**Date**: 2024-12-15
**Authors**: RuVector Team
**Supersedes**: None

---

## Context

While sheaf cohomology (ADR-001) provides the foundation for coherence measurement, we need higher-level abstractions for:

1. **Functorial Retrieval**: Structure-preserving access to knowledge across different representations
2. **Belief Dynamics**: Modeling how beliefs change under new evidence
3. **Higher Coherence Laws**: Ensuring consistency not just of facts, but of relationships between facts
4. **Intuitionistic Logic**: Handling partial or uncertain knowledge appropriately

Category theory provides the language for these abstractions, and topos theory extends this to handle logic and set-like constructions in coherent ways.

### Why Category Theory?

Category theory is the mathematics of structure and structure-preserving maps. It provides:

1. **Functors**: Maps between categories that preserve structure
2. **Natural Transformations**: Maps between functors that preserve relationships
3. **Limits and Colimits**: Universal constructions for combining and decomposing data
4. **Adjunctions**: Fundamental optimization principles

### Why Topos Theory?

A topos is a category that behaves like the category of sets but with a different internal logic. Topoi enable:

1. **Intuitionistic Logic**: Handle "not provably true" vs "provably false"
2. **Subobject Classifiers**: Generalized truth values beyond {true, false}
3. **Internal Languages**: Reason about objects using logical syntax
4. **Sheaf Semantics**: Interpret sheaves as generalized sets

---

## Decision

We implement a **functorial retrieval system** with topos-theoretic belief models for coherence management.

### Mathematical Foundation

#### Definition: Category of Knowledge Graphs

Let **KGraph** be the category where:
- Objects are knowledge graphs G = (V, E, F) with sheaf structure F
- Morphisms are graph homomorphisms that preserve sheaf structure:
  ```
  phi: G -> G' such that phi_*(F) -> F'
  ```

#### Definition: Retrieval Functor

A **retrieval functor** R: Query -> KGraph assigns:
- To each query q, a subgraph R(q) of the knowledge base
- To each query refinement q -> q', a graph inclusion R(q) -> R(q')

Functoriality ensures that refining a query gives a consistent subgraph.

#### Definition: Belief Topos

The **belief topos** B(G) over a knowledge graph G is the category:
- Objects: Belief states (assignments of credences to nodes/edges)
- Morphisms: Belief updates under new evidence
- Subobject classifier: Omega = [0, 1] (credence values)

The internal logic is intuitionistic: for a proposition P,
- "P is true" means credence(P) = 1
- "P is false" means credence(P) = 0
- Otherwise, P has partial truth value

### Implementation Architecture

#### Functorial Retrieval

```rust
/// A category of knowledge representations
pub trait Category {
    type Object;
    type Morphism;

    fn identity(obj: &Self::Object) -> Self::Morphism;
    fn compose(f: &Self::Morphism, g: &Self::Morphism) -> Self::Morphism;
}

/// A functor between categories
pub trait Functor<C: Category, D: Category> {
    fn map_object(&self, obj: &C::Object) -> D::Object;
    fn map_morphism(&self, mor: &C::Morphism) -> D::Morphism;

    // Functoriality laws (ensured by implementation)
    // F(id_A) = id_{F(A)}
    // F(g . f) = F(g) . F(f)
}

/// Query category: queries with refinement morphisms
pub struct QueryCategory;

impl Category for QueryCategory {
    type Object = Query;
    type Morphism = QueryRefinement;

    fn identity(q: &Query) -> QueryRefinement {
        QueryRefinement::identity(q.clone())
    }

    fn compose(f: &QueryRefinement, g: &QueryRefinement) -> QueryRefinement {
        QueryRefinement::compose(f, g)
    }
}

/// Retrieval functor from queries to knowledge subgraphs
pub struct RetrievalFunctor {
    knowledge_base: Arc<SheafGraph>,
    index: VectorIndex,
}

impl Functor<QueryCategory, KGraphCategory> for RetrievalFunctor {
    fn map_object(&self, query: &Query) -> SheafSubgraph {
        // Retrieve relevant subgraph for query
        let node_ids = self.index.search(&query.embedding, query.k);
        self.knowledge_base.extract_subgraph(&node_ids, query.hops)
    }

    fn map_morphism(&self, refinement: &QueryRefinement) -> SubgraphInclusion {
        // Refinement yields inclusion of subgraphs
        let source = self.map_object(&refinement.source);
        let target = self.map_object(&refinement.target);
        SubgraphInclusion::compute(&source, &target)
    }
}
```

#### Natural Transformations

```rust
/// A natural transformation between functors
pub trait NaturalTransformation<C, D, F, G>
where
    C: Category,
    D: Category,
    F: Functor<C, D>,
    G: Functor<C, D>,
{
    /// Component at object A: eta_A: F(A) -> G(A)
    fn component(&self, obj: &C::Object) -> D::Morphism;

    // Naturality: for f: A -> B,
    // G(f) . eta_A = eta_B . F(f)
}

/// Coherence preservation transformation
pub struct CoherencePreservation {
    source_functor: RetrievalFunctor,
    target_functor: CoherenceAwareFunctor,
}

impl NaturalTransformation<QueryCategory, KGraphCategory,
                           RetrievalFunctor, CoherenceAwareFunctor>
for CoherencePreservation {
    fn component(&self, query: &Query) -> SubgraphMap {
        // Transform retrieval into coherence-filtered retrieval
        let raw_subgraph = self.source_functor.map_object(query);
        let filtered = self.filter_incoherent_edges(&raw_subgraph);
        SubgraphMap::new(raw_subgraph, filtered)
    }
}
```

#### Topos-Theoretic Belief Model

```rust
/// A topos of belief states over a knowledge graph
pub struct BeliefTopos {
    graph: Arc<SheafGraph>,
    /// Credence assignments: node/edge -> [0, 1]
    credences: HashMap<EntityId, f32>,
    /// Update history for rollback
    history: Vec<BeliefUpdate>,
}

/// The subobject classifier Omega
pub struct TruthValue(f32);

impl TruthValue {
    pub const TRUE: TruthValue = TruthValue(1.0);
    pub const FALSE: TruthValue = TruthValue(0.0);
    pub const UNKNOWN: TruthValue = TruthValue(0.5);

    /// Intuitionistic negation: not(p) = p -> FALSE
    pub fn not(&self) -> TruthValue {
        if self.0 == 0.0 {
            TruthValue::TRUE
        } else {
            TruthValue::FALSE
        }
    }

    /// Intuitionistic conjunction
    pub fn and(&self, other: &TruthValue) -> TruthValue {
        TruthValue(self.0.min(other.0))
    }

    /// Intuitionistic disjunction
    pub fn or(&self, other: &TruthValue) -> TruthValue {
        TruthValue(self.0.max(other.0))
    }

    /// Intuitionistic implication
    pub fn implies(&self, other: &TruthValue) -> TruthValue {
        if self.0 <= other.0 {
            TruthValue::TRUE
        } else {
            other.clone()
        }
    }
}

impl BeliefTopos {
    /// Bayesian update under new evidence
    pub fn update(&mut self, evidence: Evidence) -> BeliefUpdate {
        let prior = self.credence(evidence.entity);

        // Compute likelihood based on coherence
        let likelihood = self.compute_likelihood(&evidence);

        // Bayesian update (simplified)
        let posterior = (prior * likelihood) /
            (prior * likelihood + (1.0 - prior) * (1.0 - likelihood));

        let update = BeliefUpdate {
            entity: evidence.entity,
            prior,
            posterior,
            evidence: evidence.clone(),
        };

        self.credences.insert(evidence.entity, posterior);
        self.history.push(update.clone());
        update
    }

    /// Compute likelihood based on coherence with existing beliefs
    fn compute_likelihood(&self, evidence: &Evidence) -> f32 {
        // High coherence with existing beliefs -> high likelihood
        let subgraph = self.graph.neighborhood(evidence.entity, 2);
        let energy = subgraph.compute_energy();

        // Convert energy to probability (lower energy = higher likelihood)
        (-energy / self.temperature()).exp()
    }

    /// Check if proposition holds in current belief state
    pub fn holds(&self, prop: &Proposition) -> TruthValue {
        match prop {
            Proposition::Atom(entity) => {
                TruthValue(self.credence(*entity))
            }
            Proposition::And(p, q) => {
                self.holds(p).and(&self.holds(q))
            }
            Proposition::Or(p, q) => {
                self.holds(p).or(&self.holds(q))
            }
            Proposition::Implies(p, q) => {
                self.holds(p).implies(&self.holds(q))
            }
            Proposition::Not(p) => {
                self.holds(p).not()
            }
            Proposition::Coherent(region) => {
                // Region is coherent if energy below threshold
                let energy = self.graph.region_energy(region);
                if energy < COHERENCE_THRESHOLD {
                    TruthValue::TRUE
                } else if energy > INCOHERENCE_THRESHOLD {
                    TruthValue::FALSE
                } else {
                    TruthValue(1.0 - energy / INCOHERENCE_THRESHOLD)
                }
            }
        }
    }
}
```

### Higher Category Structure

For advanced applications, we model **2-morphisms** (relationships between relationships):

```rust
/// A 2-category with objects, 1-morphisms, and 2-morphisms
pub trait TwoCategory {
    type Object;
    type Morphism1;
    type Morphism2;

    fn id_1(obj: &Self::Object) -> Self::Morphism1;
    fn id_2(mor: &Self::Morphism1) -> Self::Morphism2;

    fn compose_1(f: &Self::Morphism1, g: &Self::Morphism1) -> Self::Morphism1;
    fn compose_2_vertical(
        alpha: &Self::Morphism2,
        beta: &Self::Morphism2
    ) -> Self::Morphism2;
    fn compose_2_horizontal(
        alpha: &Self::Morphism2,
        beta: &Self::Morphism2
    ) -> Self::Morphism2;
}

/// Coherence laws form 2-morphisms in the belief 2-category
pub struct CoherenceLaw {
    /// Source belief update sequence
    source: Vec<BeliefUpdate>,
    /// Target belief update sequence
    target: Vec<BeliefUpdate>,
    /// Witness that they're equivalent
    witness: CoherenceWitness,
}

impl CoherenceLaw {
    /// Associativity: (f . g) . h = f . (g . h)
    pub fn associativity(f: BeliefUpdate, g: BeliefUpdate, h: BeliefUpdate) -> Self {
        CoherenceLaw {
            source: vec![f.clone(), g.clone(), h.clone()], // Left-associated
            target: vec![f, g, h],                          // Right-associated
            witness: CoherenceWitness::Associativity,
        }
    }

    /// Unit law: id . f = f = f . id
    pub fn left_unit(f: BeliefUpdate) -> Self {
        CoherenceLaw {
            source: vec![BeliefUpdate::identity(), f.clone()],
            target: vec![f],
            witness: CoherenceWitness::LeftUnit,
        }
    }
}
```

---

## Consequences

### Positive

1. **Structure Preservation**: Functors ensure retrieval respects knowledge structure
2. **Intuitionistic Reasoning**: Handles partial/uncertain knowledge properly
3. **Compositionality**: Complex operations built from simple primitives
4. **Higher Coherence**: 2-morphisms capture meta-level consistency
5. **Belief Dynamics**: Topos semantics enable principled belief update

### Negative

1. **Abstraction Overhead**: Category theory requires learning curve
2. **Performance Cost**: Functor laws verification has runtime cost
3. **Complexity**: 2-categorical structures can be overwhelming
4. **Implementation Fidelity**: Ensuring Rust code matches category theory is subtle

### Mitigations

1. **Gradual Adoption**: Use basic functors first, add higher structures as needed
2. **Type-Level Enforcement**: Use Rust's type system to enforce laws statically
3. **Documentation**: Extensive examples linking code to mathematical concepts
4. **Testing**: Property-based tests for categorical laws

---

## Mathematical Properties

### Theorem: Yoneda Lemma

For a functor F: C -> Set and object A in C:

```
Nat(Hom(A, -), F) ≅ F(A)
```

Natural transformations from a representable functor to F are determined by elements of F(A).

**Application**: This allows us to reconstruct knowledge graph structure from query patterns.

### Theorem: Subobject Classifier in Presheaves

In the topos of presheaves Set^{C^op}:

```
Omega(c) = {sieves on c}
```

The truth values for an object c are sieves (downward-closed collections of morphisms into c).

**Application**: Partial truth values are determined by how much of the knowledge graph supports a proposition.

### Theorem: Adjoint Functors Preserve Limits

If F ⊣ G (F left adjoint to G), then:
- F preserves colimits
- G preserves limits

**Application**: Retrieval (right adjoint) preserves finite products of query results.

---

## Integration with Sheaf Cohomology

The belief topos connects to sheaf cohomology:

```rust
/// Coherence as a global section
pub fn coherent_section(&self) -> Option<GlobalSection> {
    // Check if current beliefs form a global section
    let cohomology_dim = self.graph.cohomology_dimension();

    if cohomology_dim == 0 {
        Some(self.construct_global_section())
    } else {
        None // Obstruction exists
    }
}

/// Credence from cohomology class
pub fn credence_from_cohomology(&self, node: NodeId) -> f32 {
    // Higher cohomology -> lower credence
    let local_cohomology = self.graph.local_cohomology(node);
    1.0 / (1.0 + local_cohomology as f32)
}
```

---

## Related Decisions

- [ADR-001: Sheaf Cohomology](ADR-001-sheaf-cohomology.md) - Mathematical foundation
- [ADR-003: Homotopy Type Theory](ADR-003-homotopy-type-theory.md) - Higher categories and paths
- [ADR-005: Causal Abstraction](ADR-005-causal-abstraction.md) - Causal categories

---

## References

1. Mac Lane, S. (1978). "Categories for the Working Mathematician." Springer.

2. Lawvere, F.W. & Schanuel, S. (2009). "Conceptual Mathematics." Cambridge University Press.

3. Goldblatt, R. (1984). "Topoi: The Categorical Analysis of Logic." North-Holland.

4. Awodey, S. (2010). "Category Theory." Oxford University Press.

5. Johnstone, P.T. (2002). "Sketches of an Elephant: A Topos Theory Compendium." Oxford University Press.

6. Spivak, D.I. (2014). "Category Theory for the Sciences." MIT Press.

---

## Appendix: Category Theory Primer

### Objects and Morphisms

A category C consists of:
- A collection ob(C) of **objects**
- For each pair of objects A, B, a collection Hom(A, B) of **morphisms**
- For each object A, an **identity morphism** id_A: A -> A
- **Composition**: For f: A -> B and g: B -> C, g . f: A -> C

Subject to:
- Associativity: (h . g) . f = h . (g . f)
- Identity: f . id_A = f = id_B . f

### Functors

A functor F: C -> D consists of:
- An object map: A |-> F(A)
- A morphism map: f |-> F(f)

Subject to:
- F(id_A) = id_{F(A)}
- F(g . f) = F(g) . F(f)

### Natural Transformations

A natural transformation eta: F => G between functors F, G: C -> D consists of:
- For each object A in C, a morphism eta_A: F(A) -> G(A)

Subject to naturality: For f: A -> B,
- G(f) . eta_A = eta_B . F(f)
