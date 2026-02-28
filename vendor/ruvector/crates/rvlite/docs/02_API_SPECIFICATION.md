# Phase 1: API Specification

## Complete API Design for RvLite

**Version**: 1.0.0
**Target**: TypeScript API (primary), Rust API (internal)

---

## 1. TypeScript API (Public Interface)

### 1.1 Database Creation & Lifecycle

```typescript
class RvLite {
  /**
   * Create a new in-memory database
   * @param options Database configuration
   * @returns Promise<RvLite> Initialized database instance
   */
  static async create(options?: DatabaseOptions): Promise<RvLite>

  /**
   * Load database from persistent storage
   * @param source Storage backend (IndexedDB, OPFS, file path)
   * @returns Promise<RvLite> Loaded database instance
   */
  static async load(source?: StorageSource): Promise<RvLite>

  /**
   * Close database and release resources
   */
  async close(): Promise<void>

  /**
   * Save database to persistent storage
   * @param target Storage backend
   */
  async save(target?: StorageSource): Promise<void>

  /**
   * Export database to various formats
   * @param format Export format (json, arrow, sql)
   * @returns Promise<Uint8Array | string>
   */
  async export(format: ExportFormat): Promise<Uint8Array | string>

  /**
   * Import database from various formats
   * @param data Import data
   * @param format Import format
   */
  async import(data: Uint8Array | string, format: ImportFormat): Promise<void>
}

interface DatabaseOptions {
  /** Maximum memory usage in MB (default: 512) */
  maxMemoryMB?: number

  /** Enable debug logging */
  debug?: boolean

  /** Storage backend */
  storage?: StorageBackend

  /** SIMD optimization level */
  simd?: 'auto' | 'on' | 'off'
}

type StorageBackend = 'memory' | 'indexeddb' | 'opfs' | 'filesystem'
type StorageSource = string | { type: StorageBackend; path?: string }
type ExportFormat = 'json' | 'arrow' | 'sql' | 'binary'
type ImportFormat = 'json' | 'arrow' | 'sql' | 'binary'
```

### 1.2 SQL Interface

```typescript
class RvLite {
  /**
   * Execute SQL query
   * @param query SQL query string
   * @param params Query parameters (positional)
   * @returns Promise<QueryResult<T>>
   */
  async sql<T = any>(query: string, params?: any[]): Promise<QueryResult<T>>

  /**
   * Execute SQL query and return first row
   * @param query SQL query string
   * @param params Query parameters
   * @returns Promise<T | null>
   */
  async sqlOne<T = any>(query: string, params?: any[]): Promise<T | null>

  /**
   * Execute SQL DDL statement (CREATE, DROP, ALTER)
   * @param ddl DDL statement
   */
  async exec(ddl: string): Promise<void>

  /**
   * Prepare a parameterized query for reuse
   * @param query SQL query with placeholders
   * @returns PreparedStatement
   */
  prepare<T = any>(query: string): PreparedStatement<T>
}

interface QueryResult<T> {
  /** Result rows */
  rows: T[]

  /** Number of rows returned */
  rowCount: number

  /** Execution time in ms */
  executionTime: number

  /** Column metadata */
  columns: ColumnInfo[]
}

interface ColumnInfo {
  name: string
  type: DataType
  nullable: boolean
}

type DataType =
  | 'integer'
  | 'bigint'
  | 'real'
  | 'text'
  | 'blob'
  | 'vector'
  | 'halfvec'
  | 'binaryvec'
  | 'sparsevec'

class PreparedStatement<T> {
  /** Execute with parameters */
  async execute(params: any[]): Promise<QueryResult<T>>

  /** Execute and return first row */
  async executeOne(params: any[]): Promise<T | null>

  /** Release resources */
  close(): void
}
```

**SQL Examples**:

```typescript
// Table creation
await db.exec(`
  CREATE TABLE documents (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT,
    embedding VECTOR(384)
  )
