# BMSSP API Integration Strategy

## 🔗 Integration Architecture

### Current Sublinear Solver Architecture
```
src/
├── core/
│   ├── solver.ts              # SublinearSolver class
│   ├── matrix.ts              # MatrixOperations
│   ├── types.ts               # Core interfaces
│   └── utils.ts               # VectorOperations, utilities
├── mcp/tools/
│   ├── solver.ts              # MCP solver tools
│   ├── graph.ts               # GraphTools for PageRank/centrality
│   └── matrix.ts              # Matrix analysis tools
└── index.ts                   # Main exports
```

### BMSSP Integration Points
```typescript
// @ruvnet/bmssp exports
import {
  WasmGraph,           // Basic graph pathfinding
  WasmNeuralBMSSP,     // Neural/semantic pathfinding
  InitOutput           // WASM initialization
} from '@ruvnet/bmssp';
```

## 🛠 Core Integration Strategy

### 1. BMSSP Wrapper Class

**File**: `src/core/bmssp-wrapper.ts`
```typescript
import { WasmGraph, WasmNeuralBMSSP } from '@ruvnet/bmssp';
import { Matrix, Vector, SolverError, ErrorCodes } from './types.js';

export class BMSSPWrapper {
  private wasmGraph?: WasmGraph;
  private neuralBMSSP?: WasmNeuralBMSSP;
  private isInitialized = false;

  constructor(
    private vertices: number,
    private enableNeural = false,
    private embeddingDim = 128
  ) {}

  async initialize(): Promise<void> {
    try {
      // Initialize basic graph
      this.wasmGraph = new WasmGraph(this.vertices, true);

      // Initialize neural BMSSP if enabled
      if (this.enableNeural) {
        this.neuralBMSSP = new WasmNeuralBMSSP(this.vertices, this.embeddingDim);
      }

      this.isInitialized = true;
    } catch (error) {
      throw new SolverError(
        `Failed to initialize BMSSP: ${error}`,
        ErrorCodes.INVALID_PARAMETERS
      );
    }
  }

  addEdge(from: number, to: number, weight: number): boolean {
    this.ensureInitialized();
    return this.wasmGraph!.add_edge(from, to, weight);
  }

  computeShortestPaths(source: number): Float64Array {
    this.ensureInitialized();
    return this.wasmGraph!.compute_shortest_paths(source);
  }

  // Neural methods
  setEmbedding(node: number, embedding: Float64Array): boolean {
    if (!this.neuralBMSSP) {
      throw new SolverError('Neural BMSSP not initialized', ErrorCodes.INVALID_PARAMETERS);
    }
    return this.neuralBMSSP.set_embedding(node, embedding);
  }

  addSemanticEdge(from: number, to: number, alpha: number): void {
    if (!this.neuralBMSSP) {
      throw new SolverError('Neural BMSSP not initialized', ErrorCodes.INVALID_PARAMETERS);
    }
    this.neuralBMSSP.add_semantic_edge(from, to, alpha);
  }

  computeNeuralPaths(source: number): Float64Array {
    if (!this.neuralBMSSP) {
      throw new SolverError('Neural BMSSP not initialized', ErrorCodes.INVALID_PARAMETERS);
    }
    return this.neuralBMSSP.compute_neural_paths(source);
  }

  semanticDistance(node1: number, node2: number): number {
    if (!this.neuralBMSSP) {
      throw new SolverError('Neural BMSSP not initialized', ErrorCodes.INVALID_PARAMETERS);
    }
    return this.neuralBMSSP.semantic_distance(node1, node2);
  }

  updateEmbeddings(gradients: Float64Array, learningRate: number): boolean {
    if (!this.neuralBMSSP) {
      throw new SolverError('Neural BMSSP not initialized', ErrorCodes.INVALID_PARAMETERS);
    }
    return this.neuralBMSSP.update_embeddings(
      gradients,
      learningRate,
      this.embeddingDim
    );
  }

  cleanup(): void {
    if (this.wasmGraph) {
      this.wasmGraph.free();
      this.wasmGraph = undefined;
    }
    if (this.neuralBMSSP) {
      this.neuralBMSSP.free();
      this.neuralBMSSP = undefined;
    }
    this.isInitialized = false;
  }

  get vertexCount(): number {
    return this.wasmGraph?.vertex_count ?? 0;
  }

  get edgeCount(): number {
    return this.wasmGraph?.edge_count ?? 0;
  }

  private ensureInitialized(): void {
    if (!this.isInitialized || !this.wasmGraph) {
      throw new SolverError('BMSSP not initialized', ErrorCodes.INVALID_PARAMETERS);
    }
  }
}
```

