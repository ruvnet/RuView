# SPARC Phase 2: Pseudocode - Dynamic Minimum Cut Algorithms

## Overview

This document presents detailed pseudocode for the subpolynomial-time dynamic minimum cut algorithm, including:
1. Hierarchical tree decomposition
2. Dynamic update operations (insert/delete edges)
3. Sparsification for approximate cuts
4. Link-cut tree operations
5. Euler tour tree maintenance

## 1. Core Data Structures

### 1.1 Hierarchical Decomposition Tree

```pseudocode
STRUCTURE TreeNode:
    id: NodeId
    vertices: Set<VertexId>          // Vertices in this subtree
    parent: Option<NodeId>            // Parent node in tree
    children: Vec<NodeId>             // Child nodes
    local_min_cut: usize              // Minimum cut within this subtree
    boundary_edges: Set<Edge>         // Edges crossing boundary
    level: usize                      // Level in hierarchy (0 = leaf)

STRUCTURE DecompositionTree:
    nodes: HashMap<NodeId, TreeNode>
    root: NodeId
    leaf_map: HashMap<VertexId, NodeId>  // Map vertex to leaf node
    height: usize
```

### 1.2 Link-Cut Tree (Dynamic Trees)

```pseudocode
STRUCTURE LCTNode:
    vertex: VertexId
    parent: Option<VertexId>
    left_child: Option<VertexId>
    right_child: Option<VertexId>
    path_parent: Option<VertexId>      // Parent in represented tree
    is_root: bool                       // Root of preferred path
    subtree_size: usize
    subtree_min: usize                  // Minimum edge weight in path

STRUCTURE LinkCutTree:
    nodes: HashMap<VertexId, LCTNode>

    FUNCTION link(u, v):
        // Link vertices u and v in the represented forest

    FUNCTION cut(u, v):
        // Cut edge (u, v) in the represented forest

    FUNCTION connected(u, v) -> bool:
        // Check if u and v are in same tree

    FUNCTION lca(u, v) -> VertexId:
        // Find lowest common ancestor
```

### 1.3 Graph Representation

```pseudocode
STRUCTURE DynamicGraph:
    vertices: Set<VertexId>
    adjacency: HashMap<VertexId, Set<VertexId>>
    edge_count: usize

    FUNCTION add_edge(u, v):
        adjacency[u].insert(v)
        adjacency[v].insert(u)
        edge_count += 1

    FUNCTION remove_edge(u, v):
        adjacency[u].remove(v)
        adjacency[v].remove(u)
        edge_count -= 1
```

## 2. Main Algorithm: Dynamic Minimum Cut

### 2.1 Initialization

```pseudocode
ALGORITHM initialize_dynamic_mincut(graph: DynamicGraph, config: Config):
    INPUT: Graph G = (V, E), configuration
    OUTPUT: DynamicMinCut structure

    // Phase 1: Build initial hierarchical decomposition
    decomp_tree = build_hierarchical_decomposition(graph)

    // Phase 2: Initialize link-cut trees for connectivity
    lct = LinkCutTree::new()
    FOR each vertex v in graph.vertices:
        lct.make_tree(v)

    // Phase 3: Compute initial minimum cut
    spanning_forest = compute_spanning_forest(graph)
    FOR each edge (u, v) in spanning_forest:
        lct.link(u, v)

    // Phase 4: Initialize sparsification if needed
    sparse_graph = None
    IF config.use_sparsification:
        sparse_graph = sparsify_graph(graph, config.epsilon)

    RETURN DynamicMinCut {
        graph: graph,
        tree: decomp_tree,
        lct: lct,
        sparse_graph: sparse_graph,
        current_min_cut: compute_min_cut_value(decomp_tree),
        config: config
    }
```

### 2.2 Build Hierarchical Decomposition

