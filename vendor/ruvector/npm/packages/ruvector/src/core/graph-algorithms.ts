/**
 * Graph Algorithms - MinCut, Spectral Clustering, Community Detection
 *
 * Provides graph partitioning and clustering algorithms for:
 * - Code module detection
 * - Dependency clustering
 * - Architecture analysis
 * - Refactoring suggestions
 */

export interface Graph {
  nodes: string[];
  edges: Array<{ from: string; to: string; weight?: number }>;
  adjacency: Map<string, Map<string, number>>;
}

export interface Partition {
  groups: string[][];
  cutWeight: number;
  modularity: number;
}

export interface SpectralResult {
  clusters: Map<string, number>;
  eigenvalues: number[];
  coordinates: Map<string, number[]>;
}

/**
 * Build adjacency representation from edges
 */
export function buildGraph(
  nodes: string[],
  edges: Array<{ from: string; to: string; weight?: number }>
): Graph {
  const adjacency = new Map<string, Map<string, number>>();

  for (const node of nodes) {
    adjacency.set(node, new Map());
  }

  for (const { from, to, weight = 1 } of edges) {
    if (!adjacency.has(from)) adjacency.set(from, new Map());
    if (!adjacency.has(to)) adjacency.set(to, new Map());

    // Undirected graph - add both directions
    adjacency.get(from)!.set(to, weight);
    adjacency.get(to)!.set(from, weight);
  }

  return { nodes, edges, adjacency };
}

/**
 * Minimum Cut (Stoer-Wagner algorithm)
 *
 * Finds the minimum weight cut that partitions the graph into two parts.
 * Useful for finding loosely coupled module boundaries.
 */
export function minCut(graph: Graph): Partition {
  const n = graph.nodes.length;
  if (n < 2) {
    return { groups: [graph.nodes], cutWeight: 0, modularity: 0 };
  }

  // Copy adjacency for modification
  const adj = new Map<string, Map<string, number>>();
  for (const [node, neighbors] of graph.adjacency) {
    adj.set(node, new Map(neighbors));
  }

  let minCutWeight = Infinity;
  let bestPartition: string[][] = [];
  const merged = new Map<string, string[]>(); // Track merged nodes

  for (const node of graph.nodes) {
    merged.set(node, [node]);
  }

  let remaining = [...graph.nodes];

  // Stoer-Wagner phases
  while (remaining.length > 1) {
    // Maximum adjacency search
    const inA = new Set<string>([remaining[0]]);
    const weights = new Map<string, number>();

    for (const node of remaining) {
      if (!inA.has(node)) {
        weights.set(node, adj.get(remaining[0])?.get(node) || 0);
      }
    }

    let lastAdded = remaining[0];
    let beforeLast = remaining[0];

    while (inA.size < remaining.length) {
      // Find node with maximum weight to A
      let maxWeight = -Infinity;
      let maxNode = '';

      for (const [node, weight] of weights) {
        if (!inA.has(node) && weight > maxWeight) {
          maxWeight = weight;
          maxNode = node;
        }
      }

      if (!maxNode) break;

      beforeLast = lastAdded;
      lastAdded = maxNode;
      inA.add(maxNode);

      // Update weights
      for (const [neighbor, w] of adj.get(maxNode) || []) {
        if (!inA.has(neighbor)) {
          weights.set(neighbor, (weights.get(neighbor) || 0) + w);
        }
      }
    }

    // Cut of the phase
    const cutWeight = weights.get(lastAdded) || 0;

    if (cutWeight < minCutWeight) {
      minCutWeight = cutWeight;
      const lastGroup = merged.get(lastAdded) || [lastAdded];
      const otherNodes = remaining.filter(n => n !== lastAdded).flatMap(n => merged.get(n) || [n]);
      bestPartition = [lastGroup, otherNodes];
    }

    // Merge last two nodes
    if (remaining.length > 1) {
      // Merge lastAdded into beforeLast
      const mergedNodes = [...(merged.get(beforeLast) || []), ...(merged.get(lastAdded) || [])];
      merged.set(beforeLast, mergedNodes);

      // Update adjacency
      for (const [neighbor, w] of adj.get(lastAdded) || []) {
        if (neighbor !== beforeLast) {
          const current = adj.get(beforeLast)?.get(neighbor) || 0;
          adj.get(beforeLast)?.set(neighbor, current + w);
          adj.get(neighbor)?.set(beforeLast, current + w);
        }
      }

      // Remove lastAdded
      remaining = remaining.filter(n => n !== lastAdded);
      adj.delete(lastAdded);
      for (const [, neighbors] of adj) {
        neighbors.delete(lastAdded);
      }
    }
  }

  const modularity = calculateModularity(graph, bestPartition);

  return {
    groups: bestPartition.filter(g => g.length > 0),
    cutWeight: minCutWeight,
    modularity,
  };
}