### 2. Matrix to Graph Bridge

**File**: `src/core/bmssp-bridge.ts`
```typescript
import { Matrix, Vector } from './types.js';
import { MatrixOperations } from './matrix.js';
import { BMSSPWrapper } from './bmssp-wrapper.js';

export class BMSSPBridge {
  /**
   * Convert adjacency matrix to BMSSP graph
   */
  static async createGraphFromMatrix(
    adjacency: Matrix,
    enableNeural = false,
    embeddingDim = 128
  ): Promise<BMSSPWrapper> {
    MatrixOperations.validateMatrix(adjacency);

    if (adjacency.rows !== adjacency.cols) {
      throw new Error('Adjacency matrix must be square');
    }

    const graph = new BMSSPWrapper(adjacency.rows, enableNeural, embeddingDim);
    await graph.initialize();

    // Add edges from matrix
    for (let i = 0; i < adjacency.rows; i++) {
      for (let j = 0; j < adjacency.cols; j++) {
        const weight = MatrixOperations.getEntry(adjacency, i, j);
        if (weight !== 0) {
          graph.addEdge(i, j, weight);
        }
      }
    }

    return graph;
  }

  /**
   * Convert Laplacian matrix to graph (for effective resistance)
   */
  static async createGraphFromLaplacian(laplacian: Matrix): Promise<BMSSPWrapper> {
    // Convert Laplacian to adjacency: A = D - L
    const adjacency = this.laplacianToAdjacency(laplacian);
    return this.createGraphFromMatrix(adjacency);
  }

  /**
   * Extract adjacency matrix from Laplacian
   */
  private static laplacianToAdjacency(laplacian: Matrix): Matrix {
    const n = laplacian.rows;

    if (laplacian.format === 'dense') {
      const data: number[][] = Array(n).fill(null).map(() => Array(n).fill(0));

      for (let i = 0; i < n; i++) {
        const diagonal = MatrixOperations.getEntry(laplacian, i, i);

        for (let j = 0; j < n; j++) {
          if (i !== j) {
            data[i][j] = -MatrixOperations.getEntry(laplacian, i, j);
          }
        }
      }

      return { rows: n, cols: n, data, format: 'dense' };
    } else {
      // Handle sparse format
      const values: number[] = [];
      const rowIndices: number[] = [];
      const colIndices: number[] = [];

      const sparse = laplacian as any;
      for (let k = 0; k < sparse.values.length; k++) {
        const i = sparse.rowIndices[k];
        const j = sparse.colIndices[k];

        if (i !== j) {
          values.push(-sparse.values[k]);
          rowIndices.push(i);
          colIndices.push(j);
        }
      }

      return {
        rows: n,
        cols: n,
        values,
        rowIndices,
        colIndices,
        format: 'coo'
      };
    }
  }

  /**
   * Set node embeddings for neural pathfinding
   */
  static async setNodeEmbeddings(
    graph: BMSSPWrapper,
    embeddings: Float64Array[],
    embeddingDim: number
  ): Promise<void> {
    for (let i = 0; i < embeddings.length; i++) {
      if (embeddings[i].length !== embeddingDim) {
        throw new Error(`Embedding ${i} has wrong dimension: ${embeddings[i].length} vs ${embeddingDim}`);
      }
      graph.setEmbedding(i, embeddings[i]);
    }
  }

  /**
   * Convert BMSSP distances back to vector format
   */
  static distancesToVector(distances: Float64Array): Vector {
    return Array.from(distances);
  }
}
```

### 3. Hybrid Solver Integration