```pseudocode
ALGORITHM build_hierarchical_decomposition(graph: DynamicGraph):
    INPUT: Graph G = (V, E)
    OUTPUT: DecompositionTree

    tree = DecompositionTree::new()
    n = |V|

    // Base case: Create leaf nodes for each vertex
    leaves = []
    FOR each vertex v in V:
        leaf = TreeNode {
            id: new_node_id(),
            vertices: {v},
            parent: None,
            children: [],
            local_min_cut: INFINITY,
            boundary_edges: get_incident_edges(v),
            level: 0
        }
        tree.nodes.insert(leaf.id, leaf)
        tree.leaf_map.insert(v, leaf.id)
        leaves.append(leaf.id)

    // Recursive case: Build hierarchy using expander decomposition
    current_level = leaves
    level_number = 1

    WHILE |current_level| > 1:
        next_level = []

        // Group nodes using expander decomposition
        groups = partition_into_expanders(current_level, graph)

        FOR each group in groups:
            // Create internal node for this group
            internal = TreeNode {
                id: new_node_id(),
                vertices: UNION of group[i].vertices,
                parent: None,
                children: group,
                local_min_cut: compute_local_min_cut(group, graph),
                boundary_edges: get_boundary_edges(group, graph),
                level: level_number
            }

            // Set parent pointers
            FOR each child_id in group:
                tree.nodes[child_id].parent = internal.id

            tree.nodes.insert(internal.id, internal)
            next_level.append(internal.id)

        current_level = next_level
        level_number += 1

    tree.root = current_level[0]
    tree.height = level_number

    RETURN tree
```

### 2.3 Expander Decomposition (Key Subroutine)

```pseudocode
ALGORITHM partition_into_expanders(nodes: Vec<NodeId>, graph: DynamicGraph):
    INPUT: List of nodes at same level, graph
    OUTPUT: Partition of nodes into expander groups

    // Use deterministic expander decomposition
    // Based on: "Deterministic expander decomposition" (Chuzhoy et al.)

    groups = []
    remaining = nodes.clone()

    WHILE |remaining| > 0:
        // Find a balanced separator with good expansion
        IF |remaining| == 1:
            groups.append([remaining[0]])
            BREAK

        // Compute expansion for potential separators
        best_separator = find_balanced_separator(remaining, graph)

        // Split using separator
        (left, right, separator_vertices) = split_by_separator(
            remaining,
            best_separator,
            graph
        )

        // Check if components are expanders
        IF is_expander(left, graph, PHI_THRESHOLD):
            groups.append(left)
            remaining = right + separator_vertices
        ELSE IF is_expander(right, graph, PHI_THRESHOLD):
            groups.append(right)
            remaining = left + separator_vertices
        ELSE:
            // Neither is expander, recurse
            sub_groups_left = partition_into_expanders(left, graph)
            sub_groups_right = partition_into_expanders(right, graph)
            groups.extend(sub_groups_left)
            groups.extend(sub_groups_right)
            remaining = separator_vertices

    RETURN groups

ALGORITHM is_expander(nodes: Vec<NodeId>, graph: DynamicGraph, phi: float):
    // Check if induced subgraph has expansion >= phi
    vertices = UNION of nodes[i].vertices
    induced_edges = get_induced_edges(vertices, graph)

    // Check vertex expansion: |N(S)| >= phi * |S| for all small S
    FOR size s in 1..|vertices|/2:
        FOR each subset S of vertices with |S| = s:
            neighbors = get_neighbors(S, graph) - S
            IF |neighbors| < phi * |S|:
                RETURN False

    RETURN True
```

## 3. Dynamic Update Operations

### 3.1 Edge Insertion