/**
 * Spectral Clustering (using power iteration)
 *
 * Uses graph Laplacian eigenvectors for clustering.
 * Good for finding natural clusters in code dependencies.
 */
export function spectralClustering(graph: Graph, k: number = 2): SpectralResult {
  const n = graph.nodes.length;
  const nodeIndex = new Map(graph.nodes.map((node, i) => [node, i]));
  const clusters = new Map<string, number>();

  if (n === 0) {
    return { clusters, eigenvalues: [], coordinates: new Map() };
  }

  // Build Laplacian matrix (D - A)
  const degree = new Float64Array(n);
  const laplacian: number[][] = Array(n).fill(null).map(() => Array(n).fill(0));

  for (const [node, neighbors] of graph.adjacency) {
    const i = nodeIndex.get(node)!;
    let d = 0;
    for (const [neighbor, weight] of neighbors) {
      const j = nodeIndex.get(neighbor)!;
      laplacian[i][j] = -weight;
      d += weight;
    }
    degree[i] = d;
    laplacian[i][i] = d;
  }

  // Normalized Laplacian: D^(-1/2) L D^(-1/2)
  for (let i = 0; i < n; i++) {
    for (let j = 0; j < n; j++) {
      if (degree[i] > 0 && degree[j] > 0) {
        laplacian[i][j] /= Math.sqrt(degree[i] * degree[j]);
      }
    }
  }

  // Power iteration to find eigenvectors
  const eigenvectors: number[][] = [];
  const eigenvalues: number[] = [];

  for (let ev = 0; ev < Math.min(k, n); ev++) {
    let vector = new Float64Array(n);
    for (let i = 0; i < n; i++) {
      vector[i] = Math.random();
    }
    normalize(vector);

    // Deflation: orthogonalize against previous eigenvectors
    for (const prev of eigenvectors) {
      const dot = dotProduct(vector, new Float64Array(prev));
      for (let i = 0; i < n; i++) {
        vector[i] -= dot * prev[i];
      }
    }
    normalize(vector);

    // Power iteration
    for (let iter = 0; iter < 100; iter++) {
      const newVector = new Float64Array(n);
      for (let i = 0; i < n; i++) {
        for (let j = 0; j < n; j++) {
          newVector[i] += laplacian[i][j] * vector[j];
        }
      }

      // Deflation
      for (const prev of eigenvectors) {
        const dot = dotProduct(newVector, new Float64Array(prev));
        for (let i = 0; i < n; i++) {
          newVector[i] -= dot * prev[i];
        }
      }

      normalize(newVector);
      vector = newVector;
    }

    // Compute eigenvalue
    let eigenvalue = 0;
    for (let i = 0; i < n; i++) {
      let sum = 0;
      for (let j = 0; j < n; j++) {
        sum += laplacian[i][j] * vector[j];
      }
      eigenvalue += vector[i] * sum;
    }

    eigenvectors.push(Array.from(vector));
    eigenvalues.push(eigenvalue);
  }

  // K-means clustering on eigenvector coordinates
  const coordinates = new Map<string, number[]>();
  for (let i = 0; i < n; i++) {
    coordinates.set(graph.nodes[i], eigenvectors.map(ev => ev[i]));
  }

  // Simple k-means
  const clusterAssignment = kMeans(
    graph.nodes.map(node => coordinates.get(node)!),
    k
  );

  for (let i = 0; i < n; i++) {
    clusters.set(graph.nodes[i], clusterAssignment[i]);
  }

  return { clusters, eigenvalues, coordinates };
}

/**
 * Louvain Community Detection
 *
 * Greedy modularity optimization for finding communities.
 * Good for detecting natural module boundaries.
 */