`);

// Index creation
await db.exec(`
  CREATE INDEX idx_embedding
  ON documents
  USING hnsw (embedding vector_cosine_ops)
  WITH (m = 16, ef_construction = 64)
`);

// Insert
await db.sql(
  'INSERT INTO documents (title, content, embedding) VALUES ($1, $2, $3)',
  ['Doc 1', 'Content...', embedding]
);

// Vector search
const results = await db.sql<{title: string; distance: number}>(`
  SELECT title, embedding <=> $1 AS distance
  FROM documents
  ORDER BY distance
  LIMIT 10
`, [queryEmbedding]);

// Prepared statement
const search = db.prepare<{title: string}>(`
  SELECT title FROM documents ORDER BY embedding <=> $1 LIMIT 10
`);
const results1 = await search.execute([embedding1]);
const results2 = await search.execute([embedding2]);
```

### 1.3 SPARQL Interface

```typescript
class RvLite {
  /**
   * Execute SPARQL query
   * @param query SPARQL query string
   * @param options Query options
   * @returns Promise<SparqlResult>
   */
  async sparql(query: string, options?: SparqlOptions): Promise<SparqlResult>

  /**
   * Load RDF data into triple store
   * @param data RDF data
   * @param format RDF format (turtle, ntriples, jsonld, rdfxml)
   */
  async loadRDF(data: string, format: RDFFormat): Promise<void>

  /**
   * Export triple store as RDF
   * @param format RDF format
   * @returns Promise<string>
   */
  async exportRDF(format: RDFFormat): Promise<string>
}

interface SparqlOptions {
  /** Result format */
  format?: 'json' | 'xml' | 'csv' | 'tsv'

  /** Timeout in ms */
  timeout?: number

  /** Base IRI for relative IRIs */
  base?: string
}

interface SparqlResult {
  /** Result type */
  type: 'bindings' | 'boolean' | 'graph'

  /** Variable bindings (SELECT) */
  bindings?: SparqlBinding[]

  /** Boolean result (ASK) */
  boolean?: boolean

  /** RDF triples (CONSTRUCT/DESCRIBE) */
  triples?: RDFTriple[]

  /** Variables in SELECT */
  variables?: string[]
}

interface SparqlBinding {
  [variable: string]: RDFTerm
}

type RDFTerm =
  | { type: 'uri'; value: string }
  | { type: 'literal'; value: string; datatype?: string; lang?: string }
  | { type: 'bnode'; value: string }

interface RDFTriple {
  subject: RDFTerm
  predicate: RDFTerm
  object: RDFTerm
}

type RDFFormat = 'turtle' | 'ntriples' | 'jsonld' | 'rdfxml'
```

**SPARQL Examples**:

```typescript
// Load RDF data
await db.loadRDF(`
  @prefix foaf: <http://xmlns.com/foaf/0.1/> .
  @prefix : <http://example.org/> .

  :alice foaf:name "Alice" ;
         foaf:knows :bob .
  :bob foaf:name "Bob" .
`, 'turtle');

// SELECT query
const result = await db.sparql(`
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  SELECT ?name
  WHERE {
    ?person foaf:name ?name .
    FILTER(lang(?name) = "en")
  }
  ORDER BY ?name
`);

console.log(result.bindings); // [{ name: { type: 'literal', value: 'Alice' } }, ...]

// CONSTRUCT query
const constructed = await db.sparql(`
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>

  CONSTRUCT { ?p1 :knows_transitively ?p2 }
  WHERE {
    ?p1 foaf:knows+ ?p2 .
  }
`);

// ASK query
const askResult = await db.sparql(`
  ASK { ?s foaf:knows ?o }
`);
console.log(askResult.boolean); // true

// Vector-enhanced SPARQL
const vectorResults = await db.sparql(`
  PREFIX vec: <http://rvlite.dev/vector/>

  SELECT ?doc ?title
  WHERE {
    ?doc :title ?title ;
         :embedding ?emb .
    FILTER(vec:cosine(?emb, $queryVector) > 0.8)
  }
  ORDER BY DESC(vec:cosine(?emb, $queryVector))
  LIMIT 10