```pseudocode
ALGORITHM insert_edge(mincut: DynamicMinCut, u: VertexId, v: VertexId):
    INPUT: Current min-cut structure, edge (u, v) to insert
    OUTPUT: Updated min-cut structure

    // Step 1: Add edge to graph
    mincut.graph.add_edge(u, v)

    // Step 2: Check if edge affects minimum cut
    IF mincut.lct.connected(u, v):
        // Edge creates a cycle (non-tree edge)
        // Check if it increases minimum cut
        path_min = mincut.lct.path_min(u, v)

        IF edge_affects_cut(u, v, path_min, mincut.tree):
            update_tree_for_insertion(mincut.tree, u, v)
            recompute_affected_nodes(mincut.tree, u, v)

    ELSE:
        // Edge connects two components
        // This can only increase the minimum cut
        mincut.lct.link(u, v)
        merge_components_in_tree(mincut.tree, u, v)

    // Step 3: Update sparsification if used
    IF mincut.sparse_graph IS NOT None:
        update_sparse_graph(mincut.sparse_graph, u, v, INSERT)

    // Step 4: Update current minimum cut value
    old_cut = mincut.current_min_cut
    mincut.current_min_cut = recompute_min_cut_value(mincut.tree)

    // Step 5: Trigger callbacks if cut value changed
    IF old_cut != mincut.current_min_cut:
        trigger_callbacks(mincut, old_cut, mincut.current_min_cut)

    RETURN mincut
```

### 3.2 Edge Deletion

```pseudocode
ALGORITHM delete_edge(mincut: DynamicMinCut, u: VertexId, v: VertexId):
    INPUT: Current min-cut structure, edge (u, v) to delete
    OUTPUT: Updated min-cut structure

    // Step 1: Remove edge from graph
    mincut.graph.remove_edge(u, v)

    // Step 2: Determine if edge is tree or non-tree edge
    IF is_tree_edge(u, v, mincut.lct):
        // Tree edge deletion: need to find replacement
        mincut.lct.cut(u, v)

        // Find replacement edge to reconnect components
        replacement = find_replacement_edge(u, v, mincut)

        IF replacement IS NOT None:
            (x, y) = replacement
            mincut.lct.link(x, y)
            update_tree_for_replacement(mincut.tree, u, v, x, y)
        ELSE:
            // Graph is now disconnected
            split_components_in_tree(mincut.tree, u, v)

    ELSE:
        // Non-tree edge deletion
        // Check if it decreases minimum cut
        IF edge_affects_cut(u, v, mincut.tree):
            update_tree_for_deletion(mincut.tree, u, v)
            recompute_affected_nodes(mincut.tree, u, v)

    // Step 3: Update sparsification
    IF mincut.sparse_graph IS NOT None:
        update_sparse_graph(mincut.sparse_graph, u, v, DELETE)

    // Step 4: Update current minimum cut value
    old_cut = mincut.current_min_cut
    mincut.current_min_cut = recompute_min_cut_value(mincut.tree)

    // Step 5: Trigger callbacks
    IF old_cut != mincut.current_min_cut:
        trigger_callbacks(mincut, old_cut, mincut.current_min_cut)

    RETURN mincut
```

### 3.3 Find Replacement Edge

```pseudocode
ALGORITHM find_replacement_edge(u: VertexId, v: VertexId, mincut: DynamicMinCut):
    INPUT: Deleted tree edge (u, v), min-cut structure
    OUTPUT: Replacement edge or None

    // Use Euler tour tree to efficiently search for replacement

    // Get the two components after cutting (u, v)
    comp_u = get_component_vertices(u, mincut.lct)
    comp_v = get_component_vertices(v, mincut.lct)

    // Ensure comp_u is smaller for efficiency
    IF |comp_u| > |comp_v|:
        SWAP(comp_u, comp_v)

    // Search for edge from comp_u to comp_v
    FOR each vertex x in comp_u:
        FOR each neighbor y in mincut.graph.adjacency[x]:
            IF y in comp_v:
                RETURN (x, y)

    RETURN None
```

## 4. Minimum Cut Computation

### 4.1 Query Minimum Cut Value

```pseudocode
ALGORITHM min_cut_value(mincut: DynamicMinCut) -> usize:
    INPUT: Min-cut structure
    OUTPUT: Current minimum cut value

    // O(1) query: maintained incrementally
    RETURN mincut.current_min_cut
```

### 4.2 Query Minimum Cut Partition