**File**: `src/core/hybrid-solver.ts`
```typescript
import { SublinearSolver } from './solver.js';
import { BMSSPWrapper } from './bmssp-wrapper.js';
import { BMSSPBridge } from './bmssp-bridge.js';
import {
  Matrix,
  Vector,
  SolverConfig,
  SolverResult,
  PageRankConfig,
  SolverError,
  ErrorCodes
} from './types.js';

interface HybridConfig extends SolverConfig {
  useBMSSP?: boolean;
  bmsspThreshold?: {
    minGraphSize: number;
    minSparsity: number;
    multiSourceMin: number;
  };
  enableNeural?: boolean;
}

export class HybridSolver extends SublinearSolver {
  private bmsspGraph?: BMSSPWrapper;

  constructor(private hybridConfig: HybridConfig) {
    super(hybridConfig);
  }

  /**
   * Enhanced PageRank using BMSSP when beneficial
   */
  async computePageRank(adjacency: Matrix, config: PageRankConfig): Promise<Vector> {
    const shouldUseBMSSP = this.shouldUseBMSSP(adjacency, 'pagerank');

    if (shouldUseBMSSP) {
      return this.computePageRankBMSSP(adjacency, config);
    } else {
      return super.computePageRank(adjacency, config);
    }
  }

  /**
   * Multi-source shortest paths using BMSSP
   */
  async multiSourceShortestPaths(
    adjacency: Matrix,
    sources: number[],
    targets?: number[]
  ): Promise<{
    distances: Map<number, Vector>;
    paths: Map<number, number[][]>;
    computeTime: number;
  }> {
    const startTime = performance.now();

    this.bmsspGraph = await BMSSPBridge.createGraphFromMatrix(adjacency);

    const distances = new Map<number, Vector>();
    const paths = new Map<number, number[][]>();

    try {
      for (const source of sources) {
        const sourceDistances = this.bmsspGraph.computeShortestPaths(source);
        distances.set(source, BMSSPBridge.distancesToVector(sourceDistances));

        // Reconstruct paths (simplified - BMSSP focuses on distances)
        const sourcePaths: number[][] = [];
        if (targets) {
          for (const target of targets) {
            sourcePaths.push(this.reconstructPath(adjacency, source, target, sourceDistances));
          }
        }
        paths.set(source, sourcePaths);
      }

      return {
        distances,
        paths,
        computeTime: performance.now() - startTime
      };
    } finally {
      this.bmsspGraph.cleanup();
      this.bmsspGraph = undefined;
    }
  }

  /**
   * Semantic pathfinding using Neural BMSSP
   */
  async semanticPathfinding(
    adjacency: Matrix,
    embeddings: Float64Array[],
    source: number,
    target: number,
    alpha: number = 0.5
  ): Promise<{
    distance: number;
    semanticDistance: number;
    path: number[];
    computeTime: number;
  }> {
    const startTime = performance.now();

    this.bmsspGraph = await BMSSPBridge.createGraphFromMatrix(
      adjacency,
      true, // Enable neural
      embeddings[0].length
    );

    try {
      // Set embeddings
      await BMSSPBridge.setNodeEmbeddings(
        this.bmsspGraph,
        embeddings,
        embeddings[0].length
      );

      // Add semantic edges
      for (let i = 0; i < adjacency.rows; i++) {
        for (let j = 0; j < adjacency.cols; j++) {
          if (i !== j) {
            this.bmsspGraph.addSemanticEdge(i, j, alpha);
          }
        }
      }

      // Compute neural paths
      const neuralDistances = this.bmsspGraph.computeNeuralPaths(source);
      const semanticDist = this.bmsspGraph.semanticDistance(source, target);

      // Reconstruct semantic path (simplified)
      const path = this.reconstructSemanticPath(source, target, neuralDistances);

      return {
        distance: neuralDistances[target],
        semanticDistance: semanticDist,
        path,
        computeTime: performance.now() - startTime
      };
    } finally {
      this.bmsspGraph.cleanup();
      this.bmsspGraph = undefined;
    }
  }

  /**
   * Decide whether to use BMSSP based on problem characteristics
   */
  private shouldUseBMSSP(matrix: Matrix, operation: string): boolean {
    if (!this.hybridConfig.useBMSSP) return false;

    const threshold = this.hybridConfig.bmsspThreshold || {
      minGraphSize: 1000,
      minSparsity: 0.9,
      multiSourceMin: 2
    };

    const size = matrix.rows;
    const sparsity = this.calculateSparsity(matrix);

    // Size threshold
    if (size < threshold.minGraphSize) return false;

    // Sparsity threshold (BMSSP excels with sparse graphs)
    if (sparsity < threshold.minSparsity) return false;

    // Operation-specific logic
    switch (operation) {
      case 'pagerank':
        return size > 5000; // BMSSP beneficial for large PageRank
      case 'shortest-path':
        return true; // BMSSP always good for shortest paths
      case 'multi-source':
        return true; // BMSSP designed for multi-source
      default:
        return false;
    }
  }

  private calculateSparsity(matrix: Matrix): number {
    let nonZeros = 0;
    const total = matrix.rows * matrix.cols;

    if (matrix.format === 'dense') {
      const dense = matrix as any;
      for (let i = 0; i < matrix.rows; i++) {
        for (let j = 0; j < matrix.cols; j++) {
          if (dense.data[i][j] !== 0) nonZeros++;
        }
      }
    } else {
      const sparse = matrix as any;
      nonZeros = sparse.values.length;
    }

    return 1 - (nonZeros / total);
  }

  private async computePageRankBMSSP(adjacency: Matrix, config: PageRankConfig): Promise<Vector> {
    // Convert PageRank to shortest path problem for BMSSP
    // This is a simplified approach - full implementation would be more complex

    this.bmsspGraph = await BMSSPBridge.createGraphFromMatrix(adjacency);

    try {
      const n = adjacency.rows;
      const pagerank = new Array(n).fill(0);

      // Compute influence from each node (simplified)
      for (let i = 0; i < n; i++) {
        const distances = this.bmsspGraph.computeShortestPaths(i);
        const influence = this.computeInfluence(distances, config.damping);
        pagerank[i] = influence;
      }

      // Normalize
      const sum = pagerank.reduce((a, b) => a + b, 0);
      return pagerank.map(p => p / sum);
    } finally {
      this.bmsspGraph.cleanup();
      this.bmsspGraph = undefined;
    }
  }

  private computeInfluence(distances: Float64Array, damping: number): number {
    // Convert distances to influence scores
    let influence = 0;
    for (let i = 0; i < distances.length; i++) {
      if (distances[i] < Infinity) {
        influence += damping / (1 + distances[i]);
      }
    }
    return influence;
  }

  private reconstructPath(
    adjacency: Matrix,
    source: number,
    target: number,
    distances: Float64Array
  ): number[] {
    // Simple path reconstruction (breadth-first approach)
    const path: number[] = [];
    let current = target;

    while (current !== source) {
      path.unshift(current);

      // Find predecessor with minimum distance
      let minDist = Infinity;
      let predecessor = -1;

      for (let i = 0; i < adjacency.rows; i++) {
        if (MatrixOperations.getEntry(adjacency, i, current) > 0) {
          if (distances[i] < minDist) {
            minDist = distances[i];
            predecessor = i;
          }
        }
      }

      if (predecessor === -1) break;
      current = predecessor;
    }

    path.unshift(source);
    return path;
  }

  private reconstructSemanticPath(
    source: number,
    target: number,
    neuralDistances: Float64Array
  ): number[] {
    // Simplified semantic path reconstruction
    // In practice, would use more sophisticated neural pathfinding
    const path: number[] = [source];

    let current = source;
    const visited = new Set([source]);

    while (current !== target && path.length < neuralDistances.length) {
      let nextNode = -1;
      let minDist = Infinity;

      for (let i = 0; i < neuralDistances.length; i++) {
        if (!visited.has(i) && neuralDistances[i] < minDist) {
          minDist = neuralDistances[i];
          nextNode = i;
        }
      }

      if (nextNode === -1) break;

      path.push(nextNode);
      visited.add(nextNode);
      current = nextNode;
    }

    return path;
  }

  override async cleanup(): Promise<void> {
    if (this.bmsspGraph) {
      this.bmsspGraph.cleanup();
      this.bmsspGraph = undefined;
    }
  }
}
```