`);
```

### 1.4 Cypher Interface

```typescript
class RvLite {
  /**
   * Execute Cypher query
   * @param query Cypher query string
   * @param params Query parameters
   * @returns Promise<CypherResult>
   */
  async cypher(query: string, params?: Record<string, any>): Promise<CypherResult>

  /**
   * Create graph from edges
   * @param edges Array of [source, target, properties?]
   */
  async createGraph(edges: GraphEdge[]): Promise<void>
}

interface CypherResult {
  /** Result records */
  records: CypherRecord[]

  /** Summary statistics */
  summary: {
    nodesCreated: number
    nodesDeleted: number
    relationshipsCreated: number
    relationshipsDeleted: number
    propertiesSet: number
  }
}

interface CypherRecord {
  /** Column values */
  [column: string]: CypherValue
}

type CypherValue =
  | null
  | boolean
  | number
  | string
  | CypherNode
  | CypherRelationship
  | CypherPath
  | CypherValue[]
  | { [key: string]: CypherValue }

interface CypherNode {
  identity: number
  labels: string[]
  properties: Record<string, any>
}

interface CypherRelationship {
  identity: number
  type: string
  start: number
  end: number
  properties: Record<string, any>
}

interface CypherPath {
  start: CypherNode
  end: CypherNode
  segments: Array<{
    start: CypherNode
    relationship: CypherRelationship
    end: CypherNode
  }>
}

type GraphEdge = [string | number, string | number, Record<string, any>?]
```

**Cypher Examples**:

```typescript
// Create nodes and relationships
await db.cypher(`
  CREATE (alice:Person {name: 'Alice', age: 30, embedding: $aliceEmb})
  CREATE (bob:Person {name: 'Bob', age: 25, embedding: $bobEmb})
  CREATE (alice)-[:KNOWS {since: 2020}]->(bob)
`, { aliceEmb: [1, 2, 3], bobEmb: [4, 5, 6] });

// Pattern matching
const friends = await db.cypher(`
  MATCH (a:Person)-[:KNOWS]->(b:Person)
  WHERE a.age > $minAge
  RETURN a.name AS person, b.name AS friend, a.age
`, { minAge: 25 });

// Vector-enhanced Cypher
const similar = await db.cypher(`
  MATCH (p:Person)
  WHERE vector.cosine(p.embedding, $query) > 0.8
  RETURN p.name, p.age, vector.cosine(p.embedding, $query) AS similarity
  ORDER BY similarity DESC
  LIMIT 10
`, { query: queryEmbedding });

// Shortest path
const path = await db.cypher(`
  MATCH path = shortestPath((a:Person {name: 'Alice'})-[:KNOWS*]-(b:Person {name: 'Charlie'}))
  RETURN path
`);

// Graph algorithms
const pagerank = await db.cypher(`
  CALL graph.pagerank('Person', 'KNOWS')
  YIELD nodeId, score
  MATCH (p:Person) WHERE id(p) = nodeId
  RETURN p.name, score
  ORDER BY score DESC
`);
```

### 1.5 Vector Operations