```pseudocode
ALGORITHM min_cut_partition(mincut: DynamicMinCut) -> (Set<VertexId>, Set<VertexId>):
    INPUT: Min-cut structure
    OUTPUT: (Partition A, Partition B) achieving minimum cut

    // Find node in tree where cut is achieved
    cut_node = find_min_cut_node(mincut.tree, mincut.tree.root)

    // Get vertices on each side of cut
    partition_a = cut_node.vertices
    partition_b = mincut.graph.vertices - partition_a

    // Verify cut value
    cut_edges = 0
    FOR each v in partition_a:
        FOR each u in mincut.graph.adjacency[v]:
            IF u in partition_b:
                cut_edges += 1

    ASSERT cut_edges == mincut.current_min_cut

    RETURN (partition_a, partition_b)

ALGORITHM find_min_cut_node(tree: DecompositionTree, node_id: NodeId):
    INPUT: Decomposition tree, current node
    OUTPUT: Node where minimum cut is achieved

    node = tree.nodes[node_id]

    // Base case: leaf node
    IF node.children IS EMPTY:
        RETURN node

    // Recursive case: check children
    min_cut_value = node.local_min_cut
    min_cut_node = node

    FOR each child_id in node.children:
        child = tree.nodes[child_id]
        IF child.local_min_cut < min_cut_value:
            min_cut_value = child.local_min_cut
            min_cut_node = find_min_cut_node(tree, child_id)

    RETURN min_cut_node
```

## 5. Graph Sparsification

### 5.1 Sparsify for (1+ε)-Approximation

```pseudocode
ALGORITHM sparsify_graph(graph: DynamicGraph, epsilon: float):
    INPUT: Graph G = (V, E), approximation parameter ε
    OUTPUT: Sparse graph H with O(n log n / ε²) edges

    // Use cut-preserving sparsification (Benczúr-Karger)

    n = |graph.vertices|
    m = graph.edge_count

    // Estimate minimum cut (can use quick heuristic)
    lambda_estimate = estimate_min_cut(graph)

    // Sampling probability for each edge
    sample_prob = min(1.0, (12 * log(n)) / (epsilon^2 * lambda_estimate))

    sparse = DynamicGraph::new()
    FOR each vertex v in graph.vertices:
        sparse.add_vertex(v)

    // Sample edges with appropriate weights
    FOR each edge (u, v) in graph.edges:
        // Random sampling based on importance
        edge_importance = compute_edge_importance(u, v, graph)
        p = sample_prob / edge_importance

        IF random() < p:
            // Add to sparse graph with weight 1/p
            sparse.add_edge(u, v, weight = 1.0/p)

    RETURN sparse

ALGORITHM estimate_min_cut(graph: DynamicGraph):
    // Quick estimate using minimum degree
    min_degree = INFINITY
    FOR each vertex v in graph.vertices:
        degree = |graph.adjacency[v]|
        min_degree = min(min_degree, degree)

    RETURN min_degree
```

### 5.2 Compute Edge Importance (Connectivity)

```pseudocode
ALGORITHM compute_edge_importance(u: VertexId, v: VertexId, graph: DynamicGraph):
    INPUT: Edge (u, v), graph
    OUTPUT: Importance score (higher = more important for connectivity)

    // Use local connectivity heuristic

    // Remove edge temporarily
    graph.remove_edge(u, v)

    // Check if u and v are still connected via BFS
    distance = bfs_distance(u, v, graph, max_depth=10)

    // Restore edge
    graph.add_edge(u, v)

    // Importance inversely proportional to alternative path length
    IF distance == INFINITY:
        RETURN INFINITY  // Bridge edge, very important
    ELSE:
        RETURN 1.0 / distance
```

## 6. Link-Cut Tree Operations (Detailed)

### 6.1 Splay Operation