## 🔧 MCP Tools Integration

### Enhanced Graph Tools

**File**: `src/mcp/tools/bmssp-tools.ts`
```typescript
import { HybridSolver } from '../../core/hybrid-solver.js';
import { BMSSPBridge } from '../../core/bmssp-bridge.js';
import { Matrix, Vector, SolverError, ErrorCodes } from '../../core/types.js';

export class BMSSPTools {
  /**
   * Ultra-fast shortest path using BMSSP WASM
   */
  static async shortestPath(params: {
    adjacency: Matrix;
    source: number;
    target: number;
    method?: 'bmssp' | 'hybrid';
  }) {
    const graph = await BMSSPBridge.createGraphFromMatrix(params.adjacency);

    try {
      const distances = graph.computeShortestPaths(params.source);
      const distance = distances[params.target];

      return {
        distance,
        source: params.source,
        target: params.target,
        algorithm: 'bmssp-wasm',
        performance: {
          vertices: graph.vertexCount,
          edges: graph.edgeCount,
          complexity: 'O(m·log^(2/3) n)'
        }
      };
    } finally {
      graph.cleanup();
    }
  }

  /**
   * Multi-source PageRank using BMSSP
   */
  static async multiSourcePageRank(params: {
    adjacency: Matrix;
    sources: number[];
    damping?: number;
    epsilon?: number;
    maxIterations?: number;
  }) {
    const config = {
      method: 'neumann' as const,
      epsilon: params.epsilon || 1e-6,
      maxIterations: params.maxIterations || 1000,
      useBMSSP: true,
      bmsspThreshold: {
        minGraphSize: 100,
        minSparsity: 0.7,
        multiSourceMin: 2
      }
    };

    const solver = new HybridSolver(config);

    const pageRankConfig = {
      damping: params.damping || 0.85,
      epsilon: params.epsilon || 1e-6,
      maxIterations: params.maxIterations || 1000
    };

    try {
      const result = await solver.multiSourceShortestPaths(
        params.adjacency,
        params.sources
      );

      return {
        sources: params.sources,
        distances: Object.fromEntries(result.distances),
        computeTime: result.computeTime,
        algorithm: 'bmssp-multi-source',
        statistics: {
          totalSources: params.sources.length,
          averageDistance: this.calculateAverageDistance(result.distances),
          performance: result.computeTime
        }
      };
    } finally {
      await solver.cleanup();
    }
  }

  /**
   * Semantic pathfinding with neural BMSSP
   */
  static async semanticPathfinding(params: {
    adjacency: Matrix;
    embeddings: Float64Array[];
    source: number;
    target: number;
    alpha?: number;
    embeddingDim?: number;
  }) {
    const config = {
      method: 'neumann' as const,
      epsilon: 1e-6,
      maxIterations: 1000,
      useBMSSP: true,
      enableNeural: true
    };

    const solver = new HybridSolver(config);

    try {
      const result = await solver.semanticPathfinding(
        params.adjacency,
        params.embeddings,
        params.source,
        params.target,
        params.alpha || 0.5
      );

      return {
        ...result,
        algorithm: 'neural-bmssp',
        semantics: {
          embeddingDim: params.embeddings[0].length,
          alpha: params.alpha || 0.5,
          semanticSimilarity: 1 / (1 + result.semanticDistance)
        }
      };
    } finally {
      await solver.cleanup();
    }
  }

  /**
   * Batch shortest paths computation
   */
  static async batchShortestPaths(params: {
    adjacency: Matrix;
    queries: Array<{ source: number; target: number }>;
  }) {
    const graph = await BMSSPBridge.createGraphFromMatrix(params.adjacency);
    const results: Array<{
      source: number;
      target: number;
      distance: number;
    }> = [];

    try {
      // Group queries by source for efficiency
      const sourceGroups = new Map<number, number[]>();
      for (const query of params.queries) {
        if (!sourceGroups.has(query.source)) {
          sourceGroups.set(query.source, []);
        }
        sourceGroups.get(query.source)!.push(query.target);
      }

      // Compute distances for each source group
      for (const [source, targets] of sourceGroups) {
        const distances = graph.computeShortestPaths(source);

        for (const target of targets) {
          results.push({
            source,
            target,
            distance: distances[target]
          });
        }
      }

      return {
        results,
        statistics: {
          totalQueries: params.queries.length,
          uniqueSources: sourceGroups.size,
          algorithm: 'bmssp-batch',
          performance: {
            vertices: graph.vertexCount,
            edges: graph.edgeCount
          }
        }
      };
    } finally {
      graph.cleanup();
    }
  }

  private static calculateAverageDistance(distances: Map<number, Vector>): number {
    let total = 0;
    let count = 0;

    for (const distanceVector of distances.values()) {
      for (const distance of distanceVector) {
        if (distance < Infinity) {
          total += distance;
          count++;
        }
      }
    }

    return count > 0 ? total / count : 0;
  }
}
```

