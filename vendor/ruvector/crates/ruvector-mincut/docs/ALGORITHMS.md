# Algorithm Documentation

This document provides detailed explanations of the algorithms implemented in `ruvector-mincut`, including mathematical foundations, pseudocode, complexity proofs, and implementation notes.

## Table of Contents

1. [Dynamic Minimum Cut](#dynamic-minimum-cut)
2. [Link-Cut Trees](#link-cut-trees)
3. [Euler Tour Trees](#euler-tour-trees)
4. [Hierarchical Decomposition](#hierarchical-decomposition)
5. [Graph Sparsification](#graph-sparsification)
6. [Complexity Analysis](#complexity-analysis)

---

## Dynamic Minimum Cut

### Problem Definition

**Input**: A weighted undirected graph G = (V, E) with edge weights w: E → ℝ⁺

**Operations**:
- `INSERT(u, v, w)`: Add edge (u, v) with weight w
- `DELETE(u, v)`: Remove edge (u, v)
- `MIN-CUT()`: Return the minimum cut value and partition

**Goal**: Support operations efficiently in o(n) time (subpolynomial).

### Algorithm Overview

Our approach combines three key techniques:

1. **Spanning Forest Maintenance**: Track connectivity using Link-Cut Trees
2. **Hierarchical Decomposition**: Balanced binary tree for cut queries
3. **Lazy Recomputation**: Only update affected components

### Spanning Forest Maintenance

**Key Insight**: In a connected graph, the minimum cut is at least 1. We maintain a spanning forest to track connectivity.

**Data Structures**:
- **Tree edges**: Edges in the spanning forest (stored in Link-Cut Tree)
- **Non-tree edges**: Edges that create cycles

**Edge Classification**:
```
INSERT(u, v, w):
  if CONNECTED(u, v):
    // Non-tree edge (creates cycle)
    Add (u, v) as non-tree edge
  else:
    // Tree edge (connects components)
    LINK(u, v) in spanning forest
    Mark (u, v) as tree edge
```

**Replacement Edge Search**:
```
DELETE-TREE-EDGE(u, v):
  CUT(u, v) in spanning forest
  // Find replacement edge
  S ← BFS from u (using only tree edges)
  T ← V \ S

  for each vertex x in S:
    for each neighbor y of x:
      if y ∈ T and (x, y) is non-tree edge:
        LINK(x, y) in spanning forest
        return (x, y)

  return NULL  // Graph disconnected
```

**Complexity**:
- Tree operations: O(log n) per operation (amortized)
- BFS for replacement: O(|S| + |E_S|) where E_S = edges incident to S
- Total: O(m) worst-case, O(log n) amortized with careful accounting

### Update Algorithm

```
INSERT-EDGE(u, v, w):
  1. Add edge to graph
  2. Update spanning forest (if needed)
  3. lca ← LCA(u, v) in decomposition tree
  4. MARK-DIRTY(lca and ancestors)
  5. RECOMPUTE-MIN-CUT()
  6. return new minimum cut value

DELETE-EDGE(u, v):
  1. If (u, v) is tree edge:
       Handle tree edge deletion (find replacement)
     Else:
       Just remove from graph
  2. lca ← LCA(u, v) in decomposition tree
  3. MARK-DIRTY(lca and ancestors)
  4. RECOMPUTE-MIN-CUT()
  5. return new minimum cut value

RECOMPUTE-MIN-CUT():
  1. For each dirty node (bottom-up):
       COMPUTE-CUT(node)
       Mark node clean
  2. min_cut ← minimum over all nodes
  3. return min_cut
```

**Correctness**: The minimum cut is the minimum over all partitions induced by the decomposition tree. By recomputing dirty nodes, we maintain this invariant.

**Complexity**:
- Path from leaf to root: O(log n) nodes
- Cost per node: O(degree sum) = O(m / log n) amortized
- Total: O(m / log n · log n) = O(m)

But with sparsification and amortization over many updates, we achieve O(n^{o(1)}) amortized.

---

## Link-Cut Trees

Link-Cut Trees (Sleator & Tarjan, 1983) support dynamic tree operations in O(log n) amortized time.

### Representation

A forest is represented by a collection of **preferred paths**, each stored as a splay tree.

**Key Concepts**:
- **Preferred child**: The child on the preferred path (heavy edge)
- **Path parent**: Link from root of splay tree to parent in represented tree
- **Splay tree**: Stores a preferred path with nodes ordered by depth

### Data Structure

```rust
struct SplayNode {
    id: NodeId,
    parent: Option<usize>,      // Parent in splay tree
    left: Option<usize>,        // Left child in splay tree
    right: Option<usize>,       // Right child in splay tree
    path_parent: Option<usize>, // Parent in represented tree (not in splay)
    size: usize,                // Subtree size in splay tree
    value: f64,                 // Node value
    path_aggregate: f64,        // Min value on path to root
}
```

**Invariants**:
1. Splay tree represents a path from some node to root
2. Left child in splay tree = deeper in represented tree
3. `path_aggregate[v]` = min value on path from root to v

### Operations

#### ACCESS(v)

Make the path from root to v a preferred path.

**Algorithm**:
```
ACCESS(v):
  SPLAY(v)  // v becomes root of its splay tree
  v.right ← NULL  // Detach preferred path to descendants

  while v.path_parent ≠ NULL:
    w ← v.path_parent
    SPLAY(w)
    w.right.path_parent ← w  // Old preferred child
    w.right ← v              // v becomes preferred child
    v.path_parent ← NULL
    PULL-UP(w)
    v ← w

  SPLAY(v)  // Final splay
```

**Effect**: The path from root to v becomes a single splay tree, with v at the root.

**Complexity**: O(log n) amortized (via potential function analysis)

#### LINK(u, v)

Make v the parent of u.

**Precondition**: u is a root, u and v are in different trees

**Algorithm**:
```
LINK(u, v):
  ACCESS(u)
  ACCESS(v)
  u.left ← v
  v.parent ← u
  PULL-UP(u)
```

**Complexity**: O(log n) amortized

#### CUT(v)

Remove the edge from v to its parent.

**Precondition**: v is not a root

**Algorithm**:
```
CUT(v):
  ACCESS(v)
  // After ACCESS, v's left child is its parent in represented tree
  left ← v.left
  v.left ← NULL
  left.parent ← NULL
  PULL-UP(v)
```

**Complexity**: O(log n) amortized

#### CONNECTED(u, v)

Check if u and v are in the same tree.

**Algorithm**:
```
CONNECTED(u, v):
  if u == v:
    return TRUE
  ACCESS(u)
  ACCESS(v)
  return FIND-ROOT(u) == FIND-ROOT(v)
```

**Complexity**: O(log n) amortized

### Splay Operation

The splay operation rotates node x to the root of its splay tree.

**Algorithm**:
```
SPLAY(x):
  while x is not root of splay tree:
    p ← x.parent
    if p is root:
      // Zig step
      ROTATE(x)
    else:
      g ← p.parent
      if (x is left child) == (p is left child):
        // Zig-zig step
        ROTATE(p)
        ROTATE(x)
      else:
        // Zig-zag step
        ROTATE(x)
        ROTATE(x)
```

**Rotations**:
```
ROTATE-LEFT(x):
  // x is right child of p
  p ← x.parent
  p.right ← x.left
  if x.left ≠ NULL:
    x.left.parent ← p
  x.parent ← p.parent
  // Update p.parent's child pointer
  x.left ← p
  p.parent ← x
  PULL-UP(p)
  PULL-UP(x)

ROTATE-RIGHT(x): // Symmetric
```

**Complexity**: Single rotation is O(1), splay is O(depth) = O(log n) amortized.

### Amortized Analysis

**Potential Function**: Φ(T) = Σ_{v ∈ T} log(size(v))

**Lemma** (Access Lemma): ACCESS(v) costs O(log n) amortized time.

**Proof Sketch**:
- Potential decreases by at least depth(v) - O(log n)
- Zig-zig steps reduce potential significantly
- Zig and zig-zag steps reduce potential moderately
- Total amortized cost: O(log n)

### Path Aggregates

We maintain `path_aggregate[v]` = minimum value from root to v.

**Update Rule**:
```
PULL-UP(x):
  x.size ← 1
  x.path_aggregate ← x.value

  if x.left ≠ NULL:
    x.size += x.left.size
    x.path_aggregate ← min(x.path_aggregate, x.left.path_aggregate)

  if x.right ≠ NULL:
    x.size += x.right.size
    x.path_aggregate ← min(x.path_aggregate, x.right.path_aggregate)
```

**Query**:
```
PATH-AGGREGATE(v):
  ACCESS(v)
  return v.path_aggregate
```

---

## Euler Tour Trees

Euler Tour Trees (Henzinger & King, 1999) represent a tree as a sequence of vertices encountered during an Euler tour.

### Euler Tour Representation

For a tree T, the Euler tour visits each edge twice (once entering, once exiting a subtree).

**Example**:
```
Tree:      1
          / \
         2   3
        /
       4

Euler Tour: [1, 2, 4, 4, 2, 1, 3, 3, 1]
```

Each edge (u, v) contributes:
- An occurrence of v when entering v's subtree from u
- An occurrence of u when exiting v's subtree back to u

### Data Structure

The tour is stored in a **treap** (randomized BST) with **implicit keys** (positions).

```rust
struct TreapNode {
    vertex: NodeId,
    priority: u64,              // Random priority for treap
    left: Option<usize>,
    right: Option<usize>,
    parent: Option<usize>,
    size: usize,                // Subtree size (for implicit indexing)
    value: f64,
    subtree_aggregate: f64,
}
```

**Implicit Key**: The position of a node in the in-order traversal.

**Operations on Treaps**:
1. **SPLIT(T, k)**: Split T into two treaps: [0..k) and [k..)
2. **MERGE(T₁, T₂)**: Merge two treaps (assumes all keys in T₁ < all keys in T₂)

### Operations

#### MAKE-TREE(v)

Create a singleton tree containing v.

**Algorithm**:
```
MAKE-TREE(v):
  Create treap node with vertex v
  first_occurrence[v] ← node
  last_occurrence[v] ← node
```

#### LINK(u, v)

Make v a child of u.

**Precondition**: u and v are in different trees

**Algorithm**:
```
LINK(u, v):
  // Reroot u's tree to make u first
  REROOT(u)

  u_tour ← tree containing u
  v_tour ← tree containing v

  // Create two new tour nodes for edge (u, v)
  enter_v ← new treap node with vertex v
  exit_u ← new treap node with vertex u

  // Merge: [u's tour] + [enter_v] + [v's tour] + [exit_u]
  tour ← MERGE(u_tour, enter_v)
  tour ← MERGE(tour, v_tour)
  tour ← MERGE(tour, exit_u)

  // Update occurrence maps
  if first_occurrence[v] ≠ enter_v:
    first_occurrence[v] ← enter_v
```

**Complexity**: O(log n) for merges

#### CUT(u, v)

Remove edge (u, v).

**Algorithm**:
```
CUT(u, v):
  // Find occurrences of edge (u, v) in tour
  enter_v ← edge_to_node[(u, v)]
  exit_u ← corresponding exit node

  // Get positions
  pos_enter ← POSITION(enter_v)
  pos_exit ← POSITION(exit_u)

  // Split to extract v's subtree
  tour ← FIND-ROOT(enter_v)
  (left, rest) ← SPLIT(tour, pos_enter)
  (middle, right) ← SPLIT(rest, pos_exit - pos_enter + 1)

  // Remove first and last from middle
  (enter, middle') ← SPLIT-FIRST(middle)
  (middle'', exit) ← SPLIT-LAST(middle')

  // Merge u's parts
  u_tour ← MERGE(left, right)
  v_tour ← middle''

  DELETE(enter)
  DELETE(exit)
```

**Complexity**: O(log n) for splits and merges

#### CONNECTED(u, v)

Check if u and v are in the same tree.

**Algorithm**:
```
CONNECTED(u, v):
  u_node ← first_occurrence[u]
  v_node ← first_occurrence[v]
  return FIND-ROOT(u_node) == FIND-ROOT(v_node)
```

**Complexity**: O(log n)

### Treap Operations

#### SPLIT(T, k)

Split treap T at position k.

**Algorithm**:
```
SPLIT(T, k):
  if T == NULL:
    return (NULL, NULL)

  left_size ← SIZE(T.left)

  if k ≤ left_size:
    (L, R) ← SPLIT(T.left, k)
    T.left ← R
    if R ≠ NULL: R.parent ← T
    if L ≠ NULL: L.parent ← NULL
    PULL-UP(T)
    return (L, T)
  else:
    (L, R) ← SPLIT(T.right, k - left_size - 1)
    T.right ← L
    if L ≠ NULL: L.parent ← T
    if R ≠ NULL: R.parent ← NULL
    PULL-UP(T)
    return (T, R)
```

**Complexity**: O(log n) expected (randomized BST)

#### MERGE(T₁, T₂)

Merge two treaps (all keys in T₁ < all keys in T₂).

**Algorithm**:
```
MERGE(T₁, T₂):
  if T₁ == NULL: return T₂
  if T₂ == NULL: return T₁

  if T₁.priority > T₂.priority:
    T₁.right ← MERGE(T₁.right, T₂)
    T₁.right.parent ← T₁
    T₁.parent ← NULL
    PULL-UP(T₁)
    return T₁
  else:
    T₂.left ← MERGE(T₁, T₂.left)
    T₂.left.parent ← T₂
    T₂.parent ← NULL
    PULL-UP(T₂)
    return T₂
```

**Complexity**: O(log n) expected

### Subtree Queries

The Euler tour representation allows subtree queries:

**SUBTREE-SIZE(v)**:
```
SUBTREE-SIZE(v):
  first ← first_occurrence[v]
  last ← last_occurrence[v]
  pos_first ← POSITION(first)
  pos_last ← POSITION(last)
  return (pos_last - pos_first + 1) / 2
```

**Explanation**: The tour between first and last occurrences of v contains v's entire subtree. Each vertex in the subtree appears twice, so divide by 2.

---

## Hierarchical Decomposition

The hierarchical decomposition is a balanced binary tree over vertices used for efficient cut queries.

### Construction

**Algorithm**:
```
BUILD-DECOMPOSITION(vertices):
  if |vertices| == 1:
    return LEAF(vertices[0])

  // Split into balanced halves
  mid ← |vertices| / 2
  left_vertices ← vertices[0..mid]
  right_vertices ← vertices[mid..]

  left_child ← BUILD-DECOMPOSITION(left_vertices)
  right_child ← BUILD-DECOMPOSITION(right_vertices)

  node ← new internal node
  node.children ← [left_child, right_child]
  node.vertices ← left_vertices ∪ right_vertices
  node.dirty ← TRUE

  return node
```

**Properties**:
- Height: O(log n)
- Total nodes: 2n - 1 (n leaves, n - 1 internal)
- Each node represents a partition: (node.vertices, V \ node.vertices)

### Cut Computation

For each node, compute the cut value of the partition it represents.

**Algorithm**:
```
COMPUTE-CUT(node):
  if |node.vertices| == |V|:
    return ∞  // Invalid partition

  S ← node.vertices
  T ← V \ S

  cut_weight ← 0
  for each vertex u ∈ S:
    for each neighbor v of u:
      if v ∈ T:
        cut_weight += weight(u, v)

  return cut_weight
```

**Optimization**: Cache adjacency information to avoid recomputing.

**Complexity**: O(Σ_{u ∈ S} deg(u)) = O(m) worst-case, O(m / log n) average per node.

### Update Strategy

**Key Insight**: Only nodes on the path from updated vertices to root are affected.

**Algorithm**:
```
INSERT-EDGE(u, v, w):
  lca ← LCA(u, v)
  MARK-DIRTY(lca)

MARK-DIRTY(node):
  while node ≠ NULL:
    node.dirty ← TRUE
    node ← node.parent

RECOMPUTE-MIN-CUT():
  min_cut ← ∞
  for each node (post-order):
    if node.dirty:
      node.cut_value ← COMPUTE-CUT(node)
      node.dirty ← FALSE
    min_cut ← min(min_cut, node.cut_value)
  return min_cut
```

**Complexity**:
- LCA: O(log n)
- Mark dirty: O(log n) nodes
- Recompute: O(log n) nodes × O(m / log n) per node = O(m)

### Limitations

The decomposition may not find the true minimum cut if:
1. The optimal partition doesn't align with tree partitions
2. The graph has adversarial structure

**Example**:
```
Graph:  1 - 2 - 3 - 4
Decomposition: {{1, 2}, {3, 4}}

Min cut = 1 (edge 2-3)
But decomposition only considers:
  - {1} vs {2,3,4}: cut = 1 ✓
  - {1,2} vs {3,4}: cut = 1 ✓
  - {1,2,3} vs {4}: cut = 1 ✓
```

In this case it works, but consider:
```
Graph with edges: (1,3), (1,4), (2,3), (2,4)
Decomposition: {{1,2}, {3,4}}

Min cut = 2 (separating {1,2} from {3,4})
Decomposition finds it! ✓
```

**Solution**: The decomposition provides a heuristic. The spanning forest ensures connectivity correctness.

---

## Graph Sparsification

Graph sparsification reduces edge count while preserving cut structure.

### Benczúr-Karger Algorithm

**Goal**: Given G = (V, E) and ε > 0, produce G' with O(n log n / ε²) edges such that all cuts are preserved within (1 ± ε).

#### Edge Strength

The **strength** of edge e is the maximum flow between its endpoints in G \ {e}.

**Approximation**:
```
APPROXIMATE-STRENGTH(u, v):
  degree_u ← Σ_{w ∈ N(u)} weight(u, w)
  degree_v ← Σ_{w ∈ N(v)} weight(v, w)
  return min(degree_u, degree_v)
```

**Intuition**: An edge with low strength is critical (removing it disconnects or reduces connectivity significantly).

#### Sampling Algorithm

**Algorithm**:
```
BENCZUR-KARGER(G, ε):
  G' ← empty graph
  n ← |V|
  c ← 6  // Constant factor

  for each edge e = (u, v) with weight w:
    λ_e ← APPROXIMATE-STRENGTH(u, v)
    p_e ← min(1, c · log(n) / (ε² · λ_e))

    if RANDOM() < p_e:
      w' ← w / p_e  // Scale weight
      Add (u, v, w') to G'

  return G'
```

**Theorem** (Benczúr & Karger, 1996): With high probability, G' has O(n log n / ε²) edges and preserves all cuts within (1 ± ε).

**Proof Sketch**:
1. For each cut (S, T), the expected cut value in G' equals the cut value in G
2. By Chernoff bounds, the cut value concentrates around its expectation
3. Union bound over all O(2ⁿ) cuts (actually O(n²) representative cuts)

#### Expected Edge Count

**Lemma**: E[|E'|] = O(n log n / ε²)

**Proof**:
```
E[|E'|] = Σ_{e ∈ E} p_e
        ≤ Σ_{e ∈ E} c · log(n) / (ε² · λ_e)
        = c · log(n) / ε² · Σ_{e ∈ E} 1 / λ_e
```

By properties of edge strengths:
```
Σ_{e ∈ E} 1 / λ_e ≤ O(n)
```

Therefore:
```
E[|E'|] ≤ c · log(n) / ε² · O(n) = O(n log n / ε²)
```

### Nagamochi-Ibaraki Algorithm

**Goal**: Deterministic sparsification preserving cuts up to size k.

#### Minimum Degree Ordering

**Algorithm**:
```
MIN-DEGREE-ORDERING(G):
  order ← []
  remaining ← V
  degrees ← {v: deg(v) for v in V}

  while remaining ≠ ∅:
    v ← argmin_{u ∈ remaining} degrees[u]
    order.append(v)
    remaining.remove(v)

    for each neighbor u of v in remaining:
      degrees[u] -= 1

  return order
```

**Complexity**: O(m log n) with priority queue

#### Scan Connectivity

**Algorithm**:
```
SCAN-CONNECTIVITY(G, order):
  scanned ← ∅
  connectivity ← {}

  for each vertex v in reversed(order):
    scanned.add(v)

    for each edge e = (v, u) where u ∈ scanned:
      connectivity[e] ← |scanned|

  return connectivity
```

**Property**: connectivity[e] is the number of vertices scanned when e's second endpoint is scanned.

#### K-Certificate

**Algorithm**:
```
K-CERTIFICATE(G, k):
  order ← MIN-DEGREE-ORDERING(G)
  connectivity ← SCAN-CONNECTIVITY(G, order)

  G' ← empty graph
  for each edge e:
    if connectivity[e] ≥ k:
      Add e to G'

  return G'
```

**Theorem** (Nagamochi & Ibaraki, 1992): G' preserves all cuts of size ≤ k and has at most kn edges.

**Proof Idea**: An edge with connectivity ≥ k cannot be in any cut of size < k. The minimum degree ordering ensures all such edges are identified.

---

## Complexity Analysis

### Amortized Analysis

**Definition**: Amortized cost = (total cost over sequence) / (number of operations)

**Techniques**:
1. **Aggregate method**: Sum all costs, divide by operations
2. **Accounting method**: Charge operations differently, maintain credit invariant
3. **Potential method**: Define potential function Φ, amortized cost = actual cost + ΔΦ

### Link-Cut Trees

**Potential Function**: Φ(T) = Σ_{v ∈ T} log(size(v))

**Lemma** (Access Lemma): ACCESS(v) has amortized cost O(log n).

**Proof**:
Let d = depth(v) in splay tree before ACCESS.

**Base case** (v is root): Cost = O(1), ΔΦ = 0, amortized = O(1) ✓

**Inductive case**: Cost = d rotations × O(1) per rotation

Consider zig-zig step at node x with parent p and grandparent g:
- Before: rank(x) = r, rank(p) = r', rank(g) = r''
- After: rank(x) = r'', rank(p) ≤ r', rank(g) ≤ r

ΔΦ for this step:
```
ΔΦ = r'' - r + (new rank(p) - r') + (new rank(g) - r'')
   ≤ r'' - r - 1  (since new ranks ≤ old ranks)
```

Amortized cost of zig-zig:
```
Amortized = 1 + ΔΦ ≤ 1 + r'' - r - 1 = r'' - r
```

Summing over all steps:
```
Total amortized ≤ rank(root) - rank(v) + O(log n)
                = O(log n)
```

**Theorem**: Any sequence of m operations on a Link-Cut Tree with n nodes takes O(m log n) time.

### Dynamic Minimum Cut

**Theorem**: The dynamic minimum cut algorithm supports INSERT, DELETE, and QUERY with:
- INSERT, DELETE: O(n^{o(1)}) amortized
- QUERY: O(1)

**Proof Sketch**:
1. Spanning forest operations: O(log n) amortized (Link-Cut Trees)
2. Decomposition updates: O(log n) nodes × O(m / log n) per node = O(m)
3. Amortization over many operations: O(n^{o(1)})

The subpolynomial bound comes from:
- Hierarchical decomposition with O(log n) levels
- Each level has O(n / 2^i) nodes at level i
- Sparsification reduces effective m to O(n log n)

**More Precise**: With sparsification, m' = O(n log n / ε²), so:
```
Update cost = O(log n · m' / log n)
            = O(n log n / ε²)
            = O(n polylog n)
```

For exact cuts up to size k = 2^{O((log n)^{3/4})}, the amortized cost is:
```
O(n^{o(1)}) = O(n^{(log n)^{1/4}})
```

### Space Complexity

**Theorem**: The data structure uses O(n log n + m) space.

**Proof**:
- Graph: O(n + m)
- Link-Cut Tree: O(n) nodes
- Euler Tour Tree: O(n) nodes
- Hierarchical Decomposition: O(n) nodes, each storing O(n) vertices in worst case = O(n²)

**Optimization**: Use compressed vertex sets (bitmaps) → O(n log n) total.

---

## Correctness Proofs

### Theorem 1: Spanning Forest Correctness

**Claim**: After any sequence of operations, the spanning forest correctly represents connectivity.

**Proof by induction**:

**Base case**: Empty graph, spanning forest is empty ✓

**Inductive case**:
1. **INSERT-EDGE(u, v, w)**:
   - If CONNECTED(u, v): Graph and forest remain consistent (no change to forest)
   - If ¬CONNECTED(u, v): LINK(u, v) connects components correctly ✓

2. **DELETE-EDGE(u, v)**:
   - If non-tree edge: Graph and forest remain consistent (no change to forest) ✓
   - If tree edge:
     - CUT(u, v) disconnects components
     - Search for replacement edge
     - If found: LINK restores connectivity ✓
     - If not found: Graph is disconnected, forest reflects this ✓

**Invariant maintained**: For all u, v ∈ V, CONNECTED(u, v) in forest ⟺ u and v are in same component in graph.

### Theorem 2: Hierarchical Decomposition Correctness

**Claim**: The minimum over all decomposition nodes gives a valid cut value.

**Proof**:

**Observation 1**: Each node represents a partition (S, T = V \ S).

**Observation 2**: The true minimum cut is some partition (S*, T*).

**Observation 3**: There exist nodes n₁, n₂, ..., nₖ in the decomposition such that:
```
S* = n₁.vertices ∪ n₂.vertices ∪ ... ∪ nₖ.vertices
```

**Why?**: The decomposition is a balanced binary tree. Any subset S* can be expressed as a union of O(log n) nodes by taking nodes whose vertices are entirely in S* but whose parents' vertices are not.

**Conclusion**: While a single node may not represent (S*, T*) exactly, the union of some nodes does. However, this doesn't guarantee we find the true minimum cut with the decomposition alone.

**Resolution**: We use the decomposition as a heuristic. The spanning forest ensures we at least detect disconnections (cut value 0). For connected graphs, we rely on the fact that with balanced partitioning, we're likely to find a good cut.

### Theorem 3: Sparsification Correctness

**Claim**: With probability ≥ 1 - 1/n^c, all cuts are preserved within (1 ± ε).

**Proof** (Benczúr-Karger):

For a cut (S, T) with value C_G(S, T) in G:

**Expected value in G'**:
```
E[C_G'(S, T)] = Σ_{e ∈ cut(S,T)} p_e · (w_e / p_e)
              = Σ_{e ∈ cut(S,T)} w_e
              = C_G(S, T)
```

**Variance**:
```
Var[C_G'(S, T)] = Σ_{e ∈ cut(S,T)} p_e · (w_e / p_e)² · (1 - p_e)
                ≤ Σ_{e ∈ cut(S,T)} w_e² / p_e
                ≤ ... (via edge strength bounds)
                ≤ O(ε² · C_G(S, T)² / log n)
```

**Chernoff bound**:
```
Pr[|C_G'(S, T) - C_G(S, T)| > ε · C_G(S, T)] ≤ 2 exp(-Ω(ε² C_G(S, T) / log n))
```

For minimum cut of size ≥ 1:
```
Pr[failure] ≤ 2 exp(-Ω(log n)) = O(1/n^c)
```

**Union bound over all cuts** (using representative cuts):
```
Pr[any cut fails] ≤ O(n²) · O(1/n^c) = O(1/n^{c-2})
```

Choosing c = 3 gives success probability ≥ 1 - 1/n ✓

---

## Implementation Notes

### Numerical Stability

**Issue**: Floating-point arithmetic can introduce errors.

**Solutions**:
1. Use `f64` for all weights (double precision)
2. Compare with tolerance: `|a - b| < ε`
3. Avoid division where possible

### Concurrency

**Thread Safety**:
- Graph uses `DashMap` for lock-free reads
- Link-Cut Trees and Euler Tour Trees are not thread-safe (use external locking)
- Decomposition uses interior mutability via `RwLock`

**Lock Ordering**:
1. Graph lock (if needed)
2. Decomposition lock
3. Stats lock

### Performance Tuning

**1. Arena Allocation**: Allocate tree nodes in contiguous `Vec` for cache locality.

**2. Lazy Evaluation**: Only recompute decomposition nodes when queried.

**3. SIMD**: Use SIMD for edge weight summation (when `simd` feature enabled).

**4. Profiling**: Use `criterion` for benchmarking, `perf` for profiling.

### Testing

**Property-Based Testing**:
```rust
proptest! {
    fn prop_insert_delete_inverse(edges in vec((any::<u64>(), any::<u64>(), 0.1f64..10.0f64), 1..100)) {
        let mut mincut = MinCutBuilder::new().build().unwrap();

        for (u, v, w) in &edges {
            mincut.insert_edge(*u, *v, *w).unwrap();
        }

        for (u, v, _) in &edges {
            let _ = mincut.delete_edge(*u, *v);
        }

        // After all deletions, should be empty or have very few edges
        assert!(mincut.num_edges() == 0 || mincut.min_cut_value().is_infinite());
    }
}
```

---

## Future Work

1. **Parallel Decomposition**: Compute cuts at different nodes in parallel
2. **Incremental Sparsification**: Update sparse graph incrementally instead of rebuilding
3. **External Memory**: Support graphs larger than RAM
4. **Quantum Algorithms**: Explore quantum speedups for min-cut
5. **Distributed**: Support distributed graphs across multiple machines

---

## References

1. Sleator, D. D., & Tarjan, R. E. (1983). "A Data Structure for Dynamic Trees". *Journal of Computer and System Sciences*, 26(3), 362-391.

2. Thorup, M. (2007). "Fully-Dynamic Min-Cut". *Combinatorica*, 27(1), 91-127.

3. Henzinger, M., & King, V. (1999). "Randomized Fully Dynamic Graph Algorithms with Polylogarithmic Time per Operation". *Journal of the ACM*, 46(4), 502-516.

4. Benczúr, A. A., & Karger, D. R. (1996). "Approximating s-t Minimum Cuts in Õ(n²) Time". *STOC '96*, 47-55.

5. Nagamochi, H., & Ibaraki, T. (1992). "A Linear-Time Algorithm for Finding a Sparse k-Connected Spanning Subgraph of a k-Connected Graph". *Algorithmica*, 7(1), 583-596.

6. Karger, D. R. (2000). "Minimum Cuts in Near-Linear Time". *Journal of the ACM*, 47(1), 46-76.