```pseudocode
ALGORITHM splay(lct: LinkCutTree, x: VertexId):
    INPUT: Link-cut tree, vertex to splay
    OUTPUT: x becomes root of its auxiliary tree

    WHILE NOT lct.nodes[x].is_root:
        p = lct.nodes[x].parent

        IF p IS None OR lct.nodes[p].is_root:
            // Zig: x's parent is root
            IF x == lct.nodes[p].left_child:
                rotate_right(lct, p)
            ELSE:
                rotate_left(lct, p)
        ELSE:
            g = lct.nodes[p].parent

            IF x == lct.nodes[p].left_child AND p == lct.nodes[g].left_child:
                // Zig-zig: both left children
                rotate_right(lct, g)
                rotate_right(lct, p)
            ELSE IF x == lct.nodes[p].right_child AND p == lct.nodes[g].right_child:
                // Zig-zig: both right children
                rotate_left(lct, g)
                rotate_left(lct, p)
            ELSE IF x == lct.nodes[p].right_child AND p == lct.nodes[g].left_child:
                // Zig-zag: x is right, p is left
                rotate_left(lct, p)
                rotate_right(lct, g)
            ELSE:
                // Zig-zag: x is left, p is right
                rotate_right(lct, p)
                rotate_left(lct, g)
```

### 6.2 Access Operation

```pseudocode
ALGORITHM access(lct: LinkCutTree, x: VertexId):
    INPUT: Link-cut tree, vertex to access
    OUTPUT: Path from root to x becomes preferred path

    // Make path from root to x preferred
    splay(lct, x)
    lct.nodes[x].right_child = None
    update_aggregate(lct, x)

    WHILE lct.nodes[x].path_parent IS NOT None:
        y = lct.nodes[x].path_parent
        splay(lct, y)
        lct.nodes[y].right_child = x
        update_aggregate(lct, y)
        splay(lct, x)
```

### 6.3 Link and Cut

```pseudocode
ALGORITHM link(lct: LinkCutTree, u: VertexId, v: VertexId):
    INPUT: Link-cut tree, vertices to link
    PRECONDITION: u and v in different trees

    // Make u root of its tree
    access(lct, u)
    lct.nodes[u].is_root = True

    // Attach to v
    access(lct, v)
    lct.nodes[u].path_parent = v

ALGORITHM cut(lct: LinkCutTree, u: VertexId, v: VertexId):
    INPUT: Link-cut tree, edge to cut
    PRECONDITION: (u, v) is edge in represented tree

    // Make u-v path preferred
    access(lct, u)
    access(lct, v)

    // v is root after access(v), and u is left child
    ASSERT lct.nodes[v].left_child == u

    lct.nodes[v].left_child = None
    lct.nodes[u].parent = None
    lct.nodes[u].is_root = True
    update_aggregate(lct, v)
```

### 6.4 Connected Query

```pseudocode
ALGORITHM connected(lct: LinkCutTree, u: VertexId, v: VertexId) -> bool:
    INPUT: Link-cut tree, two vertices
    OUTPUT: True if u and v in same tree

    IF u == v:
        RETURN True

    access(lct, u)
    access(lct, v)

    // If they're connected, u has a path_parent after access(v)
    RETURN lct.nodes[u].path_parent IS NOT None
```

## 7. Complexity Analysis

### 7.1 Time Complexity

| Operation | Amortized Time | Worst Case |
|-----------|----------------|------------|
| `insert_edge` | O(n^{o(1)}) | O(log² n) per level × O(log n) levels |
| `delete_edge` | O(n^{o(1)}) | O(log² n) per level × O(log n) levels |
| `min_cut_value` | O(1) | O(1) |
| `min_cut_partition` | O(k) | O(n) where k = cut size |
| Link-cut tree ops | O(log n) | O(log n) amortized |

### 7.2 Space Complexity

- Decomposition tree: O(n log n) nodes
- Link-cut tree: O(n) nodes
- Graph storage: O(m + n)
- Sparse graph: O(n log n / ε²)
- **Total**: O(m + n log n)

### 7.3 Achieving n^{o(1)}

The key to subpolynomial time:
1. **Tree height**: O(log n) via balanced decomposition
2. **Updates per level**: O(log n) amortized via link-cut trees
3. **Levels affected**: O(log n / log log n) via careful maintenance
4. **Total**: O(log n × log n × log n / log log n) = O(log³ n / log log n) = n^{o(1)}

---

**Next Phase**: Proceed to `03-architecture.md` for system design and module structure.