## 📊 Performance Monitoring

**File**: `src/core/bmssp-benchmarks.ts`
```typescript
export class BMSSPBenchmarks {
  static async comparePerformance(
    adjacency: Matrix,
    testCases: Array<{
      method: 'traditional' | 'bmssp' | 'hybrid';
      operation: 'shortest-path' | 'pagerank' | 'multi-source';
      params: any;
    }>
  ) {
    const results = [];

    for (const testCase of testCases) {
      const startTime = performance.now();
      const startMemory = process.memoryUsage().heapUsed;

      let result;
      switch (testCase.method) {
        case 'traditional':
          result = await this.runTraditional(adjacency, testCase);
          break;
        case 'bmssp':
          result = await this.runBMSSP(adjacency, testCase);
          break;
        case 'hybrid':
          result = await this.runHybrid(adjacency, testCase);
          break;
      }

      const endTime = performance.now();
      const endMemory = process.memoryUsage().heapUsed;

      results.push({
        method: testCase.method,
        operation: testCase.operation,
        executionTime: endTime - startTime,
        memoryUsed: endMemory - startMemory,
        result
      });
    }

    return this.analyzeResults(results);
  }

  private static analyzeResults(results: any[]) {
    // Group by operation and compare methods
    const analysis = {
      performanceGains: {},
      memoryEfficiency: {},
      recommendations: []
    };

    // Implementation details...
    return analysis;
  }
}
```

This API integration strategy provides a comprehensive approach to incorporating BMSSP's high-performance graph algorithms while maintaining compatibility with existing sublinear solver functionality.