```typescript
class RvLite {
  /**
   * Insert vectors with metadata
   * @param table Table name
   * @param vectors Array of vector data
   */
  async insertVectors(
    table: string,
    vectors: VectorData[]
  ): Promise<void>

  /**
   * Search for similar vectors
   * @param table Table name
   * @param query Query vector
   * @param options Search options
   * @returns Promise<SearchResult[]>
   */
  async searchSimilar(
    table: string,
    query: Float32Array | number[],
    options?: SearchOptions
  ): Promise<SearchResult[]>

  /**
   * Get vector by ID
   * @param table Table name
   * @param id Row ID
   * @returns Promise<Float32Array | null>
   */
  async getVector(table: string, id: number): Promise<Float32Array | null>

  /**
   * Update vector by ID
   * @param table Table name
   * @param id Row ID
   * @param vector New vector
   */
  async updateVector(
    table: string,
    id: number,
    vector: Float32Array | number[]
  ): Promise<void>

  /**
   * Delete vector by ID
   * @param table Table name
   * @param id Row ID
   */
  async deleteVector(table: string, id: number): Promise<void>

  /**
   * Compute distance between vectors
   * @param a First vector
   * @param b Second vector
   * @param metric Distance metric
   * @returns number
   */
  distance(
    a: Float32Array | number[],
    b: Float32Array | number[],
    metric?: DistanceMetric
  ): number

  /**
   * Normalize vector
   * @param vector Input vector
   * @returns Float32Array Normalized vector
   */
  normalize(vector: Float32Array | number[]): Float32Array

  /**
   * Quantize vector
   * @param vector Input vector
   * @param method Quantization method
   * @returns Quantized vector
   */
  quantize(
    vector: Float32Array | number[],
    method: QuantizationMethod
  ): Uint8Array | Float32Array
}

interface VectorData {
  id?: number
  vector: Float32Array | number[]
  metadata?: Record<string, any>
}

interface SearchOptions {
  /** Number of results (default: 10) */
  limit?: number

  /** Distance metric (default: 'cosine') */
  metric?: DistanceMetric

  /** Minimum similarity threshold (0-1) */
  threshold?: number

  /** HNSW ef_search parameter */
  efSearch?: number

  /** Include vector in results */
  includeVector?: boolean

  /** Filter condition (SQL WHERE clause) */
  filter?: string
}

interface SearchResult {
  id: number
  distance: number
  vector?: Float32Array
  metadata?: Record<string, any>
}

type DistanceMetric =
  | 'cosine'
  | 'euclidean'
  | 'l2'
  | 'inner'
  | 'dot'
  | 'manhattan'
  | 'l1'
  | 'hamming'

type QuantizationMethod =
  | 'binary'
  | 'scalar'
  | 'product'
```

**Vector Operation Examples**:

```typescript
// Insert vectors
await db.insertVectors('documents', [
  { vector: [1, 2, 3, 4], metadata: { title: 'Doc 1' } },
  { vector: [5, 6, 7, 8], metadata: { title: 'Doc 2' } },
]);

// Search similar
const results = await db.searchSimilar(
  'documents',
  queryVector,
  {
    limit: 10,
    metric: 'cosine',
    threshold: 0.7,
    efSearch: 50,
    filter: "metadata->>'category' = 'tech'"
  }
);

// Distance computation
const dist = db.distance([1, 0, 0], [0, 1, 0], 'cosine');
console.log(dist); // 1.0 (orthogonal)

// Normalize
const normalized = db.normalize([3, 4]); // [0.6, 0.8]

// Quantize
const quantized = db.quantize(vector, 'binary'); // Uint8Array
```

### 1.6 GNN Operations

```typescript
class RvLite {
  /**
   * Graph Neural Network operations
   */
  gnn: {
    /**
     * Initialize GNN layer
     * @param type GNN layer type
     * @param config Layer configuration
     */
    createLayer(type: GNNLayerType, config: GNNConfig): GNNLayer

    /**
     * Compute node embeddings
     * @param graph Graph ID or name
     * @param layers Array of GNN layers
     * @returns Promise<Map<number, Float32Array>>
     */
    computeEmbeddings(
      graph: string,
      layers: GNNLayer[]
    ): Promise<Map<number, Float32Array>>

    /**
     * Train GNN model
     * @param config Training configuration
     */
    train(config: GNNTrainConfig): Promise<GNNModel>

    /**
     * Graph classification
     * @param graph Graph ID
     * @param model Trained model
     * @returns Promise<number> Class label
     */
    classify(graph: string, model: GNNModel): Promise<number>
  }
}

type GNNLayerType = 'gcn' | 'graphsage' | 'gat' | 'gin'

interface GNNConfig {
  /** Input feature dimension */
  inputDim: number

  /** Output feature dimension */
  outputDim: number

  /** Activation function */
  activation?: 'relu' | 'sigmoid' | 'tanh'

  /** Dropout rate */
  dropout?: number

  /** Layer-specific parameters */
  params?: {
    /** GAT: Number of attention heads */
    heads?: number

    /** GraphSage: Aggregation method */
    aggregation?: 'mean' | 'max' | 'lstm'
  }
}

interface GNNLayer {
  type: GNNLayerType
  forward(
    nodeFeatures: Map<number, Float32Array>,
    edges: Array<[number, number]>
  ): Map<number, Float32Array>
}

interface GNNTrainConfig {
  graph: string
  layers: GNNLayer[]
  epochs: number
  learningRate: number
  labels: Map<number, number>
}

interface GNNModel {
  layers: GNNLayer[]
  weights: Float32Array[]
}
```