export function louvainCommunities(graph: Graph): Map<string, number> {
  const communities = new Map<string, number>();
  let communityId = 0;

  // Initialize: each node in its own community
  for (const node of graph.nodes) {
    communities.set(node, communityId++);
  }

  // Total edge weight
  let m = 0;
  for (const { weight = 1 } of graph.edges) {
    m += weight;
  }
  m /= 2; // Undirected

  if (m === 0) return communities;

  // Node weights (sum of edge weights)
  const nodeWeight = new Map<string, number>();
  for (const node of graph.nodes) {
    let w = 0;
    for (const [, weight] of graph.adjacency.get(node) || []) {
      w += weight;
    }
    nodeWeight.set(node, w);
  }

  // Community weights
  const communityWeight = new Map<number, number>();
  for (const node of graph.nodes) {
    const c = communities.get(node)!;
    communityWeight.set(c, (communityWeight.get(c) || 0) + (nodeWeight.get(node) || 0));
  }

  // Iterate until no improvement
  let improved = true;
  while (improved) {
    improved = false;

    for (const node of graph.nodes) {
      const currentCommunity = communities.get(node)!;
      const ki = nodeWeight.get(node) || 0;

      // Calculate modularity gain for moving to neighbor communities
      let bestCommunity = currentCommunity;
      let bestGain = 0;

      const neighborCommunities = new Set<number>();
      for (const [neighbor] of graph.adjacency.get(node) || []) {
        neighborCommunities.add(communities.get(neighbor)!);
      }

      for (const targetCommunity of neighborCommunities) {
        if (targetCommunity === currentCommunity) continue;

        // Calculate edge weight to target community
        let ki_in = 0;
        for (const [neighbor, weight] of graph.adjacency.get(node) || []) {
          if (communities.get(neighbor) === targetCommunity) {
            ki_in += weight;
          }
        }

        const sumTot = communityWeight.get(targetCommunity) || 0;
        const gain = ki_in / m - (ki * sumTot) / (2 * m * m);

        if (gain > bestGain) {
          bestGain = gain;
          bestCommunity = targetCommunity;
        }
      }

      // Move node if beneficial
      if (bestCommunity !== currentCommunity) {
        communities.set(node, bestCommunity);

        // Update community weights
        communityWeight.set(currentCommunity, (communityWeight.get(currentCommunity) || 0) - ki);
        communityWeight.set(bestCommunity, (communityWeight.get(bestCommunity) || 0) + ki);

        improved = true;
      }
    }
  }

  // Renumber communities to be contiguous
  const renumber = new Map<number, number>();
  let newId = 0;
  for (const [node, c] of communities) {
    if (!renumber.has(c)) {
      renumber.set(c, newId++);
    }
    communities.set(node, renumber.get(c)!);
  }

  return communities;
}

/**
 * Calculate modularity of a partition
 */
export function calculateModularity(graph: Graph, partition: string[][]): number {
  let m = 0;
  for (const { weight = 1 } of graph.edges) {
    m += weight;
  }
  m /= 2;

  if (m === 0) return 0;

  let modularity = 0;

  for (const group of partition) {
    const groupSet = new Set(group);

    // Edges within group
    let inGroup = 0;
    let degreeSum = 0;

    for (const node of group) {
      for (const [neighbor, weight] of graph.adjacency.get(node) || []) {
        if (groupSet.has(neighbor)) {
          inGroup += weight;
        }
        degreeSum += weight;
      }
    }

    inGroup /= 2; // Count each edge once
    modularity += inGroup / m - Math.pow(degreeSum / (2 * m), 2);
  }

  return modularity;
}

/**
 * Find bridges (edges whose removal disconnects components)
 */
export function findBridges(graph: Graph): Array<{ from: string; to: string }> {
  const bridges: Array<{ from: string; to: string }> = [];
  const visited = new Set<string>();
  const discovery = new Map<string, number>();
  const low = new Map<string, number>();
  const parent = new Map<string, string | null>();
  let time = 0;

  function dfs(node: string) {
    visited.add(node);
    discovery.set(node, time);
    low.set(node, time);
    time++;

    for (const [neighbor] of graph.adjacency.get(node) || []) {
      if (!visited.has(neighbor)) {
        parent.set(neighbor, node);
        dfs(neighbor);

        low.set(node, Math.min(low.get(node)!, low.get(neighbor)!));

        if (low.get(neighbor)! > discovery.get(node)!) {
          bridges.push({ from: node, to: neighbor });
        }
      } else if (neighbor !== parent.get(node)) {
        low.set(node, Math.min(low.get(node)!, discovery.get(neighbor)!));
      }
    }
  }

  for (const node of graph.nodes) {
    if (!visited.has(node)) {
      parent.set(node, null);
      dfs(node);
    }
  }

  return bridges;
}

/**
 * Find articulation points (nodes whose removal disconnects components)
 */
export function findArticulationPoints(graph: Graph): string[] {
  const points: string[] = [];
  const visited = new Set<string>();
  const discovery = new Map<string, number>();
  const low = new Map<string, number>();
  const parent = new Map<string, string | null>();
  let time = 0;

  function dfs(node: string) {
    visited.add(node);
    discovery.set(node, time);
    low.set(node, time);
    time++;

    let children = 0;

    for (const [neighbor] of graph.adjacency.get(node) || []) {
      if (!visited.has(neighbor)) {
        children++;
        parent.set(neighbor, node);
        dfs(neighbor);

        low.set(node, Math.min(low.get(node)!, low.get(neighbor)!));

        // Root with 2+ children or non-root with low[v] >= disc[u]
        if (
          (parent.get(node) === null && children > 1) ||
          (parent.get(node) !== null && low.get(neighbor)! >= discovery.get(node)!)
        ) {
          if (!points.includes(node)) {
            points.push(node);
          }
        }
      } else if (neighbor !== parent.get(node)) {
        low.set(node, Math.min(low.get(node)!, discovery.get(neighbor)!));
      }
    }
  }

  for (const node of graph.nodes) {
    if (!visited.has(node)) {
      parent.set(node, null);
      dfs(node);
    }
  }

  return points;
}

// Helper functions
function normalize(v: Float64Array) {
  let sum = 0;
  for (let i = 0; i < v.length; i++) {
    sum += v[i] * v[i];
  }
  const norm = Math.sqrt(sum);
  if (norm > 0) {
    for (let i = 0; i < v.length; i++) {
      v[i] /= norm;
    }
  }
}

function dotProduct(a: Float64Array, b: Float64Array): number {
  let sum = 0;
  for (let i = 0; i < a.length; i++) {
    sum += a[i] * b[i];
  }
  return sum;
}

function kMeans(points: number[][], k: number, maxIter: number = 100): number[] {
  const n = points.length;
  if (n === 0 || k === 0) return [];

  const dim = points[0].length;

  // Random initialization
  const centroids: number[][] = [];
  const used = new Set<number>();
  while (centroids.length < Math.min(k, n)) {
    const idx = Math.floor(Math.random() * n);
    if (!used.has(idx)) {
      used.add(idx);
      centroids.push([...points[idx]]);
    }
  }

  const assignment = new Array(n).fill(0);

  for (let iter = 0; iter < maxIter; iter++) {
    // Assign points to nearest centroid
    let changed = false;
    for (let i = 0; i < n; i++) {
      let minDist = Infinity;
      let minC = 0;
      for (let c = 0; c < centroids.length; c++) {
        let dist = 0;
        for (let d = 0; d < dim; d++) {
          dist += Math.pow(points[i][d] - centroids[c][d], 2);
        }
        if (dist < minDist) {
          minDist = dist;
          minC = c;
        }
      }
      if (assignment[i] !== minC) {
        assignment[i] = minC;
        changed = true;
      }
    }

    if (!changed) break;

    // Update centroids
    const counts = new Array(k).fill(0);
    for (let c = 0; c < centroids.length; c++) {
      for (let d = 0; d < dim; d++) {
        centroids[c][d] = 0;
      }
    }

    for (let i = 0; i < n; i++) {
      const c = assignment[i];
      counts[c]++;
      for (let d = 0; d < dim; d++) {
        centroids[c][d] += points[i][d];
      }
    }

    for (let c = 0; c < centroids.length; c++) {
      if (counts[c] > 0) {
        for (let d = 0; d < dim; d++) {
          centroids[c][d] /= counts[c];
        }
      }
    }
  }

  return assignment;
}

export default {
  buildGraph,
  minCut,
  spectralClustering,
  louvainCommunities,
  calculateModularity,
  findBridges,
  findArticulationPoints,
};