**GNN Examples**:

```typescript
// Create GNN layers
const gcn1 = db.gnn.createLayer('gcn', {
  inputDim: 128,
  outputDim: 64,
  activation: 'relu'
});

const gcn2 = db.gnn.createLayer('gcn', {
  inputDim: 64,
  outputDim: 32
});

// Compute embeddings
const embeddings = await db.gnn.computeEmbeddings('social_network', [gcn1, gcn2]);

// Node classification
const model = await db.gnn.train({
  graph: 'citation_network',
  layers: [gcn1, gcn2],
  epochs: 100,
  learningRate: 0.01,
  labels: nodeLabels
});

const predicted = await db.gnn.classify('new_graph', model);
```

### 1.7 Self-Learning (ReasoningBank)

```typescript
class RvLite {
  /**
   * Self-learning operations
   */
  learning: {
    /**
     * Record trajectory (state, action, reward)
     * @param trajectory Trajectory data
     */
    recordTrajectory(trajectory: Trajectory): Promise<void>

    /**
     * Learn from recorded trajectories
     * @param config Learning configuration
     */
    train(config: LearningConfig): Promise<void>

    /**
     * Predict best action for state
     * @param state Current state
     * @returns Promise<number> Action ID
     */
    predict(state: Float32Array | number[]): Promise<number>

    /**
     * Get learned patterns
     * @returns Promise<Pattern[]>
     */
    getPatterns(): Promise<Pattern[]>

    /**
     * Memory distillation
     * @param minSupport Minimum support threshold
     */
    distill(minSupport?: number): Promise<void>
  }
}

interface Trajectory {
  state: Float32Array | number[]
  action: number
  reward: number
  nextState?: Float32Array | number[]
  done?: boolean
  metadata?: Record<string, any>
}

interface LearningConfig {
  /** Learning algorithm */
  algorithm: 'q-learning' | 'sarsa' | 'decision-transformer' | 'actor-critic'

  /** Learning rate */
  learningRate: number

  /** Discount factor */
  gamma: number

  /** Exploration rate */
  epsilon?: number

  /** Number of training iterations */
  iterations: number
}

interface Pattern {
  state: Float32Array
  action: number
  value: number
  support: number
  confidence: number
}
```

**Learning Examples**:

```typescript
// Record agent trajectories
await db.learning.recordTrajectory({
  state: [0.1, 0.5, 0.3],
  action: 2,
  reward: 1.0,
  nextState: [0.2, 0.6, 0.4],
  done: false
});

// Train from experiences
await db.learning.train({
  algorithm: 'q-learning',
  learningRate: 0.1,
  gamma: 0.99,
  epsilon: 0.1,
  iterations: 1000
});

// Predict best action
const action = await db.learning.predict([0.1, 0.5, 0.3]);

// Get learned patterns
const patterns = await db.learning.getPatterns();
console.log(patterns);

// Distill memory
await db.learning.distill(0.7); // Keep patterns with >70% confidence
```

---

## 2. Rust API (Internal)

See [10_IMPLEMENTATION_GUIDE.md](./10_IMPLEMENTATION_GUIDE.md) for detailed Rust API documentation.

---

**Next**: [03_DATA_MODEL.md](./03_DATA_MODEL.md) - Storage and type system
